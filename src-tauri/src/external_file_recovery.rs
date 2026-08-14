//! Crash recovery for external-file transfers that were interrupted between
//! filesystem promotion and transfer-ledger commit.
//!
//! Selection and byte transfer can complete immediately before Android or the
//! process is terminated. The ready file is therefore treated as authoritative
//! evidence when a ledger row is still `staging`; a remaining partial file is
//! promoted, verified, and synchronized before the row becomes studyable.

use std::{
    fs::{self, File},
    io::Read,
    path::Path,
};

use rusqlite::params;
use sha2::{Digest, Sha256};

use crate::{database::Database, error::CommandError, paths::AppPaths, util::unix_time_millis};

const HASH_BUFFER_BYTES: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StagingRecoveryAction {
    Ready,
    PromotePartial,
    Missing,
}

#[derive(Debug)]
struct StagingRow {
    id: String,
    detected_kind: String,
    relative_path: String,
    byte_size: i64,
    sha256: String,
}

pub(crate) fn reconcile_interrupted_staging(
    database: &Database,
    paths: &AppPaths,
) -> Result<(), CommandError> {
    let rows = database.with_connection(|connection| {
        let mut statement = connection
            .prepare(
                "SELECT id, detected_kind, relative_path, byte_size, sha256
                 FROM staged_external_imports
                 WHERE state = 'staging'",
            )
            .map_err(CommandError::database)?;
        let rows = statement
            .query_map([], |row| {
                Ok(StagingRow {
                    id: row.get(0)?,
                    detected_kind: row.get(1)?,
                    relative_path: row.get(2)?,
                    byte_size: row.get(3)?,
                    sha256: row.get(4)?,
                })
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        Ok(rows)
    })?;

    let now = unix_time_millis()?;
    for row in rows {
        let Some(extension) = extension_for_kind(&row.detected_kind) else {
            set_state(database, &row.id, "failed", now)?;
            continue;
        };
        let expected = AppPaths::staged_import_relative_path(&row.id, extension);
        if row.relative_path != expected {
            set_state(database, &row.id, "failed", now)?;
            continue;
        }

        let ready = paths.resolve_relative(&row.relative_path);
        let partial =
            paths.resolve_relative(&AppPaths::staged_import_partial_relative_path(&row.id));
        let available = match staging_recovery_action(ready.is_file(), partial.is_file()) {
            StagingRecoveryAction::Ready => {
                remove_file_if_exists(&partial)?;
                true
            }
            StagingRecoveryAction::PromotePartial => {
                fs::rename(&partial, &ready).map_err(CommandError::io)?;
                sync_file(&ready)?;
                sync_directory(ready.parent())?;
                true
            }
            StagingRecoveryAction::Missing => false,
        };

        let valid = available && file_matches(&ready, row.byte_size, &row.sha256)?;
        if valid {
            set_state(database, &row.id, "staged", now)?;
        } else {
            remove_file_if_exists(&partial)?;
            remove_file_if_exists(&ready)?;
            set_state(database, &row.id, "failed", now)?;
        }
    }
    Ok(())
}

fn staging_recovery_action(ready_is_file: bool, partial_is_file: bool) -> StagingRecoveryAction {
    if ready_is_file {
        StagingRecoveryAction::Ready
    } else if partial_is_file {
        StagingRecoveryAction::PromotePartial
    } else {
        StagingRecoveryAction::Missing
    }
}

fn set_state(database: &Database, id: &str, state: &str, now: i64) -> Result<(), CommandError> {
    database.with_connection(|connection| {
        let changed = connection
            .execute(
                "UPDATE staged_external_imports SET state = ?1, updated_at = ?2
                 WHERE id = ?3 AND state = 'staging'",
                params![state, now, id],
            )
            .map_err(CommandError::database)?;
        if changed != 1 {
            return Err(CommandError::new(
                "external_file_state_changed",
                "The staged external-file transfer changed during startup recovery.",
            ));
        }
        Ok(())
    })
}

fn extension_for_kind(kind: &str) -> Option<&'static str> {
    match kind {
        "png" => Some("png"),
        "jpeg" => Some("jpg"),
        "gif" => Some("gif"),
        "webp" => Some("webp"),
        "pdf" => Some("pdf"),
        "studydeck" => Some("studydeck"),
        "generated_json" => Some("json"),
        "ostudybackup" => Some("ostudybackup"),
        _ => None,
    }
}

fn file_matches(
    path: &Path,
    expected_size: i64,
    expected_hash: &str,
) -> Result<bool, CommandError> {
    if !path.is_file() {
        return Ok(false);
    }
    let metadata = fs::metadata(path).map_err(CommandError::io)?;
    if i64::try_from(metadata.len()).unwrap_or(i64::MAX) != expected_size {
        return Ok(false);
    }
    Ok(hash_file(path)? == expected_hash)
}

fn hash_file(path: &Path) -> Result<String, CommandError> {
    let mut file = File::open(path).map_err(CommandError::io)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; HASH_BUFFER_BYTES];
    loop {
        let read = file.read(&mut buffer).map_err(CommandError::io)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn remove_file_if_exists(path: &Path) -> Result<(), CommandError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(CommandError::io(error)),
    }
}

fn sync_file(path: &Path) -> Result<(), CommandError> {
    File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(CommandError::io)
}

#[cfg(unix)]
fn sync_directory(path: Option<&Path>) -> Result<(), CommandError> {
    if let Some(path) = path {
        File::open(path)
            .and_then(|directory| directory.sync_all())
            .map_err(CommandError::io)?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn sync_directory(_path: Option<&Path>) -> Result<(), CommandError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    use rusqlite::params;

    use super::*;

    static NEXT_WORKSPACE: AtomicU64 = AtomicU64::new(1);

    struct TestWorkspace {
        root: PathBuf,
        paths: AppPaths,
    }

    impl TestWorkspace {
        fn new() -> Self {
            let sequence = NEXT_WORKSPACE.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "ostudy-external-recovery-{}-{sequence}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&root);
            let paths = AppPaths::new(root.clone()).unwrap();
            Self { root, paths }
        }
    }

    impl Drop for TestWorkspace {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn ready_file_wins_after_rename_before_ledger_commit() {
        assert_eq!(
            staging_recovery_action(true, false),
            StagingRecoveryAction::Ready
        );
        assert_eq!(
            staging_recovery_action(true, true),
            StagingRecoveryAction::Ready
        );
    }

    #[test]
    fn partial_file_is_promoted_when_rename_never_completed() {
        assert_eq!(
            staging_recovery_action(false, true),
            StagingRecoveryAction::PromotePartial
        );
    }

    #[test]
    fn missing_transfer_is_failed_instead_of_left_staging() {
        assert_eq!(
            staging_recovery_action(false, false),
            StagingRecoveryAction::Missing
        );
    }

    #[test]
    fn every_staged_kind_maps_to_its_canonical_extension() {
        assert_eq!(extension_for_kind("png"), Some("png"));
        assert_eq!(extension_for_kind("jpeg"), Some("jpg"));
        assert_eq!(extension_for_kind("gif"), Some("gif"));
        assert_eq!(extension_for_kind("webp"), Some("webp"));
        assert_eq!(extension_for_kind("pdf"), Some("pdf"));
        assert_eq!(extension_for_kind("studydeck"), Some("studydeck"));
        assert_eq!(extension_for_kind("generated_json"), Some("json"));
        assert_eq!(extension_for_kind("ostudybackup"), Some("ostudybackup"));
        assert_eq!(extension_for_kind("unknown"), None);
    }

    #[test]
    fn valid_partial_file_is_promoted_and_committed() {
        let database = Database::open_in_memory().unwrap();
        let workspace = TestWorkspace::new();
        let id = "11111111111111111111111111111111";
        let bytes: &[u8] = b"partial-transfer";
        let relative = AppPaths::staged_import_relative_path(id, "png");
        let partial = workspace
            .paths
            .resolve_relative(&AppPaths::staged_import_partial_relative_path(id));
        let ready = workspace.paths.resolve_relative(&relative);
        fs::write(&partial, bytes).unwrap();
        insert_staging_row(&database, id, &relative, bytes);

        reconcile_interrupted_staging(&database, &workspace.paths).unwrap();

        let actual = fs::read(&ready).unwrap();
        assert_eq!(actual.as_slice(), bytes);
        assert!(!partial.exists());
        assert_eq!(load_state(&database, id), "staged");
    }

    #[test]
    fn valid_ready_file_is_kept_and_leftover_partial_is_removed() {
        let database = Database::open_in_memory().unwrap();
        let workspace = TestWorkspace::new();
        let id = "22222222222222222222222222222222";
        let bytes: &[u8] = b"ready-transfer";
        let relative = AppPaths::staged_import_relative_path(id, "png");
        let partial = workspace
            .paths
            .resolve_relative(&AppPaths::staged_import_partial_relative_path(id));
        let ready = workspace.paths.resolve_relative(&relative);
        fs::write(&ready, bytes).unwrap();
        fs::write(&partial, b"leftover-partial").unwrap();
        insert_staging_row(&database, id, &relative, bytes);

        reconcile_interrupted_staging(&database, &workspace.paths).unwrap();

        let actual = fs::read(&ready).unwrap();
        assert_eq!(actual.as_slice(), bytes);
        assert!(!partial.exists());
        assert_eq!(load_state(&database, id), "staged");
    }

    #[test]
    fn corrupt_ready_file_is_removed_and_failed() {
        let database = Database::open_in_memory().unwrap();
        let workspace = TestWorkspace::new();
        let id = "33333333333333333333333333333333";
        let expected: &[u8] = b"expected-transfer";
        let relative = AppPaths::staged_import_relative_path(id, "png");
        let partial = workspace
            .paths
            .resolve_relative(&AppPaths::staged_import_partial_relative_path(id));
        let ready = workspace.paths.resolve_relative(&relative);
        fs::write(&ready, b"corrupt").unwrap();
        fs::write(&partial, b"leftover-partial").unwrap();
        insert_staging_row(&database, id, &relative, expected);

        reconcile_interrupted_staging(&database, &workspace.paths).unwrap();

        assert!(!ready.exists());
        assert!(!partial.exists());
        assert_eq!(load_state(&database, id), "failed");
    }

    fn insert_staging_row(database: &Database, id: &str, relative: &str, bytes: &[u8]) {
        let byte_size = i64::try_from(bytes.len()).unwrap();
        let sha256 = format!("{:x}", Sha256::digest(bytes));
        database
            .with_connection(|connection| {
                connection
                    .execute(
                        "INSERT INTO staged_external_imports (
                             id, operation_id, purpose, detected_kind, display_name,
                             media_type, byte_size, sha256, relative_path, state,
                             created_at, updated_at, expires_at
                         ) VALUES (?1, ?1, 'image', 'png', 'test.png', 'image/png',
                                   ?2, ?3, ?4, 'staging', 1, 1, 2)",
                        params![id, byte_size, sha256, relative],
                    )
                    .map_err(CommandError::database)?;
                Ok(())
            })
            .unwrap();
    }

    fn load_state(database: &Database, id: &str) -> String {
        database
            .with_connection(|connection| {
                connection
                    .query_row(
                        "SELECT state FROM staged_external_imports WHERE id = ?1",
                        [id],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)
            })
            .unwrap()
    }
}
