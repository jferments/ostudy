use tauri::{AppHandle, Manager, State};

use crate::{
    database::Database, error::CommandError, paths::AppPaths, task_activity::ActivityGuard,
    task_cancellation::run_document_task,
};

use super::{
    contracts::DocumentDto,
    image_sets::{
        admission::{self, ImageSetInspection, InspectImageSetInput},
        filter_commands::{self, SetImageFiltersInput},
        import::{self as image_import, ImportImageSetInput, ImportedImageSet},
        operations::EditResult,
        prepare::{self, PrepareImageSetInput},
        view::{self, ImagePreview, ImagePreviewInput, ImageSetDetails, ImageSetInput},
    },
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

/// Retain one whole source before extraction. A retry uses the same operation
/// identity even when the original invocation's acknowledgement was interrupted.
#[tauri::command]
pub async fn import_image_set(
    app: AppHandle,
    input: ImportImageSetInput,
) -> Result<ImportedImageSet, CommandError> {
    input.validate()?;
    let activity = ActivityGuard::begin()?;
    tauri::async_runtime::spawn_blocking(move || {
        let _activity = activity;
        image_import::import(
            app.state::<Database>().inner(),
            app.state::<AppPaths>().inner(),
            &input,
        )
    })
    .await
    .map_err(|_| {
        CommandError::new(
            "image_set_import_interrupted",
            "The import acknowledgement was interrupted. Retry the same operation to recover its saved result.",
        )
    })?
}

#[tauri::command]
pub async fn get_image_set(
    app: AppHandle,
    input: ImageSetInput,
) -> Result<ImageSetDetails, CommandError> {
    let activity = ActivityGuard::begin()?;
    tauri::async_runtime::spawn_blocking(move || {
        let _activity = activity;
        app.state::<Database>()
            .with_connection(|connection| view::details(connection, &input.document_id))
    })
    .await
    .map_err(|_| interrupted("Source inspection stopped. Reload the saved image set."))?
}

#[tauri::command]
pub async fn get_image_set_preview(
    app: AppHandle,
    input: ImagePreviewInput,
) -> Result<ImagePreview, CommandError> {
    let activity = ActivityGuard::begin()?;
    tauri::async_runtime::spawn_blocking(move || {
        let _activity = activity;
        view::preview(
            app.state::<Database>().inner(),
            app.state::<AppPaths>().inner(),
            &input,
        )
    })
    .await
    .map_err(|_| {
        interrupted("Source image inspection stopped. No source pixels were changed.")
    })?
}

#[tauri::command]
pub async fn prepare_image_set(
    app: AppHandle,
    input: PrepareImageSetInput,
) -> Result<EditResult, CommandError> {
    crate::util::validate_entity_id(&input.document_id, "image document")?;
    let id = input.document_id.clone();
    run_document_task(&id, async move {
        tauri::async_runtime::spawn_blocking(move || {
            prepare::prepare(
                app.state::<Database>().inner(),
                app.state::<AppPaths>().inner(),
                &input,
            )
        })
        .await
        .map_err(|_| {
            interrupted("Local image preparation stopped. Saved originals and completed OCR remain available.")
        })?
    })
    .await
}

#[tauri::command]
pub async fn set_image_set_filters(
    app: AppHandle,
    input: SetImageFiltersInput,
) -> Result<EditResult, CommandError> {
    crate::util::validate_entity_id(&input.document_id, "image document")?;
    let id = input.document_id.clone();
    run_document_task(&id, async move {
        tauri::async_runtime::spawn_blocking(move || {
            filter_commands::apply(app.state::<Database>().inner(), &input)
        })
        .await
        .map_err(|_| {
            interrupted("The filter acknowledgement stopped. Retry with the same operation identity to recover the saved result.")
        })?
    })
    .await
}

fn interrupted(message: &str) -> CommandError {
    CommandError::new("image_set_interrupted", message)
}
