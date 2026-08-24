use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use url::Url;

use crate::ai::{
    endpoint_policy::join_endpoint,
    gateway::{error, AiGatewayError},
    profiles::ProfileSnapshot,
};

use super::{openai_compatible, ProviderProperties};

pub(super) fn openai_endpoint(base_url: &str, suffix: &str) -> Result<Url, AiGatewayError> {
    let relative = if base_includes_v1(base_url) {
        suffix.to_string()
    } else {
        format!("v1/{suffix}")
    };
    join_endpoint(base_url, &relative).map_err(command_to_gateway)
}

pub(super) fn root_endpoint(base_url: &str, suffix: &str) -> Result<Url, AiGatewayError> {
    let relative = if base_includes_v1(base_url) {
        format!("../{suffix}")
    } else {
        suffix.to_string()
    };
    join_endpoint(base_url, &relative).map_err(command_to_gateway)
}

pub(super) fn completion_endpoint(profile: &ProfileSnapshot) -> Result<Url, AiGatewayError> {
    openai_endpoint(&profile.base_url, "chat/completions")
}

pub(super) fn apply_probe_nonthinking(body: &mut Value) {
    body["reasoning_effort"] = Value::String("none".to_string());
    body["chat_template_kwargs"] = json!({"enable_thinking": false});
}

pub(super) fn apply_document_nonthinking(body: &mut Value) {
    body["chat_template_kwargs"] = json!({"enable_thinking": false});
}

/// Keep llama.cpp's established document contract: json_object + schema is
/// the primary form and the top-level json_schema extension is the single
/// compatibility retry. The shared provider layer owns when to retry; this
/// dialect owns only the alternate wire representation.
pub(super) fn document_schema_bodies(body: &Value, schema: &Value) -> (Value, Value) {
    let mut primary = body.clone();
    apply_document_nonthinking(&mut primary);
    openai_compatible::set_json_object_schema(&mut primary, schema.clone());

    let mut compatibility = body.clone();
    compatibility["json_schema"] = schema.clone();
    (primary, compatibility)
}

pub(super) fn parse_properties(value: &Value) -> ProviderProperties {
    let chat_template = value
        .get("chat_template")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty());
    let server_chat_template_hash = chat_template.map(|template| {
        let digest = Sha256::digest(template.as_bytes());
        format!("{digest:x}")
    });
    let image_input_declared = value
        .get("modalities")
        .and_then(Value::as_array)
        .map(|modalities| {
            modalities.iter().any(|item| {
                item.as_str()
                    .is_some_and(|value| matches!(value, "image" | "vision"))
            })
        })
        .or_else(|| value.get("has_vision_encoder").and_then(Value::as_bool));
    let context_window_tokens = value.get("n_ctx").and_then(Value::as_i64).or_else(|| {
        value
            .pointer("/default_generation_settings/n_ctx")
            .and_then(Value::as_i64)
    });
    ProviderProperties {
        server_chat_template_present: Some(chat_template.is_some()),
        server_chat_template_hash,
        image_input_declared,
        context_window_tokens,
    }
}

fn base_includes_v1(base_url: &str) -> bool {
    Url::parse(base_url)
        .ok()
        .is_some_and(|url| url.path().trim_end_matches('/').ends_with("/v1"))
}

fn command_to_gateway(error_value: crate::error::CommandError) -> AiGatewayError {
    error(&error_value.code, &error_value.message, false, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_accept_server_root_or_v1_base() {
        assert_eq!(
            openai_endpoint("http://127.0.0.1:8080/", "models")
                .unwrap()
                .as_str(),
            "http://127.0.0.1:8080/v1/models"
        );
        assert_eq!(
            openai_endpoint("http://127.0.0.1:8080/v1/", "models")
                .unwrap()
                .as_str(),
            "http://127.0.0.1:8080/v1/models"
        );
        assert_eq!(
            root_endpoint("http://127.0.0.1:8080/v1/", "props")
                .unwrap()
                .as_str(),
            "http://127.0.0.1:8080/props"
        );
    }

    #[test]
    fn document_schema_uses_one_explicit_compatibility_form() {
        let body = json!({"model":"local","messages":[]});
        let (primary, compatibility) = document_schema_bodies(&body, &json!({"type":"object"}));
        assert_eq!(
            primary.pointer("/response_format/type"),
            Some(&json!("json_object"))
        );
        assert_eq!(
            compatibility.pointer("/json_schema/type"),
            Some(&json!("object"))
        );
        assert!(compatibility.get("response_format").is_none());
    }

    #[test]
    fn hashes_but_does_not_retain_server_chat_templates() {
        let properties = parse_properties(&json!({
            "chat_template": "{% for message in messages %}{{ message.content }}{% endfor %}",
            "modalities": ["text", "image"],
            "n_ctx": 8192
        }));
        assert_eq!(properties.server_chat_template_present, Some(true));
        assert_eq!(properties.image_input_declared, Some(true));
        assert_eq!(properties.context_window_tokens, Some(8192));
        assert_eq!(
            properties
                .server_chat_template_hash
                .as_deref()
                .map(str::len),
            Some(64)
        );
    }
}
