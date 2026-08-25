use serde_json::json;

use crate::ai::catalog::{AuthMode, ProviderKind};

use super::super::contracts::{CreateAiProviderProfileInput, ProfileConfigurationStatus};

#[test]
fn create_input_keeps_camel_case_and_rejects_unknown_fields() {
    let value = json!({
        "operationId": "1".repeat(32),
        "displayName": "Local",
        "providerKind": "llama_cpp",
        "baseUrl": "http://127.0.0.1:8080/v1/",
        "modelId": "model",
        "authMode": "none",
        "enabled": true,
        "allowInsecureHttp": false,
        "requireZeroDataRetention": false,
        "requestTimeoutMs": 60000
    });
    let decoded: CreateAiProviderProfileInput = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(decoded.provider_kind, ProviderKind::LlamaCpp);
    assert_eq!(decoded.auth_mode, AuthMode::None);

    let mut invalid = value;
    invalid["credential"] = json!("must-not-be-admitted");
    assert!(serde_json::from_value::<CreateAiProviderProfileInput>(invalid).is_err());
}

#[test]
fn configuration_status_keeps_snake_case_wire_values() {
    assert_eq!(
        serde_json::to_value(ProfileConfigurationStatus::Ready).unwrap(),
        json!("ready")
    );
    assert_eq!(
        serde_json::to_value(ProfileConfigurationStatus::Planned).unwrap(),
        json!("planned")
    );
}
