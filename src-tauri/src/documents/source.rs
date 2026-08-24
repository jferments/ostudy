use std::path::PathBuf;

use crate::{
    database::Database,
    document_domain::DocumentFormat,
    error::CommandError,
    paths::AppPaths,
    util::validate_entity_id,
};

use super::repository;

pub(crate) fn resolve_document_source(
    database: &Database,
    paths: &AppPaths,
    document_id: &str,
) -> Result<PathBuf, CommandError> {
    validate_entity_id(document_id, "document")?;
    let stored =
        database.with_connection(|connection| repository::load_document(connection, document_id))?;
    let format = DocumentFormat::from_db(&stored.dto.source_format)?;
    let expected_relative = AppPaths::document_source_relative_path_with_extension(
        document_id,
        format.default_extension(),
    );
    if stored.source_relative_path != expected_relative {
        return Err(CommandError::new(
            "invalid_document_path",
            "The managed source path does not match its document identity.",
        ));
    }
    let source = paths.resolve_relative(&stored.source_relative_path);
    if !source.is_file() {
        return Err(CommandError::new(
            "document_source_missing",
            "The managed source document is missing. Restore the library from backup or import it again.",
        ));
    }
    Ok(source)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_resolution_rejects_a_path_outside_the_document_identity() {
        let database = Database::open_in_memory().expect("database should open");
        let root = std::env::temp_dir().join(format!(
            "ostudy-document-source-{}",
            std::process::id()
        ));
        let paths = AppPaths::new(root.clone()).expect("paths should initialize");
        let document_id = "a".repeat(32);
        database
            .with_connection(|connection| {
                connection
                    .execute(
                        "INSERT INTO documents (
                             id, title, original_filename, mime_type, source_media_type,
                             byte_size, sha256, source_relative_path, lifecycle_state,
                             source_format, source_unit_kind, created_at, updated_at
                         ) VALUES (?1, 'Source', 'source.pdf', 'application/pdf',
                                   'application/pdf', 1, ?2, 'documents/wrong/source.pdf',
                                   'failed', 'pdf', 'page', 1, 1)",
                        rusqlite::params![document_id, "b".repeat(64)],
                    )
                    .map_err(CommandError::database)?;
                Ok(())
            })
            .expect("fixture should initialize");
        let error = resolve_document_source(&database, &paths, &document_id)
            .expect_err("mismatched managed path must be rejected");
        assert_eq!(error.code, "invalid_document_path");
        let _ = std::fs::remove_dir_all(root);
    }
}
