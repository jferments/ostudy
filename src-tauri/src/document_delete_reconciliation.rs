//! Crash reconciliation for permanent managed-PDF deletion.
//!
//! `delete_pdf_document` first moves `documents/<id>` under `documents/trash/`
//! and then commits the SQLite delete. A process exit between those boundaries
//! must restore the directory when the durable document row still exists. If
//! the row is gone, the staged directory is unreachable cleanup residue and the
//! ordinary trash cleanup can remove it afterward.

use std::fs;

use rusqlite::OptionalExtension;

use crate::{database::Database, error::CommandError, paths::AppPaths};

pub(crate) fn restore_interrupted_deletions(
    database: &Database,
    paths: &AppPaths,
) -> Result<(), CommandError> {
    let trash_root = paths.app_data_dir().join("documents").join("trash");
    if !trash_root.is_dir() {
        return Ok(());
    }

    for entry in fs::read_dir(&trash_root).map_err(CommandError::io)? {
        let entry = entry.map_err(CommandError::io)?;
        let staged = entry.path();
        if !staged.is_dir() {
            continue;
        }
        let Some(document_id) = staged_document_id(&entry.file_name().to_string_lossy()) else {
            continue;
        };
        let durable_source: Option<String> = database.with_connection(|connection| {
            connection
                .query_row(
                    "SELECT source_relative_path FROM documents WHERE id = ?1",
                    [document_id.as_str()],
                    |row| row.get(0),
                )
                .optional()
                .map_err(CommandError::database)
        })?;
        let Some(relative_source) = durable_source else {
            continue;
        };
        if relative_source != AppPaths::document_source_relative_path(&document_id) {
            return Err(CommandError::new(
                "invalid_document_path",
                "A document pending deletion has an invalid managed source path.",
            ));
        }

        let destination_source = paths.resolve_relative(&relative_source);
        let destination_directory = destination_source.parent().ok_or_else(|| {
            CommandError::new(
                "invalid_document_path",
                "A document pending deletion has an invalid managed directory.",
            )
        })?;
        if destination_directory.exists() {
            // The durable source already exists; leave the duplicate staging
            // directory for the ordinary committed-trash cleanup below startup.
            continue;
        }
        fs::rename(&staged, destination_directory).map_err(CommandError::io)?;
        sync_directory(
            destination_directory
                .parent()
                .unwrap_or_else(|| paths.app_data_dir()),
        )?;
    }
    Ok(())
}

fn staged_document_id(name: &str) -> Option<String> {
    let document_id = name.split_once('-')?.0;
    (document_id.len() == 32
        && document_id
            .chars()
            .all(|character| character.is_ascii_hexdigit()))
    .then(|| document_id.to_ascii_lowercase())
}

#[cfg(unix)]
fn sync_directory(path: &std::path::Path) -> Result<(), CommandError> {
    std::fs::File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(CommandError::io)
}

#[cfg(not(unix))]
fn sync_directory(_path: &std::path::Path) -> Result<(), CommandError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::unix_time_millis;

    const DOCUMENT_ID: &str = "0123456789abcdef0123456789abcdef";

    fn temporary_paths(label: &str) -> AppPaths {
        let root = std::env::temp_dir().join(format!(
            "ostudy-document-delete-{label}-{}-{}",
            std::process::id(),
            unix_time_millis().expect("clock should be available")
        ));
        AppPaths::new(root).expect("temporary paths should initialize")
    }

    #[test]
    fn parses_only_managed_delete_staging_names() {
        assert_eq!(
            staged_document_id(&format!("{DOCUMENT_ID}-123.delete")),
            Some(DOCUMENT_ID.to_string())
        );
        assert_eq!(staged_document_id("not-a-document.delete"), None);
        assert_eq!(staged_document_id("0123-123.delete"), None);
    }

    #[test]
    fn restores_a_staged_directory_when_the_document_row_survived() {
        let paths = temporary_paths("restore");
        let database = Database::open_in_memory().expect("database should open");
        let source_relative = AppPaths::document_source_relative_path(DOCUMENT_ID);
        let now = unix_time_millis().expect("clock should be available");
        database
            .with_connection(|connection| {
                connection
                    .execute(
                        "INSERT INTO documents (
                             id, title, original_filename, mime_type, byte_size, sha256,
                             source_relative_path, lifecycle_state, created_at, updated_at, revision
                         ) VALUES (?1, 'Chapter', 'chapter.pdf', 'application/pdf', 5, ?2,
                                   ?3, 'failed', ?4, ?4, 1)",
                        rusqlite::params![DOCUMENT_ID, "0".repeat(64), source_relative, now],
                    )
                    .map_err(CommandError::database)?;
                Ok(())
            })
            .expect("document row should insert");

        let trash_root = paths.app_data_dir().join("documents").join("trash");
        let staged = trash_root.join(format!("{DOCUMENT_ID}-{now}.delete"));
        fs::create_dir_all(&staged).expect("staging directory should exist");
        fs::write(staged.join("source.pdf"), b"%PDF-").expect("source should stage");

        restore_interrupted_deletions(&database, &paths).expect("staging should reconcile");
        assert!(paths.resolve_relative(&source_relative).is_file());
        assert!(!staged.exists());

        fs::remove_dir_all(paths.app_data_dir()).expect("temporary root should clean up");
    }
}
