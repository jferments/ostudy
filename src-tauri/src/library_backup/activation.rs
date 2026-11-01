//! Recoverable library replacement. The journal is durable BEFORE the first
//! rename. A lost process is recovered from disk; Drop is only a convenience.
//! Callers must close every database/file user before installing or recovering.

use super::{
    copy_and_sync, extract_backup_package, pending_restore_path, sync_directory, write_new_file,
    AppPaths, Database, File, Path, PathBuf,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    sync::atomic::{AtomicU64, Ordering},
};

#[path = "checkpoint.rs"]
pub(crate) mod checkpoint;
#[path = "meter.rs"]
pub(crate) mod meter;

const JOURNAL: &str = "library-restore-journal.json";
const TRANSACTIONS: &str = "restore-transactions";
// Paths come from code, NEVER from a package or a writable journal.
const ENTRIES: &[&str] = &[
    "library.sqlite3",
    "library.sqlite3-wal",
    "library.sqlite3-shm",
    "library.sqlite3-journal",
    "assets",
    "documents",
    "cache/semantic-v1.sqlite3",
    "cache/semantic-v1.sqlite3-wal",
    "cache/semantic-v1.sqlite3-shm",
    "cache/semantic-v1.sqlite3-journal",
];
const INSTALL: &[&str] = &["library.sqlite3", "assets", "documents"];
// Version 1's path ordering is immutable recovery input. Version 2 covers
// every library-owned tree which startup reconciliation can mutate, including
// transfer staging and the semantic cache; paths never come from disk metadata.
const UPGRADE_ENTRIES: &[&str] = &[
    "library.sqlite3",
    "library.sqlite3-wal",
    "library.sqlite3-shm",
    "library.sqlite3-journal",
    "assets",
    "documents",
    "transfers",
    "cache",
    "library-build.json",
];
const UPGRADE_INSTALL: &[&str] = &[
    "library.sqlite3",
    "assets",
    "documents",
    "transfers",
    "cache",
    "library-build.json",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Layout {
    Package,
    Checkpoint,
}

impl Layout {
    fn from_version(version: u32) -> Result<Self, String> {
        match version {
            1 => Ok(Self::Package),
            2 => Ok(Self::Checkpoint),
            _ => Err(
                "The library activation journal is unsupported. Recovery files were preserved."
                    .into(),
            ),
        }
    }

    fn version(self) -> u32 {
        match self {
            Self::Package => 1,
            Self::Checkpoint => 2,
        }
    }

    fn entries(self) -> &'static [&'static str] {
        match self {
            Self::Package => ENTRIES,
            Self::Checkpoint => UPGRADE_ENTRIES,
        }
    }

    fn install(self) -> &'static [&'static str] {
        match self {
            Self::Package => INSTALL,
            Self::Checkpoint => UPGRADE_INSTALL,
        }
    }

    fn source(self) -> &'static str {
        match self {
            Self::Package => "source.ostudybackup",
            Self::Checkpoint => "checkpoint.json",
        }
    }
}

const COMMITTED: &[u8] = b"ostudy-restore-committed-v1\n";
static SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Journal {
    version: u32,
    operation_id: String,
    original: Vec<bool>,
    source_sha256: String,
}

#[derive(Debug)]
pub(crate) struct PendingRestoreGuard {
    root: PathBuf,
    journal: Journal,
    installed: bool,
    finished: bool,
}

impl PendingRestoreGuard {
    pub(super) fn transaction(&self) -> PathBuf {
        self.root
            .join(TRANSACTIONS)
            .join(&self.journal.operation_id)
    }

    pub(super) fn operation_id(&self) -> &str {
        &self.journal.operation_id
    }

    pub(super) fn commit_decided(&self) -> bool {
        self.finished
    }

    fn install(&mut self) -> Result<(), String> {
        self.cutpoint("journal");
        let transaction = self.transaction();
        let layout = Layout::from_version(self.journal.version)?;
        for (index, relative) in layout.entries().iter().enumerate() {
            if self.journal.original[index] {
                move_synced(
                    &self.root.join(relative),
                    &transaction.join("previous").join(relative),
                )?;
            }
            self.cutpoint(&format!("previous-{index}"));
        }
        for (index, relative) in layout.install().iter().enumerate() {
            // Historic libraries have no build receipt. Its absence is an exact
            // state to restore, not a reason to synthesize a historic version.
            if layout == Layout::Checkpoint
                && *relative == "library-build.json"
                && !transaction.join("candidate").join(relative).exists()
            {
                continue;
            }
            move_synced(
                &transaction.join("candidate").join(relative),
                &self.root.join(relative),
            )?;
            self.cutpoint(&format!("install-{index}"));
        }
        self.installed = true;
        Ok(())
    }

    fn cutpoint(&self, _phase: &str) {
        #[cfg(test)]
        if std::env::var_os("OSTUDY_RESTORE_CRASH_ROOT").as_deref() == Some(self.root.as_os_str())
            && std::env::var("OSTUDY_RESTORE_CRASH_PHASE").as_deref() == Ok(_phase)
        {
            // Abrupt process exit deliberately bypasses all Rust destructors.
            std::process::exit(77);
        }
    }

    /// The caller has reopened and reconciled the replacement before this call.
    /// A durable commit marker is the decision; cleanup cannot undo that decision.
    pub(crate) fn try_commit(&mut self) -> Result<(), String> {
        if !self.installed {
            return Err("The restored library has not been completely installed.".into());
        }
        let transaction = self.transaction();
        let temporary = transaction.join("commit.tmp");
        write_new_file(&temporary, COMMITTED).map_err(|error| error.message)?;
        fs::rename(&temporary, transaction.join("committed")).map_err(io_error)?;
        // Once the marker has been renamed, never roll back in Drop: a failed
        // fsync is indeterminate, and the next launch must honor the disk record.
        self.finished = true;
        sync(&transaction)?;
        self.cutpoint("committed");
        if let Err(error) = self.finish() {
            crate::logging::warn_target(
                "library.restore",
                "restore_cleanup_pending",
                "The library was restored; its recovery receipt needs cleanup.",
                serde_json::json!({"operationId": self.journal.operation_id, "error": error}),
            );
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn commit(mut self) {
        self.try_commit().expect("restore commit must be durable");
    }

    pub(crate) fn rollback(&mut self) -> Result<(), String> {
        let transaction = self.transaction();
        let layout = Layout::from_version(self.journal.version)?;
        for (index, relative) in layout.entries().iter().enumerate().rev() {
            let active = self.root.join(relative);
            let previous = transaction.join("previous").join(relative);
            check_path(&self.root, Path::new(relative))?;
            check_path(&transaction, &Path::new("previous").join(relative))?;
            if self.journal.original[index] {
                if previous.exists() {
                    self.retain_failed_entry(relative)?;
                    move_synced(&previous, &active)?;
                } else if !active.exists() {
                    return Err(format!(
                        "Restore recovery cannot find the previous {relative}. All recovery files were preserved."
                    ));
                }
                // No previous path means either the old entry was never moved
                // or this entry has already been rolled back. Both are no-ops.
            } else {
                self.retain_failed_entry(relative)?;
            }
            self.cutpoint(&format!("rollback-{index}"));
        }
        // Keep the input backup and transaction evidence. Do not auto-repeat an
        // interrupted restore at every launch after restoring the prior library.
        self.finish()?;
        self.finished = true;
        Ok(())
    }

    fn retain_failed_entry(&self, relative: &str) -> Result<(), String> {
        let active = self.root.join(relative);
        if self.journal.version == 1 {
            return remove_entry(&active);
        }
        if !active.exists() {
            return Ok(());
        }
        let transaction = self.transaction();
        let retained = Path::new("failed").join(relative);
        check_path(&transaction, &retained)?;
        let retained = transaction.join(retained);
        if retained.exists() {
            return Err("Conflicting failed-library recovery files were preserved. No copy was overwritten.".into());
        }
        move_synced(&active, &retained)
    }

    fn finish(&self) -> Result<(), String> {
        let pending = self.root.join("pending-library-restore.ostudybackup");
        if pending.is_file() && hash_file(&pending)? == self.journal.source_sha256 {
            let retained = self.transaction().join("pending.ostudybackup");
            check_path(&self.transaction(), Path::new("pending.ostudybackup"))?;
            if retained.exists() {
                if hash_file(&retained)? != self.journal.source_sha256 {
                    return Err(
                        "A different retained pending backup exists. Both copies were preserved."
                            .into(),
                    );
                }
                // The retained, synced identical copy makes retirement retry-safe.
                // Never remove a different backup merely to finish bookkeeping.
                crate::util::durable_fs::sync_file(&retained).map_err(io_error)?;
                sync(&self.transaction())?;
                super::remove_file_if_exists(&pending).map_err(io_error)?;
                sync(&self.root)?;
            } else {
                move_synced(&pending, &retained)?;
            }
        }
        super::remove_file_if_exists(&self.root.join(JOURNAL)).map_err(io_error)?;
        sync(&self.root)
    }
}

impl Drop for PendingRestoreGuard {
    fn drop(&mut self) {
        if !self.finished {
            if let Err(error) = self.rollback() {
                crate::logging::error_target(
                    "library.restore", "restore_recovery_required",
                    "Restore rollback was interrupted. The durable journal and previous files were preserved.",
                    serde_json::json!({"operationId": self.journal.operation_id, "error": error}),
                );
            }
        }
    }
}

/// Returns true when a prior operation was recovered/finalized. The caller must
/// then open the resulting library, not start another restore in the same pass.
pub(crate) fn recover(paths: &AppPaths) -> Result<bool, String> {
    let root = paths.app_data_dir();
    check_path(root, Path::new(JOURNAL))?;
    if !root.join(JOURNAL).exists() {
        return Ok(false);
    }
    let bytes = fs::read(root.join(JOURNAL)).map_err(io_error)?;
    if bytes.len() > 4096 {
        return Err("The restore journal is invalid. Recovery files were preserved.".into());
    }
    let journal: Journal = serde_json::from_slice(&bytes).map_err(|_| {
        "The restore journal is unreadable. Recovery files were preserved.".to_string()
    })?;
    crate::util::validate_entity_id(&journal.operation_id, "restore operation")
        .map_err(|error| error.message)?;
    let layout = Layout::from_version(journal.version)?;
    if journal.original.len() != layout.entries().len()
        || journal.source_sha256.len() != 64
        || !journal
            .source_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err("The restore journal is unsupported. Recovery files were preserved.".into());
    }
    check_path(root, &Path::new(TRANSACTIONS).join(&journal.operation_id))?;
    let mut guard = PendingRestoreGuard {
        root: root.to_path_buf(),
        journal,
        installed: true,
        finished: true,
    };
    let marker = guard.transaction().join("committed");
    check_path(&guard.transaction(), Path::new("committed"))?;
    if marker.exists() {
        if fs::read(&marker).map_err(io_error)? != COMMITTED {
            return Err(
                "The restore commit record is invalid. Recovery files were preserved.".into(),
            );
        }
        guard.finish()?;
    } else {
        guard.rollback()?;
    }
    Ok(true)
}

pub(crate) fn apply_pending_restore(
    paths: &AppPaths,
) -> Result<Option<PendingRestoreGuard>, String> {
    check_path(
        paths.app_data_dir(),
        Path::new("pending-library-restore.ostudybackup"),
    )?;
    if checkpoint::has_intent(paths)? {
        // Resolve any activation decision first. The upgrade owner then handles
        // its durable intent; never apply an unrelated pending package over it.
        recover(paths)?;
        return Ok(None);
    }
    apply_source_restore(paths, &pending_restore_path(paths))
}

/// Process-owned, validated input. No journal exists until exclusive activation.
/// Dropping this value preserves its source/candidate for inspection; it never
/// installs or replays an abandoned preparation at startup.
#[derive(Debug)]
pub(super) struct PreparedRestore {
    layout: Layout,
    root: PathBuf,
    transaction: PathBuf,
    source_sha256: String,
    candidate_sha256: String,
}

/// Startup already owns a closed library. Live restore uses preparation and
/// installation separately so extraction/validation does not close IPC admission.
pub(super) fn apply_source_restore(
    paths: &AppPaths,
    pending: &Path,
) -> Result<Option<PendingRestoreGuard>, String> {
    if recover(paths)? || !pending.is_file() {
        return Ok(None);
    }
    let prepared = prepare_source_restore(paths, pending)?;
    install_prepared_restore(paths, prepared).map(Some)
}

/// Copy, extract, migrate and validate only private candidate files. The active
/// library may remain open. Caller owns maintenance and an original-generation
/// request lease, not exclusive replacement admission.
pub(super) fn prepare_source_restore(
    paths: &AppPaths,
    pending: &Path,
) -> Result<PreparedRestore, String> {
    let root = paths.app_data_dir();
    check_path(root, Path::new(JOURNAL))?;
    if root.join(JOURNAL).exists() {
        return Err("Recover the interrupted restore before preparing another library.".into());
    }
    // These belong to the pre-journal implementation. Never delete an unknown
    // interrupted operation's only rollback copy to make room for a new one.
    for legacy in [
        "library.sqlite3.pre-restore",
        "assets.pre-restore",
        "documents.pre-restore",
    ] {
        if fs::symlink_metadata(root.join(legacy)).is_ok() {
            return Err(format!(
                "An older interrupted restore left {legacy}. It was preserved; inspect the recovery files before retrying."
            ));
        }
    }
    let transaction = new_transaction(root)?;
    let source = transaction.join("source.ostudybackup");
    copy_and_sync(pending, &source).map_err(|error| error.message)?;
    let candidate = transaction.join("candidate");
    fs::create_dir(&candidate).map_err(io_error)?;
    extract_backup_package(
        &source,
        &candidate.join("library.sqlite3"),
        &candidate.join("assets"),
    )
    .map_err(|error| error.message)?;
    // Upgrade and validate the isolated candidate, never the active library.
    let staged = Database::open(&candidate.join("library.sqlite3"))?;
    staged.with_connection(|connection| {
        connection.execute_batch(
            "UPDATE knowledge_preparations SET state = 'interrupted',
                 safe_error_code = 'semantic_index_not_restored',
                 safe_error_message = 'The library was restored without its device-local vector cache. Prepare the selected material to rebuild it.'
             WHERE state IN ('complete', 'running');
             PRAGMA wal_checkpoint(TRUNCATE);
             PRAGMA journal_mode = DELETE;"
        ).map_err(crate::error::CommandError::database)
    }).map_err(|error| error.message)?;
    drop(staged);
    sync_tree(&candidate)?;
    Ok(PreparedRestore {
        layout: Layout::Package,
        root: root.to_path_buf(),
        source_sha256: hash_file(&source)?,
        candidate_sha256: hash_tree(&candidate)?,
        transaction,
    })
}

/// Called only after the original-generation lease has been released and all
/// other library users have drained. Verify the exact prepared bytes BEFORE
/// writing a journal or moving any active file; never re-resolve the old handle.
pub(super) fn install_prepared_restore(
    paths: &AppPaths,
    prepared: PreparedRestore,
) -> Result<PendingRestoreGuard, String> {
    let root = paths.app_data_dir();
    if prepared.root != root {
        return Err("A restore candidate belongs to a different library.".into());
    }
    check_path(root, Path::new(JOURNAL))?;
    if root.join(JOURNAL).exists() {
        return Err("An interrupted restore must be recovered before activation.".into());
    }
    let layout = prepared.layout;
    let transaction = prepared.transaction;
    let relative = transaction
        .strip_prefix(root)
        .map_err(|_| "Restore candidate escaped its library directory.".to_string())?;
    check_path(root, relative)?;
    check_path(&transaction, Path::new(layout.source()))?;
    check_path(&transaction, Path::new("candidate"))?;
    if hash_file(&transaction.join(layout.source()))? != prepared.source_sha256
        || hash_tree(&transaction.join("candidate"))? != prepared.candidate_sha256
    {
        return Err("The prepared restore candidate changed. The active library was not replaced; recovery files were preserved.".into());
    }
    if layout == Layout::Checkpoint {
        checkpoint::verify_checkpoint(&transaction)?;
    }
    let original = layout
        .entries()
        .iter()
        .map(|relative| {
            check_path(root, Path::new(relative))?;
            Ok(root.join(relative).exists())
        })
        .collect::<Result<Vec<_>, String>>()?;
    if !original[0] && original[1..4].iter().any(|exists| *exists) {
        return Err("The current database is missing but its journal exists. Both the backup and journal were preserved.".into());
    }
    let journal = Journal {
        version: layout.version(),
        operation_id: transaction
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| "Invalid restore transaction directory.".to_string())?
            .to_string(),
        original,
        source_sha256: prepared.source_sha256,
    };
    let previous = transaction.join("previous");
    fs::create_dir(&previous).map_err(io_error)?;
    if layout == Layout::Package {
        fs::create_dir(previous.join("cache")).map_err(io_error)?;
    }
    sync(&previous)?;
    sync(&transaction)?;
    let bytes = serde_json::to_vec(&journal).map_err(|error| error.to_string())?;
    let temporary = transaction.join("journal.json");
    write_new_file(&temporary, &bytes).map_err(|error| error.message)?;
    move_synced(&temporary, &root.join(JOURNAL))?;
    let mut guard = PendingRestoreGuard {
        root: root.to_path_buf(),
        journal,
        installed: false,
        finished: false,
    };
    guard.install()?;
    Ok(guard)
}

pub(super) fn transaction_committed(root: &Path, operation: &str) -> Result<bool, String> {
    crate::util::validate_entity_id(operation, "restore transaction")
        .map_err(|error| error.message)?;
    let relative = Path::new(TRANSACTIONS).join(operation).join("committed");
    check_path(root, &relative)?;
    let marker = root.join(relative);
    if !marker.exists() {
        return Ok(false);
    }
    if fs::read(marker).map_err(io_error)? != COMMITTED {
        return Err("The restore commit record is invalid. Recovery files were preserved.".into());
    }
    Ok(true)
}

/// An explicitly requested new restore supersedes, but never destroys, an old
/// prepared package. Moving it out of the startup slot prevents a later launch
/// from silently applying a superseded restore over newly created learner data.
pub(super) fn preserve_pending(paths: &AppPaths) -> Result<(), String> {
    let root = paths.app_data_dir();
    check_path(root, Path::new("pending-library-restore.ostudybackup"))?;
    let pending = pending_restore_path(paths);
    if !pending.is_file() {
        return Ok(());
    }
    let hash = hash_file(&pending)?;
    let relative = Path::new("restore-inputs").join(format!("{hash}.ostudybackup"));
    check_path(root, &relative)?;
    let retained = root.join(relative);
    if retained.exists() {
        if hash_file(&retained)? != hash {
            return Err(
                "The retained pending backup does not match its hash. Both copies were preserved."
                    .into(),
            );
        }
        crate::util::durable_fs::sync_file(&retained).map_err(io_error)?;
        sync(retained.parent().unwrap())?;
        super::remove_file_if_exists(&pending).map_err(io_error)?;
        sync(root)
    } else {
        move_synced(&pending, &retained)
    }
}

fn new_transaction(root: &Path) -> Result<PathBuf, String> {
    check_path(root, Path::new(TRANSACTIONS))?;
    let parent = root.join(TRANSACTIONS);
    fs::create_dir_all(&parent).map_err(io_error)?;
    sync(root)?;
    let now = crate::util::unix_time_millis().map_err(|error| error.message)?;
    let sequence = u32::try_from(SEQUENCE.fetch_add(1, Ordering::Relaxed))
        .map_err(|_| "Restore operation sequence exhausted.".to_string())?;
    let transaction = parent.join(format!(
        "{:016x}{:08x}{:08x}",
        now,
        std::process::id(),
        sequence
    ));
    let builder = fs::DirBuilder::new();
    #[cfg(unix)]
    let mut builder = builder;
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(&transaction).map_err(io_error)?;
    sync(&parent)?;
    Ok(transaction)
}

pub(super) fn hash_file(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(io_error)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(io_error)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
        meter::advance(count as u64);
    }
    Ok(format!("{:x}", digest.finalize()))
}

pub(super) fn check_path(root: &Path, relative: &Path) -> Result<(), String> {
    let mut path = root.to_path_buf();
    for component in relative.components() {
        if !matches!(component, std::path::Component::Normal(_)) {
            return Err("Restore paths must stay inside the application directory.".into());
        }
        path.push(component);
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(
                    "Restore refuses symbolic links in managed library/recovery paths.".into(),
                );
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(io_error(error)),
        }
    }
    Ok(())
}

fn move_synced(source: &Path, destination: &Path) -> Result<(), String> {
    let parent = destination
        .parent()
        .ok_or_else(|| "Invalid restore destination.".to_string())?;
    fs::create_dir_all(parent).map_err(io_error)?;
    fs::rename(source, destination).map_err(io_error)?;
    sync(parent)?;
    sync(
        source
            .parent()
            .ok_or_else(|| "Invalid restore source.".to_string())?,
    )
}

fn remove_entry(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => fs::remove_dir_all(path).map_err(io_error)?,
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err("Restore refuses to remove a symbolic link.".into());
        }
        Ok(_) => fs::remove_file(path).map_err(io_error)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(io_error(error)),
    }
    sync(
        path.parent()
            .ok_or_else(|| "Invalid restore path.".to_string())?,
    )
}

fn sync(path: &Path) -> Result<(), String> {
    sync_directory(path).map_err(|error| error.message)
}

fn sync_tree(path: &Path) -> Result<(), String> {
    for entry in fs::read_dir(path).map_err(io_error)? {
        let entry = entry.map_err(io_error)?;
        let kind = entry.file_type().map_err(io_error)?;
        if kind.is_symlink() {
            return Err("Restore candidate contains a symbolic link.".into());
        }
        if kind.is_dir() {
            sync_tree(&entry.path())?;
        } else {
            crate::util::durable_fs::sync_file(&entry.path()).map_err(io_error)?;
        }
    }
    sync(path)
}

/// Length-framed names, entry types and file digests bind every candidate
/// file and empty directory. Sorting makes enumeration order irrelevant.
fn hash_tree(path: &Path) -> Result<String, String> {
    let mut entries = fs::read_dir(path)
        .map_err(io_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(io_error)?;
    entries.sort_by_key(|entry| entry.file_name());
    let mut digest = Sha256::new();
    for entry in entries {
        let name = entry.file_name();
        let name = name
            .to_str()
            .ok_or_else(|| "A restore candidate filename is not UTF-8.".to_string())?;
        digest.update((name.len() as u64).to_be_bytes());
        digest.update(name.as_bytes());
        let kind = entry.file_type().map_err(io_error)?;
        let content = if kind.is_dir() {
            digest.update(b"directory");
            hash_tree(&entry.path())?
        } else if kind.is_file() {
            digest.update(b"file");
            hash_file(&entry.path())?
        } else {
            return Err(
                "Restore candidates must contain only regular files and directories.".into(),
            );
        };
        digest.update(content.as_bytes());
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn io_error(error: std::io::Error) -> String {
    format!("Library restore filesystem operation failed: {error}. Recovery files were preserved.")
}

#[cfg(test)]
#[path = "activation_tests.rs"]
mod current_format_tests;

#[cfg(test)]
#[path = "preparation_tests.rs"]
mod preparation_tests;
