//! Crash-recoverable managed-data staging for image amendments, not source code.
//! Cleanup removes only exact candidate files with no durable database owner.

use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::Path,
};

use serde::{Deserialize, Serialize};

use super::super::{backup, manifest};
use crate::{
    database::Database, document_canonical::sha256_hex, error::CommandError, paths::AppPaths, util,
};

const MAX_JOURNAL: usize = 64 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Journal {
    version: u32,
    document_id: String,
    files: Vec<(String, String)>,
}

pub(super) struct Batch<'a> {
    database: &'a Database,
    paths: &'a AppPaths,
    marker: String,
    journal: Journal,
}

impl<'a> Batch<'a> {
    pub(super) fn begin(
        database: &'a Database,
        paths: &'a AppPaths,
        document: &str,
    ) -> Result<Self, CommandError> {
        util::validate_entity_id(document, "image document")?;
        let attempt = database.with_connection(|connection| util::generate_id(connection))?;
        Ok(Self {
            database,
            paths,
            marker: format!("documents/{document}/amend-{attempt}.pending"),
            journal: Journal {
                version: 1,
                document_id: document.to_string(),
                files: Vec::new(),
            },
        })
    }

    pub(super) fn write(&mut self, relative: &str, bytes: &[u8]) -> Result<(), CommandError> {
        valid_path(&self.journal.document_id, relative)?;
        let target = self.paths.resolve_relative(relative);
        if target.exists() {
            if exact_file(&target, bytes.len(), &sha256_hex(bytes))? {
                return Ok(());
            }
            return Err(manifest::invalid(
                "An image candidate path already contains different bytes.",
            ));
        }
        self.journal
            .files
            .push((relative.to_string(), sha256_hex(bytes)));
        let journal = serde_json::to_vec(&self.journal).map_err(CommandError::serialization)?;
        if journal.len() > MAX_JOURNAL || self.journal.files.len() > 129 {
            return Err(manifest::invalid(
                "The image amendment staging journal is too large.",
            ));
        }
        let marker = self.paths.resolve_relative(&self.marker);
        let temporary = marker.with_extension("writing");
        write_new(&temporary, &journal)?;
        fs::rename(&temporary, &marker).map_err(CommandError::io)?;
        sync_parent(&marker)?;
        write_new(&target, bytes)?;
        sync_parent(&target)
    }

    pub(super) fn finish(self) {
        // Drop reconciles ownership after commit too. A concurrent winning
        // receipt must not cause this attempt's unreferenced additions to leak.
    }
}

impl Drop for Batch<'_> {
    fn drop(&mut self) {
        if cleanup(self.database, self.paths, &self.journal).is_ok() {
            let _ = fs::remove_file(self.paths.resolve_relative(&self.marker));
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
fn sync_parent(path: &Path) -> Result<(), CommandError> {
    File::open(
        path.parent()
            .ok_or_else(|| manifest::invalid("Missing image parent directory."))?,
    )
    .and_then(|directory| directory.sync_all())
    .map_err(CommandError::io)
}

#[cfg(not(unix))]
fn sync_parent(_path: &Path) -> Result<(), CommandError> {
    Ok(())
}

fn valid_path(document: &str, path: &str) -> Result<(), CommandError> {
    if backup::valid_original_path(document, path) || backup::valid_manifest_path(document, path) {
        Ok(())
    } else {
        Err(manifest::invalid(
            "An amendment file escaped its document owner.",
        ))
    }
}

fn exact_file(path: &Path, size: usize, hash: &str) -> Result<bool, CommandError> {
    let metadata = fs::symlink_metadata(path).map_err(CommandError::io)?;
    if !metadata.file_type().is_file() || metadata.len() != size as u64 {
        return Ok(false);
    }
    let mut data = Vec::new();
    File::open(path)
        .map_err(CommandError::io)?
        .take(size as u64 + 1)
        .read_to_end(&mut data)
        .map_err(CommandError::io)?;
    Ok(data.len() == size && sha256_hex(&data) == hash)
}

fn cleanup(database: &Database, paths: &AppPaths, journal: &Journal) -> Result<(), CommandError> {
    if journal.version != 1 || journal.files.len() > 129 {
        return Err(manifest::invalid("Unsupported image amendment journal."));
    }
    util::validate_entity_id(&journal.document_id, "image document")?;
    for (relative, hash) in &journal.files {
        valid_path(&journal.document_id, relative)?;
        manifest::validate_hash(hash)?;
    }
    let unowned = database.with_connection(|connection| {
        journal
            .files
            .iter()
            .map(|(path, hash)| {
                let owned: bool = connection
                    .query_row(
                        "SELECT EXISTS(SELECT 1 FROM documents WHERE source_relative_path = ?1)
                 OR EXISTS(SELECT 1 FROM document_image_members WHERE relative_path = ?1)
                 OR EXISTS(SELECT 1 FROM document_source_files WHERE relative_path = ?1)",
                        [path],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                Ok((path.clone(), hash.clone(), owned))
            })
            .collect::<Result<Vec<_>, CommandError>>()
    })?;
    for (path, hash, owned) in unowned {
        if owned {
            continue;
        }
        let path = paths.resolve_relative(&path);
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(CommandError::io(error)),
        };
        if metadata.len() > 25 * 1024 * 1024 || !exact_file(&path, metadata.len() as usize, &hash)?
        {
            return Err(manifest::invalid(
                "An unowned amendment file was altered; it was preserved.",
            ));
        }
        fs::remove_file(&path).map_err(CommandError::io)?;
        sync_parent(&path)?;
    }
    Ok(())
}

/// Called only during startup before commands become available. Unknown markers
/// remain untouched; a failed cleanup is retained for a later explicit diagnosis.
pub(crate) fn reconcile(database: &Database, paths: &AppPaths) -> Result<(), CommandError> {
    for entry in fs::read_dir(paths.resolve_relative("documents")).map_err(CommandError::io)? {
        let entry = entry.map_err(CommandError::io)?;
        if !entry.file_type().map_err(CommandError::io)?.is_dir() {
            continue;
        }
        let id = entry.file_name().to_string_lossy().into_owned();
        if util::validate_entity_id(&id, "image document").is_err() {
            continue;
        }
        for file in fs::read_dir(entry.path()).map_err(CommandError::io)? {
            let file = file.map_err(CommandError::io)?;
            let name = file.file_name().to_string_lossy().into_owned();
            let Some(attempt) = name
                .strip_prefix("amend-")
                .and_then(|name| name.strip_suffix(".pending"))
            else {
                continue;
            };
            if util::validate_entity_id(attempt, "amendment attempt").is_err()
                || !file.file_type().map_err(CommandError::io)?.is_file()
            {
                continue;
            }
            let mut bytes = Vec::new();
            File::open(file.path())
                .map_err(CommandError::io)?
                .take(MAX_JOURNAL as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(CommandError::io)?;
            if bytes.len() > MAX_JOURNAL {
                continue;
            }
            let Ok(journal) = serde_json::from_slice::<Journal>(&bytes) else {
                continue;
            };
            if journal.document_id != id {
                continue;
            }
            cleanup(database, paths, &journal)?;
            fs::remove_file(file.path()).map_err(CommandError::io)?;
        }
    }
    Ok(())
}
