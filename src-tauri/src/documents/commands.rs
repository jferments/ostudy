use tauri::{AppHandle, Manager, State};

use crate::{
    database::Database, error::CommandError, paths::AppPaths, task_activity::ActivityGuard,
};

use super::{
    contracts::DocumentDto,
    image_sets::admission::{self, ImageSetInspection, InspectImageSetInput},
    repository,
};

#[tauri::command]
pub fn list_documents(database: State<'_, Database>) -> Result<Vec<DocumentDto>, CommandError> {
    database.with_connection(|connection| repository::list_document_rows(connection))
}

/// Only explicit selection invokes this command. CPU/IO work stays off Tauri's
/// event loop; the shared activity lease excludes a concurrent library restore.
#[tauri::command]
pub async fn inspect_image_set(
    app: AppHandle,
    input: InspectImageSetInput,
) -> Result<ImageSetInspection, CommandError> {
    admission::validate_selection(&input.staged_file_ids)?;
    let activity = ActivityGuard::begin()?;
    tauri::async_runtime::spawn_blocking(move || {
        let _activity = activity;
        admission::inspect(
            app.state::<Database>().inner(),
            app.state::<AppPaths>().inner(),
            &input.staged_file_ids,
        )
    })
    .await
    .map_err(|_| {
        CommandError::new(
            "image_set_inspection_interrupted",
            "Image inspection stopped. The selected files were not imported or removed.",
        )
    })?
}
