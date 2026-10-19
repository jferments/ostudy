//! Anthropic Messages API dialect.
//!
//! The shared provider layer builds one OpenAI-style chat body. Claude lowers
//! that body to a Messages request at the edge and normalizes the response
//! back, so routing, admission and accounting stay provider-neutral. The
//! gateway decodes Claude's typed SSE events itself (`ProviderWire`).

use serde_json::{json, Map, Value};
use url::Url;

use crate::ai::{
    endpoint_policy::join_endpoint,
    gateway::{anthropic_finish_reason, anthropic_usage_envelope, error, AiGatewayError},
    profiles::ProfileSnapshot,
    reasoning::REASONING_EFFORTS,
};

use super::{
    dialect::command_to_gateway, schema as schema_support, ProviderModelReasoning,
    ProviderProperties,
};

/// The Models API pages at up to 1,000 entries; one page covers every
/// model an account can see today.
const MODEL_PAGE_LIMIT: &str = "1000";
/// Probes are tiny, but a model that cannot turn thinking off still needs room
/// to think before it answers.
const THINKING_PROBE_MAX_TOKENS: usize = 2_048;
const CLAUDE_EFFORTS: [&str; 5] = ["low", "medium", "high", "xhigh", "max"];

pub(super) fn messages_endpoint(profile: &ProfileSnapshot) -> Result<Url, AiGatewayError> {
    join_endpoint(&profile.base_url, "messages").map_err(command_to_gateway)
}

pub(super) fn models_endpoint(profile: &ProfileSnapshot) -> Result<Url, AiGatewayError> {
    let mut url = join_endpoint(&profile.base_url, "models").map_err(command_to_gateway)?;
    url.query_pairs_mut().append_pair("limit", MODEL_PAGE_LIMIT);
    Ok(url)
}

pub(super) fn model_endpoint(
    profile: &ProfileSnapshot,
    model_id: &str,
) -> Result<Url, AiGatewayError> {
    let mut url = join_endpoint(&profile.base_url, "models/").map_err(command_to_gateway)?;
    url.path_segments_mut()
        .map_err(|()| {
            error(
                "invalid_ai_base_url",
                "The AI provider endpoint could not be constructed.",
                false,
                None,
            )
        })?
        .pop_if_empty()
        .push(model_id);
    Ok(url)
}

/// Lower an OpenAI-style chat body to a Messages request.
///
/// - System messages become one cached system block: document work repeats
///   the same instructions and schema across many requests.
/// - Current Claude models reject non-default sampling parameters, so the
///   chat body's temperature is dropped.
/// - `effort` is the already resolved reasoning effort: `none` turns thinking
///   off, `automatic` leaves the model's own default, anything else is
///   adaptive thinking at that effort.
/// - `schema`, when present, is enforced with `output_config.format`.
pub(super) fn lower_chat_body(chat: &Value, effort: &str, schema: Option<Value>) -> Value {
    let mut system = Vec::new();
    let mut messages = Vec::new();
    for message in chat
        .get("messages")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let role = message
            .get("role")
            .and_then(Value::as_str)
            .unwrap_or("user");
        let content = message.get("content").unwrap_or(&Value::Null);
        if role == "system" {
            if let Some(text) = content.as_str() {
                system.push(text.to_string());
            }
            continue;
        }
        let role = if role == "assistant" {
            "assistant"
        } else {
            "user"
        };
        messages.push(json!({"role": role, "content": lower_content(content)}));
    }
    let max_tokens = chat
        .get("max_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(1_024);
    let mut body = json!({
        "model": chat.get("model").cloned().unwrap_or(Value::Null),
        "max_tokens": max_tokens,
        "messages": messages,
        "stream": false,
    });
    if !system.is_empty() {
        body["system"] = json!([{
            "type": "text",
            "text": system.join("\n\n"),
            "cache_control": {"type": "ephemeral"}
        }]);
    }
    let mut output_config = Map::new();
    match effort {
        "none" => body["thinking"] = json!({"type": "disabled"}),
        "automatic" => {}
        requested => {
            body["thinking"] = json!({"type": "adaptive"});
            let effort = if requested == "minimal" {
                "low"
            } else {
                requested
            };
            output_config.insert("effort".into(), json!(effort));
        }
    }
    if let Some(schema) = schema {
        output_config.insert(
            "format".into(),
            json!({"type": "json_schema", "schema": schema}),
        );
    }
    if !output_config.is_empty() {
        body["output_config"] = Value::Object(output_config);
    }
    body
}

/// Give connectivity probes room to think when the model cannot skip it.
pub(super) fn probe_max_tokens(effort: &str, visible_tokens: usize) -> usize {
    if effort == "none" {
        visible_tokens
    } else {
        visible_tokens.max(THINKING_PROBE_MAX_TOKENS)
    }
}

/// Claude enforces a smaller JSON-Schema language than ostudy's contracts.
/// A schema outside it is still sent in the system prompt, and native
/// admission stays authoritative; it is just not grammar-constrained.
pub(super) fn strict_schema(schema: &Value) -> Option<Value> {
    schema_support::claude_strict_schema(schema)
}

fn lower_content(content: &Value) -> Value {
    let Some(parts) = content.as_array() else {
        return content.clone();
    };
    Value::Array(parts.iter().map(lower_part).collect())
}

fn lower_part(part: &Value) -> Value {
    if part.get("type").and_then(Value::as_str) != Some("image_url") {
        return part.clone();
    }
    let url = part
        .pointer("/image_url/url")
        .and_then(Value::as_str)
        .unwrap_or("");
    if let Some((media_type, data)) = url
        .strip_prefix("data:")
        .and_then(|rest| rest.split_once(";base64,"))
    {
        json!({
            "type": "image",
            "source": {"type": "base64", "media_type": media_type, "data": data}
        })
    } else {
        json!({"type": "image", "source": {"type": "url", "url": url}})
    }
}

/// Normalize a non-streaming Messages response to the chat-completion shape
/// every provider-neutral reader uses.
pub(super) fn normalize_message(value: &Value) -> Result<Value, AiGatewayError> {
    let blocks = value
        .get("content")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            error(
                "ai_provider_protocol_error",
                "The provider response did not use the expected Messages shape.",
                false,
                None,
            )
        })?;
    let mut content = String::new();
    let mut tool_calls = Vec::new();
    for block in blocks {
        match block.get("type").and_then(Value::as_str) {
            Some("text") => {
                content.push_str(block.get("text").and_then(Value::as_str).unwrap_or(""));
            }
            Some("tool_use") => tool_calls.push(json!({
                "index": tool_calls.len(),
                "id": block.get("id"),
                "type": "function",
                "function": {
                    "name": block.get("name"),
                    "arguments": block.get("input").map(Value::to_string).unwrap_or_default(),
                }
            })),
            _ => {}
        }
    }
    let mut message = json!({"content": content});
    if !tool_calls.is_empty() {
        message["tool_calls"] = Value::Array(tool_calls);
    }
    let finish_reason = value
        .get("stop_reason")
        .and_then(Value::as_str)
        .map(anthropic_finish_reason);
    let mut normalized = json!({
        "id": value.get("id"),
        "model": value.get("model"),
        "choices": [{"message": message, "finish_reason": finish_reason}],
    });
    let read = |key: &str| {
        value
            .pointer(&format!("/usage/{key}"))
            .and_then(Value::as_u64)
    };
    if let Some(usage) = anthropic_usage_envelope(
        read("input_tokens"),
        read("output_tokens"),
        read("cache_read_input_tokens"),
        read("cache_creation_input_tokens"),
    ) {
        normalized["usage"] = usage;
    }
    Ok(normalized)
}

/// Models that cannot enforce a JSON schema are hidden, as OpenRouter's are:
/// every learning task depends on structured output.
pub(super) fn model_structured_outputs_declared(item: &Value) -> Option<bool> {
    item.pointer("/capabilities/structured_outputs/supported")
        .and_then(Value::as_bool)
}

pub(super) fn model_display_name(item: &Value, id: &str) -> String {
    item.get("display_name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty() && value.len() <= 200)
        .filter(|value| !value.chars().any(char::is_control))
        .unwrap_or(id)
        .to_string()
}

pub(super) fn model_context_window_tokens(item: &Value) -> Option<i64> {
    item.get("max_input_tokens")
        .and_then(Value::as_i64)
        .filter(|value| *value > 0)
}

/// Map the Models API capability tree onto ostudy's reasoning facts. A model
/// that rejects `thinking: disabled` reasons mandatorily, so "no reasoning"
/// resolves to its lowest effort instead of a request it would refuse.
pub(super) fn model_reasoning(item: &Value) -> Option<ProviderModelReasoning> {
    let capabilities = item.get("capabilities").filter(|value| value.is_object())?;
    let supported = |pointer: &str| {
        capabilities
            .pointer(pointer)
            .and_then(Value::as_bool)
            .unwrap_or(false)
    };
    let efforts =
        if supported("/effort/supported") && supported("/thinking/types/adaptive/supported") {
            CLAUDE_EFFORTS
                .into_iter()
                .filter(|effort| supported(&format!("/effort/{effort}/supported")))
                .filter(|effort| REASONING_EFFORTS.contains(effort))
                .map(ToString::to_string)
                .collect()
        } else {
            Vec::new()
        };
    let disabled_accepted = capabilities
        .pointer("/thinking/types/disabled/supported")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    Some(ProviderModelReasoning {
        mandatory: !disabled_accepted && !efforts.is_empty(),
        supported_efforts: Some(efforts),
        default_effort: None,
        default_enabled: None,
        supports_max_tokens: false,
    })
}

pub(super) fn parse_properties(value: &Value) -> ProviderProperties {
    ProviderProperties {
        image_input_declared: value
            .pointer("/capabilities/image_input/supported")
            .and_then(Value::as_bool),
        context_window_tokens: model_context_window_tokens(value),
        ..ProviderProperties::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_bodies_lower_to_cached_messages_requests_without_sampling() {
        let chat = json!({
            "model": "claude-haiku-5-5",
            "messages": [
                {"role": "system", "content": "Follow the schema."},
                {"role": "user", "content": [
                    {"type": "text", "text": "Page 1"},
                    {"type": "image_url", "image_url": {"url": "data:image/png;base64,AAAA"}}
                ]}
            ],
            "temperature": 0.12,
            "max_tokens": 900,
            "stream": false
        });
        let body = lower_chat_body(&chat, "none", Some(json!({"type": "object"})));
        assert_eq!(body["model"], "claude-haiku-5-5");
        assert_eq!(body["max_tokens"], 900);
        assert!(body.get("temperature").is_none());
        assert_eq!(
            body.pointer("/system/0/text"),
            Some(&json!("Follow the schema."))
        );
        assert_eq!(
            body.pointer("/system/0/cache_control/type"),
            Some(&json!("ephemeral"))
        );
        assert_eq!(body.pointer("/messages/0/role"), Some(&json!("user")));
        assert_eq!(
            body.pointer("/messages/0/content/1"),
            Some(&json!({
                "type": "image",
                "source": {"type": "base64", "media_type": "image/png", "data": "AAAA"}
            }))
        );
        assert_eq!(body.pointer("/thinking/type"), Some(&json!("disabled")));
        assert!(body.pointer("/output_config/effort").is_none());
        assert_eq!(
            body.pointer("/output_config/format/type"),
            Some(&json!("json_schema"))
        );
    }

    #[test]
    fn resolved_effort_controls_adaptive_thinking() {
        let chat = json!({"model": "m", "messages": [], "max_tokens": 10});
        let low = lower_chat_body(&chat, "minimal", None);
        assert_eq!(low.pointer("/thinking/type"), Some(&json!("adaptive")));
        assert_eq!(low.pointer("/output_config/effort"), Some(&json!("low")));
        let medium = lower_chat_body(&chat, "medium", None);
        assert_eq!(
            medium.pointer("/output_config/effort"),
            Some(&json!("medium"))
        );
        let automatic = lower_chat_body(&chat, "automatic", None);
        assert!(automatic.get("thinking").is_none());
        assert!(automatic.get("output_config").is_none());
    }

    #[test]
    fn messages_responses_normalize_to_the_completion_shape() {
        let normalized = normalize_message(&json!({
            "id": "msg_1",
            "type": "message",
            "model": "claude-haiku-5-5",
            "content": [
                {"type": "thinking", "thinking": "", "signature": "x"},
                {"type": "text", "text": "OSTUDY_"},
                {"type": "text", "text": "OK"}
            ],
            "stop_reason": "max_tokens",
            "usage": {
                "input_tokens": 10,
                "output_tokens": 4,
                "cache_read_input_tokens": 100,
                "cache_creation_input_tokens": 20
            }
        }))
        .unwrap();
        assert_eq!(
            normalized.pointer("/choices/0/message/content"),
            Some(&json!("OSTUDY_OK"))
        );
        assert_eq!(
            normalized.pointer("/choices/0/finish_reason"),
            Some(&json!("length"))
        );
        assert_eq!(
            normalized.pointer("/usage/prompt_tokens"),
            Some(&json!(130))
        );
        assert_eq!(
            normalized.pointer("/usage/completion_tokens"),
            Some(&json!(4))
        );
        assert_eq!(normalized.pointer("/usage/total_tokens"), Some(&json!(134)));
        assert_eq!(
            normalized.pointer("/usage/prompt_tokens_details/cached_tokens"),
            Some(&json!(100))
        );
        assert_eq!(
            normalized.pointer("/usage/prompt_tokens_details/cache_write_tokens"),
            Some(&json!(20))
        );
    }

    #[test]
    fn model_capabilities_map_to_reasoning_facts() {
        let always_thinks = json!({
            "id": "claude-opus-5-5",
            "capabilities": {
                "thinking": {"supported": true, "types": {
                    "adaptive": {"supported": true},
                    "disabled": {"supported": false},
                    "enabled": {"supported": false}
                }},
                "effort": {
                    "supported": true,
                    "low": {"supported": true}, "medium": {"supported": true},
                    "high": {"supported": true}, "xhigh": {"supported": true},
                    "max": {"supported": true}
                }
            }
        });
        let reasoning = model_reasoning(&always_thinks).unwrap();
        assert!(reasoning.mandatory);
        assert_eq!(
            reasoning.supported_efforts.as_deref(),
            Some(&["low", "medium", "high", "xhigh", "max"].map(String::from)[..])
        );

        let budget_only = json!({
            "id": "claude-haiku-4-5",
            "capabilities": {
                "thinking": {"supported": true, "types": {
                    "adaptive": {"supported": false},
                    "disabled": {"supported": true},
                    "enabled": {"supported": true}
                }},
                "effort": {"supported": false}
            }
        });
        let reasoning = model_reasoning(&budget_only).unwrap();
        assert!(!reasoning.mandatory);
        assert_eq!(reasoning.supported_efforts, Some(Vec::new()));
        assert!(model_reasoning(&json!({"id": "x"})).is_none());
    }

    #[test]
    fn model_detail_urls_escape_the_model_id() {
        let profile = ProfileSnapshot {
            id: "a".repeat(32),
            provider_kind: crate::ai::catalog::ProviderKind::Claude,
            base_url: crate::ai::catalog::CLAUDE_BASE_URL.to_string(),
            model_id: "claude-haiku-5-5".to_string(),
            reasoning_effort: "none".to_string(),
            auth_mode: crate::ai::catalog::AuthMode::Bearer,
            enabled: true,
            allow_insecure_http: false,
            require_zero_data_retention: false,
            request_timeout_ms: 60_000,
            credential_epoch: 0,
            revision: 1,
        };
        assert_eq!(
            model_endpoint(&profile, "a/b").unwrap().as_str(),
            "https://api.anthropic.com/v1/models/a%2Fb"
        );
        assert_eq!(
            models_endpoint(&profile).unwrap().as_str(),
            "https://api.anthropic.com/v1/models?limit=1000"
        );
        assert_eq!(
            messages_endpoint(&profile).unwrap().as_str(),
            "https://api.anthropic.com/v1/messages"
        );
    }
}
