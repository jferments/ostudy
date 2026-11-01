//! Process-owned whole-library upgrade checkpoints. SQLite is copied through
//! its Backup API; managed bytes are streamed into a private sibling tree.
//! A durable intent prevents an interrupted preparation from silently retrying.
//! Activation and rollback use the existing library replacement journal.

use super::{
    check_path, hash_file, hash_tree, install_prepared_restore, io_error, meter, move_synced,
    new_transaction, sync, sync_tree, transaction_committed, write_new_file, AppPaths, Database,
    Layout, Path, PathBuf, PendingRestoreGuard, PreparedRestore, JOURNAL, TRANSACTIONS,
};
use rusqlite::{
    backup::{Backup, StepResult},
    Connection, OpenFlags,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{fs, io::Read};

#[path = "checkpoint_preservation.rs"]
mod preservation;

const INTENT: &str = "library-upgrade-intent.json";
const MANIFEST: &str = "checkpoint.json";
const BUILD: &str = "library-build.json";
const MANAGED: &[&str] = &["assets", "documents", "transfers", "cache"];
const MAX_METADATA_BYTES: u64 = 16 * 1024;
pub(crate) const RECOVERY_REQUIRED: &str = "OSTUDY_UPGRADE_RECOVERY_REQUIRED";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BuildIdentity {
    version: String,
    commit: Option<String>,
}

impl BuildIdentity {
    fn current() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").into(),
            commit: option_env!("QUALIFIED_SHA")
                .or(option_env!("OSTUDY_BUILD_SHA"))
                .filter(|value| {
                    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
                })
                .map(str::to_string),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BuildRecord {
    version: u32,
    application: String,
    lineage: String,
    schema: i64,
    build: BuildIdentity,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
enum IntentKind {
    Upgrade,
    Revert,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Intent {
    version: u32,
    operation_id: String,
    kind: IntentKind,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Manifest {
    version: u32,
    operation_id: String,
    application: String,
    source_lineage: String,
    source_schema: i64,
    target_lineage: String,
    target_schema: i64,
    from_build: Option<BuildIdentity>,
    to_build: Option<BuildIdentity>,
    created_at: i64,
    checkpoint_sha256: String,
}

pub(crate) fn has_intent(paths: &AppPaths) -> Result<bool, String> {
    check_path(paths.app_data_dir(), Path::new(INTENT))?;
    Ok(paths.app_data_dir().join(INTENT).exists())
}

/// Called after journal recovery but before any Database::open. A failed or
/// interrupted upgrade is a user decision, not a request to run it again.
pub(crate) fn admit_startup(paths: &AppPaths) -> Result<(), String> {
    admit_startup_for_schema(
        paths,
        crate::database::CURRENT_SCHEMA_LINEAGE,
        crate::database::CURRENT_SCHEMA_VERSION,
    )
}

fn admit_startup_for_schema(paths: &AppPaths, lineage: &str, version: i64) -> Result<(), String> {
    let Some(intent) = read_intent(paths)? else {
        return Ok(());
    };
    ensure_no_journal(paths.app_data_dir())?;
    if transaction_committed(paths.app_data_dir(), &intent.operation_id)? {
        let compatible = if intent.kind == IntentKind::Revert {
            let transaction = transaction_path(paths, &intent.operation_id)?;
            let manifest = verify_checkpoint(&transaction)?;
            let expected = (manifest.source_lineage.clone(), manifest.source_schema);
            manifest.source_lineage == lineage
                && manifest.source_schema == version
                && validate_database(&read_database(paths.database_path())?)? == expected
        } else {
            true
        };
        if compatible {
            if let Err(error) = archive_intent(paths, &intent, "completed-intent.json") {
                crate::logging::warn_target(
                    "library.upgrade",
                    "upgrade_receipt_cleanup_pending",
                    "The activation committed; retained bookkeeping must not undo that decision.",
                    serde_json::json!({"operationId": intent.operation_id, "error": error}),
                );
            }
            return Ok(());
        }
    }
    Err(format!("{RECOVERY_REQUIRED}: A previous library update needs an explicit recovery decision. All checkpoint and library files were retained."))
}

pub(crate) fn begin_upgrade(
    paths: &AppPaths,
    application: &str,
    expected_source: (&str, i64),
    progress: &mut dyn FnMut(&str),
) -> Result<PendingRestoreGuard, String> {
    let prepared = prepare_upgrade_with(
        paths,
        application,
        expected_source,
        progress,
        None,
        |path| {
            let database = Database::open(path)?;
            settle_candidate(&database)
        },
    )?;
    progress("Activating the verified library update…");
    install_prepared_restore(paths, prepared)
}

fn prepare_upgrade_with<F>(
    paths: &AppPaths,
    application: &str,
    expected_source: (&str, i64),
    progress: &mut dyn FnMut(&str),
    page_limit: Option<i64>,
    migrate: F,
) -> Result<PreparedRestore, String>
where
    F: FnOnce(&Path) -> Result<(), String>,
{
    admit_startup(paths)?;
    let root = paths.app_data_dir();
    ensure_no_journal(root)?;
    validate_source_paths(root)?;
    let source_presence = MANAGED
        .iter()
        .map(|relative| root.join(relative).try_exists().map_err(io_error))
        .collect::<Result<Vec<_>, _>>()?;
    let source = read_database(paths.database_path())?;
    let identity = validate_database(&source)?;
    if identity.0 != expected_source.0 || identity.1 != expected_source.1 {
        return Err(
            "The library changed after upgrade planning. No migration was attempted.".into(),
        );
    }
    let source_revision = data_version(&source)?;
    meter::plan(planned_upgrade_work(root, paths.database_path())?);
    let transaction = new_transaction(root)?;
    let operation_id = operation_id(&transaction)?;
    write_intent(
        paths,
        &transaction,
        &Intent {
            version: 1,
            operation_id: operation_id.clone(),
            kind: IntentKind::Upgrade,
        },
        false,
    )?;

    progress("Creating a recovery checkpoint…");
    let checkpoint = transaction.join("checkpoint");
    fs::create_dir(&checkpoint).map_err(io_error)?;
    snapshot_database(&source, &checkpoint.join("library.sqlite3"), page_limit)?;
    meter::advance(file_size(&checkpoint.join("library.sqlite3"))?);
    for relative in MANAGED {
        copy_managed_directory(root, &checkpoint, relative)?;
    }
    if root.join(BUILD).exists() {
        copy_verified_file(&root.join(BUILD), &checkpoint.join(BUILD))?;
    }
    let snapshot = read_database(&checkpoint.join("library.sqlite3"))?;
    if validate_database(&snapshot)? != identity {
        return Err("The recovery checkpoint changed the source schema identity.".into());
    }
    drop(snapshot);
    let from_build = read_build(&checkpoint, application, &identity);
    sync_tree(&checkpoint)?;
    let manifest = Manifest {
        version: 1,
        operation_id,
        application: application.into(),
        source_lineage: identity.0,
        source_schema: identity.1,
        target_lineage: crate::database::CURRENT_SCHEMA_LINEAGE.into(),
        target_schema: crate::database::CURRENT_SCHEMA_VERSION,
        from_build,
        to_build: Some(BuildIdentity::current()),
        created_at: crate::util::unix_time_millis().map_err(|error| error.message)?,
        checkpoint_sha256: hash_tree(&checkpoint)?,
    };
    write_json(&transaction.join(MANIFEST), &manifest)?;
    sync(&transaction)?;
    verify_checkpoint(&transaction)?;

    progress("Updating a private library copy…");
    let candidate = transaction.join("candidate");
    copy_tree(&checkpoint, &candidate)?;
    migrate(&candidate.join("library.sqlite3"))?;
    meter::advance(file_size(&candidate.join("library.sqlite3"))?);
    progress("Verifying the updated library…");
    let migrated = read_database(&candidate.join("library.sqlite3"))?;
    let target = validate_database(&migrated)?;
    if target != (manifest.target_lineage.clone(), manifest.target_schema) {
        return Err("The private library did not reach the required target schema.".into());
    }
    preservation::verify(
        &source,
        &migrated,
        &manifest.source_lineage,
        manifest.source_schema,
    )?;
    meter::advance(file_size(&candidate.join("library.sqlite3"))?.saturating_mul(2));
    drop(migrated);
    // Schema migration may change the database, never source document bytes.
    for relative in MANAGED {
        if hash_tree(&candidate.join(relative))? != hash_tree(&checkpoint.join(relative))? {
            return Err("Migration changed managed files before activation. The original library was preserved.".into());
        }
    }
    write_build(&candidate, application, &target)?;
    sync_tree(&candidate)?;
    verify_checkpoint(&transaction)?;
    if data_version(&source)? != source_revision {
        return Err("Another writer changed the source library during checkpoint preparation. The active library was not replaced.".into());
    }
    for (relative, was_present) in MANAGED.iter().zip(source_presence) {
        let present = root.join(relative).try_exists().map_err(io_error)?;
        if present != was_present
            || (present
                && hash_tree(&root.join(relative))? != hash_tree(&checkpoint.join(relative))?)
        {
            return Err("Managed files changed during checkpoint preparation. The active library was not replaced.".into());
        }
    }
    drop(source);
    prepared_checkpoint(root, transaction)
}

pub(crate) fn commit_upgrade(
    paths: &AppPaths,
    guard: &mut PendingRestoreGuard,
) -> Result<(), String> {
    // Reconciliation can mutate the newly activated database. Validate its
    // final state before deciding commit, not only the pre-activation candidate.
    let active = read_database(paths.database_path())?;
    let identity = validate_database(&active)?;
    if identity
        != (
            crate::database::CURRENT_SCHEMA_LINEAGE.into(),
            crate::database::CURRENT_SCHEMA_VERSION,
        )
    {
        return Err("The reconciled library did not retain the target schema.".into());
    }
    drop(active);
    guard.try_commit()?;
    // The activation decision is already committed. Bookkeeping failure must
    // not misreport a rollback or invite another migration of committed data.
    let cleanup = (|| {
        let Some(intent) = read_intent(paths)? else {
            return Ok(());
        };
        if intent.operation_id != guard.operation_id() {
            return Err(
                "The committed library update has a different recovery intent.".to_string(),
            );
        }
        archive_intent(paths, &intent, "completed-intent.json")
    })();
    if let Err(error) = cleanup {
        crate::logging::warn_target(
            "library.upgrade",
            "upgrade_receipt_cleanup_pending",
            "The library update committed; recovery bookkeeping needs attention.",
            serde_json::json!({"operationId": guard.operation_id(), "error": error}),
        );
    }
    Ok(())
}

/// An explicit user action only. It authorizes a new attempt on the next launch,
/// keeping the old attempt and all checkpoint/candidate bytes for inspection.
pub(crate) fn allow_retry(paths: &AppPaths, expected_operation: &str) -> Result<(), String> {
    ensure_no_journal(paths.app_data_dir())?;
    let intent =
        read_intent(paths)?.ok_or_else(|| "No pending library update exists.".to_string())?;
    if intent.operation_id != expected_operation {
        return Err("The recovery operation changed. No library was replaced.".into());
    }
    archive_intent(paths, &intent, "acknowledged-intent.json")
}

/// Restore old bytes, not reverse SQL. The existing activation owner retains
/// the replaced current library too. The Revert intent deliberately survives
/// success so this new executable cannot immediately migrate the old copy again.
pub(crate) fn restore_checkpoint(
    paths: &AppPaths,
    expected_operation: &str,
    progress: &mut dyn FnMut(&str),
) -> Result<(), String> {
    ensure_no_journal(paths.app_data_dir())?;
    let intent =
        read_intent(paths)?.ok_or_else(|| "No pending library update exists.".to_string())?;
    if intent.operation_id != expected_operation {
        return Err("The recovery operation changed. No library was replaced.".into());
    }
    let original = transaction_path(paths, &intent.operation_id)?;
    let source_manifest = verify_checkpoint(&original)?;
    progress("Verifying the previous library checkpoint…");
    let transaction = new_transaction(paths.app_data_dir())?;
    let operation_id = operation_id(&transaction)?;
    let checkpoint = transaction.join("checkpoint");
    copy_tree(&original.join("checkpoint"), &checkpoint)?;
    let mut manifest = source_manifest;
    manifest.operation_id = operation_id.clone();
    manifest.target_lineage = manifest.source_lineage.clone();
    manifest.target_schema = manifest.source_schema;
    manifest.to_build = manifest.from_build.clone();
    manifest.created_at = crate::util::unix_time_millis().map_err(|error| error.message)?;
    manifest.checkpoint_sha256 = hash_tree(&checkpoint)?;
    write_json(&transaction.join(MANIFEST), &manifest)?;
    copy_tree(&checkpoint, &transaction.join("candidate"))?;
    sync_tree(&transaction)?;
    let prepared = prepared_checkpoint(paths.app_data_dir(), transaction.clone())?;
    write_intent(
        paths,
        &transaction,
        &Intent {
            version: 1,
            operation_id,
            kind: IntentKind::Revert,
        },
        true,
    )?;
    progress("Restoring the previous library…");
    let mut guard = install_prepared_restore(paths, prepared)?;
    let restored = read_database(paths.database_path())?;
    if validate_database(&restored)? != (manifest.source_lineage, manifest.source_schema) {
        return Err(
            "The restored checkpoint failed schema validation. Recovery files were preserved."
                .into(),
        );
    }
    drop(restored);
    guard.try_commit()?;
    Ok(())
}

/// Only nonsecret build evidence. A missing or malformed old receipt is unknown,
/// never inferred from the version of the binary that is opening it now.
pub(crate) fn record_current_build(paths: &AppPaths, application: &str) -> Result<(), String> {
    write_build(
        paths.app_data_dir(),
        application,
        &(
            crate::database::CURRENT_SCHEMA_LINEAGE.into(),
            crate::database::CURRENT_SCHEMA_VERSION,
        ),
    )
}

pub(crate) struct RecoveryDetails {
    pub operation_id: String,
    pub checkpoint_available: bool,
    pub previous_restored: bool,
    pub activation_unresolved: bool,
}

pub(crate) fn recovery_details(paths: &AppPaths) -> Result<Option<RecoveryDetails>, String> {
    let Some(intent) = read_intent(paths)? else {
        return Ok(None);
    };
    let transaction = transaction_path(paths, &intent.operation_id)?;
    check_path(&transaction, Path::new(MANIFEST))?;
    check_path(paths.app_data_dir(), Path::new(JOURNAL))?;
    Ok(Some(RecoveryDetails {
        previous_restored: intent.kind == IntentKind::Revert
            && transaction_committed(paths.app_data_dir(), &intent.operation_id)?,
        operation_id: intent.operation_id,
        checkpoint_available: transaction.join(MANIFEST).is_file(),
        activation_unresolved: paths.app_data_dir().join(JOURNAL).exists(),
    }))
}

fn prepared_checkpoint(root: &Path, transaction: PathBuf) -> Result<PreparedRestore, String> {
    verify_checkpoint(&transaction)?;
    Ok(PreparedRestore {
        layout: Layout::Checkpoint,
        root: root.into(),
        source_sha256: hash_file(&transaction.join(MANIFEST))?,
        candidate_sha256: hash_tree(&transaction.join("candidate"))?,
        transaction,
    })
}

pub(super) fn verify_checkpoint(transaction: &Path) -> Result<Manifest, String> {
    check_path(transaction, Path::new(MANIFEST))?;
    check_path(transaction, Path::new("checkpoint"))?;
    let manifest: Manifest = read_json(&transaction.join(MANIFEST))?;
    if manifest.version != 1
        || manifest.operation_id != operation_id(transaction)?
        || manifest.checkpoint_sha256 != hash_tree(&transaction.join("checkpoint"))?
    {
        return Err(
            "The recovery checkpoint changed or is unsupported. All copies were preserved.".into(),
        );
    }
    let database = read_database(&transaction.join("checkpoint/library.sqlite3"))?;
    if validate_database(&database)? != (manifest.source_lineage.clone(), manifest.source_schema) {
        return Err("The recovery checkpoint schema does not match its manifest.".into());
    }
    Ok(manifest)
}

fn snapshot_database(
    source: &Connection,
    destination: &Path,
    page_limit: Option<i64>,
) -> Result<(), String> {
    let mut target = Connection::open(destination).map_err(|error| error.to_string())?;
    target.execute_batch("PRAGMA journal_mode = DELETE; PRAGMA synchronous = FULL; PRAGMA cache_size = -16384; PRAGMA temp_store = FILE; PRAGMA mmap_size = 0;")
        .map_err(|error| error.to_string())?;
    if let Some(limit) = page_limit {
        target
            .pragma_update(None, "max_page_count", limit)
            .map_err(|error| error.to_string())?;
    }
    {
        let revision = data_version(source)?;
        let backup = Backup::new(source, &mut target).map_err(|error| error.to_string())?;
        loop {
            if data_version(source)? != revision {
                return Err(
                    "Another writer changed the library during its checkpoint copy.".into(),
                );
            }
            match backup
                .step(128)
                .map_err(|error| format!("Could not create the recovery checkpoint: {error}"))?
            {
                StepResult::Done => break,
                StepResult::More => {}
                _ => return Err(
                    "The library is busy. Close other library users before retrying the update."
                        .into(),
                ),
            }
        }
    }
    let (busy, _, _): (i64, i64, i64) = target
        .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })
        .map_err(|error| error.to_string())?;
    let mode: String = target
        .query_row("PRAGMA journal_mode = DELETE", [], |row| row.get(0))
        .map_err(|error| error.to_string())?;
    if busy != 0 || mode != "delete" {
        return Err("The recovery snapshot could not settle its SQLite journal.".into());
    }
    target.close().map_err(|(_, error)| error.to_string())?;
    crate::util::durable_fs::sync_file(destination).map_err(io_error)
}

fn settle_candidate(database: &Database) -> Result<(), String> {
    database
        .with_connection(|connection| {
            let (busy, _, _): (i64, i64, i64) = connection
                .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
                    Ok((row.get(0)?, row.get(1)?, row.get(2)?))
                })
                .map_err(crate::error::CommandError::database)?;
            if busy != 0 {
                return Err(crate::error::CommandError::new(
                    "upgrade_candidate_busy",
                    "The private migration candidate could not be checkpointed.",
                ));
            }
            let mode: String = connection
                .query_row("PRAGMA journal_mode = DELETE", [], |row| row.get(0))
                .map_err(crate::error::CommandError::database)?;
            if mode != "delete" {
                return Err(crate::error::CommandError::new(
                    "upgrade_candidate_journal",
                    "The private candidate still requires a journal.",
                ));
            }
            Ok(())
        })
        .map_err(|error| error.message)
}

fn validate_database(connection: &Connection) -> Result<(String, i64), String> {
    let identity = crate::database::schema_identity(connection).map_err(|error| error.message)?;
    let integrity: String = connection
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(|error| error.to_string())?;
    let broken: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM pragma_foreign_key_check)",
            [],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    if integrity != "ok" || broken {
        return Err("The library failed integrity or relationship validation.".into());
    }
    Ok(identity)
}

fn read_database(path: &Path) -> Result<Connection, String> {
    if !fs::symlink_metadata(path)
        .map_err(io_error)?
        .file_type()
        .is_file()
    {
        return Err("A checkpoint database must be a regular file.".into());
    }
    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|error| error.to_string())
}

fn data_version(connection: &Connection) -> Result<i64, String> {
    connection
        .query_row("PRAGMA data_version", [], |row| row.get(0))
        .map_err(|error| error.to_string())
}

fn validate_source_paths(root: &Path) -> Result<(), String> {
    for relative in super::UPGRADE_ENTRIES {
        check_path(root, Path::new(relative))?;
    }
    for legacy in [
        "library.sqlite3.pre-restore",
        "assets.pre-restore",
        "documents.pre-restore",
    ] {
        if fs::symlink_metadata(root.join(legacy)).is_ok() {
            return Err(
                "A previous unjournaled recovery needs attention. Its files were preserved.".into(),
            );
        }
    }
    Ok(())
}

fn copy_managed_directory(source: &Path, destination: &Path, relative: &str) -> Result<(), String> {
    let source = source.join(relative);
    if source.exists() {
        copy_tree(&source, &destination.join(relative))
    } else {
        fs::create_dir(destination.join(relative)).map_err(io_error)
    }
}

/// Copies a directory tree, then proves the whole copy once against its
/// source. Each file is also checked as it is copied.
fn copy_tree(source: &Path, destination: &Path) -> Result<(), String> {
    copy_entries(source, destination)?;
    if hash_tree(source)? != hash_tree(destination)? {
        return Err("Checkpoint copy verification failed.".into());
    }
    Ok(())
}

// Hashing both trees at every level read each file once per folder above it,
// which made deep document folders slow to checkpoint; the copy is proved
// once, by `copy_tree`.
fn copy_entries(source: &Path, destination: &Path) -> Result<(), String> {
    if !fs::symlink_metadata(source)
        .map_err(io_error)?
        .file_type()
        .is_dir()
    {
        return Err("Checkpoint trees must be directories, not symbolic links.".into());
    }
    fs::create_dir(destination).map_err(io_error)?;
    for entry in fs::read_dir(source).map_err(io_error)? {
        let entry = entry.map_err(io_error)?;
        let kind = entry.file_type().map_err(io_error)?;
        if kind.is_dir() {
            copy_entries(&entry.path(), &destination.join(entry.file_name()))?;
        } else if kind.is_file() {
            copy_verified_file(&entry.path(), &destination.join(entry.file_name()))?;
        } else {
            return Err("Recovery checkpoints refuse symbolic links and special files.".into());
        }
    }
    sync(destination)
}

/// The bytes an update reads and writes, for its progress bar. A managed
/// byte is copied twice (checkpoint, then candidate) at five units a copy
/// (the write, both file hashes, both tree hashes) and hashed seven more
/// times (the manifest, two checkpoint checks, and the candidate and source
/// comparisons at two each): 17. The database is snapshotted, hashed for the
/// manifest and both checkpoint checks, copied once at five, migrated and
/// read twice to compare: 12. One more of each stands for opening and
/// checking the activated library. Advisory only: a plan that is too small
/// holds the bar just short of done until the update ends.
fn planned_upgrade_work(root: &Path, database: &Path) -> Result<u64, String> {
    let mut managed = 0_u64;
    for relative in MANAGED {
        let path = root.join(relative);
        if path.try_exists().map_err(io_error)? {
            managed = managed.saturating_add(tree_size(&path)?);
        }
    }
    let database = file_size(database)?;
    Ok(managed
        .saturating_mul(18)
        .saturating_add(database.saturating_mul(13)))
}

fn tree_size(path: &Path) -> Result<u64, String> {
    let mut total = 0_u64;
    for entry in fs::read_dir(path).map_err(io_error)? {
        let entry = entry.map_err(io_error)?;
        let kind = entry.file_type().map_err(io_error)?;
        if kind.is_dir() {
            total = total.saturating_add(tree_size(&entry.path())?);
        } else if kind.is_file() {
            total = total.saturating_add(entry.metadata().map_err(io_error)?.len());
        }
    }
    Ok(total)
}

fn file_size(path: &Path) -> Result<u64, String> {
    fs::metadata(path)
        .map(|metadata| metadata.len())
        .map_err(io_error)
}

fn copy_verified_file(source: &Path, destination: &Path) -> Result<(), String> {
    if !fs::symlink_metadata(source)
        .map_err(io_error)?
        .file_type()
        .is_file()
    {
        return Err("Recovery checkpoints copy only regular files.".into());
    }
    super::copy_and_sync(source, destination).map_err(|error| error.message)?;
    if hash_file(source)? != hash_file(destination)? {
        return Err("Checkpoint file verification failed.".into());
    }
    Ok(())
}

fn read_intent(paths: &AppPaths) -> Result<Option<Intent>, String> {
    if !has_intent(paths)? {
        return Ok(None);
    }
    let intent: Intent = read_json(&paths.app_data_dir().join(INTENT))?;
    if intent.version != 1 {
        return Err(
            "The library upgrade intent is unsupported. Preserve its recovery files.".into(),
        );
    }
    transaction_path(paths, &intent.operation_id)?;
    Ok(Some(intent))
}

fn write_intent(
    paths: &AppPaths,
    transaction: &Path,
    intent: &Intent,
    replace: bool,
) -> Result<(), String> {
    if has_intent(paths)? && !replace {
        return Err("A library update already needs recovery.".into());
    }
    // Retain an immutable copy before publishing/replacing the startup intent.
    write_json(&transaction.join("intent.json"), intent)?;
    write_json(&transaction.join("intent.pending.json"), intent)?;
    move_synced(
        &transaction.join("intent.pending.json"),
        &paths.app_data_dir().join(INTENT),
    )
}

fn archive_intent(paths: &AppPaths, intent: &Intent, name: &str) -> Result<(), String> {
    let transaction = transaction_path(paths, &intent.operation_id)?;
    check_path(&transaction, Path::new(name))?;
    let destination = transaction.join(name);
    if destination.exists() {
        if hash_file(&destination)? != hash_file(&paths.app_data_dir().join(INTENT))? {
            return Err("Conflicting library update intents were retained.".into());
        }
        crate::util::durable_fs::sync_file(&destination).map_err(io_error)?;
        sync(&transaction)?;
        fs::remove_file(paths.app_data_dir().join(INTENT)).map_err(io_error)?;
        sync(paths.app_data_dir())
    } else {
        move_synced(&paths.app_data_dir().join(INTENT), &destination)
    }
}

fn transaction_path(paths: &AppPaths, operation: &str) -> Result<PathBuf, String> {
    crate::util::validate_entity_id(operation, "upgrade operation")
        .map_err(|error| error.message)?;
    let relative = Path::new(TRANSACTIONS).join(operation);
    check_path(paths.app_data_dir(), &relative)?;
    Ok(paths.app_data_dir().join(relative))
}

fn operation_id(transaction: &Path) -> Result<String, String> {
    let value = transaction
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "Invalid checkpoint operation directory.".to_string())?;
    crate::util::validate_entity_id(value, "checkpoint operation")
        .map_err(|error| error.message)?;
    Ok(value.into())
}

fn ensure_no_journal(root: &Path) -> Result<(), String> {
    check_path(root, Path::new(JOURNAL))?;
    if root.join(JOURNAL).exists() {
        return Err("Recover the interrupted library activation before another operation.".into());
    }
    Ok(())
}

fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T, String> {
    let metadata = fs::symlink_metadata(path).map_err(io_error)?;
    if !metadata.file_type().is_file() || metadata.len() > MAX_METADATA_BYTES {
        return Err("Checkpoint metadata is not a bounded regular file.".into());
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(io_error)?
        .take(MAX_METADATA_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(io_error)?;
    if bytes.len() as u64 > MAX_METADATA_BYTES {
        return Err("Checkpoint metadata exceeded its safe size.".into());
    }
    serde_json::from_slice(&bytes)
        .map_err(|_| "Checkpoint metadata is invalid. Preserve recovery files.".into())
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let bytes = serde_json::to_vec(value).map_err(|error| error.to_string())?;
    write_new_file(path, &bytes).map_err(|error| error.message)
}

fn read_build(root: &Path, application: &str, identity: &(String, i64)) -> Option<BuildIdentity> {
    let record: BuildRecord = read_json(&root.join(BUILD)).ok()?;
    (record.version == 1
        && record.application == application
        && record.lineage == identity.0
        && record.schema == identity.1)
        .then_some(record.build)
}

fn write_build(root: &Path, application: &str, identity: &(String, i64)) -> Result<(), String> {
    check_path(root, Path::new(BUILD))?;
    let record = BuildRecord {
        version: 1,
        application: application.into(),
        lineage: identity.0.clone(),
        schema: identity.1,
        build: BuildIdentity::current(),
    };
    if read_json::<BuildRecord>(&root.join(BUILD)).ok().as_ref() == Some(&record) {
        return Ok(());
    }
    let temporary = root.join("library-build.pending.json");
    check_path(root, Path::new("library-build.pending.json"))?;
    // Unknown remnants are evidence; never overwrite them to hide interrupted IO.
    write_json(&temporary, &record)?;
    move_synced(&temporary, &root.join(BUILD))
}

#[cfg(test)]
#[path = "checkpoint_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "checkpoint_commit_tests.rs"]
mod commit_tests;
