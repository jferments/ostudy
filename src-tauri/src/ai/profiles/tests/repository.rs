use crate::{
    ai::{
        catalog::ProviderKind,
        profiles::{policy, repository},
    },
    database::Database,
    error::CommandError,
};

use super::default_input;

fn create_test_profile(database: &Database, operation_id: &str, name: &str) -> String {
    let mut input = default_input(ProviderKind::LlamaCpp);
    input.operation_id = operation_id.to_string();
    input.display_name = name.to_string();
    input.model_id = "test-model".to_string();
    let fields = policy::validate_create_profile(&input).expect("profile should validate");
    database
        .with_connection(|connection| repository::create_profile(connection, input, fields))
        .expect("profile should create")
}

#[test]
fn profile_schema_never_persists_credentials() {
    let database = Database::open_in_memory().expect("database should open");
    let profile_id = create_test_profile(
        &database,
        "11111111111111111111111111111111",
        "Local model",
    );
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
                column.contains("secret")
                    || column.contains("token")
                    || column.contains("api_key")
            }));
            let snapshot = repository::load_profile_snapshot(connection, &profile_id)?;
            assert_eq!(snapshot.provider_kind, ProviderKind::LlamaCpp);
            Ok(())
        })
        .unwrap();
}

#[test]
fn operation_replay_is_idempotent_and_conflict_sensitive() {
    let database = Database::open_in_memory().expect("database should open");
    let operation_id = "22222222222222222222222222222222";
    let profile_id = create_test_profile(&database, operation_id, "Replay profile");
    let replayed = database
        .with_connection(|connection| {
            repository::operation_replay(connection, operation_id, "create_profile", None)
        })
        .unwrap()
        .flatten();
    assert_eq!(replayed.as_deref(), Some(profile_id.as_str()));

    let conflict = database
        .with_connection(|connection| {
            repository::operation_replay(
                connection,
                operation_id,
                "delete_profile",
                Some(&profile_id),
            )
        })
        .expect_err("one operation id cannot be reused for another mutation");
    assert_eq!(conflict.code, "ai_operation_conflict");
}

#[test]
fn active_profile_names_remain_unique() {
    let database = Database::open_in_memory().expect("database should open");
    create_test_profile(
        &database,
        "33333333333333333333333333333333",
        "Duplicate",
    );
    let mut input = default_input(ProviderKind::LlamaCpp);
    input.operation_id = "44444444444444444444444444444444".to_string();
    input.display_name = "duplicate".to_string();
    input.model_id = "other".to_string();
    let fields = policy::validate_create_profile(&input).expect("duplicate should validate");
    let error = database
        .with_connection(|connection| repository::create_profile(connection, input, fields))
        .expect_err("normalized names must remain unique");
    assert_eq!(error.code, "ai_profile_name_exists");
}
