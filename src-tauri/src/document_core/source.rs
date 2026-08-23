use tauri::State;

use crate::{
    database::Database,
    error::CommandError,
    paths::AppPaths,
    util::validate_entity_id,
};

use super::repository;

#[tauri::command]
pub fn get_document_source_path(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    document_id: String,
) -> Result<String, CommandError> {
    validate_entity_id(&document_id, "document")?;
    let stored = database.with_connection(|connection| {
        repository::load_document(connection, &document_id)
    })?;
    let source = paths.resolve_relative(&stored.source_relative_path);
    if !source.is_file() {
        return Err(CommandError::new(
            "document_source_missing",
            "The managed source document is missing. Restore the library from backup or import it again.",
        ));
    }
    Ok(source.to_string_lossy().into_owned())
}
