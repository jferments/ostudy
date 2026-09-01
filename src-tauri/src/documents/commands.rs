use tauri::State;

use crate::{database::Database, error::CommandError};

use super::{contracts::DocumentDto, repository};

#[tauri::command]
pub fn list_documents(database: State<'_, Database>) -> Result<Vec<DocumentDto>, CommandError> {
    database.with_connection(repository::list_document_rows)
}
