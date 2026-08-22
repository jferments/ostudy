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
    let bearer = credential.as_ref().map(|secret| secret.as_utf8()).transpose()?;

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
                    "response_format": { "type": "json_object", "schema": request.schema }
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
                    "tool_choice": { "type": "function", "function": { "name": DEEPSEEK_STRICT_TOOL_NAME } }
                }),
            )
        }
        ProviderKind::Openrouter => (
            completion_endpoint(profile)?,
            openrouter_document_body(profile, &task.reasoning_effort, &request)?,
        ),
        _ => return Err(CommandError::new(
            "ai_provider_not_implemented",
            "The selected AI provider cannot analyze documents in this ostudy build.",
        )),
    };

    if serde_json::to_vec(&body).map_err(CommandError::serialization)?.len() > MAX_REQUEST_JSON_BYTES {
        return Err(CommandError::new(
            "ai_request_too_large",
            "This semantic document window is too large for ostudy's bounded provider request.",
        ));
    }

    let first_response = runtime.gateway.post_json(
        url.clone(), bearer, &body, profile.request_timeout_ms, MAX_COMPLETION_BYTES,
    ).await;
    let response = match first_response {
        Ok(response) => Ok(response),
        Err(first_error) if profile.provider_kind == ProviderKind::LlamaCpp
            && first_error.safe.code == "ai_http_bad_request" => {
            let mut compatibility_body = body.clone();
            if let Some(object) = compatibility_body.as_object_mut() {
                object.remove("chat_template_kwargs");
                object.remove("response_format");
                object.insert("json_schema".to_string(), request.schema.clone());
            }
            runtime.gateway.post_json(
                url, bearer, &compatibility_body, profile.request_timeout_ms, MAX_COMPLETION_BYTES,
            ).await
        }
        Err(error) => Err(error),
    }.map_err(gateway_to_command)?;

    record_document_usage(task, &response);
    ensure_document_response_not_blocked(&response)?;
    let finish_reason = response.pointer("/choices/0/finish_reason").and_then(Value::as_str);
    let parsed = match profile.provider_kind {
        ProviderKind::Deepseek => parse_strict_tool_arguments(&response),
        _ => parse_completion_content(&response).and_then(|content| {
            serde_json::from_str(normalize_json_content(content)).map_err(|_| CommandError::new(
                "ai_document_output_invalid",
                "The AI provider returned content that was not valid JSON for the document task.",
            ))
        }),
    };
    match parsed {
        Ok(value) => Ok(value),
        Err(_) if finish_reason == Some("length") => Err(truncated_output_error()),
        Err(error) => Err(error),
    }
}

fn multimodal_user_parts(user_text: &str, images: &[SourcePageImage]) -> Result<Vec<Value>, CommandError> {
    let mut parts = vec![json!({ "type": "text", "text": user_text })];
    for image in images.iter().take(MAX_INLINE_IMAGES) {
        let Some(media_type) = document_image_media_type(&image.bytes) else { continue; };
        parts.push(json!({
            "type": "text",
            "text": format!(
                "Rendered source unit {} follows. Use it only as source evidence for diagrams, tables, equations, and layout-dependent or image-only content.",
                image.page_number
            )
        }));
        parts.push(json!({
            "type": "image_url",
            "image_url": { "url": format!("data:{media_type};base64,{}", STANDARD.encode(&image.bytes)) }
        }));
    }
    Ok(parts)
}

fn document_image_media_type(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]) { return Some("image/png"); }
    if bytes.starts_with(&[0xff, 0xd8, 0xff]) { return Some("image/jpeg"); }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") { return Some("image/gif"); }
    if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" { return Some("image/webp"); }
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
        "provider": providers::openrouter_provider_preferences(true, profile.require_zero_data_retention),
        "response_format": {
            "type": "json_schema",
            "json_schema": {
                "name": "ostudy_document_task",
                "strict": true,
                "schema": providers::openrouter_strict_schema(request.schema)
            }
        }
    });
    if reasoning_effort != "none" { body["reasoning"] = json!({"effort": reasoning_effort}); }
    Ok(body)
}

fn record_document_usage(task: &DocumentTaskContext, response: &Value) {
    let (prompt_tokens, completion_tokens, total_tokens) = response_token_usage(response);
    let cached_prompt_tokens = response.pointer("/usage/prompt_tokens_details/cached_tokens").and_then(nonnegative_json_integer);
    let cache_write_tokens = response.pointer("/usage/prompt_tokens_details/cache_write_tokens").and_then(nonnegative_json_integer);
    let reasoning_tokens = response.pointer("/usage/completion_tokens_details/reasoning_tokens").and_then(nonnegative_json_integer);
    task.record_request_usage(total_tokens);
    crate::logging::debug_target(
        "ai.document.usage", "document_ai_usage",
        "Recorded provider token usage without logging prompt or model content.",
        json!({
            "provider": task.provider_kind.as_str(), "model": task.model_id.as_str(),
            "reported": total_tokens.is_some(), "promptTokens": prompt_tokens,
            "completionTokens": completion_tokens, "totalTokens": total_tokens,
            "cachedPromptTokens": cached_prompt_tokens, "cacheWriteTokens": cache_write_tokens,
            "reasoningTokens": reasoning_tokens,
        }),
    );
}

fn response_token_usage(response: &Value) -> (Option<u64>, Option<u64>, Option<u64>) {
    let read = |pointers: &[&str]| pointers.iter().find_map(|pointer| response.pointer(pointer).and_then(nonnegative_json_integer));
    let standard_prompt = read(&["/usage/prompt_tokens", "/usage/input_tokens", "/response/usage/prompt_tokens", "/data/usage/prompt_tokens"]);
    let standard_completion = read(&["/usage/completion_tokens", "/usage/output_tokens", "/response/usage/completion_tokens", "/data/usage/completion_tokens"]);
    let standard_total = read(&["/usage/total_tokens", "/response/usage/total_tokens", "/data/usage/total_tokens"]);
    if standard_prompt.is_some() || standard_completion.is_some() || standard_total.is_some() {
        let total = standard_total.or_else(|| standard_prompt.zip(standard_completion).map(|(left, right)| left.saturating_add(right)));
        return (standard_prompt, standard_completion, total);
    }
    let prompt_processed = read(&["/timings/prompt_n", "/__verbose/timings/prompt_n", "/tokens_evaluated", "/__verbose/tokens_evaluated"]);
    let cached = read(&["/timings/cache_n", "/__verbose/timings/cache_n", "/tokens_cached", "/__verbose/tokens_cached"]).unwrap_or(0);
    let completion = read(&["/timings/predicted_n", "/__verbose/timings/predicted_n", "/tokens_predicted", "/__verbose/tokens_predicted"]);
    let prompt = prompt_processed.map(|value| value.saturating_add(cached));
    let total = prompt.zip(completion).map(|(left, right)| left.saturating_add(right));
    (prompt, completion, total)
}

fn nonnegative_json_integer(value: &Value) -> Option<u64> {
    value.as_u64().or_else(|| value.as_i64().and_then(|value| u64::try_from(value).ok()))
}

fn deepseek_strict_completion_endpoint(profile: &ProfileSnapshot) -> Result<Url, CommandError> {
    join_endpoint(&profile.base_url, "beta/chat/completions")
}

fn parse_strict_tool_arguments(value: &Value) -> Result<Value, CommandError> {
    let function = value.pointer("/choices/0/message/tool_calls/0/function")
        .ok_or_else(|| output_error("DeepSeek did not return the required schema-constrained tool result."))?;
    if function.get("name").and_then(Value::as_str) != Some(DEEPSEEK_STRICT_TOOL_NAME) {
        return Err(output_error("DeepSeek returned an unexpected structured-output tool name."));
    }
    let arguments = function.get("arguments").and_then(Value::as_str).filter(|value| !value.trim().is_empty())
        .ok_or_else(|| output_error("DeepSeek returned empty structured-output arguments."))?;
    serde_json::from_str(arguments).map_err(|_| output_error("DeepSeek returned tool arguments that were not valid JSON."))
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
                return json!({ "type": value_type, "enum": [constant.clone()] });
            }
            let mut normalized = serde_json::Map::new();
            for (key, child) in object {
                match key.as_str() {
                    "minLength" | "maxLength" | "minItems" | "maxItems" | "required" => {}
                    "oneOf" => { normalized.insert("anyOf".to_string(), deepseek_strict_schema(child)); }
                    _ => { normalized.insert(key.clone(), deepseek_strict_schema(child)); }
                }
            }
            if let Some(properties) = normalized.get("properties").and_then(Value::as_object) {
                let required = properties.keys().cloned().map(Value::String).collect::<Vec<_>>();
                normalized.insert("required".to_string(), Value::Array(required));
                normalized.insert("additionalProperties".to_string(), Value::Bool(false));
            }
            Value::Object(normalized)
        }
        _ => value.clone(),
    }
}

fn parse_completion_content(value: &Value) -> Result<&str, CommandError> {
    value.get("choices").and_then(Value::as_array).and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message")).and_then(|message| message.get("content"))
        .and_then(Value::as_str).filter(|content| !content.trim().is_empty())
        .ok_or_else(|| CommandError::new("ai_empty_response", "The model returned no final document-task content."))
}

fn normalize_json_content(value: &str) -> &str {
    let trimmed = value.trim();
    if let Some(inner) = trimmed.strip_prefix("```json").and_then(|value| value.strip_suffix("```")) { return inner.trim(); }
    if let Some(inner) = trimmed.strip_prefix("```").and_then(|value| value.strip_suffix("```")) { return inner.trim(); }
    trimmed
}

fn completion_endpoint(profile: &ProfileSnapshot) -> Result<Url, CommandError> {
    match profile.provider_kind {
        ProviderKind::LlamaCpp => {
            let base_includes_v1 = Url::parse(&profile.base_url).ok().is_some_and(|url| url.path().trim_end_matches('/').ends_with("/v1"));
            join_endpoint(&profile.base_url, if base_includes_v1 { "chat/completions" } else { "v1/chat/completions" })
        }
        ProviderKind::Deepseek => deepseek_strict_completion_endpoint(profile),
        ProviderKind::Openrouter => join_endpoint(&profile.base_url, "chat/completions"),
        _ => Err(CommandError::new("ai_provider_not_implemented", "The selected AI provider cannot analyze documents in this ostudy build.")),
    }
}

fn ensure_document_response_not_blocked(value: &Value) -> Result<(), CommandError> {
    match value.pointer("/choices/0/finish_reason").and_then(Value::as_str) {
        Some("content_filter") => Err(CommandError::new("ai_document_output_filtered", "The AI provider stopped the document result because of its content policy.")),
        Some("insufficient_system_resource") => Err(CommandError::new("ai_provider_resource_exhausted", "The AI provider stopped the document request because it did not have enough inference resources.")),
        _ => Ok(()),
    }
}

fn gateway_to_command(error: AiGatewayError) -> CommandError {
    let SafeAiError { code, message, .. } = error.safe;
    CommandError::new(code, message)
}

#[cfg(test)]
mod tests {
    use super::super::schemas::flashcard_schema;
    use super::*;
    use crate::ai::catalog::AuthMode;

    fn openrouter_profile() -> ProfileSnapshot {
        ProfileSnapshot {
            id: "a".repeat(32), provider_kind: ProviderKind::Openrouter,
            base_url: "https://openrouter.ai/api/v1/".to_string(), model_id: "openai/example".to_string(),
            auth_mode: AuthMode::Bearer, enabled: true, allow_insecure_http: false,
            require_zero_data_retention: false, request_timeout_ms: 60_000, credential_epoch: 0, revision: 1,
        }
    }

    #[test]
    fn token_usage_accepts_openai_and_llama_cpp_envelopes() {
        assert_eq!(response_token_usage(&json!({"usage":{"prompt_tokens":120,"completion_tokens":30,"total_tokens":150}})), (Some(120),Some(30),Some(150)));
        assert_eq!(response_token_usage(&json!({"timings":{"cache_n":180,"prompt_n":40,"predicted_n":45}})), (Some(220),Some(45),Some(265)));
        assert_eq!(response_token_usage(&json!({"tokens_cached":20,"tokens_evaluated":80,"tokens_predicted":20})), (Some(100),Some(20),Some(120)));
        assert_eq!(response_token_usage(&json!({"__verbose":{"timings":{"cache_n":60,"prompt_n":15,"predicted_n":25}}})), (Some(75),Some(25),Some(100)));
        assert_eq!(response_token_usage(&json!({"response":{"usage":{"prompt_tokens":9,"completion_tokens":4,"total_tokens":13}}})), (Some(9),Some(4),Some(13)));
    }

    #[test]
    fn canonical_image_media_types_are_derived_from_bytes() {
        assert_eq!(document_image_media_type(&[0x89,b'P',b'N',b'G',0x0d,0x0a,0x1a,0x0a]), Some("image/png"));
        assert_eq!(document_image_media_type(&[0xff,0xd8,0xff,0x00]), Some("image/jpeg"));
        assert_eq!(document_image_media_type(b"GIF89a"), Some("image/gif"));
        assert_eq!(document_image_media_type(b"RIFFxxxxWEBP"), Some("image/webp"));
        assert_eq!(document_image_media_type(b"not-an-image"), None);
    }

    #[test]
    fn nonportable_canonical_images_are_omitted_from_provider_parts() {
        let images = [
            SourcePageImage { page_number: 2, bytes: b"BMbitmap".to_vec() },
            SourcePageImage { page_number: 3, bytes: vec![0xff,0xd8,0xff,0x00] },
        ];
        let parts = multimodal_user_parts("source", &images).unwrap();
        assert_eq!(parts.len(), 3);
        assert!(parts[1]["text"].as_str().unwrap().contains("unit 3"));
        assert!(parts[2]["image_url"]["url"].as_str().unwrap().starts_with("data:image/jpeg;base64,"));
    }

    #[test]
    fn openrouter_document_requests_use_portable_strict_schema_and_reasoning_effort() {
        let profile = openrouter_profile();
        let schema = flashcard_schema(4);
        let request = JsonTaskRequest { system_prompt: "system", user_text: "source", schema: &schema, max_tokens: 128, images: &[] };
        let body = openrouter_document_body(&profile, "low", &request).unwrap();
        assert_eq!(body.pointer("/provider/require_parameters"), Some(&json!(true)));
        assert_eq!(body.pointer("/provider/data_collection"), Some(&json!("deny")));
        assert_eq!(body.pointer("/provider/sort"), Some(&json!("throughput")));
        assert_eq!(body.pointer("/reasoning/effort"), Some(&json!("low")));
        assert_eq!(body.pointer("/response_format/type"), Some(&json!("json_schema")));
        let wire_schema = body.pointer("/response_format/json_schema/schema").expect("wire schema should exist");
        let serialized = serde_json::to_string(wire_schema).unwrap();
        assert!(!serialized.contains("\"oneOf\""));
        assert!(serialized.contains("\"anyOf\""));
        assert!(!serialized.contains("\"const\""));
        assert!(!serialized.contains("maxItems"));
        let first_variant = wire_schema.pointer("/properties/cards/items/anyOf/0").unwrap();
        let property_count = first_variant["properties"].as_object().unwrap().len();
        assert_eq!(first_variant["required"].as_array().unwrap().len(), property_count);
        assert!(body.get("plugins").is_none());
        assert!(body.get("debug").is_none());
    }

    #[test]
    fn openrouter_document_requests_include_canonical_images() {
        let profile = openrouter_profile();
        let schema = flashcard_schema(1);
        let image = SourcePageImage { page_number: 7, bytes: vec![0xff,0xd8,0xff,0x00] };
        let request = JsonTaskRequest { system_prompt: "system", user_text: "source", schema: &schema, max_tokens: 64, images: &[image] };
        let body = openrouter_document_body(&profile, "none", &request).unwrap();
        assert_eq!(body.pointer("/messages/1/content/1/text"), Some(&json!("Rendered source unit 7 follows. Use it only as source evidence for diagrams, tables, equations, and layout-dependent or image-only content.")));
        let url = body.pointer("/messages/1/content/2/image_url/url").and_then(Value::as_str).expect("canonical image data URL should exist");
        assert!(url.starts_with("data:image/jpeg;base64,"));
    }

    #[test]
    fn deepseek_strict_schema_uses_supported_required_shapes() {
        let schema = deepseek_strict_schema(&flashcard_schema(8));
        let serialized = serde_json::to_string(&schema).unwrap();
        assert!(serialized.contains("\"anyOf\""));
        assert!(!serialized.contains("\"oneOf\""));
        assert!(!serialized.contains("maxLength"));
        assert!(!serialized.contains("maxItems"));
        let card_properties = schema.pointer("/properties/cards/items/anyOf/0/properties").and_then(Value::as_object).unwrap();
        let required = schema.pointer("/properties/cards/items/anyOf/0/required").and_then(Value::as_array).unwrap();
        assert_eq!(required.len(), card_properties.len());
    }

    #[test]
    fn strict_tool_arguments_parse_without_prompt_schema_repair() {
        let value = json!({"choices":[{"message":{"tool_calls":[{"function":{"name":DEEPSEEK_STRICT_TOOL_NAME,"arguments":"{\"cards\":[]}"}}]}}]});
        assert_eq!(parse_strict_tool_arguments(&value).unwrap(), json!({"cards":[]}));
    }

    #[test]
    fn length_is_truncation_only_when_the_structured_result_is_incomplete() {
        ensure_document_response_not_blocked(&json!({"choices":[{"finish_reason":"length"}]})).expect("length alone is not a policy or resource failure");
        let error = truncated_output_error();
        assert_eq!(error.code, "ai_document_output_truncated");
    }
}
