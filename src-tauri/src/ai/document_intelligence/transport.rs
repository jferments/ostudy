use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::{json, Value};
use url::Url;

use crate::{ai::providers, error::CommandError};

use super::super::{
    catalog::ProviderKind,
    endpoint_policy::join_endpoint,
    gateway::{AiGatewayError, SafeAiError},
    profiles::ProfileSnapshot,
    AiRuntimeState,
};
use super::{
    context::DocumentTaskContext,
    contracts::{SourcePageImage, MAX_COMPLETION_BYTES, MAX_INLINE_IMAGES, MAX_REQUEST_JSON_BYTES},
    support::{output_error, truncated_output_error},
};

const DEEPSEEK_STRICT_TOOL_NAME: &str = "submit_ostudy_document_result";

/// Provider-neutral structured task request after a domain module has selected
/// and bounded its canonical source material.
pub(super) struct JsonTaskRequest<'a> {
    pub system_prompt: &'a str,
    pub user_text: &'a str,
    pub schema: &'a Value,
    pub max_tokens: usize,
    pub images: &'a [SourcePageImage],
}

pub(super) async fn request_json(
    runtime: &AiRuntimeState,
    task: &DocumentTaskContext,
    request: JsonTaskRequest<'_>,
) -> Result<Value, CommandError> {
    let profile = task.profile();
    let credential = runtime
        .secrets
        .resolve(&profile.id, profile.auth_mode, task.credential_storage())?;
    let bearer = credential
        .as_ref()
        .map(|secret| secret.as_utf8())
        .transpose()?;

    let (url, body) = match profile.provider_kind {
        ProviderKind::LlamaCpp => {
            let parts = multimodal_user_parts(request.user_text, request.images)?;
            (
                completion_endpoint(profile)?,
                json!({
                    "model": profile.model_id.as_str(),
                    "messages": [
                        { "role": "system", "content": request.system_prompt },
                        { "role": "user", "content": parts }
                    ],
                    "temperature": 0.12,
                    "max_tokens": request.max_tokens,
                    "stream": false,
                    "chat_template_kwargs": { "enable_thinking": false },
                    "response_format": {
                        "type": "json_object",
                        "schema": request.schema
                    }
                }),
            )
        }
        ProviderKind::Deepseek => {
            let strict_schema = deepseek_strict_schema(request.schema);
            (
                deepseek_strict_completion_endpoint(profile)?,
                json!({
                    "model": profile.model_id.as_str(),
                    "messages": [
                        { "role": "system", "content": request.system_prompt },
                        { "role": "user", "content": request.user_text }
                    ],
                    "temperature": 0.12,
                    "max_tokens": request.max_tokens,
                    "stream": false,
                    "thinking": { "type": "disabled" },
                    "tools": [{
                        "type": "function",
                        "function": {
                            "name": DEEPSEEK_STRICT_TOOL_NAME,
                            "description": "Submit the exact structured ostudy document-task result.",
                            "strict": true,
                            "parameters": strict_schema
                        }
                    }],
                    "tool_choice": {
                        "type": "function",
                        "function": { "name": DEEPSEEK_STRICT_TOOL_NAME }
                    }
                }),
            )
        }
        ProviderKind::Openrouter => (
            completion_endpoint(profile)?,
            openrouter_document_body(profile, &task.reasoning_effort, &request)?,
        ),
        _ => {
            return Err(CommandError::new(
                "ai_provider_not_implemented",
                "The selected AI provider cannot analyze documents in this ostudy build.",
            ));
        }
    };

    if serde_json::to_vec(&body)
        .map_err(CommandError::serialization)?
        .len()
        > MAX_REQUEST_JSON_BYTES
    {
        return Err(CommandError::new(
            "ai_request_too_large",
            "This semantic document window is too large for ostudy's bounded provider request.",
        ));
    }

    let first_response = runtime
        .gateway
        .post_document_json(
            url.clone(),
            bearer,
            &body,
            profile.request_timeout_ms,
            MAX_COMPLETION_BYTES,
        )
        .await;
    let response = match first_response {
        Ok(response) => Ok(response),
        Err(first_error)
            if profile.provider_kind == ProviderKind::LlamaCpp
                && first_error.safe.code == "ai_http_bad_request" =>
        {
            let mut compatibility_body = body.clone();
            if let Some(object) = compatibility_body.as_object_mut() {
                object.remove("chat_template_kwargs");
                object.remove("response_format");
                object.insert("json_schema".to_string(), request.schema.clone());
            }
            runtime
                .gateway
                .post_document_json(
                    url,
                    bearer,
                    &compatibility_body,
                    profile.request_timeout_ms,
                    MAX_COMPLETION_BYTES,
                )
                .await
        }
        Err(error) => Err(error),
    }
    .map_err(gateway_to_command)?;

    record_document_usage(task, &response);
    ensure_document_response_not_blocked(&response)?;
    let finish_reason = response
        .pointer("/choices/0/finish_reason")
        .and_then(Value::as_str);
    let parsed = match profile.provider_kind {
        ProviderKind::Deepseek => parse_strict_tool_arguments(&response),
        _ => parse_completion_content(&response).and_then(|content| {
            serde_json::from_str(normalize_json_content(content)).map_err(|_| {
                CommandError::new(
                    "ai_document_output_invalid",
                    "The AI provider returned content that was not valid JSON for the document task.",
                )
            })
        }),
    };
    match parsed {
        Ok(value) => Ok(value),
        Err(_) if finish_reason == Some("length") => Err(truncated_output_error()),
        Err(error) => Err(error),
    }
}

fn multimodal_user_parts(
    user_text: &str,
    images: &[SourcePageImage],
) -> Result<Vec<Value>, CommandError> {
    let mut parts = vec![json!({ "type": "text", "text": user_text })];
    for image in images.iter().take(MAX_INLINE_IMAGES) {
        let Some(media_type) = document_image_media_type(&image.bytes) else {
            continue;
        };
        parts.push(json!({
            "type": "text",
            "text": format!(
                "Rendered source unit {} follows. Use it only as source evidence for diagrams, tables, equations, and layout-dependent or image-only content.",
                image.page_number
            )
        }));
        parts.push(json!({
            "type": "image_url",
            "image_url": {
                "url": format!(
                    "data:{media_type};base64,{}",
                    STANDARD.encode(&image.bytes)
                )
            }
        }));
    }
    Ok(parts)
}

fn document_image_media_type(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]) {
        return Some("image/png");
    }
    if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        return Some("image/jpeg");
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some("image/gif");
    }
    if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        return Some("image/webp");
    }
    None
}

fn openrouter_document_body(
    profile: &ProfileSnapshot,
    reasoning_effort: &str,
    request: &JsonTaskRequest<'_>,
) -> Result<Value, CommandError> {
    let user_content = if request.images.is_empty() {
        Value::String(request.user_text.to_string())
    } else {
        Value::Array(multimodal_user_parts(request.user_text, request.images)?)
    };
    let mut body = json!({
        "model": profile.model_id.as_str(),
        "messages": [
            { "role": "system", "content": request.system_prompt },
            { "role": "user", "content": user_content }
        ],
        "temperature": 0.12,
        "max_tokens": request.max_tokens,
        "stream": false,
        "plugins": [{
            "id": "context-compression",
            "enabled": false
        }],
        "provider": providers::openrouter_provider_preferences(
            true,
            profile.require_zero_data_retention,
        ),
        "response_format": {
            "type": "json_schema",
            "json_schema": {
                "name": "ostudy_document_task",
                "strict": true,
                "schema": providers::openrouter_strict_schema(request.schema)
            }
        }
    });
    if reasoning_effort != "none" && reasoning_effort != "automatic" {
        body["reasoning"] = json!({
            "effort": reasoning_effort,
            "exclude": true
        });
    }
    Ok(body)
}

fn record_document_usage(task: &DocumentTaskContext, response: &Value) {
    let (prompt_tokens, completion_tokens, total_tokens) = response_token_usage(response);
    let cached_prompt_tokens = response
        .pointer("/usage/prompt_tokens_details/cached_tokens")
        .and_then(nonnegative_json_integer);
    let cache_write_tokens = response
        .pointer("/usage/prompt_tokens_details/cache_write_tokens")
        .and_then(nonnegative_json_integer);
    let reasoning_tokens = response
        .pointer("/usage/completion_tokens_details/reasoning_tokens")
        .and_then(nonnegative_json_integer);
    task.record_request_usage(total_tokens);
    crate::logging::debug_target(
        "ai.document.usage",
        "document_ai_usage",
        "Recorded provider token usage without logging prompt or model content.",
        json!({
            "provider": task.provider_kind.as_str(),
            "model": task.model_id.as_str(),
            "reported": total_tokens.is_some(),
            "promptTokens": prompt_tokens,
            "completionTokens": completion_tokens,
            "totalTokens": total_tokens,
            "cachedPromptTokens": cached_prompt_tokens,
            "cacheWriteTokens": cache_write_tokens,
            "reasoningTokens": reasoning_tokens,
        }),
    );
}

fn response_token_usage(response: &Value) -> (Option<u64>, Option<u64>, Option<u64>) {
    let read = |pointers: &[&str]| {
        pointers
            .iter()
            .find_map(|pointer| response.pointer(pointer).and_then(nonnegative_json_integer))
    };
    let standard_prompt = read(&[
        "/usage/prompt_tokens",
        "/usage/input_tokens",
        "/response/usage/prompt_tokens",
        "/data/usage/prompt_tokens",
    ]);
    let standard_completion = read(&[
        "/usage/completion_tokens",
        "/usage/output_tokens",
        "/response/usage/completion_tokens",
        "/data/usage/completion_tokens",
    ]);
    let standard_total = read(&[
        "/usage/total_tokens",
        "/response/usage/total_tokens",
        "/data/usage/total_tokens",
    ]);
    if standard_prompt.is_some() || standard_completion.is_some() || standard_total.is_some() {
        let total = standard_total.or_else(|| {
            standard_prompt
                .zip(standard_completion)
                .map(|(left, right)| left.saturating_add(right))
        });
        return (standard_prompt, standard_completion, total);
    }

    let prompt_processed = read(&[
        "/timings/prompt_n",
        "/__verbose/timings/prompt_n",
        "/tokens_evaluated",
        "/__verbose/tokens_evaluated",
    ]);
    let cached = read(&[
        "/timings/cache_n",
        "/__verbose/timings/cache_n",
        "/tokens_cached",
        "/__verbose/tokens_cached",
    ])
    .unwrap_or(0);
    let completion = read(&[
        "/timings/predicted_n",
        "/__verbose/timings/predicted_n",
        "/tokens_predicted",
        "/__verbose/tokens_predicted",
    ]);
    let prompt = prompt_processed.map(|value| value.saturating_add(cached));
    let total = prompt
        .zip(completion)
        .map(|(left, right)| left.saturating_add(right));
    (prompt, completion, total)
}

fn nonnegative_json_integer(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_i64().and_then(|value| u64::try_from(value).ok()))
}

fn deepseek_strict_completion_endpoint(profile: &ProfileSnapshot) -> Result<Url, CommandError> {
    join_endpoint(&profile.base_url, "beta/chat/completions")
}

fn completion_endpoint(profile: &ProfileSnapshot) -> Result<Url, CommandError> {
    join_endpoint(&profile.base_url, "chat/completions")
}

fn parse_strict_tool_arguments(value: &Value) -> Result<Value, CommandError> {
    let function = value
        .pointer("/choices/0/message/tool_calls/0/function")
        .ok_or_else(|| {
            output_error("DeepSeek did not return the required schema-constrained tool result.")
        })?;
    if function.get("name").and_then(Value::as_str) != Some(DEEPSEEK_STRICT_TOOL_NAME) {
        return Err(output_error(
            "DeepSeek returned an unexpected structured-output tool name.",
        ));
    }
    let arguments = function
        .get("arguments")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| output_error("DeepSeek returned empty structured-output arguments."))?;
    serde_json::from_str(arguments)
        .map_err(|_| output_error("DeepSeek returned tool arguments that were not valid JSON."))
}

fn parse_completion_content(value: &Value) -> Result<&str, CommandError> {
    value
        .pointer("/choices/0/message/content")
        .and_then(Value::as_str)
        .or_else(|| value.pointer("/choices/0/text").and_then(Value::as_str))
        .filter(|content| !content.trim().is_empty())
        .ok_or_else(|| output_error("The AI provider returned no structured document content."))
}

fn normalize_json_content(value: &str) -> &str {
    let trimmed = value.trim();
    let without_open = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```JSON"))
        .or_else(|| trimmed.strip_prefix("```"))
        .unwrap_or(trimmed)
        .trim();
    without_open
        .strip_suffix("```")
        .unwrap_or(without_open)
        .trim()
}

fn deepseek_strict_schema(value: &Value) -> Value {
    match value {
        Value::Array(values) => Value::Array(values.iter().map(deepseek_strict_schema).collect()),
        Value::Object(object) => {
            if let Some(constant) = object.get("const") {
                let value_type = match constant {
                    Value::String(_) => "string",
                    Value::Number(number) if number.is_i64() || number.is_u64() => "integer",
                    Value::Number(_) => "number",
                    Value::Bool(_) => "boolean",
                    _ => "string",
                };
                return json!({
                    "type": value_type,
                    "enum": [constant.clone()]
                });
            }
            let mut normalized = serde_json::Map::new();
            for (key, child) in object {
                match key.as_str() {
                    "minLength" | "maxLength" | "minItems" | "maxItems" | "required" => {}
                    "oneOf" => {
                        normalized.insert("anyOf".to_string(), deepseek_strict_schema(child));
                    }
                    _ => {
                        normalized.insert(key.clone(), deepseek_strict_schema(child));
                    }
                }
            }
            if let Some(properties) = normalized.get("properties").and_then(Value::as_object) {
                normalized.insert(
                    "required".to_string(),
                    Value::Array(properties.keys().cloned().map(Value::String).collect()),
                );
                normalized.insert("additionalProperties".to_string(), Value::Bool(false));
            }
            Value::Object(normalized)
        }
        _ => value.clone(),
    }
}

fn ensure_document_response_not_blocked(response: &Value) -> Result<(), CommandError> {
    let finish_reason = response
        .pointer("/choices/0/finish_reason")
        .and_then(Value::as_str);
    let refusal = response
        .pointer("/choices/0/message/refusal")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty());
    if finish_reason == Some("content_filter") || refusal.is_some() {
        return Err(CommandError::new(
            "ai_content_policy_rejected",
            "The AI provider stopped this document request because of its content policy.",
        ));
    }
    Ok(())
}

fn gateway_to_command(error_value: AiGatewayError) -> CommandError {
    CommandError::new(error_value.safe.code, error_value.safe.message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_media_type_is_content_authoritative() {
        assert_eq!(
            document_image_media_type(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]),
            Some("image/png")
        );
        assert_eq!(document_image_media_type(b"not-an-image"), None);
    }

    #[test]
    fn deepseek_schema_requires_all_properties_and_removes_unsupported_bounds() {
        let normalized = deepseek_strict_schema(&json!({
            "type": "object",
            "properties": {
                "name": {"type": "string", "minLength": 1, "maxLength": 20},
                "kind": {"const": "basic"}
            },
            "required": ["name"]
        }));
        assert_eq!(normalized["additionalProperties"], false);
        assert_eq!(normalized["required"], json!(["kind", "name"]));
        assert!(normalized["properties"]["name"].get("minLength").is_none());
        assert_eq!(normalized["properties"]["kind"]["enum"], json!(["basic"]));
    }

    #[test]
    fn normalizes_optional_markdown_json_fences() {
        assert_eq!(normalize_json_content("```json\n{\"ok\":true}\n```"), "{\"ok\":true}");
        assert_eq!(normalize_json_content(" {\"ok\":true} "), "{\"ok\":true}");
    }

    #[test]
    fn openrouter_document_policy_disables_compression_and_excludes_reasoning() {
        let profile = ProfileSnapshot {
            id: "a".repeat(32),
            provider_kind: ProviderKind::Openrouter,
            base_url: "https://openrouter.ai/api/v1/".to_string(),
            model_id: "openai/example".to_string(),
            auth_mode: super::super::super::catalog::AuthMode::Bearer,
            enabled: true,
            allow_insecure_http: false,
            require_zero_data_retention: false,
            request_timeout_ms: 60_000,
            credential_epoch: 0,
            revision: 1,
        };
        let request = JsonTaskRequest {
            system_prompt: "system",
            user_text: "user",
            schema: &json!({"type":"object"}),
            max_tokens: 100,
            images: &[],
        };
        let body = openrouter_document_body(&profile, "low", &request).unwrap();
        assert_eq!(body["plugins"][0]["id"], "context-compression");
        assert_eq!(body["plugins"][0]["enabled"], false);
        assert_eq!(body["reasoning"]["effort"], "low");
        assert_eq!(body["reasoning"]["exclude"], true);
        assert!(body.get("session_id").is_none());
    }

    #[test]
    fn safe_error_contract_remains_serializable() {
        let error = SafeAiError {
            code: "code".to_string(),
            message: "message".to_string(),
            retryable: false,
            retry_after_seconds: None,
        };
        assert_eq!(serde_json::to_value(error).unwrap()["retryable"], false);
    }
}
