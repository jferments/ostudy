use crate::ai::{
    catalog::{AuthMode, ProviderKind},
    profiles::{
        contracts::{ProfileConfigurationStatus, StoredProfile},
        projections::configuration_status_for_test,
    },
    secrets::{CredentialStatus, CredentialStorage},
};

fn stored(enabled: bool, model_id: &str, auth_mode: AuthMode) -> StoredProfile {
    StoredProfile {
        id: "a".repeat(32),
        display_name: "Profile".to_string(),
        provider_kind: ProviderKind::LlamaCpp,
        base_url: "http://127.0.0.1:8080/v1/".to_string(),
        model_id: model_id.to_string(),
        reasoning_effort: "low".to_string(),
        auth_mode,
        enabled,
        allow_insecure_http: false,
        require_zero_data_retention: false,
        request_timeout_ms: 60_000,
        credential_storage: CredentialStorage::None,
        credential_epoch: 0,
        revision: 1,
        created_at: 1,
        updated_at: 1,
    }
}

#[test]
fn configuration_projection_distinguishes_disabled_incomplete_and_ready() {
    assert_eq!(
        configuration_status_for_test(
            &stored(false, "model", AuthMode::None),
            CredentialStatus::NotRequired,
        ),
        ProfileConfigurationStatus::Disabled
    );
    assert_eq!(
        configuration_status_for_test(
            &stored(true, "", AuthMode::None),
            CredentialStatus::NotRequired,
        ),
        ProfileConfigurationStatus::Incomplete
    );
    assert_eq!(
        configuration_status_for_test(
            &stored(true, "model", AuthMode::Bearer),
            CredentialStatus::Missing,
        ),
        ProfileConfigurationStatus::Incomplete
    );
    assert_eq!(
        configuration_status_for_test(
            &stored(true, "model", AuthMode::Bearer),
            CredentialStatus::SessionOnly,
        ),
        ProfileConfigurationStatus::Ready
    );
}