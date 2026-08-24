use super::super::super::{
    catalog::{AuthMode, ProviderKind},
    profiles::ProfileSnapshot,
    providers::ProviderProperties,
};
use super::super::{
    capabilities::{build_capabilities, default_unknown_capabilities, CapabilityEvidence},
    contracts::CapabilityStatus,
};

fn profile(provider_kind: ProviderKind, model_id: &str) -> ProfileSnapshot {
    ProfileSnapshot {
        id: "0123456789abcdef0123456789abcdef".to_string(),
        provider_kind,
        base_url: match provider_kind {
            ProviderKind::LlamaCpp => "http://127.0.0.1:8080/".to_string(),
            ProviderKind::Openrouter => "https://openrouter.ai/api/v1/".to_string(),
            _ => "https://api.deepseek.com/".to_string(),
        },
        model_id: model_id.to_string(),
        auth_mode: if matches!(
            provider_kind,
            ProviderKind::Deepseek | ProviderKind::Openrouter
        ) {
            AuthMode::Bearer
        } else {
            AuthMode::None
        },
        enabled: true,
        allow_insecure_http: false,
        require_zero_data_retention: false,
        request_timeout_ms: 60_000,
        credential_epoch: 2,
        revision: 3,
    }
}

#[test]
fn deepseek_never_claims_provider_enforced_json_schema() {
    let capabilities = default_unknown_capabilities(ProviderKind::Deepseek);
    let schema = capabilities
        .iter()
        .find(|capability| capability.key == "json_schema_output")
        .expect("schema capability should exist");
    assert_eq!(schema.status, CapabilityStatus::Unsupported);
}

#[test]
fn openrouter_can_report_json_schema_and_document_contract_support() {
    let profile = profile(ProviderKind::Openrouter, "openai/example");
    let properties = ProviderProperties::default();
    let capabilities = build_capabilities(
        &profile,
        CapabilityEvidence {
            models: None,
            model_listing: true,
            text_completion: true,
            structured_output: true,
            document_generation_schema: Some(true),
            usage_reported: true,
            properties: &properties,
        },
    );
    assert_eq!(
        capabilities
            .iter()
            .find(|item| item.key == "json_schema_output")
            .unwrap()
            .status,
        CapabilityStatus::Supported
    );
    assert_eq!(
        capabilities
            .iter()
            .find(|item| item.key == "document_generation_schema")
            .unwrap()
            .status,
        CapabilityStatus::Supported
    );
}

#[test]
fn failed_text_probe_does_not_claim_structured_support() {
    let profile = profile(ProviderKind::LlamaCpp, "model");
    let properties = ProviderProperties::default();
    let capabilities = build_capabilities(
        &profile,
        CapabilityEvidence {
            models: None,
            model_listing: false,
            text_completion: false,
            structured_output: false,
            document_generation_schema: None,
            usage_reported: false,
            properties: &properties,
        },
    );
    assert_eq!(
        capabilities
            .iter()
            .find(|item| item.key == "text_input")
            .unwrap()
            .status,
        CapabilityStatus::Unknown
    );
    assert_eq!(
        capabilities
            .iter()
            .find(|item| item.key == "json_schema_output")
            .unwrap()
            .status,
        CapabilityStatus::Unknown
    );
}
