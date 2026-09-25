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
        for (index, relative) in ENTRIES.iter().enumerate() {
            if self.journal.original[index] {
                move_synced(
                    &self.root.join(relative),
                    &transaction.join("previous").join(relative),
                )?;
            }
            self.cutpoint(&format!("previous-{index}"));
        }
        for (index, relative) in INSTALL.iter().enumerate() {
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
        for (index, relative) in ENTRIES.iter().enumerate().rev() {
            let active = self.root.join(relative);
            let previous = transaction.join("previous").join(relative);
            check_path(&self.root, Path::new(relative))?;
            check_path(&transaction, &Path::new("previous").join(relative))?;
            if self.journal.original[index] {
                if previous.exists() {
                    remove_entry(&active)?;
                    move_synced(&previous, &active)?;
                } else if !active.exists() {
                    return Err(format!(
                        "Restore recovery cannot find the previous {relative}. All recovery files were preserved."
                    ));
                }
                // No previous path means either the old entry was never moved
                // or this entry has already been rolled back. Both are no-ops.
            } else {
                remove_entry(&active)?;
            }
            self.cutpoint(&format!("rollback-{index}"));
        }
        // Keep the input backup and transaction evidence. Do not auto-repeat an
        // interrupted restore at every launch after restoring the prior library.
        self.finish()?;
        self.finished = true;
        Ok(())
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
                File::open(&retained)
                    .and_then(|file| file.sync_all())
                    .map_err(io_error)?;
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
    if journal.version != 1
        || journal.original.len() != ENTRIES.len()
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
    apply_source_restore(paths, &pending_restore_path(paths))
}

/// The selected source is a native-owned staged file, never a caller path.
pub(super) fn apply_source_restore(
    paths: &AppPaths,
    pending: &Path,
) -> Result<Option<PendingRestoreGuard>, String> {
    if recover(paths)? {
        return Ok(None);
    }
    let root = paths.app_data_dir();
    if !pending.is_file() {
        return Ok(None);
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
    copy_and_sync(&pending, &source).map_err(|error| error.message)?;
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
    let original = ENTRIES
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
        version: 1,
        operation_id: transaction
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| "Invalid restore transaction directory.".to_string())?
            .to_string(),
        original,
        source_sha256: hash_file(&source)?,
    };
    let previous = transaction.join("previous");
    fs::create_dir(&previous).map_err(io_error)?;
    fs::create_dir(previous.join("cache")).map_err(io_error)?;
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
    Ok(Some(guard))
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
        File::open(&retained)
            .and_then(|file| file.sync_all())
            .map_err(io_error)?;
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
    fs::create_dir(&transaction).map_err(io_error)?;
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
            File::open(entry.path())
                .and_then(|file| file.sync_all())
                .map_err(io_error)?;
        }
    }
    sync(path)
}

fn io_error(error: std::io::Error) -> String {
    format!("Library restore filesystem operation failed: {error}. Recovery files were preserved.")
}

#[cfg(test)]
#[path = "activation_tests.rs"]
mod current_format_tests;
