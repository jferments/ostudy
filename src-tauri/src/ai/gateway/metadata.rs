use serde_json::{json, Value};
use url::Url;

pub(super) const OPENROUTER_HOST: &str = "openrouter.ai";
pub(super) const MAX_SAFE_PROVIDER_METADATA_CHARS: usize = 300;

pub(super) fn is_openrouter_url(url: &Url) -> bool {
    url.scheme() == "https"
        && url.host_str() == Some(OPENROUTER_HOST)
        && url.path().starts_with("/api/v1/")
}

pub(super) fn safe_json_string(value: Option<&Value>, maximum_chars: usize) -> Option<String> {
    value
        .and_then(Value::as_str)
        .and_then(|value| safe_metadata_string(value, maximum_chars))
}

pub(super) fn safe_metadata_string(value: &str, maximum_chars: usize) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.chars().any(char::is_control) {
        return None;
    }
    Some(trimmed.chars().take(maximum_chars).collect())
}

pub(super) fn selected_openrouter_provider(value: &Value) -> Option<String> {
    safe_json_string(value.get("provider"), MAX_SAFE_PROVIDER_METADATA_CHARS).or_else(|| {
        value
            .pointer("/openrouter_metadata/endpoints/available")
            .and_then(Value::as_array)
            .and_then(|endpoints| {
                endpoints.iter().find(|endpoint| {
                    endpoint.get("selected").and_then(Value::as_bool) == Some(true)
                })
            })
            .and_then(|endpoint| {
                safe_json_string(endpoint.get("provider"), MAX_SAFE_PROVIDER_METADATA_CHARS)
            })
    })
}

pub(super) fn safe_openrouter_attempts(value: &Value) -> Value {
    let attempts = value
        .pointer("/openrouter_metadata/attempts")
        .and_then(Value::as_array)
        .map(|attempts| {
            attempts
                .iter()
                .take(8)
                .map(|attempt| {
                    json!({
                        "provider": safe_json_string(
                            attempt.get("provider"),
                            MAX_SAFE_PROVIDER_METADATA_CHARS,
                        ),
                        "model": safe_json_string(
                            attempt.get("model"),
                            MAX_SAFE_PROVIDER_METADATA_CHARS,
                        ),
                        "status": attempt.get("status").and_then(Value::as_u64),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Value::Array(attempts)
}
