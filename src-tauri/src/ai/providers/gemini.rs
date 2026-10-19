//! Google's Gemini API (`generateContent`). The shared chat body is lowered
//! to Gemini's `contents`/`systemInstruction`/`generationConfig` at the edge,
//! and one-shot responses are normalized back to the chat-completion shape;
//! the gateway decodes streamed chunks itself.

use serde_json::{json, Map, Value};
use url::Url;

use crate::ai::{
    endpoint_policy::join_endpoint,
    gateway::{gemini_finish_reason, gemini_usage_envelope, AiGatewayError},
    profiles::ProfileSnapshot,
};

use super::{
    dialect::command_to_gateway, schema as schema_support, ProviderModelReasoning,
    ProviderProperties,
};

const MODEL_PAGE_SIZE: &str = "1000";
const THINKING_PROBE_MAX_TOKENS: usize = 2_048;
/// Model ids naming another modality or a non-chat use.
const NON_CHAT_MARKERS: [&str; 8] = [
    "embedding",
    "image",
    "tts",
    "live",
    "audio",
    "aqa",
    "robotics",
    "computer-use",
];

pub(super) fn generate_endpoint(profile: &ProfileSnapshot) -> Result<Url, AiGatewayError> {
    let mut url = join_endpoint(&profile.base_url, "models").map_err(command_to_gateway)?;
    url.path_segments_mut()
        .map_err(|()| invalid_endpoint())?
        .push(&format!("{}:generateContent", profile.model_id));
    Ok(url)
}

pub(super) fn models_endpoint(profile: &ProfileSnapshot) -> Result<Url, AiGatewayError> {
    let mut url = join_endpoint(&profile.base_url, "models").map_err(command_to_gateway)?;
    url.query_pairs_mut()
        .append_pair("pageSize", MODEL_PAGE_SIZE);
    Ok(url)
}

pub(super) fn model_endpoint(
    profile: &ProfileSnapshot,
    model_id: &str,
) -> Result<Url, AiGatewayError> {
    let mut url = join_endpoint(&profile.base_url, "models").map_err(command_to_gateway)?;
    url.path_segments_mut()
        .map_err(|()| invalid_endpoint())?
        .push(model_id);
    Ok(url)
}

/// Lower the shared chat body to a Gemini request. `effort` is already
/// resolved against the model's declared efforts; "automatic" leaves the
/// model's own thinking default. A schema is enforced with
/// `responseJsonSchema`; JSON mode alone sets only the response MIME type.
pub(super) fn lower_chat_body(chat: &Value, effort: &str, schema: Option<Value>) -> Value {
    let model = chat
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let mut system = Vec::new();
    let mut contents = Vec::new();
    for message in chat
        .get("messages")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let content = message.get("content").unwrap_or(&Value::Null);
        match message.get("role").and_then(Value::as_str) {
            Some("system") => system.extend(lower_parts(content)),
            Some("assistant") => {
                contents.push(json!({"role": "model", "parts": lower_parts(content)}))
            }
            _ => contents.push(json!({"role": "user", "parts": lower_parts(content)})),
        }
    }

    let mut config = Map::new();
    if let Some(max_tokens) = chat.get("max_tokens").filter(|value| value.is_u64()) {
        config.insert("maxOutputTokens".to_string(), max_tokens.clone());
    }
    // Gemini 3 is tuned for its default temperature; lower values can loop.
    if !is_gemini_3(model) {
        if let Some(temperature) = chat.get("temperature").filter(|value| value.is_number()) {
            config.insert("temperature".to_string(), temperature.clone());
        }
    }
    let json_mode = chat
        .pointer("/response_format/type")
        .and_then(Value::as_str)
        .is_some();
    if let Some(schema) = schema {
        config.insert("responseMimeType".to_string(), json!("application/json"));
        config.insert("responseJsonSchema".to_string(), schema);
    } else if json_mode {
        config.insert("responseMimeType".to_string(), json!("application/json"));
    }
    if let Some(thinking) = thinking_config(model, effort) {
        config.insert("thinkingConfig".to_string(), thinking);
    }

    let mut body = json!({ "contents": contents, "generationConfig": config });
    if !system.is_empty() {
        body["systemInstruction"] = json!({ "parts": system });
    }
    body
}

fn lower_parts(content: &Value) -> Vec<Value> {
    match content {
        Value::String(text) => vec![json!({ "text": text })],
        Value::Array(parts) => parts.iter().filter_map(lower_part).collect(),
        _ => Vec::new(),
    }
}

fn lower_part(part: &Value) -> Option<Value> {
    match part.get("type").and_then(Value::as_str) {
        Some("text") => Some(json!({ "text": part.get("text")?.as_str()? })),
        Some("image_url") => {
            let url = part.pointer("/image_url/url")?.as_str()?;
            let (header, data) = url.strip_prefix("data:")?.split_once(',')?;
            let mime_type = header.strip_suffix(";base64")?;
            Some(json!({ "inlineData": { "mimeType": mime_type, "data": data } }))
        }
        _ => None,
    }
}

/// Thinking settings by model family. Gemini 3 takes a thinking level and
/// cannot turn thinking off; Gemini 2.5 takes a token budget, and only its
/// Flash models accept a zero budget.
fn thinking_config(model: &str, effort: &str) -> Option<Value> {
    let efforts = reasoning_efforts(model)?;
    if effort == "automatic" || !efforts.contains(&effort) {
        return None;
    }
    if is_gemini_3(model) {
        return Some(json!({ "thinkingLevel": effort.to_ascii_uppercase() }));
    }
    let budget = match effort {
        "none" => 0,
        "low" => 1_024,
        "medium" => 8_192,
        _ => 24_576,
    };
    Some(json!({ "thinkingBudget": budget }))
}

fn is_gemini_3(model: &str) -> bool {
    model.to_ascii_lowercase().starts_with("gemini-3")
}

fn reasoning_efforts(model: &str) -> Option<&'static [&'static str]> {
    let model = model.to_ascii_lowercase();
    if model.starts_with("gemini-3") {
        return Some(if model.contains("pro") {
            &["low", "high"]
        } else {
            &["minimal", "low", "medium", "high"]
        });
    }
    if model.starts_with("gemini-2.5-pro") {
        return Some(&["low", "medium", "high"]);
    }
    if model.starts_with("gemini-2.5-flash") {
        return Some(&["none", "low", "medium", "high"]);
    }
    None
}

/// Probes ask for a few visible tokens; a model that thinks spends hidden
/// tokens first, so its cap leaves room for them.
pub(super) fn probe_max_tokens(model: &str, effort: &str, visible_tokens: usize) -> usize {
    match reasoning_efforts(model) {
        Some(efforts) if !(effort == "none" && efforts.contains(&"none")) => {
            visible_tokens.max(THINKING_PROBE_MAX_TOKENS)
        }
        _ => visible_tokens,
    }
}

pub(super) fn strict_schema(schema: &Value) -> Value {
    schema_support::portable_strict_schema(schema)
}

/// Bring a one-shot `generateContent` response to the chat-completion shape.
pub(super) fn normalize_response(value: &Value) -> Result<Value, AiGatewayError> {
    if !value.is_object() {
        return Err(crate::ai::gateway::error(
            "ai_provider_protocol_error",
            "The Gemini response did not use the expected shape.",
            false,
            None,
        ));
    }
    let candidate = value
        .get("candidates")
        .and_then(Value::as_array)
        .and_then(|candidates| candidates.first());
    let content = candidate
        .and_then(|candidate| candidate.pointer("/content/parts"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|part| part.get("thought").and_then(Value::as_bool) != Some(true))
        .filter_map(|part| part.get("text").and_then(Value::as_str))
        .collect::<String>();
    let finish_reason = if value.pointer("/promptFeedback/blockReason").is_some() {
        Some("content_filter")
    } else {
        candidate
            .and_then(|candidate| candidate.get("finishReason"))
            .and_then(Value::as_str)
            .and_then(gemini_finish_reason)
    };
    let mut normalized = json!({
        "model": value.get("modelVersion").cloned().unwrap_or(Value::Null),
        "choices": [{
            "index": 0,
            "message": { "role": "assistant", "content": content },
            "finish_reason": finish_reason,
        }],
    });
    if let Some(usage) = value.get("usageMetadata").and_then(gemini_usage_envelope) {
        normalized["usage"] = usage;
    }
    Ok(normalized)
}

/// Gemini's model list as the shared `{"data": [...]}` shape, keeping only
/// models that generate content and are chat models.
pub(super) fn model_list_as_data(value: &Value) -> Value {
    let Some(models) = value.get("models").and_then(Value::as_array) else {
        // Let the shared parser report the unexpected shape.
        return value.clone();
    };
    let data = models
        .iter()
        .filter(|model| {
            model
                .get("supportedGenerationMethods")
                .and_then(Value::as_array)
                .is_some_and(|methods| {
                    methods
                        .iter()
                        .any(|method| method.as_str() == Some("generateContent"))
                })
        })
        .filter_map(|model| {
            let id = model.get("name")?.as_str()?.strip_prefix("models/")?;
            is_chat_model(id).then(|| {
                let mut entry = model.clone();
                entry["id"] = json!(id);
                entry
            })
        })
        .collect::<Vec<_>>();
    json!({ "data": data })
}

fn is_chat_model(id: &str) -> bool {
    let id = id.to_ascii_lowercase();
    (id.starts_with("gemini-") || id.starts_with("gemma-"))
        && !NON_CHAT_MARKERS.iter().any(|marker| id.contains(marker))
}

pub(super) fn model_display_name(item: &Value, id: &str) -> String {
    item.get("displayName")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty() && name.len() <= 200 && !name.chars().any(char::is_control))
        .unwrap_or(id)
        .to_string()
}

pub(super) fn model_context_window_tokens(item: &Value) -> Option<i64> {
    item.get("inputTokenLimit")
        .and_then(Value::as_i64)
        .filter(|tokens| *tokens > 0)
}

pub(super) fn model_reasoning(item: &Value, id: &str) -> Option<ProviderModelReasoning> {
    if item.get("thinking").and_then(Value::as_bool) == Some(false) {
        return None;
    }
    let efforts = reasoning_efforts(id)?;
    let mandatory = !efforts.contains(&"none");
    Some(ProviderModelReasoning {
        supported_efforts: Some(efforts.iter().map(ToString::to_string).collect()),
        default_effort: None,
        default_enabled: Some(true),
        supports_max_tokens: false,
        mandatory,
    })
}

/// Every Gemini chat model takes image input; Gemma text models may not.
pub(super) fn model_image_input(id: &str) -> Option<bool> {
    Some(id.to_ascii_lowercase().starts_with("gemini-"))
}

pub(super) fn parse_properties(value: &Value) -> ProviderProperties {
    ProviderProperties {
        image_input_declared: value
            .get("name")
            .and_then(Value::as_str)
            .and_then(|name| name.strip_prefix("models/"))
            .and_then(model_image_input),
        context_window_tokens: model_context_window_tokens(value),
        ..ProviderProperties::default()
    }
}

fn invalid_endpoint() -> AiGatewayError {
    crate::ai::gateway::error(
        "invalid_ai_endpoint",
        "The Gemini endpoint could not be built.",
        false,
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chat(model: &str) -> Value {
        json!({
            "model": model,
            "messages": [
                {"role": "system", "content": "Be brief."},
                {"role": "user", "content": [
                    {"type": "text", "text": "What is this?"},
                    {"type": "image_url", "image_url": {"url": "data:image/png;base64,iVBORw0KGgo="}}
                ]}
            ],
            "temperature": 0.12,
            "max_tokens": 900,
            "stream": false,
        })
    }

    #[test]
    fn chat_bodies_become_gemini_requests() {
        let body = lower_chat_body(&chat("gemini-2.5-flash"), "none", None);
        assert!(body.get("model").is_none());
        assert!(body.get("stream").is_none());
        assert_eq!(body["systemInstruction"]["parts"][0]["text"], "Be brief.");
        assert_eq!(body["contents"][0]["role"], "user");
        assert_eq!(body["contents"][0]["parts"][0]["text"], "What is this?");
        assert_eq!(
            body["contents"][0]["parts"][1]["inlineData"]["mimeType"],
            "image/png"
        );
        assert_eq!(body["generationConfig"]["maxOutputTokens"], 900);
        assert_eq!(body["generationConfig"]["temperature"], 0.12);
        assert_eq!(
            body["generationConfig"]["thinkingConfig"]["thinkingBudget"],
            0
        );
    }

    #[test]
    fn gemini_3_uses_thinking_levels_and_default_temperature() {
        let schema = json!({"type": "object", "properties": {}, "additionalProperties": false});
        let body = lower_chat_body(&chat("gemini-3-flash"), "minimal", Some(schema.clone()));
        let config = &body["generationConfig"];
        assert!(config.get("temperature").is_none());
        assert_eq!(config["thinkingConfig"]["thinkingLevel"], "MINIMAL");
        assert_eq!(config["responseMimeType"], "application/json");
        assert_eq!(config["responseJsonSchema"], schema);

        let automatic = lower_chat_body(&chat("gemini-3-pro"), "automatic", None);
        assert!(automatic["generationConfig"]
            .get("thinkingConfig")
            .is_none());
    }

    #[test]
    fn families_declare_whether_thinking_can_stop() {
        let flash = model_reasoning(&json!({"thinking": true}), "gemini-2.5-flash-lite").unwrap();
        assert!(!flash.mandatory);
        let pro = model_reasoning(&json!({"thinking": true}), "gemini-2.5-pro").unwrap();
        assert!(pro.mandatory);
        let gemini3 = model_reasoning(&json!({"thinking": true}), "gemini-3-pro").unwrap();
        assert_eq!(
            gemini3.supported_efforts.as_deref(),
            Some(&["low".to_string(), "high".to_string()][..])
        );
        assert!(model_reasoning(&json!({"thinking": false}), "gemini-2.5-flash").is_none());
        assert!(model_reasoning(&json!({}), "gemma-3-27b-it").is_none());
    }

    #[test]
    fn responses_normalize_to_chat_completions() {
        let normalized = normalize_response(&json!({
            "candidates": [{
                "content": {"role": "model", "parts": [
                    {"text": "planning", "thought": true},
                    {"text": "OSTUDY_"},
                    {"text": "OK"}
                ]},
                "finishReason": "STOP"
            }],
            "usageMetadata": {
                "promptTokenCount": 12,
                "candidatesTokenCount": 3,
                "thoughtsTokenCount": 40,
                "cachedContentTokenCount": 8
            },
            "modelVersion": "gemini-2.5-flash"
        }))
        .unwrap();
        assert_eq!(normalized["choices"][0]["message"]["content"], "OSTUDY_OK");
        assert_eq!(normalized["choices"][0]["finish_reason"], "stop");
        assert_eq!(normalized["usage"]["prompt_tokens"], 12);
        assert_eq!(normalized["usage"]["completion_tokens"], 43);
        assert_eq!(
            normalized["usage"]["completion_tokens_details"]["reasoning_tokens"],
            40
        );
        assert_eq!(
            normalized["usage"]["prompt_tokens_details"]["cached_tokens"],
            8
        );

        let blocked = normalize_response(&json!({
            "promptFeedback": {"blockReason": "SAFETY"}
        }))
        .unwrap();
        assert_eq!(blocked["choices"][0]["finish_reason"], "content_filter");
    }

    #[test]
    fn model_lists_keep_generating_chat_models() {
        let data = model_list_as_data(&json!({"models": [
            {"name": "models/gemini-2.5-flash", "supportedGenerationMethods": ["generateContent", "countTokens"]},
            {"name": "models/gemini-embedding-001", "supportedGenerationMethods": ["embedContent"]},
            {"name": "models/gemini-2.5-flash-preview-tts", "supportedGenerationMethods": ["generateContent"]},
            {"name": "models/gemini-2.5-flash-image", "supportedGenerationMethods": ["generateContent"]}
        ]}));
        let ids = data["data"]
            .as_array()
            .unwrap()
            .iter()
            .map(|model| model["id"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(ids, vec!["gemini-2.5-flash"]);
    }

    #[test]
    fn endpoints_put_the_model_in_the_path() {
        let profile = ProfileSnapshot {
            id: "a".repeat(32),
            provider_kind: crate::ai::catalog::ProviderKind::Gemini,
            base_url: "https://generativelanguage.googleapis.com/v1beta/".to_string(),
            model_id: "gemini-2.5-flash".to_string(),
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
            generate_endpoint(&profile).unwrap().as_str(),
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:generateContent"
        );
        assert_eq!(
            models_endpoint(&profile).unwrap().as_str(),
            "https://generativelanguage.googleapis.com/v1beta/models?pageSize=1000"
        );
        assert_eq!(
            model_endpoint(&profile, "gemini-2.5-pro").unwrap().as_str(),
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-pro"
        );
    }
}
