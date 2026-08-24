use serde_json::{json, Value};

use super::{
    contracts::{AiGatewayError, SafeAiError},
    metadata::{
        safe_json_string, safe_metadata_string, safe_openrouter_attempts,
        selected_openrouter_provider, MAX_SAFE_PROVIDER_METADATA_CHARS,
    },
};

pub(crate) fn error(
    code: &str,
    message: &str,
    retryable: bool,
    retry_after_seconds: Option<u64>,
) -> AiGatewayError {
    AiGatewayError {
        safe: SafeAiError {
            code: code.to_string(),
            message: message.to_string(),
            retryable,
            retry_after_seconds,
        },
    }
}

pub(super) fn canceled_error() -> AiGatewayError {
    error(
        "ai_request_canceled",
        "Document generation was stopped. The managed source remains available to retry.",
        false,
        None,
    )
}

pub(super) fn classify_reqwest_error(error_value: reqwest::Error) -> AiGatewayError {
    if error_value.is_timeout() {
        return error(
            "ai_request_timeout",
            "The AI provider did not respond before the configured timeout.",
            true,
            None,
        );
    }
    if error_value.is_connect() {
        return error(
            "ai_connection_failed",
            "ostudy could not connect to the AI provider endpoint.",
            true,
            None,
        );
    }
    error(
        "ai_network_error",
        "The AI provider request failed before a valid response was received.",
        true,
        None,
    )
}

pub(super) fn classify_http_status(
    status: u16,
    retry_after_seconds: Option<u64>,
) -> AiGatewayError {
    match status {
        400 | 422 => error(
            "ai_http_bad_request",
            "The AI provider rejected this request as unsupported or invalid. Verify the selected model and provider/server version, then retry.",
            false,
            retry_after_seconds,
        ),
        401 => error(
            "ai_http_unauthorized",
            "The AI provider rejected this credential.",
            false,
            None,
        ),
        403 => error(
            "ai_http_forbidden",
            "The AI provider does not permit this request.",
            false,
            None,
        ),
        404 => error(
            "ai_http_not_found",
            "The configured AI endpoint or model was not found.",
            false,
            None,
        ),
        408 => error(
            "ai_request_timeout",
            "The AI provider timed out while handling the request.",
            true,
            None,
        ),
        429 => error(
            "ai_http_rate_limited",
            "The AI provider rate limit was reached.",
            true,
            retry_after_seconds,
        ),
        500..=599 => error(
            "ai_server_unavailable",
            "The AI provider is temporarily unavailable.",
            true,
            retry_after_seconds,
        ),
        _ => error(
            "ai_http_error",
            "The AI provider rejected the request.",
            false,
            retry_after_seconds,
        ),
    }
}

pub(super) fn classify_provider_error_response(
    status: u16,
    retry_after_seconds: Option<u64>,
    body: &[u8],
    openrouter: bool,
    generation_id: Option<&str>,
    streaming: bool,
) -> AiGatewayError {
    if openrouter {
        if let Ok(value) = serde_json::from_slice::<Value>(body) {
            if provider_error_object(&value).is_some() {
                return classify_provider_error_value(
                    &value,
                    Some(status),
                    retry_after_seconds,
                    true,
                    generation_id,
                    streaming,
                );
            }
        }
    }
    classify_http_status(status, retry_after_seconds)
}

pub(super) fn classify_stream_event_error(
    value: &Value,
    openrouter: bool,
    generation_id: Option<&str>,
    retry_after_seconds: Option<u64>,
) -> AiGatewayError {
    if provider_error_object(value).is_some() {
        return classify_provider_error_value(
            value,
            None,
            retry_after_seconds,
            openrouter,
            generation_id,
            true,
        );
    }
    error(
        "ai_stream_interrupted",
        "The AI provider ended the streaming response with an unrecognized error event. Partial output was discarded.",
        true,
        retry_after_seconds,
    )
}

pub(super) fn stream_inactivity_error(timeout_ms: i64) -> AiGatewayError {
    let timeout_seconds = (timeout_ms.max(1) as f64 / 1_000.0).max(0.001);
    error(
        "ai_stream_inactive",
        &format!(
            "The AI provider sent no response data for {timeout_seconds:.1} seconds. ostudy canceled this inactive connection; active generation is never stopped by a total wall-clock deadline."
        ),
        true,
        None,
    )
}

pub(super) fn empty_stream_failure(finish_reason: &str) -> AiGatewayError {
    let code = if finish_reason == "length" {
        "ai_document_output_truncated"
    } else {
        "ai_empty_response"
    };
    error(
        code,
        &format!(
            "The provider ended the streamed document request with finish_reason={finish_reason}, but returned no final content or structured tool arguments."
        ),
        true,
        None,
    )
}

pub(super) fn initial_timeout_error(message: &'static str) -> AiGatewayError {
    error("ai_initial_response_timeout", message, true, None)
}

pub(super) fn provider_error_object(value: &Value) -> Option<&Value> {
    value
        .get("error")
        .filter(|error| !error.is_null())
        .or_else(|| {
            value
                .pointer("/choices/0/error")
                .filter(|error| !error.is_null())
        })
}

fn provider_error_code(error_value: &Value) -> String {
    error_value
        .get("code")
        .and_then(Value::as_str)
        .and_then(|value| safe_metadata_string(value, 120))
        .or_else(|| {
            error_value
                .get("code")
                .and_then(Value::as_i64)
                .map(|value| value.to_string())
        })
        .unwrap_or_else(|| "unknown".to_string())
}

fn provider_error_status(error_value: &Value) -> Option<u16> {
    error_value
        .get("code")
        .and_then(Value::as_u64)
        .and_then(|value| u16::try_from(value).ok())
        .or_else(|| {
            error_value
                .get("code")
                .and_then(Value::as_i64)
                .and_then(|value| u16::try_from(value).ok())
        })
        .or_else(|| {
            error_value
                .get("code")
                .and_then(Value::as_str)
                .and_then(|value| value.parse::<u16>().ok())
        })
}

pub(super) fn classify_provider_error_value(
    value: &Value,
    http_status: Option<u16>,
    retry_after_seconds: Option<u64>,
    openrouter: bool,
    generation_id: Option<&str>,
    streaming: bool,
) -> AiGatewayError {
    let null_error = Value::Null;
    let error_value = provider_error_object(value).unwrap_or(&null_error);
    let code = provider_error_code(error_value);
    let provider_status =
        provider_error_status(error_value).or_else(|| http_status.filter(|status| *status >= 400));
    let error_type = safe_json_string(
        error_value
            .pointer("/metadata/error_type")
            .or_else(|| error_value.get("error_type")),
        120,
    );
    let provider_code = safe_json_string(
        error_value.pointer("/metadata/provider_code"),
        MAX_SAFE_PROVIDER_METADATA_CHARS,
    );
    let upstream_provider = selected_openrouter_provider(value);
    let model =
        safe_json_string(value.get("model"), MAX_SAFE_PROVIDER_METADATA_CHARS).or_else(|| {
            safe_json_string(
                value.pointer("/openrouter_metadata/requested"),
                MAX_SAFE_PROVIDER_METADATA_CHARS,
            )
        });
    let finish_reason = safe_json_string(value.pointer("/choices/0/finish_reason"), 120);
    let native_finish_reason =
        safe_json_string(value.pointer("/choices/0/native_finish_reason"), 120);
    let routing_strategy = safe_json_string(value.pointer("/openrouter_metadata/strategy"), 120);
    let routing_summary = safe_json_string(
        value.pointer("/openrouter_metadata/summary"),
        MAX_SAFE_PROVIDER_METADATA_CHARS,
    );
    let routing_attempt = value
        .pointer("/openrouter_metadata/attempt")
        .and_then(Value::as_u64);
    let normalized_code = code.to_ascii_lowercase();
    let normalized_type = error_type
        .as_deref()
        .unwrap_or(normalized_code.as_str())
        .to_ascii_lowercase();
    let status_label = provider_status
        .map(|status| format!(" (HTTP {status})"))
        .unwrap_or_default();
    let route_label = upstream_provider
        .as_deref()
        .map(|provider| format!(" through {provider}"))
        .unwrap_or_default();
    let partial_label = if streaming {
        " Partial output was discarded."
    } else {
        ""
    };
    let (safe_code, message, retryable) = match normalized_type.as_str() {
        "authentication" | "unauthorized" | "401" => (
            "ai_authentication_failed",
            format!("The AI provider rejected the saved credential{status_label}."),
            false,
        ),
        "insufficient_credits" | "402" => (
            "ai_insufficient_credits",
            format!(
                "The AI provider account does not have enough credits for this request{status_label}."
            ),
            false,
        ),
        "rate_limit_exceeded" | "429" => (
            "ai_rate_limited",
            format!("The AI provider rate-limited the request{status_label}."),
            true,
        ),
        "provider_unavailable" | "502" => (
            "ai_provider_unavailable",
            if openrouter {
                format!(
                    "OpenRouter reported provider_unavailable{status_label}{route_label}: the upstream model endpoint became unavailable or returned an invalid or empty response.{partial_label}"
                )
            } else {
                format!(
                    "The upstream model endpoint became unavailable or returned an invalid response{status_label}.{partial_label}"
                )
            },
            true,
        ),
        "provider_overloaded" | "529" => (
            "ai_provider_overloaded",
            if openrouter {
                format!(
                    "OpenRouter reported provider_overloaded{status_label}{route_label}: the upstream model endpoint is temporarily overloaded.{partial_label}"
                )
            } else {
                format!(
                    "The AI provider is temporarily overloaded{status_label}.{partial_label}"
                )
            },
            true,
        ),
        "no_available_providers" => (
            "ai_no_compatible_endpoint",
            format!(
                "No compatible model endpoint was available for the requested parameters and privacy policy{status_label}."
            ),
            true,
        ),
        "timeout" | "504" | "408" => (
            "ai_provider_timeout",
            if openrouter {
                format!(
                    "OpenRouter reported a provider timeout{status_label}{route_label}.{partial_label}"
                )
            } else {
                format!("The AI provider timed out{status_label}.{partial_label}")
            },
            true,
        ),
        "context_length_exceeded" => (
            "ai_context_length_exceeded",
            format!(
                "The provider rejected the request because it exceeded the model context window{status_label}."
            ),
            false,
        ),
        "max_tokens_exceeded" | "token_limit_exceeded" | "string_too_long" => (
            "ai_output_limit_reached",
            format!(
                "The provider reached a model/output token limit before the structured result completed{status_label}.{partial_label}"
            ),
            false,
        ),
        "content_policy_violation" | "refusal" => (
            "ai_content_policy_rejected",
            format!(
                "The provider stopped this request because of its content policy{status_label}."
            ),
            false,
        ),
        "invalid_request" | "invalid_prompt" | "unprocessable" | "400" | "422" => (
            "ai_http_bad_request",
            format!(
                "The AI provider rejected this request as unsupported or invalid{status_label}. Verify the selected model and provider capabilities, then retry."
            ),
            false,
        ),
        "server" | "server_error" | "500" => (
            "ai_server_unavailable",
            format!(
                "The AI provider reported an internal server error{status_label}.{partial_label}"
            ),
            true,
        ),
        "503" => (
            "ai_server_unavailable",
            format!(
                "The AI provider reported HTTP 503: routing capacity or a compatible upstream endpoint was unavailable.{partial_label}"
            ),
            true,
        ),
        _ => (
            if streaming {
                "ai_stream_interrupted"
            } else {
                "ai_provider_error"
            },
            if openrouter {
                format!(
                    "OpenRouter reported provider error type {}{status_label}{route_label}.{partial_label}",
                    error_type.as_deref().unwrap_or(code.as_str())
                )
            } else {
                format!(
                    "The AI provider reported an error{status_label}.{partial_label}"
                )
            },
            true,
        ),
    };

    crate::logging::warn_target(
        if openrouter {
            "ai.gateway.openrouter"
        } else {
            "ai.gateway.streaming"
        },
        if streaming {
            "provider_stream_error"
        } else {
            "provider_response_error"
        },
        &message,
        json!({
            "transport": if streaming { "sse" } else { "json" },
            "providerStatus": provider_status,
            "errorType": error_type.as_deref(),
            "providerCode": provider_code.as_deref(),
            "upstreamProvider": upstream_provider.as_deref(),
            "model": model.as_deref(),
            "finishReason": finish_reason.as_deref(),
            "nativeFinishReason": native_finish_reason.as_deref(),
            "routingStrategy": routing_strategy.as_deref(),
            "routingAttempt": routing_attempt,
            "routingSummary": routing_summary.as_deref(),
            "routingAttempts": safe_openrouter_attempts(value),
            "generationId": generation_id,
        }),
    );
    error(safe_code, &message, retryable, retry_after_seconds)
}
