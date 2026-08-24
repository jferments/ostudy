use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::{json, Value};

use crate::{
    ai::{
        gateway::{normalized_usage, AiGatewayError, SafeAiError},
        providers,
    },
    error::CommandError,
};

use super::super::AiRuntimeState;
use super::{
    context::DocumentTaskContext,
    contracts::{SourcePageImage, MAX_COMPLETION_BYTES, MAX_INLINE_IMAGES, MAX_REQUEST_JSON_BYTES},
    support::{output_error, truncated_output_error},
};

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

    let user_content = if request.images.is_empty() {
        Value::String(request.user_text.to_string())
    } else {
        Value::Array(multimodal_user_parts(request.user_text, request.images)?)
    };
    let prepared = providers::prepare_document_request(
        profile,
        request.system_prompt,
        user_content,
        request.schema,
        request.max_tokens,
        &task.reasoning_effort,
    )
    .map_err(gateway_to_command)?;

    ensure_request_size(&prepared.body)?;
    if let Some(compatibility) = prepared.compatibility_body.as_ref() {
        ensure_request_size(compatibility)?;
    }

    let first = runtime
        .gateway
        .post_document_json(
            prepared.url.clone(),
            bearer,
            &prepared.body,
            profile.request_timeout_ms,
            MAX_COMPLETION_BYTES,
        )
        .await;
    let response = match first {
        Err(first_error) if first_error.safe.code == "ai_http_bad_request" => {
            if let Some(compatibility) = prepared.compatibility_body.as_ref() {
                runtime
                    .gateway
                    .post_document_json(
                        prepared.url.clone(),
                        bearer,
                        compatibility,
                        profile.request_timeout_ms,
                        MAX_COMPLETION_BYTES,
                    )
                    .await
            } else {
                Err(first_error)
            }
        }
        other => other,
    }
    .map_err(gateway_to_command)?;

    record_document_usage(task, &response);
    ensure_document_response_not_blocked(&response)?;
    let finish_reason = response
        .pointer("/choices/0/finish_reason")
        .and_then(Value::as_str);
    match providers::decode_document_output(&prepared, &response) {
        Ok(value) => Ok(value),
        Err(_) if finish_reason == Some("length") => Err(truncated_output_error()),
        Err(failure) => Err(output_error(failure.message)),
    }
}

fn ensure_request_size(body: &Value) -> Result<(), CommandError> {
    if serde_json::to_vec(body)
        .map_err(CommandError::serialization)?
        .len()
        > MAX_REQUEST_JSON_BYTES
    {
        return Err(CommandError::new(
            "ai_request_too_large",
            "This semantic document window is too large for ostudy's bounded provider request.",
        ));
    }
    Ok(())
}

fn multimodal_user_parts(
    user_text: &str,
    images: &[SourcePageImage],
) -> Result<Vec<Value>, CommandError> {
    let mut parts = vec![json!({"type": "text", "text": user_text})];
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
                "url": format!("data:{media_type};base64,{}", STANDARD.encode(&image.bytes))
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

fn record_document_usage(task: &DocumentTaskContext, response: &Value) {
    let usage = normalized_usage(response);
    let read = |pointer: &str| {
        usage
            .as_ref()
            .and_then(|usage| usage.pointer(pointer))
            .and_then(nonnegative_json_integer)
    };
    let prompt_tokens = read("/prompt_tokens");
    let completion_tokens = read("/completion_tokens");
    let total_tokens = read("/total_tokens");
    let cached_prompt_tokens = read("/prompt_tokens_details/cached_tokens");
    let cache_write_tokens = read("/prompt_tokens_details/cache_write_tokens");
    let reasoning_tokens = read("/completion_tokens_details/reasoning_tokens");
    task.record_request_usage(total_tokens);
    crate::logging::debug_target(
        "ai.document.usage",
        "document_ai_usage",
        "Recorded normalized provider token usage without logging prompt or model content.",
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

fn nonnegative_json_integer(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_i64().and_then(|value| u64::try_from(value).ok()))
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
    let SafeAiError { code, message, .. } = error_value.safe;
    CommandError::new(code, message)
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
