use tauri::AppHandle;
use tauri_plugin_dialog::{DialogExt, FilePath};

use crate::{
    error::CommandError,
    platform::{ExternalDestination, ExternalFilePort, ExternalFileRequest, ExternalSource},
};

pub(super) struct DesktopExternalFilePort;

impl ExternalFilePort for DesktopExternalFilePort {
    fn select_import(
        &self,
        app: &AppHandle,
        request: &ExternalFileRequest,
    ) -> Result<Option<ExternalSource>, CommandError> {
        let mut dialog = app.dialog().file().set_title(&request.title);
        for filter in &request.filters {
            let extensions = filter
                .extensions
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>();
            dialog = dialog.add_filter(&filter.name, &extensions);
        }
        dialog
            .blocking_pick_file()
            .map(source_from_file_path)
            .transpose()
    }

    fn select_export(
        &self,
        app: &AppHandle,
        request: &ExternalFileRequest,
    ) -> Result<Option<ExternalDestination>, CommandError> {
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
        dialog
            .blocking_save_file()
            .map(destination_from_file_path)
            .transpose()
    }
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
