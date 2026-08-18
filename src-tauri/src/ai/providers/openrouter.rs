use serde_json::{json, Value};

use crate::ai::gateway::{error, AiGatewayError};

use super::ProviderModelReasoning;

pub(super) const MAX_MODEL_LIST_BYTES: usize = 4 * 1024 * 1024;
pub(super) const MAX_MODELS: usize = 2_000;

const REASONING_EFFORTS: [&str; 7] = ["none", "minimal", "low", "medium", "high", "xhigh", "max"];
const HIGH_PRECISION_QUANTIZATIONS: [&str; 6] = ["int8", "fp8", "mxfp8", "fp16", "bf16", "fp32"];
const PREFERRED_MIN_THROUGHPUT_TPS: u64 = 40;
const PREFERRED_MAX_P90_LATENCY_SECONDS: u64 = 20;

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
///
/// Endpoint precision is a hard quality boundary: INT4/FP4/FP6 and endpoints
/// whose quantization is unknown are never selected. Within the eligible set,
/// price is the primary sort while throughput/latency are soft preferences so
/// an outage can still fall back to another high-precision endpoint.
pub(super) fn provider_preferences(
    require_parameters: bool,
    require_zero_data_retention: bool,
) -> Value {
    json!({
        "require_parameters": require_parameters,
        "data_collection": "deny",
        "zdr": require_zero_data_retention,
        "allow_fallbacks": true,
        "quantizations": HIGH_PRECISION_QUANTIZATIONS,
        "sort": "price",
        "preferred_min_throughput": {
            "p50": PREFERRED_MIN_THROUGHPUT_TPS
        },
        "preferred_max_latency": {
            "p90": PREFERRED_MAX_P90_LATENCY_SECONDS
        }
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

pub(super) fn model_context_window_tokens(item: &Value) -> Option<i64> {
    item.get("context_length")
        .and_then(Value::as_i64)
        .filter(|value| (2_048..=16_777_216).contains(value))
}

pub(super) fn model_reasoning(item: &Value) -> Option<ProviderModelReasoning> {
    let reasoning = item.get("reasoning")?.as_object()?;
    let supported_efforts = match reasoning.get("supported_efforts") {
        Some(Value::Array(values)) => {
            let efforts = values
                .iter()
                .filter_map(Value::as_str)
                .filter(|value| REASONING_EFFORTS.contains(value))
                .map(ToString::to_string)
                .collect::<Vec<_>>();
            Some(efforts)
        }
        Some(Value::Null) => None,
        _ => None,
    };
    let default_effort = reasoning
        .get("default_effort")
        .and_then(Value::as_str)
        .filter(|value| REASONING_EFFORTS.contains(value))
        .map(ToString::to_string);
    Some(ProviderModelReasoning {
        supported_efforts,
        default_effort,
        default_enabled: reasoning.get("default_enabled").and_then(Value::as_bool),
        supports_max_tokens: reasoning
            .get("supports_max_tokens")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        mandatory: reasoning
            .get("mandatory")
            .and_then(Value::as_bool)
            .unwrap_or(false),
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
    fn provider_preferences_enforce_precision_and_prefer_cheap_fast_routes() {
        let ordinary = provider_preferences(true, false);
        assert_eq!(ordinary.pointer("/data_collection"), Some(&json!("deny")));
        assert_eq!(ordinary.pointer("/zdr"), Some(&json!(false)));

        let strict = provider_preferences(true, true);
        assert_eq!(strict.pointer("/data_collection"), Some(&json!("deny")));
        assert_eq!(strict.pointer("/zdr"), Some(&json!(true)));
        assert_eq!(strict.pointer("/require_parameters"), Some(&json!(true)));
        assert_eq!(strict.pointer("/sort"), Some(&json!("price")));
        assert_eq!(
            strict.pointer("/preferred_min_throughput/p50"),
            Some(&json!(40))
        );
        assert_eq!(
            strict.pointer("/preferred_max_latency/p90"),
            Some(&json!(20))
        );
        assert_eq!(
            strict.pointer("/quantizations"),
            Some(&json!(["int8", "fp8", "mxfp8", "fp16", "bf16", "fp32"]))
        );
        let serialized = strict["quantizations"].to_string();
        for rejected in ["int4", "fp4", "fp6", "unknown"] {
            assert!(!serialized.contains(rejected));
        }
    }

    #[test]
    fn model_metadata_retains_context_and_reasoning_controls() {
        let model = json!({
            "context_length": 1_310_720,
            "reasoning": {
                "mandatory": false,
                "default_enabled": true,
                "supported_efforts": ["max", "high", "low"],
                "default_effort": "high"
            }
        });
        assert_eq!(model_context_window_tokens(&model), Some(1_310_720));
        let reasoning = model_reasoning(&model).expect("reasoning metadata should parse");
        assert_eq!(
            reasoning.supported_efforts.unwrap(),
            vec!["max", "high", "low"]
        );
        assert_eq!(reasoning.default_effort.as_deref(), Some("high"));
        assert_eq!(reasoning.default_enabled, Some(true));
        assert!(!reasoning.mandatory);
    }

    #[test]
    fn non_reasoning_models_omit_reasoning_metadata() {
        assert!(model_reasoning(&json!({ "context_length": 16_384 })).is_none());
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
