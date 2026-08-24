use crate::{database::Database, error::CommandError};

use super::super::{
    contracts::{CreateAiProviderProfileInput, ProfileConfigurationStatus, StoredProfile},
    defaults,
    policy,
    projections::configuration_status_for_test,
    repository,
};
use super::super::super::{
    catalog::{AuthMode, ProviderKind, DEFAULT_TIMEOUT_MS, MAX_TIMEOUT_MS, MIN_TIMEOUT_MS},
    secrets::{CredentialStatus, CredentialStorage},
};

fn default_input(provider_kind: ProviderKind) -> CreateAiProviderProfileInput {
    CreateAiProviderProfileInput {
        operation_id: String::new(),
        display_name: provider_kind.display_name().to_string(),
        provider_kind,
        base_url: provider_kind.default_base_url().to_string(),
        model_id: String::new(),
        auth_mode: provider_kind.default_auth_mode(),
        enabled: true,
        allow_insecure_http: false,
        require_zero_data_retention: false,
        request_timeout_ms: DEFAULT_TIMEOUT_MS,
        editor_draft: None,
    }
}

fn create_test_profile(database: &Database, provider_kind: ProviderKind) -> String {
    let mut input = default_input(provider_kind);
    input.operation_id = "11111111111111111111111111111111".to_string();
    input.display_name = "Local model".to_string();
    input.model_id = "test-model".to_string();
    let fields = policy::validate_create_profile(&input).expect("profile should validate");
    database
        .with_connection(|connection| repository::create_profile(connection, input, fields))
        .expect("profile should create")
}

#[test]
fn creates_profiles_without_persisting_credentials() {
    let database = Database::open_in_memory().expect("database should open");
    let profile_id = create_test_profile(&database, ProviderKind::LlamaCpp);
    database
        .with_connection(|connection| {
            let columns: Vec<String> = connection
                .prepare("SELECT name FROM pragma_table_info('ai_provider_profiles')")
                .map_err(CommandError::database)?
                .query_map([], |row| row.get(0))
                .map_err(CommandError::database)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(CommandError::database)?;
            assert!(!columns.iter().any(|column| {
                column.contains("secret") || column.contains("token") || column.contains("api_key")
            }));
            let storage: String = connection
                .query_row(
                    "SELECT credential_storage FROM ai_provider_profiles WHERE id = ?1",
                    [profile_id.as_str()],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            assert_eq!(storage, "none");
            let snapshot = repository::load_profile_snapshot(connection, &profile_id)?;
            assert_eq!(snapshot.provider_kind, ProviderKind::LlamaCpp);
            Ok(())
        })
        .expect("profile should load");
}

#[test]
fn replays_create_operations_without_duplicate_profiles() {
    let database = Database::open_in_memory().expect("database should open");
    let mut input = default_input(ProviderKind::LlamaCpp);
    input.operation_id = "22222222222222222222222222222222".to_string();
    input.display_name = "Replay profile".to_string();
    let fields = policy::validate_create_profile(&input).unwrap();
    let first = database
        .with_connection(|connection| repository::create_profile(connection, input, fields))
        .expect("profile should create");

    let replay = database
        .with_connection(|connection| {
            repository::operation_replay(
                connection,
                "22222222222222222222222222222222",
                "create_profile",
                None,
            )
        })
        .expect("replay should load")
        .expect("operation should exist")
        .expect("create operation should retain profile id");
    assert_eq!(first, replay);
}

#[test]
fn credential_epoch_operations_are_idempotent_and_redacted() {
    let database = Database::open_in_memory().expect("database should open");
    let profile_id = create_test_profile(&database, ProviderKind::LlamaCpp);
    database
        .with_connection(|connection| {
            repository::commit_credential_epoch(
                connection,
                &profile_id,
                1,
                "44444444444444444444444444444444",
                "set_credential",
                CredentialStorage::Session,
            )
        })
        .expect("credential epoch should commit");
    assert!(repository::operation_was_replayed(
        &database,
        "44444444444444444444444444444444",
        "set_credential",
        Some(&profile_id),
    )
    .expect("operation replay should load"));
    database
        .with_connection(|connection| {
            let (storage, epoch): (String, i64) = connection
                .query_row(
                    "SELECT credential_storage, credential_epoch
                     FROM ai_provider_profiles WHERE id = ?1",
                    [profile_id.as_str()],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .map_err(CommandError::database)?;
            assert_eq!(storage, "session");
            assert_eq!(epoch, 1);
            let operation_columns: Vec<String> = connection
                .prepare("SELECT name FROM pragma_table_info('ai_profile_operations')")
                .map_err(CommandError::database)?
                .query_map([], |row| row.get(0))
                .map_err(CommandError::database)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(CommandError::database)?;
            assert!(!operation_columns.iter().any(|column| {
                column.contains("credential_value") || column.contains("secret")
            }));
            Ok(())
        })
        .expect("credential operation should remain redacted");
}

#[test]
fn zero_data_retention_is_openrouter_only() {
    let mut valid = default_input(ProviderKind::Openrouter);
    valid.operation_id = "17171717171717171717171717171717".to_string();
    valid.display_name = "OpenRouter ZDR".to_string();
    valid.model_id = "openai/example".to_string();
    valid.require_zero_data_retention = true;
    assert!(policy::validate_create_profile(&valid).is_ok());

    let mut invalid = default_input(ProviderKind::LlamaCpp);
    invalid.require_zero_data_retention = true;
    assert_eq!(
        policy::validate_create_profile(&invalid).unwrap_err().code,
        "invalid_ai_privacy_policy"
    );
}

#[test]
fn rejects_planned_provider_profiles_and_invalid_cloud_auth() {
    let planned = default_input(ProviderKind::Openai);
    assert_eq!(
        policy::validate_create_profile(&planned).unwrap_err().code,
        "ai_provider_not_implemented"
    );

    let mut cloud = default_input(ProviderKind::Deepseek);
    cloud.auth_mode = AuthMode::None;
    assert_eq!(
        policy::validate_create_profile(&cloud).unwrap_err().code,
        "invalid_ai_auth_mode"
    );
}

#[test]
fn normalizes_profile_names_and_bounds_timeouts() {
    assert_eq!(
        policy::validate_display_name("  Local   Model  ").unwrap(),
        "Local Model"
    );
    assert!(policy::validate_timeout(MIN_TIMEOUT_MS - 1).is_err());
    assert!(policy::validate_timeout(MAX_TIMEOUT_MS + 1).is_err());
}

#[test]
fn task_defaults_decode_nullable_profile_ids() {
    let database = Database::open_in_memory().expect("database should open");
    let result = database
        .with_connection(defaults::load_task_defaults)
        .expect("defaults should load");
    assert_eq!(result.flashcard_generation_profile_id, None);
}

#[test]
fn configuration_status_remains_credential_and_enablement_aware() {
    let mut profile = StoredProfile {
        id: "a".repeat(32),
        display_name: "Cloud".to_string(),
        provider_kind: ProviderKind::Deepseek,
        base_url: "https://api.deepseek.com/".to_string(),
        model_id: "deepseek-chat".to_string(),
        auth_mode: AuthMode::Bearer,
        enabled: true,
        allow_insecure_http: false,
        require_zero_data_retention: false,
        request_timeout_ms: DEFAULT_TIMEOUT_MS,
        credential_storage: CredentialStorage::None,
        credential_epoch: 0,
        revision: 1,
        created_at: 1,
        updated_at: 1,
    };
    assert_eq!(
        configuration_status_for_test(&profile, CredentialStatus::Missing),
        ProfileConfigurationStatus::Incomplete
    );
    assert_eq!(
        configuration_status_for_test(&profile, CredentialStatus::SavedSecurely),
        ProfileConfigurationStatus::Ready
    );
    profile.enabled = false;
    assert_eq!(
        configuration_status_for_test(&profile, CredentialStatus::SavedSecurely),
        ProfileConfigurationStatus::Disabled
    );
}
