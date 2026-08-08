use std::{fs, path::PathBuf};

use rusqlite::{backup::Backup, params, Connection};

use crate::{database::Database, error::CommandError};

use super::{
    catalog::{AuthMode, ProviderKind},
    secrets::{CredentialPersistence, SecretManager},
};

#[test]
fn database_backups_preserve_profile_metadata_without_session_credentials() {
    let database = Database::open_in_memory().expect("database should open");
    let profile_id = "abababababababababababababababab";
    database
        .with_connection(|connection| {
            connection
                .execute(
                    "INSERT INTO ai_provider_profiles (
                         id, display_name, normalized_name, provider_kind, base_url,
                         model_id, auth_mode, enabled, allow_insecure_http,
                         request_timeout_ms, credential_epoch, revision,
                         created_at, updated_at, deleted_at
                     ) VALUES (?1, 'Backup profile', 'backup profile', ?2,
                               'https://api.deepseek.com/', 'deepseek-chat', ?3,
                               1, 0, 60000, 0, 1, 1, 1, NULL)",
                    params![
                        profile_id,
                        ProviderKind::Deepseek.as_str(),
                        AuthMode::Bearer.as_str()
                    ],
                )
                .map_err(CommandError::database)?;
            Ok(())
        })
        .expect("profile metadata should save");

    let secrets = SecretManager::default();
    let sentinel = "ostudy-backup-must-not-contain-this-credential";
    secrets
        .set(
            profile_id,
            sentinel.to_string(),
            CredentialPersistence::Session,
        )
        .expect("session credential should save");

    let snapshot_path = unique_snapshot_path();
    database
        .with_connection(|connection| {
            let mut target = Connection::open(&snapshot_path).map_err(CommandError::database)?;
            let backup = Backup::new(connection, &mut target).map_err(CommandError::database)?;
            backup
                .run_to_completion(32, std::time::Duration::from_millis(0), None)
                .map_err(CommandError::database)?;
            Ok(())
        })
        .expect("database snapshot should succeed");

    let restored = Connection::open(&snapshot_path).expect("snapshot should open");
    let restored_name: String = restored
        .query_row(
            "SELECT display_name FROM ai_provider_profiles WHERE id = ?1",
            [profile_id],
            |row| row.get(0),
        )
        .expect("profile metadata should be present");
    assert_eq!(restored_name, "Backup profile");

    let bytes = fs::read(&snapshot_path).expect("snapshot bytes should read");
    assert!(!contains_bytes(&bytes, sentinel.as_bytes()));
    fs::remove_file(snapshot_path).expect("snapshot should clean up");
}

fn unique_snapshot_path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "ostudy-ai-backup-test-{}-{}.sqlite3",
        std::process::id(),
        std::thread::current().name().unwrap_or("thread")
    ))
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}
