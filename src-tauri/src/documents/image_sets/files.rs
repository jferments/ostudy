//! Managed image-set publication. New files precede one short SQL transaction;
//! a failed transaction removes only this attempt's unreferenced directory.

use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{database::Database, error::CommandError, paths::AppPaths};

use super::manifest::{invalid, ImageManifest};

const PENDING_FILE: &str = "image-set.pending";

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PendingSource {
    version: u32,
    document_id: String,
    operation_id: String,
}

pub(crate) struct InitialSource {
    directory: PathBuf,
    committed: bool,
}

impl InitialSource {
    pub(crate) fn begin(
        paths: &AppPaths,
        manifest: &ImageManifest,
        operation_id: &str,
    ) -> Result<Self, CommandError> {
        manifest.validate()?;
        crate::util::validate_entity_id(operation_id, "image import operation")?;
        let directory = paths.resolve_relative(&format!("documents/{}", manifest.document_id));
        fs::create_dir(&directory).map_err(CommandError::io)?;
        let staged = Self {
            directory,
            committed: false,
        };
        let marker = PendingSource {
            version: 1,
            document_id: manifest.document_id.clone(),
            operation_id: operation_id.to_string(),
        };
        let bytes = serde_json::to_vec(&marker).map_err(CommandError::serialization)?;
        write_new(&staged.directory.join(PENDING_FILE), &bytes)?;
        sync_directory(&staged.directory)?;
        if let Some(parent) = staged.directory.parent() {
            sync_directory(parent)?;
        }
        Ok(staged)
    }

    pub(crate) fn write(
        &self,
        paths: &AppPaths,
        relative: &str,
        bytes: &[u8],
    ) -> Result<(), CommandError> {
        let target = paths.resolve_relative(relative);
        if !target.starts_with(&self.directory)
            || Path::new(relative)
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(invalid(
                "An image-set file escaped its candidate directory.",
            ));
        }
        let parent = target
            .parent()
            .ok_or_else(|| invalid("Missing managed image directory."))?;
        fs::create_dir_all(parent).map_err(CommandError::io)?;
        write_new(&target, bytes)?;
        sync_directory(parent)
    }

    /// Call immediately after SQL commit. A marker-cleanup failure must never
    /// turn a committed import into a failed write or delete its original files.
    pub(crate) fn mark_committed(&mut self) {
        self.committed = true;
        if fs::remove_file(self.directory.join(PENDING_FILE)).is_ok() {
            let _ = sync_directory(&self.directory);
        }
    }
}

impl Drop for InitialSource {
    fn drop(&mut self) {
        if !self.committed {
            let _ = fs::remove_dir_all(&self.directory);
        }
    }
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), CommandError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(CommandError::io)?;
    file.write_all(bytes).map_err(CommandError::io)?;
    file.sync_all().map_err(CommandError::io)
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<(), CommandError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(CommandError::io)
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> Result<(), CommandError> {
    Ok(())
}

/// Startup only. Native pending markers prove this is our incomplete initial
/// publication. Unknown directories/markers are preserved, not guessed away.
/// Filesystem cleanup never runs while holding the database connection lock.
pub(crate) fn reconcile_pending(database: &Database, paths: &AppPaths) -> Result<(), CommandError> {
    let directory = paths.resolve_relative("documents");
    for entry in fs::read_dir(directory).map_err(CommandError::io)? {
        let entry = entry.map_err(CommandError::io)?;
        if !entry.file_type().map_err(CommandError::io)?.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if crate::util::validate_entity_id(&name, "image-set document").is_err() {
            continue;
        }
        let marker_path = entry.path().join(PENDING_FILE);
        if !fs::symlink_metadata(&marker_path).is_ok_and(|metadata| metadata.is_file()) {
            continue;
        }
        let Ok(file) = File::open(&marker_path) else {
            continue;
        };
        let mut bytes = Vec::new();
        if file.take(1025).read_to_end(&mut bytes).is_err() || bytes.len() > 1024 {
            continue;
        }
        let Ok(marker) = serde_json::from_slice::<PendingSource>(&bytes) else {
            continue;
        };
        if marker.version != 1
            || marker.document_id != name
            || crate::util::validate_entity_id(&marker.operation_id, "image import operation")
                .is_err()
        {
            continue;
        }
        let owned = database.with_connection(|connection| {
            connection
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM documents WHERE id = ?1)",
                    [&name],
                    |row| row.get::<_, bool>(0),
                )
                .map_err(CommandError::database)
        })?;
        if owned {
            fs::remove_file(marker_path).map_err(CommandError::io)?;
        } else {
            fs::remove_dir_all(entry.path()).map_err(CommandError::io)?;
        }
    }
    super::amend::files::reconcile(database, paths)
}

/// Missing originals are not reconstructed from OCR or a derivative. Mark the
/// owner explicitly failed; retained evidence and learner artifacts remain intact.
pub(crate) fn reconcile_missing_originals(
    database: &Database,
    paths: &AppPaths,
    now: i64,
) -> Result<(), CommandError> {
    let originals = database.with_connection(|conn| super::backup::originals(conn))?;
    let missing = originals
        .into_iter()
        .filter(|(_, _, _, path, _, _)| !paths.resolve_relative(path).is_file())
        .map(|(owner, _, _, _, _, _)| owner)
        .collect::<std::collections::BTreeSet<_>>();
    database.with_connection(|connection| {
        for document in missing {
            connection
                .execute(
                    "UPDATE documents SET lifecycle_state = 'failed',
                         safe_error_code = 'image_set_original_missing',
                         safe_error_message = 'An original image is missing. Restore a complete library backup; OCR text is not a replacement for the original.',
                         updated_at = ?1, revision = revision + 1
                     WHERE id = ?2 AND safe_error_code <> 'image_set_original_missing'",
                    rusqlite::params![now, document],
                )
                .map_err(CommandError::database)?;
        }
        Ok(())
    })
}
