use serde_json::{json, Value};

use super::ProviderModelReasoning;

pub(super) const MAX_MODEL_LIST_BYTES: usize = 4 * 1024 * 1024;
pub(super) const MAX_MODELS: usize = 2_000;

const REASONING_EFFORTS: [&str; 7] = ["none", "minimal", "low", "medium", "high", "xhigh", "max"];
const HIGH_PRECISION_QUANTIZATIONS: [&str; 6] = ["int8", "fp8", "mxfp8", "fp16", "bf16", "fp32"];
const PREFERRED_MIN_P90_THROUGHPUT_TPS: u64 = 50;
const PREFERRED_MAX_P90_LATENCY_SECONDS: u64 = 20;

pub(super) fn apply_request_policy(
    body: &mut Value,
    require_parameters: bool,
    require_zero_data_retention: bool,
) {
    body["provider"] = provider_preferences(require_parameters, require_zero_data_retention);
}

pub(super) fn apply_document_policy(
    body: &mut Value,
    reasoning_effort: &str,
    require_zero_data_retention: bool,
) {
    apply_request_policy(body, true, require_zero_data_retention);
    body["plugins"] = json!([{
        "id": "context-compression",
        "enabled": false,
    }]);
    if reasoning_effort != "none" && reasoning_effort != "automatic" {
        body["reasoning"] = json!({
            "effort": reasoning_effort,
            "exclude": true,
        });
    }
}

/// OpenRouter's model catalog declares `structured_outputs` separately from
/// ordinary JSON mode/response formatting. Only an explicit declaration is
/// treated as authoritative; older catalog shapes that omit the field remain
/// unknown rather than being silently rejected.
pub(super) fn model_structured_outputs_declared(item: &Value) -> Option<bool> {
    let parameters = item.get("supported_parameters")?.as_array()?;
    Some(parameters.iter().any(|value| {
        value
            .as_str()
            .is_some_and(|value| value == "structured_outputs")
    }))
}

pub(super) fn provider_preferences(
    require_parameters: bool,
    require_zero_data_retention: bool,
) -> Value {
    json!({
        "require_parameters": require_parameters,
        "data_collection": "deny",
        "zdr": require_zero_data_retention,
        "allow_fallbacks": true,
        "sort": "throughput",
        "quantizations": HIGH_PRECISION_QUANTIZATIONS,
        "preferred_min_throughput": {
            "p90": PREFERRED_MIN_P90_THROUGHPUT_TPS
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_policy_enforces_private_parameter_compatible_routing() {
        let mut body = json!({});
        apply_request_policy(&mut body, true, true);
        assert_eq!(
            body.pointer("/provider/require_parameters"),
            Some(&json!(true))
        );
        assert_eq!(
            body.pointer("/provider/data_collection"),
            Some(&json!("deny"))
        );
        assert_eq!(body.pointer("/provider/zdr"), Some(&json!(true)));
        assert_eq!(body.pointer("/provider/sort"), Some(&json!("throughput")));
    }

    #[test]
    fn document_policy_disables_compression_and_excludes_returned_reasoning() {
        let mut body = json!({});
        apply_document_policy(&mut body, "low", false);
        assert_eq!(
            body.pointer("/plugins/0/id"),
            Some(&json!("context-compression"))
        );
        assert_eq!(body.pointer("/plugins/0/enabled"), Some(&json!(false)));
        assert_eq!(body.pointer("/reasoning/effort"), Some(&json!("low")));
        assert_eq!(body.pointer("/reasoning/exclude"), Some(&json!(true)));
        assert!(body.get("session_id").is_none());
    }

    #[test]
    fn model_catalog_structured_output_declaration_is_explicit() {
        assert_eq!(
            model_structured_outputs_declared(&json!({
                "supported_parameters": ["temperature", "structured_outputs"]
            })),
            Some(true)
        );
        assert_eq!(
            model_structured_outputs_declared(&json!({
                "supported_parameters": ["temperature", "response_format"]
            })),
            Some(false)
        );
        assert_eq!(model_structured_outputs_declared(&json!({})), None);
    }

    #[test]
    fn model_metadata_preserves_context_owner_and_reasoning() {
        let item = json!({
            "id":"deepseek/example",
            "name":"DeepSeek Example",
            "context_length":1_310_720,
            "reasoning":{
                "mandatory":false,
                "default_enabled":true,
                "supported_efforts":["max","high","low"],
                "default_effort":"high"
            }
        });
        assert_eq!(
            model_display_name(&item, "deepseek/example"),
            "DeepSeek Example"
        );
        assert_eq!(
            model_owner(&item, "deepseek/example").as_deref(),
            Some("deepseek")
        );
        assert_eq!(model_context_window_tokens(&item), Some(1_310_720));
        let reasoning = model_reasoning(&item).expect("reasoning metadata should parse");
        assert_eq!(reasoning.default_effort.as_deref(), Some("high"));
        assert!(!reasoning.mandatory);
    }
}
