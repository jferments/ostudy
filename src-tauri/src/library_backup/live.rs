//! Same-process restore. The managed Database stays stable; its connection is
//! replaced only while all IPC leases have drained and detached work is fenced.
//! Completion receipts live outside the replaced SQLite library and become
//! authoritative only when the filesystem transaction has its commit marker.

use super::*;
use crate::{external_files, library_session, task_activity::MaintenanceGuard};

const RESULTS: &str = "restore-results";
const LAST_RESULT: &str = "library-last-restore.json";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LiveRestoreResult {
    pub operation_id: String,
    pub deck_count: i64,
    pub card_count: i64,
    pub document_count: i64,
    pub template_count: i64,
    pub review_count: i64,
    pub asset_count: i64,
    pub restart_required: bool,
    pub completed: bool,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Receipt {
    version: u32,
    transaction_id: String,
    source_sha256: String,
    result: LiveRestoreResult,
}

/// Caller runs this on the blocking pool. No application event loop is blocked.
pub(crate) fn restore(
    database: &Database,
    paths: &AppPaths,
    input: PrepareLibraryRestoreInput,
    generation: Option<u64>,
    clear_session_profiles: impl FnOnce(&[String]) -> Result<(), CommandError>,
) -> Result<LiveRestoreResult, CommandError> {
    crate::util::validate_entity_id(&input.staged_file_id, "restore operation")?;
    // A response lost after commit may be queried safely even with its old epoch.
    // This is a read of the original receipt, never a second destructive write.
    if let Some(result) = completed_result(paths, &input.staged_file_id)? {
        return Ok(result);
    }
    let _maintenance = MaintenanceGuard::begin()?;
    let mut replacement = library_session::begin_restore(generation)?;
    let staged = database.with_connection(|connection| {
        external_files::resolve_staged_import(
            connection,
            paths,
            &input.staged_file_id,
            external_files::ExternalImportPurpose::LibraryBackup,
            &[external_files::ExternalFileKind::Ostudybackup],
        )
    })?;
    // Invalid input fails before closing the old connection or moving any data.
    validate_backup_package(&staged.path, None)?;
    let outcome = activate(
        database,
        paths,
        &staged.path,
        &input.staged_file_id,
        crate::startup::reconcile_library,
        || replacement.switching(),
    );
    if outcome.connection_ready {
        if outcome.result.is_ok() {
            // Session-only credentials must not leak from the replaced library.
            // The OS keyring is not changed. A poisoned runtime stays fail-closed.
            clear_session_profiles(&outcome.profiles)?;
        }
        replacement.finish(outcome.changed)?;
    }
    outcome.result
}

struct ActivationOutcome {
    result: Result<LiveRestoreResult, CommandError>,
    connection_ready: bool,
    changed: bool,
    profiles: Vec<String>,
}

fn activate(
    database: &Database,
    paths: &AppPaths,
    source: &Path,
    operation: &str,
    reconcile: impl Fn(&Database, &AppPaths) -> Result<(), CommandError>,
    mut switching: impl FnMut(),
) -> ActivationOutcome {
    let mut ready = true;
    let mut changed = false;
    let mut profiles = Vec::new();
    let result = database.with_connection(|active| {
        profiles = profile_ids(active)?;
        let placeholder = Connection::open_in_memory().map_err(CommandError::database)?;
        let old = std::mem::replace(active, placeholder);
        if let Err((old, error)) = old.close() {
            *active = old;
            return Err(CommandError::database(error));
        }
        ready = false;
        changed = true;
        switching();
        let attempt = (|| {
            // Recovery must run with the connection closed; a leftover journal
            // is not permission to start a second activation in the same pass.
            if activation::recover(paths).map_err(recovery_error)? {
                return Err(CommandError::new(
                    "library_restore_recovered",
                    "An interrupted restore was recovered. Review the recovered library before starting another restore.",
                ));
            }
            // Preserve an older prepared backup outside its automatic startup
            // slot so it cannot overwrite this restore on a later launch.
            activation::preserve_pending(paths).map_err(recovery_error)?;
            let mut guard = activation::apply_source_restore(paths, source)
                .map_err(recovery_error)?
                .ok_or_else(|| recovery_error("The selected restore source is unavailable."))?;
            let candidate: Result<(Connection, LiveRestoreResult), CommandError> = (|| {
                let restored = Database::open(paths.database_path()).map_err(recovery_error)?;
                reconcile(&restored, paths)?;
                let result = restored.with_connection(|connection| {
                    validate_snapshot_database(connection)?;
                    let violation = connection
                        .query_row("PRAGMA foreign_key_check", [], |_| Ok(()))
                        .optional()
                        .map_err(CommandError::database)?;
                    if violation.is_some() {
                        return Err(recovery_error("The restored library has a foreign-key violation."));
                    }
                    profiles.extend(profile_ids(connection)?);
                    let summary = snapshot_summary(connection)?;
                    let counts: (i64, i64) = connection.query_row(
                        "SELECT (SELECT COUNT(*) FROM cards), (SELECT COUNT(*) FROM documents)",
                        [], |row| Ok((row.get(0)?, row.get(1)?)),
                    ).map_err(CommandError::database)?;
                    Ok(LiveRestoreResult {
                        operation_id: operation.to_string(),
                        deck_count: summary.deck_count,
                        card_count: counts.0,
                        document_count: counts.1,
                        template_count: summary.template_count,
                        review_count: summary.review_count,
                        asset_count: summary.asset_count,
                        restart_required: false,
                        completed: true,
                    })
                })?;
                write_receipt(paths, &guard, &result)?;
                let connection = take_connection(&restored)?;
                Ok((connection, result))
            })();
            let (connection, result) = match candidate {
                Ok(value) => value,
                Err(error) => {
                    guard.rollback().map_err(recovery_error)?;
                    return Err(error);
                }
            };
            if let Err(error) = guard.try_commit() {
                drop(connection);
                if guard.commit_decided() {
                    // The marker may already be durable. Never roll back or
                    // report a retryable pre-commit failure across this boundary.
                    return Err(CommandError::new(
                        "library_restore_commit_indeterminate",
                        format!("The restore reached its commit record but durability confirmation failed. Preserve recovery files and reopen oStudy to resolve the recorded transaction: {error}"),
                    ));
                }
                guard.rollback().map_err(recovery_error)?;
                return Err(recovery_error(error));
            }
            *active = connection;
            ready = true;
            Ok(result)
        })();
        match attempt {
            Ok(result) => Ok(result),
            Err(error) if error.code == "library_restore_commit_indeterminate" => Err(error),
            Err(error) => {
                // A failed rollback must not be hidden by opening a half-swapped
                // directory. Complete journal recovery first or leave IPC closed.
                activation::recover(paths).map_err(recovery_error)?;
                let original = Database::open(paths.database_path()).map_err(recovery_error)?;
                *active = take_connection(&original)?;
                ready = true;
                Err(CommandError::new(
                    "library_restore_rolled_back",
                    format!("The restore was not completed. A coherent previous/recovered library was reopened; refresh the view. The backup and recovery files were preserved. {}", error.message),
                ))
            }
        }
    });
    ActivationOutcome { result, connection_ready: ready, changed, profiles }
}

fn take_connection(database: &Database) -> Result<Connection, CommandError> {
    database.with_connection(|connection| {
        let placeholder = Connection::open_in_memory().map_err(CommandError::database)?;
        Ok(std::mem::replace(connection, placeholder))
    })
}

fn profile_ids(connection: &Connection) -> Result<Vec<String>, CommandError> {
    let mut query = connection.prepare("SELECT id FROM ai_provider_profiles")
        .map_err(CommandError::database)?;
    let result = query.query_map([], |row| row.get(0)).map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>().map_err(CommandError::database);
    result
}

fn write_receipt(
    paths: &AppPaths,
    guard: &activation::PendingRestoreGuard,
    result: &LiveRestoreResult,
) -> Result<(), CommandError> {
    let receipt = Receipt {
        version: 1,
        transaction_id: guard.operation_id().to_string(),
        source_sha256: activation::hash_file(&guard.transaction().join("source.ostudybackup"))
            .map_err(recovery_error)?,
        result: result.clone(),
    };
    let bytes = serde_json::to_vec(&receipt).map_err(CommandError::serialization)?;
    // This receipt is retained with its transaction even when an uncommitted
    // attempt is superseded. Index replacement does not delete the evidence.
    write_new_file(&guard.transaction().join("result.json"), &bytes)?;
    for (name, relative) in [
        ("result-index.tmp", Path::new(RESULTS).join(format!("{}.json", result.operation_id))),
        ("last-result.tmp", PathBuf::from(LAST_RESULT)),
    ] {
        activation::check_path(paths.app_data_dir(), &relative).map_err(recovery_error)?;
        let destination = paths.app_data_dir().join(relative);
        fs::create_dir_all(destination.parent().unwrap()).map_err(CommandError::io)?;
        let temporary = guard.transaction().join(name);
        write_new_file(&temporary, &bytes)?;
        fs::rename(&temporary, &destination).map_err(CommandError::io)?;
        sync_directory(destination.parent().unwrap())?;
        sync_directory(&guard.transaction())?;
    }
    Ok(())
}

fn read_receipt(paths: &AppPaths, relative: &Path) -> Result<Option<LiveRestoreResult>, CommandError> {
    activation::check_path(paths.app_data_dir(), relative).map_err(recovery_error)?;
    let path = paths.app_data_dir().join(relative);
    if !path.exists() {
        return Ok(None);
    }
    if fs::metadata(&path).map_err(CommandError::io)?.len() > 64 * 1024 {
        return Err(recovery_error("The restore receipt is oversized. It was preserved."));
    }
    let receipt: Receipt = serde_json::from_slice(&fs::read(path).map_err(CommandError::io)?)
        .map_err(CommandError::serialization)?;
    crate::util::validate_entity_id(&receipt.result.operation_id, "restore receipt")?;
    if receipt.version != 1 || !receipt.result.completed || receipt.result.restart_required
        || receipt.source_sha256.len() != 64
        || !receipt.source_sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(recovery_error("The restore receipt is invalid. It was preserved."));
    }
    if !activation::transaction_committed(paths.app_data_dir(), &receipt.transaction_id)
        .map_err(recovery_error)?
    {
        return Ok(None);
    }
    Ok(Some(receipt.result))
}

pub(crate) fn completed_result(paths: &AppPaths, operation: &str) -> Result<Option<LiveRestoreResult>, CommandError> {
    crate::util::validate_entity_id(operation, "restore operation")?;
    let result = read_receipt(paths, &Path::new(RESULTS).join(format!("{operation}.json")))?;
    if result.as_ref().is_some_and(|result| result.operation_id != operation) {
        return Err(recovery_error("The restore receipt does not match the requested operation."));
    }
    Ok(result)
}

pub(crate) fn last_result(paths: &AppPaths) -> Result<Option<LiveRestoreResult>, CommandError> {
    read_receipt(paths, Path::new(LAST_RESULT))
}

fn recovery_error(message: impl Into<String>) -> CommandError {
    CommandError::new("library_recovery_required", message.into())
}

#[cfg(test)]
#[path = "live_tests.rs"]
mod current_format_tests;
