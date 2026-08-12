#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const read = (path) => readFile(join(root, path), "utf8");
const write = (path, content) => writeFile(join(root, path), content, "utf8");
function replaceExact(text, before, after, label) {
  if (!text.includes(before)) throw new Error(`${label}: expected source not found`);
  return text.replace(before, after);
}

let external = await read("src-tauri/src/external_files.rs");
if (!external.includes("tauri_plugin_ostudy_platform::OstudyPlatformExt")) {
  external = replaceExact(
    external,
    `use tauri::{AppHandle, State};\n`,
    `use tauri::{AppHandle, State};\n#[cfg(target_os = "android")]\nuse tauri_plugin_ostudy_platform::OstudyPlatformExt;\n`,
    "import Android platform plugin",
  );
}
external = replaceExact(
  external,
  `pub struct ReleasePreparedExportInput {\n    pub prepared_export_id: String,\n}\n`,
  `pub struct ReleasePreparedExportInput {\n    pub prepared_export_id: String,\n}\n\n#[derive(Debug, Deserialize)]\n#[serde(rename_all = "camelCase", deny_unknown_fields)]\npub struct IncomingShareInput {\n    pub token: String,\n}\n\n#[derive(Debug, Deserialize)]\n#[serde(rename_all = "camelCase", deny_unknown_fields)]\npub struct StageIncomingShareInput {\n    pub token: String,\n    pub operation_id: String,\n}\n\n#[derive(Clone, Debug, Serialize)]\n#[serde(rename_all = "camelCase")]\npub struct IncomingShareSummaryDto {\n    pub token: String,\n    pub display_name: String,\n    pub reported_media_type: Option<String>,\n    pub byte_size: u64,\n}\n`,
  "add incoming share DTOs",
);

const incomingCommands = `
#[tauri::command]
pub fn list_incoming_shares(app: AppHandle) -> Result<Vec<IncomingShareSummaryDto>, CommandError> {
    #[cfg(target_os = "android")]
    {
        return app
            .ostudy_platform()
            .list_incoming_shares()
            .map(|items| {
                items
                    .into_iter()
                    .map(|item| IncomingShareSummaryDto {
                        token: item.token,
                        display_name: item.display_name,
                        reported_media_type: item.reported_media_type,
                        byte_size: item.byte_size,
                    })
                    .collect()
            })
            .map_err(|error| {
                CommandError::new(
                    "android_incoming_share_failed",
                    format!("Android incoming shares could not be listed: {error}"),
                )
            });
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        Ok(Vec::new())
    }
}

#[tauri::command]
pub fn stage_incoming_share(
    app: AppHandle,
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: StageIncomingShareInput,
) -> Result<StagedExternalFileDto, CommandError> {
    validate_incoming_token(&input.token)?;
    validate_entity_id(&input.operation_id, "incoming share import")?;
    #[cfg(target_os = "android")]
    {
        if let Some(existing) = database.with_connection(|connection| {
            load_import_by_operation(connection, paths.inner(), &input.operation_id)
        })? {
            let _ = app
                .ostudy_platform()
                .dismiss_incoming_share(input.token.clone());
            return Ok(existing);
        }
        let incoming = app
            .ostudy_platform()
            .take_incoming_share(input.token.clone())
            .map_err(|error| {
                CommandError::new(
                    "android_incoming_share_failed",
                    format!("The Android incoming share could not be opened: {error}"),
                )
            })?;
        let source = PathBuf::from(incoming.path);
        let kind = detect_kind(&source, ExternalImportPurpose::StudyContent)
            .or_else(|_| detect_kind(&source, ExternalImportPurpose::LibraryBackup))?;
        let purpose = match kind {
            ExternalFileKind::Ostudybackup => ExternalImportPurpose::LibraryBackup,
            ExternalFileKind::Studydeck | ExternalFileKind::GeneratedJson => {
                ExternalImportPurpose::StudyContent
            }
            _ => {
                return Err(CommandError::new(
                    "unsupported_incoming_share",
                    "Android shared a file that is not an ostudy deck, generated JSON file, or library backup.",
                ));
            }
        };
        let staged = stage_private_file(
            database.inner(),
            paths.inner(),
            &input.operation_id,
            purpose,
            &source,
            &incoming.display_name,
        )?;
        app.ostudy_platform()
            .dismiss_incoming_share(input.token)
            .map_err(|error| {
                CommandError::new(
                    "android_incoming_share_cleanup_failed",
                    format!("The incoming share was imported but its Android cache could not be cleared: {error}"),
                )
            })?;
        return Ok(staged);
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (app, database, paths, input);
        Err(CommandError::new(
            "incoming_share_unavailable",
            "Incoming system shares are available only in the Android application.",
        ))
    }
}

#[tauri::command]
pub fn dismiss_incoming_share(
    app: AppHandle,
    input: IncomingShareInput,
) -> Result<(), CommandError> {
    validate_incoming_token(&input.token)?;
    #[cfg(target_os = "android")]
    {
        return app
            .ostudy_platform()
            .dismiss_incoming_share(input.token)
            .map_err(|error| {
                CommandError::new(
                    "android_incoming_share_cleanup_failed",
                    format!("The incoming Android share could not be dismissed: {error}"),
                )
            });
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (app, input);
        Ok(())
    }
}

fn stage_private_file(
    database: &Database,
    paths: &AppPaths,
    operation_id: &str,
    purpose: ExternalImportPurpose,
    source: &Path,
    display_name: &str,
) -> Result<StagedExternalFileDto, CommandError> {
    if let Some(existing) = database.with_connection(|connection| {
        load_import_by_operation(connection, paths, operation_id)
    })? {
        return Ok(existing);
    }
    let display_name = validate_display_name(display_name)?;
    let maximum = purpose.maximum_bytes();
    let metadata = fs::metadata(source).map_err(CommandError::io)?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > maximum {
        return Err(CommandError::new(
            "external_file_size_invalid",
            "The incoming shared file is empty or exceeds the supported size limit.",
        ));
    }
    let partial_relative = AppPaths::staged_import_partial_relative_path(operation_id);
    let partial_path = paths.resolve_relative(&partial_relative);
    create_parent(&partial_path)?;
    remove_file_if_exists(&partial_path)?;
    fs::copy(source, &partial_path).map_err(CommandError::io)?;
    sync_file(&partial_path)?;
    let (byte_size, sha256) = hash_bounded_file(&partial_path, maximum)?;
    let detected_kind = detect_kind(&partial_path, purpose)?;
    let relative_path =
        AppPaths::staged_import_relative_path(operation_id, detected_kind.extension());
    let ready_path = paths.resolve_relative(&relative_path);
    remove_file_if_exists(&ready_path)?;
    let now = unix_time_millis()?;
    let expires_at = now.saturating_add(IMPORT_TTL_MS);
    database.with_connection(|connection| {
        connection
            .execute(
                "INSERT INTO staged_external_imports (
                     id, operation_id, purpose, detected_kind, display_name,
                     media_type, byte_size, sha256, relative_path, state,
                     created_at, updated_at, expires_at
                 ) VALUES (?1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'staging', ?9, ?9, ?10)",
                params![
                    operation_id,
                    purpose.as_str(),
                    detected_kind.as_str(),
                    display_name,
                    detected_kind.media_type(),
                    byte_size,
                    sha256,
                    relative_path,
                    now,
                    expires_at,
                ],
            )
            .map_err(CommandError::database)?;
        Ok(())
    })?;
    if let Err(error) = fs::rename(&partial_path, &ready_path) {
        let _ = database.with_connection(|connection| {
            connection
                .execute(
                    "UPDATE staged_external_imports SET state = 'failed', updated_at = ?1
                     WHERE id = ?2 AND state = 'staging'",
                    params![unix_time_millis().unwrap_or_default(), operation_id],
                )
                .map_err(CommandError::database)?;
            Ok(())
        });
        return Err(CommandError::io(error));
    }
    sync_file(&ready_path)?;
    sync_directory(ready_path.parent())?;
    database.with_connection(|connection| {
        let changed = connection
            .execute(
                "UPDATE staged_external_imports SET state = 'staged', updated_at = ?1
                 WHERE id = ?2 AND state = 'staging'",
                params![unix_time_millis()?, operation_id],
            )
            .map_err(CommandError::database)?;
        if changed != 1 {
            return Err(CommandError::new(
                "external_file_state_changed",
                "The incoming shared file changed while it was being staged.",
            ));
        }
        Ok(())
    })?;
    Ok(StagedExternalFileDto {
        id: operation_id.to_string(),
        purpose,
        detected_kind,
        display_name,
        media_type: detected_kind.media_type().to_string(),
        byte_size,
        sha256,
        expires_at,
    })
}

fn validate_incoming_token(value: &str) -> Result<(), CommandError> {
    if value.len() != 32
        || !value
            .bytes()
            .all(|value| value.is_ascii_digit() || (b'a'..=b'f').contains(&value))
    {
        return Err(CommandError::new(
            "invalid_incoming_share",
            "The incoming Android share identity is invalid.",
        ));
    }
    Ok(())
}

`;
if (!external.includes("pub fn list_incoming_shares(")) {
  external = replaceExact(
    external,
    `#[tauri::command]\npub fn release_staged_external_import(`,
    `${incomingCommands}#[tauri::command]\npub fn release_staged_external_import(`,
    "add incoming share commands",
  );
}
await write("src-tauri/src/external_files.rs", external);

let lib = await read("src-tauri/src/lib.rs");
if (!lib.includes("external_files::list_incoming_shares,")) {
  lib = replaceExact(
    lib,
    `            external_files::release_staged_external_import,`,
    `            external_files::release_staged_external_import,\n            external_files::list_incoming_shares,\n            external_files::stage_incoming_share,\n            external_files::dismiss_incoming_share,`,
    "register incoming share commands",
  );
}
await write("src-tauri/src/lib.rs", lib);

console.log("Applied Android incoming-share adoption and confirmation backend.");
