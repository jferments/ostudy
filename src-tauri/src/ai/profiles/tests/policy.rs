use crate::ai::{
    catalog::{AuthMode, ProviderKind, MAX_TIMEOUT_MS, MIN_TIMEOUT_MS},
    endpoint_policy::validate_endpoint,
    profiles::{
        contracts::StoredProfile,
        policy::{
            ensure_trust_boundary_can_change, validate_display_name,
            validate_privacy_configuration, validate_provider_configuration, validate_timeout,
        },
    },
    secrets::CredentialStorage,
};

fn stored_profile(
    base_url: &str,
    auth_mode: AuthMode,
    credential_storage: CredentialStorage,
) -> StoredProfile {
    StoredProfile {
        id: "a".repeat(32),
        display_name: "Local".to_string(),
        provider_kind: ProviderKind::LlamaCpp,
        base_url: base_url.to_string(),
        model_id: "model".to_string(),
        reasoning_effort: "low".to_string(),
        auth_mode,
        enabled: true,
        allow_insecure_http: false,
        require_zero_data_retention: false,
        request_timeout_ms: 60_000,
        credential_storage,
        credential_epoch: 1,
        revision: 1,
        created_at: 1,
        updated_at: 1,
    }
}

#[test]
fn normalizes_names_and_bounds_timeouts() {
    assert_eq!(
        validate_display_name("  Local   Model  ").unwrap(),
        "Local Model"
    );
    assert!(validate_timeout(MIN_TIMEOUT_MS - 1).is_err());
    assert!(validate_timeout(MAX_TIMEOUT_MS + 1).is_err());
}

#[test]
fn cloud_auth_and_privacy_policy_remain_provider_scoped() {
    assert!(validate_provider_configuration(ProviderKind::Openrouter, AuthMode::Bearer).is_ok());
    assert!(validate_provider_configuration(ProviderKind::Openrouter, AuthMode::None).is_err());
    assert!(validate_provider_configuration(ProviderKind::Deepseek, AuthMode::None).is_err());
    assert!(validate_privacy_configuration(ProviderKind::Openrouter, true).is_ok());
    assert!(validate_privacy_configuration(ProviderKind::LlamaCpp, true).is_err());
}

#[test]
fn saved_credentials_block_origin_and_auth_boundary_changes() {
    let next =
        validate_endpoint(ProviderKind::LlamaCpp, "http://127.0.0.1:9090/v1/", false).unwrap();
    let current = stored_profile(
        "http://127.0.0.1:8080/v1/",
        AuthMode::Bearer,
        CredentialStorage::Session,
    );
    let origin_failure = ensure_trust_boundary_can_change(&current, &next, AuthMode::Bearer)
        .expect_err("origin change must require credential removal");
    assert_eq!(origin_failure.code, "ai_endpoint_has_credential");

    let same =
        validate_endpoint(ProviderKind::LlamaCpp, "http://127.0.0.1:8080/v1/", false).unwrap();
    let current = stored_profile(
        "http://127.0.0.1:8080/v1/",
        AuthMode::Bearer,
        CredentialStorage::Secure,
    );
    let auth_failure = ensure_trust_boundary_can_change(&current, &same, AuthMode::None)
        .expect_err("auth removal must require credential removal");
    assert_eq!(auth_failure.code, "ai_auth_mode_has_credential");
}
