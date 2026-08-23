use tauri::State;

use crate::{database::Database, error::CommandError};

use super::{
    contracts::{DocumentDto, SetDocumentArchivedInput},
    lifecycle, repository,
};

#[tauri::command]
pub fn list_documents(
    database: State<'_, Database>,
    include_archived: bool,
) -> Result<Vec<DocumentDto>, CommandError> {
    database.with_connection(|connection| {
        repository::list_document_rows(connection, include_archived)
    })
}

#[tauri::command]
pub fn set_document_archived(
    database: State<'_, Database>,
    input: SetDocumentArchivedInput,
) -> Result<DocumentDto, CommandError> {
    lifecycle::set_document_archived(database.inner(), input)
}
