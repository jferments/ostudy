use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

use tauri::AppHandle;
use tauri_plugin_dialog::{DialogExt, FilePath};
use tokio::sync::oneshot;

use crate::{
    error::CommandError,
    platform::{
        ExternalAcquireResult, ExternalDestination, ExternalExportSource, ExternalFilePort,
        ExternalFileRequest, ExternalImportTarget, ExternalPublishResult, ExternalSource,
        PlatformFuture,
    },
};

const COPY_BUFFER_BYTES: usize = 1024 * 1024;

pub(super) struct DesktopExternalFilePort;

impl ExternalFilePort for DesktopExternalFilePort {
    fn select_import(
        &self,
        app: AppHandle,
        request: ExternalFileRequest,
    ) -> PlatformFuture<Result<Option<ExternalSource>, CommandError>> {
        Box::pin(async move {
            Ok(select_import_path(app, request)
                .await?
                .map(|(path, display_name)| ExternalSource { path, display_name }))
        })
    }

    fn select_export(
        &self,
        app: AppHandle,
        request: ExternalFileRequest,
    ) -> PlatformFuture<Result<Option<ExternalDestination>, CommandError>> {
        Box::pin(async move {
            Ok(select_export_path(app, request)
                .await?
                .map(|(path, display_name)| ExternalDestination { path, display_name }))
        })
    }

    fn acquire_import(
        &self,
        app: AppHandle,
        request: ExternalFileRequest,
        target: ExternalImportTarget,
    ) -> PlatformFuture<Result<Option<ExternalAcquireResult>, CommandError>> {
        Box::pin(async move {
            let selection = select_import_path(app, request).await?;
            let Some((source_path, display_name)) = selection else {
                return Ok(None);
            };
            let target_path = target.path;
            let copied_bytes = tauri::async_runtime::spawn_blocking(move || {
                copy_bounded(&source_path, &target_path, target.maximum_bytes)
            })
            .await
            .map_err(|_| copy_interrupted())??;
            Ok(Some(ExternalAcquireResult {
                display_name,
                reported_media_type: None,
                copied_bytes,
            }))
        })
    }

    fn publish_export(
        &self,
        app: AppHandle,
        request: ExternalFileRequest,
        source: ExternalExportSource,
    ) -> PlatformFuture<Result<Option<ExternalPublishResult>, CommandError>> {
        Box::pin(async move {
            let selection = select_export_path(app, request).await?;
            let Some((destination_path, display_name)) = selection else {
                return Ok(None);
            };
            let source_path = source.path;
            let copied_bytes = tauri::async_runtime::spawn_blocking(move || {
                publish_atomic(&source_path, &destination_path, source.maximum_bytes)
            })
            .await
            .map_err(|_| copy_interrupted())??;
            Ok(Some(ExternalPublishResult {
                display_name,
                copied_bytes,
                destination_kind: "desktop_file".to_string(),
            }))
        })
    }
}

async fn select_import_path(
    app: AppHandle,
    request: ExternalFileRequest,
) -> Result<Option<(PathBuf, String)>, CommandError> {
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
}

async fn select_export_path(
    app: AppHandle,
    request: ExternalFileRequest,
) -> Result<Option<(PathBuf, String)>, CommandError> {
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
    selection.map(source_from_file_path).transpose()
}

fn source_from_file_path(value: FilePath) -> Result<(PathBuf, String), CommandError> {
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
    Ok((path, display_name))
}

fn copy_bounded(
    source: &Path,
    destination: &Path,
    maximum_bytes: u64,
) -> Result<u64, CommandError> {
    let source = fs::canonicalize(source).map_err(CommandError::io)?;
    if !source.is_file() {
        return Err(CommandError::new(
            "external_file_missing",
            "The selected file is no longer available.",
        ));
    }
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(CommandError::io)?;
    }
    let mut input = File::open(source).map_err(CommandError::io)?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(CommandError::io)?;
    let mut buffer = vec![0_u8; COPY_BUFFER_BYTES];
    let mut copied = 0_u64;
    loop {
        let count = input.read(&mut buffer).map_err(CommandError::io)?;
        if count == 0 {
            break;
        }
        copied = copied.saturating_add(u64::try_from(count).unwrap_or(u64::MAX));
        if copied > maximum_bytes {
            drop(output);
            let _ = fs::remove_file(destination);
            return Err(CommandError::new(
                "external_file_too_large",
                "The selected file is larger than this import supports.",
            ));
        }
        output
            .write_all(&buffer[..count])
            .map_err(CommandError::io)?;
    }
    output.sync_all().map_err(CommandError::io)?;
    Ok(copied)
}

fn publish_atomic(
    source: &Path,
    destination: &Path,
    maximum_bytes: u64,
) -> Result<u64, CommandError> {
    if !source.is_file() {
        return Err(CommandError::new(
            "prepared_export_missing",
            "The prepared export is no longer available.",
        ));
    }
    let parent = destination.parent().ok_or_else(|| {
        CommandError::new(
            "invalid_export_destination",
            "The selected export destination is invalid.",
        )
    })?;
    fs::create_dir_all(parent).map_err(CommandError::io)?;
    let temporary = parent.join(format!(
        ".{}.ostudy-partial",
        destination
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("export")
    ));
    match fs::remove_file(&temporary) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(CommandError::io(error)),
    }
    let copied = copy_bounded(source, &temporary, maximum_bytes)?;
    if destination.exists() {
        fs::remove_file(destination).map_err(CommandError::io)?;
    }
    fs::rename(&temporary, destination).map_err(CommandError::io)?;
    let directory = OpenOptions::new()
        .read(true)
        .open(parent)
        .map_err(CommandError::io)?;
    directory.sync_all().map_err(CommandError::io)?;
    Ok(copied)
}

fn dialog_result_interrupted() -> CommandError {
    CommandError::new(
        "external_file_dialog_interrupted",
        "The system file dialog closed before ostudy received its result. Try again.",
    )
}

fn copy_interrupted() -> CommandError {
    CommandError::new(
        "external_file_copy_interrupted",
        "The external file operation ended before it completed. Try again.",
    )
}
