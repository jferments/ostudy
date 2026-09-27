use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::{json, Value};

use crate::{
    ai::{
        gateway::{normalized_usage, AiGatewayError, SafeAiError},
        generation::{self, RecoveryReason},
        providers,
    },
    error::CommandError,
    task_cancellation::CancellationSignal,
};

use super::super::AiRuntimeState;
use super::{
    context::DocumentTaskContext,
    contracts::{SourcePageImage, MAX_COMPLETION_BYTES, MAX_INLINE_IMAGES, MAX_REQUEST_JSON_BYTES},
    support::{output_error, truncated_output_error},
};

/// Provider-neutral structured task request after a domain module has selected
/// and bounded its canonical source material.
pub(crate) struct JsonTaskRequest<'a> {
    pub system_prompt: &'a str,
    pub user_text: &'a str,
    pub schema: &'a Value,
    pub max_tokens: usize,
    pub images: &'a [SourcePageImage],
}

// Capture the compiled call site synchronously. #[track_caller] on an async
// function would lose this attribution when its future is polled.
#[track_caller]
pub(super) fn request_json<'a>(
    runtime: &'a AiRuntimeState,
    task: &'a DocumentTaskContext,
    request: JsonTaskRequest<'a>,
) -> impl std::future::Future<Output = Result<Value, CommandError>> + 'a {
    let stage = crate::ai::generation::stage(std::panic::Location::caller().file());
    request_json_inner(runtime, task, request, None, true, stage)
}

#[track_caller]
pub(crate) fn request_json_cancellable<'a>(
    runtime: &'a AiRuntimeState,
    task: &'a DocumentTaskContext,
    request: JsonTaskRequest<'a>,
    cancellation: &'a CancellationSignal,
) -> impl std::future::Future<Output = Result<Value, CommandError>> + 'a {
    let stage = crate::ai::generation::stage(std::panic::Location::caller().file());
    request_json_inner(runtime, task, request, Some(cancellation), true, stage)
}

/// Opt out of dialect retries while retaining accounting for the single attempt.
#[track_caller]
pub(crate) fn request_json_cancellable_single_attempt<'a>(
    runtime: &'a AiRuntimeState,
    task: &'a DocumentTaskContext,
    request: JsonTaskRequest<'a>,
    cancellation: &'a CancellationSignal,
) -> impl std::future::Future<Output = Result<Value, CommandError>> + 'a {
    let stage = crate::ai::generation::stage(std::panic::Location::caller().file());
    request_json_inner(runtime, task, request, Some(cancellation), false, stage)
}

async fn request_json_inner(
    runtime: &AiRuntimeState,
    task: &DocumentTaskContext,
    request: JsonTaskRequest<'_>,
    cancellation: Option<&CancellationSignal>,
    allow_compatibility_retry: bool,
    stage: &str,
) -> Result<Value, CommandError> {
    ensure_context_budget(task, &request)?;
    crate::logging::debug_target("ai.contract", "structured_request_prepared",
        "Prepared the same explicit output contract for model instructions and provider schema enforcement.",
        json!({"provider":task.provider_kind,"model":task.model_id,
            "schemaFingerprint":crate::ai::structured_output::fingerprint(request.schema),
            "schemaBytes":request.schema.to_string().len(),"outputBudget":request.max_tokens,
            "reasoningBudget":task.reasoning_token_limit(request.max_tokens),
            "imageCount":request.images.len()}));
    let profile = task.profile();
    let credential =
        runtime
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
    let mut prepared = providers::prepare_document_request(
        profile,
        request.system_prompt,
        user_content,
        request.schema,
        request.max_tokens,
        &task.reasoning_effort,
        task.reasoning_token_limit(request.max_tokens),
    )
    .map_err(gateway_to_command)?;
    if task.provider_kind == "openrouter" && !task.profile().require_zero_data_retention {
        if let Some(diagnostics) = task.diagnostics.as_ref() {
            let session_id = diagnostics.provider_session_id();
            prepared.body["session_id"] = Value::String(session_id.clone());
            if let Some(compatibility) = prepared.compatibility_body.as_mut() {
                compatibility["session_id"] = Value::String(session_id);
            }
        }
    }

    ensure_request_size(&prepared.body)?;
    if let Some(compatibility) = prepared.compatibility_body.as_ref() {
        ensure_request_size(compatibility)?;
    }

    let first = execute_request(
        runtime,
        task,
        &prepared.body,
        prepared.url.clone(),
        bearer,
        cancellation,
        stage,
    )
    .await;
    let response = match first {
        Err(first_error)
            if allow_compatibility_retry && first_error.code == "ai_http_bad_request" =>
        {
            if let Some(compatibility) = prepared.compatibility_body.as_ref() {
                if generation::reserve_recovery(
                    task.diagnostics.as_ref(),
                    stage,
                    task.diagnostic_window_key.as_deref(),
                    RecoveryReason::ProviderCompatibility,
                    &first_error.code,
                ) {
                    execute_request(
                        runtime,
                        task,
                        compatibility,
                        prepared.url.clone(),
                        bearer,
                        cancellation,
                        stage,
                    )
                    .await
                } else {
                    Err(first_error)
                }
            } else {
                Err(first_error)
            }
        }
        other => other,
    }?;

    record_document_usage(task, &response);
    ensure_document_response_complete(&response)?;
    match providers::decode_document_output(&prepared, &response) {
        Ok(value) => Ok(value),
        Err(failure) => Err(output_error(failure.message)),
    }
}

/// Bind logical stage/window attribution around the gateway. The gateway owns
/// physical-attempt accounting because its bounded retry can send more than once.
async fn execute_request(
    runtime: &AiRuntimeState,
    task: &DocumentTaskContext,
    body: &Value,
    url: url::Url,
    bearer: Option<&str>,
    cancellation: Option<&CancellationSignal>,
    stage: &str,
) -> Result<Value, CommandError> {
    let _provider_slot =
        crate::ai::concurrency::acquire_provider_slot(&task.network_scope, cancellation).await?;
    let future = async {
        if let Some(cancellation) = cancellation {
            runtime
                .gateway
                .post_task_json(
                    url,
                    bearer,
                    body,
                    task.profile().request_timeout_ms,
                    MAX_COMPLETION_BYTES,
                    cancellation,
                )
                .await
        } else {
            runtime
                .gateway
                .post_document_json(
                    url,
                    bearer,
                    body,
                    task.profile().request_timeout_ms,
                    MAX_COMPLETION_BYTES,
                )
                .await
        }
    };
    let result = crate::ai::generation::scoped_request(
        task.diagnostics.clone(),
        stage,
        task.diagnostic_window_key.as_deref(),
        &task.provider_kind,
        future,
    )
    .await;
    crate::ai::concurrency::record_provider_outcome(
        &task.network_scope,
        result.is_ok(),
        result
            .as_ref()
            .err()
            .is_some_and(|error| error.safe.retryable),
    );
    if result.is_err() {
        task.record_request_usage(None);
    }
    result.map_err(gateway_to_command)
}

const CONSERVATIVE_TEXT_BYTES_PER_TOKEN: usize = 2;
const IMAGE_CONTEXT_RESERVE_TOKENS: usize = 8_192;
const REQUEST_CONTEXT_RESERVE_TOKENS: usize = 1_024;

pub(super) fn ensure_context_budget(
    task: &DocumentTaskContext,
    request: &JsonTaskRequest<'_>,
) -> Result<(), CommandError> {
    let system =
        crate::ai::structured_output::system_with_schema(request.system_prompt, request.schema);
    let upper_bound = conservative_text_tokens(&system)
        .saturating_add(conservative_text_tokens(request.user_text))
        .saturating_add(
            request
                .images
                .len()
                .saturating_mul(IMAGE_CONTEXT_RESERVE_TOKENS),
        )
        .saturating_add(request.max_tokens)
        .saturating_add(REQUEST_CONTEXT_RESERVE_TOKENS);
    if task
        .context_window_tokens()
        .is_some_and(|limit| upper_bound > limit)
    {
        return Err(CommandError::new(
            "ai_document_context_budget",
            "This bounded learning request still does not fit the selected model context after reserving its output contract and completion. No provider request was sent.",
        ));
    }
    Ok(())
}

fn conservative_text_tokens(text: &str) -> usize {
    // The context limit is expressed in TOKENS, while Rust string lengths are
    // UTF-8 BYTES. Comparing them directly made ordinary English/JSON requests
    // appear roughly 2–4x larger than they are. Two UTF-8 bytes per token is
    // deliberately conservative for mixed prose/JSON while remaining in the
    // same unit as the provider context window.
    text.len().div_ceil(CONSERVATIVE_TEXT_BYTES_PER_TOKEN)
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
    if images.len() > MAX_INLINE_IMAGES {
        return Err(CommandError::new(
            "ai_request_too_large",
            "The request exceeds its admitted image limit. No images were silently discarded.",
        ));
    }
    let mut parts = vec![json!({"type": "text", "text": user_text})];
    for image in images {
        let media_type = document_image_media_type(&image.bytes).ok_or_else(||
            CommandError::new("ai_document_image_invalid", "An admitted source image is not a supported image format. The request was not sent."))?;
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

fn ensure_document_response_complete(response: &Value) -> Result<(), CommandError> {
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
            "The AI provider stopped this learning request because of its content policy.",
        ));
    }
    if finish_reason == Some("length") {
        return Err(truncated_output_error());
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
    #[test]
    fn length_finish_is_rejected_even_when_the_content_is_valid_json() {
        let error = ensure_document_response_complete(&json!({"choices":[{
            "message":{"content":"{\"units\":[]}"},"finish_reason":"length"
        }]}))
        .unwrap_err();
        assert_eq!(error.code, "ai_document_output_truncated");
    }
    #[test]
    fn invalid_admitted_image_is_an_error_not_silently_dropped_evidence() {
        let images = [SourcePageImage {
            page_number: 1,
            bytes: b"not an image".to_vec(),
        }];
        assert!(multimodal_user_parts("source", &images).is_err());
    }
}

#[cfg(test)]
mod context_budget_tests {
    use super::*;

    #[test]
    fn text_budget_estimate_converts_bytes_to_tokens_instead_of_comparing_units_directly() {
        assert_eq!(conservative_text_tokens(&"a".repeat(20_000)), 10_000);
        assert_eq!(conservative_text_tokens("αβγδ"), 4);
    }
}
