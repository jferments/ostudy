use tauri::AppHandle;
use tauri_plugin_dialog::{DialogExt, FilePath};
use tokio::sync::oneshot;

use crate::{
    error::CommandError,
    platform::{
        ExternalDestination, ExternalFilePort, ExternalFileRequest, ExternalSource, PlatformFuture,
    },
};

pub(super) struct DesktopExternalFilePort;

impl ExternalFilePort for DesktopExternalFilePort {
    fn select_import(
        &self,
        app: AppHandle,
        request: ExternalFileRequest,
    ) -> PlatformFuture<Result<Option<ExternalSource>, CommandError>> {
        Box::pin(async move {
            let mut dialog = app.dialog().file().set_title(&request.title);
            for filter in &request.filters {
                let extensions = filter
                    .extensions
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>();
                dialog = dialog.add_filter(&filter.name, &extensions);
            }

            let (sender, receiver) = oneshot::channel();
            dialog.pick_file(move |selection| {
                let _ = sender.send(selection);
            });
            let selection = receiver.await.map_err(|_| dialog_result_interrupted())?;
            selection.map(source_from_file_path).transpose()
        })
    }

    fn select_export(
        &self,
        app: AppHandle,
        request: ExternalFileRequest,
    ) -> PlatformFuture<Result<Option<ExternalDestination>, CommandError>> {
        Box::pin(async move {
            let mut dialog = app.dialog().file().set_title(&request.title);
            if let Some(filename) = &request.suggested_filename {
                dialog = dialog.set_file_name(filename);
            }
            for filter in &request.filters {
                let extensions = filter
                    .extensions
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>();
                dialog = dialog.add_filter(&filter.name, &extensions);
            }

            let (sender, receiver) = oneshot::channel();
            dialog.save_file(move |selection| {
                let _ = sender.send(selection);
            });
            let selection = receiver.await.map_err(|_| dialog_result_interrupted())?;
            selection.map(destination_from_file_path).transpose()
        })
    }
}

fn dialog_result_interrupted() -> CommandError {
    CommandError::new(
        "external_file_dialog_interrupted",
        "The system file dialog closed before ostudy received its result. Try again.",
    )
}

fn source_from_file_path(value: FilePath) -> Result<ExternalSource, CommandError> {
    let path = value.into_path().map_err(|_| {
        CommandError::new(
            "external_file_reference_unsupported",
            "The selected file could not be accessed by this desktop build.",
        )
    })?;
    let display_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("selected file")
        .to_string();
    Ok(ExternalSource { path, display_name })
}

fn destination_from_file_path(value: FilePath) -> Result<ExternalDestination, CommandError> {
    let path = value.into_path().map_err(|_| {
        CommandError::new(
            "external_destination_unsupported",
            "The selected destination could not be accessed by this desktop build.",
        )
    })?;
    let display_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("export")
        .to_string();
    Ok(ExternalDestination { path, display_name })
}
