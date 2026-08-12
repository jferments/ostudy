#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const read = (path) => readFile(join(root, path), "utf8");
const write = (path, content) => writeFile(join(root, path), content, "utf8");

function replaceExact(text, oldValue, newValue, label) {
  if (!text.includes(oldValue)) {
    throw new Error(`${label}: expected source was not found`);
  }
  return text.replace(oldValue, newValue);
}

let platform = await read("src-tauri/src/platform/mod.rs");
platform = replaceExact(
  platform,
  `use std::{
    path::PathBuf,
    sync::{`,
  `use std::{
    future::Future,
    path::PathBuf,
    pin::Pin,
    sync::{`,
  "import future types for external file port",
);
platform = replaceExact(
  platform,
  `pub trait ExternalFilePort: Send + Sync {
    fn select_import(
        &self,
        app: &AppHandle,
        request: &ExternalFileRequest,
    ) -> Result<Option<ExternalSource>, CommandError>;

    fn select_export(
        &self,
        app: &AppHandle,
        request: &ExternalFileRequest,
    ) -> Result<Option<ExternalDestination>, CommandError>;
}`,
  `pub type PlatformFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

pub trait ExternalFilePort: Send + Sync {
    fn select_import(
        &self,
        app: AppHandle,
        request: ExternalFileRequest,
    ) -> PlatformFuture<Result<Option<ExternalSource>, CommandError>>;

    fn select_export(
        &self,
        app: AppHandle,
        request: ExternalFileRequest,
    ) -> PlatformFuture<Result<Option<ExternalDestination>, CommandError>>;
}`,
  "make external file port asynchronous",
);
await write("src-tauri/src/platform/mod.rs", platform);

const desktop = `use tauri::AppHandle;
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
`;
await write("src-tauri/src/platform/desktop/external_files.rs", desktop);

let mobile = await read("src-tauri/src/platform/mobile_stub.rs");
mobile = replaceExact(
  mobile,
  `        CapabilityProvider, ExternalDestination, ExternalFilePort, ExternalFileRequest,
        ExternalSource, NativeServiceCapabilities, PlatformCapabilitySnapshot, PlatformRuntime,
        ResourceProbe, ResourceSnapshot, ServiceCapability, ThermalState, TimeZoneProvider,
        TimeZoneSnapshot, UnavailableSecretStore,`,
  `        CapabilityProvider, ExternalDestination, ExternalFilePort, ExternalFileRequest,
        ExternalSource, NativeServiceCapabilities, PlatformCapabilitySnapshot, PlatformFuture,
        PlatformRuntime, ResourceProbe, ResourceSnapshot, ServiceCapability, ThermalState,
        TimeZoneProvider, TimeZoneSnapshot, UnavailableSecretStore,`,
  "import mobile platform future",
);
mobile = replaceExact(
  mobile,
  `impl ExternalFilePort for UnavailableExternalFilePort {
    fn select_import(
        &self,
        _app: &AppHandle,
        _request: &ExternalFileRequest,
    ) -> Result<Option<ExternalSource>, CommandError> {
        Err(CommandError::new(
            "external_file_adapter_unavailable",
            "This mobile build does not yet include its native file-selection adapter.",
        ))
    }

    fn select_export(
        &self,
        _app: &AppHandle,
        _request: &ExternalFileRequest,
    ) -> Result<Option<ExternalDestination>, CommandError> {
        Err(CommandError::new(
            "external_file_adapter_unavailable",
            "This mobile build does not yet include its native export adapter.",
        ))
    }
}`,
  `impl ExternalFilePort for UnavailableExternalFilePort {
    fn select_import(
        &self,
        _app: AppHandle,
        _request: ExternalFileRequest,
    ) -> PlatformFuture<Result<Option<ExternalSource>, CommandError>> {
        Box::pin(async {
            Err(CommandError::new(
                "external_file_adapter_unavailable",
                "This mobile build does not yet include its native file-selection adapter.",
            ))
        })
    }

    fn select_export(
        &self,
        _app: AppHandle,
        _request: ExternalFileRequest,
    ) -> PlatformFuture<Result<Option<ExternalDestination>, CommandError>> {
        Box::pin(async {
            Err(CommandError::new(
                "external_file_adapter_unavailable",
                "This mobile build does not yet include its native export adapter.",
            ))
        })
    }
}`,
  "make mobile external file stub asynchronous",
);
await write("src-tauri/src/platform/mobile_stub.rs", mobile);

let externalFiles = await read("src-tauri/src/external_files.rs");
externalFiles = replaceExact(
  externalFiles,
  `#[tauri::command]
pub fn select_and_stage_external_import(`,
  `#[tauri::command]
pub async fn select_and_stage_external_import(`,
  "make external import command asynchronous",
);
externalFiles = replaceExact(
  externalFiles,
  `    let Some(source) = runtime
        .external_files()
        .select_import(&app, &input.purpose.request())?
    else {`,
  `    let Some(source) = runtime
        .external_files()
        .select_import(app.clone(), input.purpose.request())
        .await?
    else {`,
  "await external import selection",
);
externalFiles = replaceExact(
  externalFiles,
  `#[tauri::command]
pub fn publish_prepared_export(`,
  `#[tauri::command]
pub async fn publish_prepared_export(`,
  "make export publication command asynchronous",
);
externalFiles = replaceExact(
  externalFiles,
  `    let Some(destination) = runtime.external_files().select_export(&app, &request)? else {`,
  `    let Some(destination) = runtime
        .external_files()
        .select_export(app.clone(), request)
        .await?
    else {`,
  "await export destination selection",
);
await write("src-tauri/src/external_files.rs", externalFiles);

console.log("Applied non-blocking external file dialog integration.");
