use serde_json::{json, Value};

use crate::ai::gateway::{error, AiGatewayError};

pub(super) const MAX_MODEL_LIST_BYTES: usize = 4 * 1024 * 1024;
pub(super) const MAX_MODELS: usize = 2_000;

pub(super) fn text_probe_body(
    model_id: &str,
    messages: Value,
    require_zero_data_retention: bool,
) -> Value {
    json!({
        "model": model_id,
        "messages": messages,
        "temperature": 0,
        "max_tokens": 64,
        "stream": false,
        "provider": provider_preferences(true, require_zero_data_retention)
    })
}

pub(super) fn structured_probe_body(
    model_id: &str,
    echo: &str,
    schema: Value,
    require_zero_data_retention: bool,
) -> Value {
    json!({
        "model": model_id,
        "messages": [
            {
                "role": "system",
                "content": "Return only the JSON object required by the response schema."
            },
            {
                "role": "user",
                "content": format!("Return ok=true and echo exactly {echo}.")
            }
        ],
        "temperature": 0,
        "max_tokens": 128,
        "stream": false,
        "provider": provider_preferences(true, require_zero_data_retention),
        "response_format": {
            "type": "json_schema",
            "json_schema": {
                "name": "ostudy_connection_probe",
                "strict": true,
                "schema": schema
            }
        }
    })
}

/// OpenRouter defaults permit endpoints that may retain data. ostudy sends a
/// stricter request policy for study content and requires parameter-compatible
/// endpoints whenever the task relies on a structured-output contract.
pub(super) fn provider_preferences(
    require_parameters: bool,
    require_zero_data_retention: bool,
) -> Value {
    json!({
        "require_parameters": require_parameters,
        "data_collection": "deny",
        "zdr": require_zero_data_retention,
        "allow_fallbacks": true
    })
}

pub(super) fn model_display_name(item: &Value, id: &str) -> String {
    item.get("name")
        .and_then(Value::as_str)
        .filter(|value| {
            !value.trim().is_empty() && value.len() <= 300 && !value.chars().any(char::is_control)
        })
        .unwrap_or(id)
        .to_string()
}

pub(super) fn model_owner(item: &Value, id: &str) -> Option<String> {
    item.get("owned_by")
        .and_then(Value::as_str)
        .filter(|value| value.len() <= 200 && !value.chars().any(char::is_control))
        .map(ToString::to_string)
        .or_else(|| {
            id.split_once('/')
                .map(|(owner, _)| owner)
                .filter(|owner| !owner.is_empty() && owner.len() <= 200)
                .map(ToString::to_string)
        })
}

pub(super) fn embedded_error(value: &Value) -> Option<AiGatewayError> {
    let provider_error = value.get("error")?;
    let code = provider_error.get("code");
    let code_text = code
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .or_else(|| code.and_then(Value::as_i64).map(|value| value.to_string()))
        .unwrap_or_else(|| "unknown".to_string());
    let normalized = code_text.to_ascii_lowercase();
    let (safe_code, message, retryable) = match normalized.as_str() {
        "401" | "unauthorized" => (
            "ai_authentication_failed",
            "OpenRouter rejected the saved API key.",
            false,
        ),
        "402" | "insufficient_credits" => (
            "ai_insufficient_credits",
            "The OpenRouter account does not have enough credits for this request.",
            false,
        ),
        "429" | "rate_limit_exceeded" => (
            "ai_rate_limited",
            "OpenRouter rate-limited the request.",
            true,
        ),
        "502" | "provider_error" | "server_error" => (
            "ai_provider_unavailable",
            "The model provider selected by OpenRouter failed during generation.",
            true,
        ),
        "503" | "no_available_providers" => (
            "ai_no_compatible_endpoint",
            "OpenRouter could not find an available endpoint compatible with the request policy.",
            true,
        ),
        "context_length_exceeded" => (
            "ai_context_length_exceeded",
            "The request exceeded the selected model endpoint's context window.",
            false,
        ),
        "invalid_model" => (
            "ai_model_unavailable",
            "The selected OpenRouter model is no longer available to this API key.",
            false,
        ),
        _ => (
            "ai_provider_error",
            "OpenRouter returned a provider error for the request.",
            false,
        ),
    };
    crate::logging::warn_target(
        "ai.provider.openrouter",
        "openrouter_embedded_error",
        message,
        json!({ "providerCode": code_text }),
    );
    Some(error(safe_code, message, retryable, None))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structured_probe_requires_parameter_compatible_private_routing() {
        let body =
            structured_probe_body("openai/example", "echo", json!({ "type": "object" }), true);
        assert_eq!(
            body.pointer("/provider/require_parameters"),
            Some(&json!(true))
        );
        assert_eq!(
            body.pointer("/provider/data_collection"),
            Some(&json!("deny"))
        );
        assert_eq!(body.pointer("/provider/zdr"), Some(&json!(true)));
        assert_eq!(
            body.pointer("/response_format/type"),
            Some(&json!("json_schema"))
        );
        assert!(body.get("debug").is_none());
        assert!(body.get("plugins").is_none());
    }

    #[test]
    fn provider_preferences_make_zdr_explicit_without_weakening_data_collection() {
        let ordinary = provider_preferences(true, false);
        assert_eq!(ordinary.pointer("/data_collection"), Some(&json!("deny")));
        assert_eq!(ordinary.pointer("/zdr"), Some(&json!(false)));

        let strict = provider_preferences(true, true);
        assert_eq!(strict.pointer("/data_collection"), Some(&json!("deny")));
        assert_eq!(strict.pointer("/zdr"), Some(&json!(true)));
        assert_eq!(strict.pointer("/require_parameters"), Some(&json!(true)));
    }

    #[test]
    fn midstream_style_errors_receive_stable_safe_categories() {
        let error_value = embedded_error(&json!({
            "error": { "code": "server_error", "message": "upstream detail" }
        }))
        .expect("embedded error should parse");
        assert_eq!(error_value.safe.code, "ai_provider_unavailable");
        assert!(error_value.safe.retryable);
        assert!(!error_value.safe.message.contains("upstream detail"));
    }
}
