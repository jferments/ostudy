use super::super::{
    super::super::{
        catalog::{AuthMode, ProviderKind, MAX_TIMEOUT_MS, MIN_TIMEOUT_MS},
        endpoint_policy::validate_endpoint,
        secrets::CredentialStorage,
    },
    policy::{
        ensure_trust_boundary_can_change, validate_display_name,
        validate_privacy_configuration, validate_provider_configuration, validate_timeout,
    },
};

#[test]
fn normalizes_names_and_bounds_timeouts() {
    assert_eq!(
        validate_display_name("  Local   Model  ".to_string()).unwrap(),
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
    let next = validate_endpoint(
        ProviderKind::LlamaCpp,
        "http://127.0.0.1:9090/v1/",
        false,
    )
    .unwrap();
    let origin_failure = ensure_trust_boundary_can_change(
        "http://127.0.0.1:8080/v1/",
        AuthMode::Bearer,
        CredentialStorage::Session,
        &next,
        AuthMode::Bearer,
    )
    .expect_err("origin change must require credential removal");
    assert_eq!(origin_failure.code, "ai_endpoint_has_credential");

    let same = validate_endpoint(
        ProviderKind::LlamaCpp,
        "http://127.0.0.1:8080/v1/",
        false,
    )
    .unwrap();
    let auth_failure = ensure_trust_boundary_can_change(
        "http://127.0.0.1:8080/v1/",
        AuthMode::Bearer,
        CredentialStorage::Secure,
        &same,
        AuthMode::None,
    )
    .expect_err("auth removal must require credential removal");
    assert_eq!(auth_failure.code, "ai_auth_mode_has_credential");
}
