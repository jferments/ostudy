//! Cross-platform system-sharing and incoming-share command adapter.
//!
//! File selection and publication are owned by `external_files` through the
//! typed `ExternalFilePort`. This module contains only the platform operations
//! that have no desktop-file equivalent: system sharing and Android incoming
//! share adoption. Validation, hashing, transfer ledgers, idempotency, and
//! durable adoption remain in shared Rust.

#[cfg(target_os = "android")]
use std::{fs::OpenOptions, io::Write, path::PathBuf};
use std::{
    fs::{self, File},
    io::Read,
    path::Path,
};

#[cfg(target_os = "android")]
use rusqlite::{params, OpenFlags};
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, State};

#[cfg(target_os = "android")]
use tauri_plugin_ostudy_platform::OstudyPlatformExt;

#[cfg(target_os = "android")]
use crate::{
    assets::detect_image_bytes,
    external_files::{ExternalFileKind, ExternalImportPurpose},
};
use crate::{
    database::Database,
    error::CommandError,
    external_files::{PublishPreparedExportInput, StagedExternalFileDto},
    paths::AppPaths,
    platform::{PlatformRuntime, ShareSource},
    util::{unix_time_millis, validate_entity_id},
};

#[cfg(target_os = "android")]
const IMPORT_TTL_MS: i64 = 7 * 24 * 60 * 60 * 1_000;
const COPY_BUFFER_BYTES: usize = 1024 * 1024;
#[cfg(target_os = "android")]
const MAX_IMAGE_BYTES: u64 = 25 * 1024 * 1024;
#[cfg(target_os = "android")]
const MAX_PDF_BYTES: u64 = 512 * 1024 * 1024;
#[cfg(target_os = "android")]
const MAX_STUDY_CONTENT_BYTES: u64 = 640 * 1024 * 1024;
#[cfg(target_os = "android")]
const MAX_BACKUP_BYTES: u64 = 8 * 1024 * 1024 * 1024;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IncomingShareInput {
    pub token: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StageIncomingShareInput {
    pub token: String,
    pub operation_id: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IncomingShareSummaryDto {
    pub token: String,
    pub display_name: String,
    pub reported_media_type: Option<String>,
    pub byte_size: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareLaunchDto {
    pub prepared_export_id: String,
    pub launched: bool,
    pub destination_kind: String,
    pub launched_at: i64,
}

#[derive(Clone, Debug)]
struct StoredPreparedExport {
    id: String,
    suggested_filename: String,
    media_type: String,
    relative_path: String,
}

#[tauri::command]
pub async fn share_prepared_export(
    app: AppHandle,
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    runtime: State<'_, PlatformRuntime>,
    input: PublishPreparedExportInput,
) -> Result<ShareLaunchDto, CommandError> {
    validate_entity_id(&input.prepared_export_id, "prepared export")?;
    let stored = database.with_connection(|connection| {
        load_prepared_export(connection, paths.inner(), &input.prepared_export_id)
    })?;
    let launched_at = unix_time_millis()?;
    let result = runtime
        .system_share()
        .share_file(
            app,
            ShareSource {
                title: "Share ostudy export".to_string(),
                path: paths.resolve_relative(&stored.relative_path),
                display_name: stored.suggested_filename,
                media_type: stored.media_type,
            },
        )
        .await?;
    Ok(ShareLaunchDto {
        prepared_export_id: stored.id,
        launched: result.launched,
        destination_kind: result.destination_kind,
        launched_at,
    })
}

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
            .map_err(|error| plugin_error("incoming-share listing", error));
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
            .map_err(|error| plugin_error("incoming-share adoption", error))?;
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
                    "The shared file is not an ostudy deck, generated JSON file, or library backup.",
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
            .map_err(|error| plugin_error("incoming-share cleanup", error))?;
        Ok(staged)
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
            .map_err(|error| plugin_error("incoming-share cleanup", error));
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (app, input);
        Ok(())
    }
}

#[cfg(target_os = "android")]
fn stage_private_file(
    database: &Database,
    paths: &AppPaths,
    operation_id: &str,
    purpose: ExternalImportPurpose,
    source: &Path,
    display_name: &str,
) -> Result<StagedExternalFileDto, CommandError> {
    if let Some(existing) = database
        .with_connection(|connection| load_import_by_operation(connection, paths, operation_id))?
    {
        return Ok(existing);
    }
    let display_name = validate_display_name(display_name)?;
    let maximum = maximum_bytes(purpose);
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
    copy_bounded(source, &partial_path, maximum)?;
    let (byte_size, sha256) = hash_bounded_file(&partial_path, maximum)?;
    let detected_kind = detect_kind(&partial_path, purpose)?;
    commit_staged_file(
        database,
        paths,
        operation_id,
        purpose,
        detected_kind,
        display_name,
        byte_size,
        sha256,
        &partial_path,
    )
}

#[cfg(target_os = "android")]
#[allow(clippy::too_many_arguments)]
fn commit_staged_file(
    database: &Database,
    paths: &AppPaths,
    operation_id: &str,
    purpose: ExternalImportPurpose,
    detected_kind: ExternalFileKind,
    display_name: String,
    byte_size: i64,
    sha256: String,
    partial_path: &Path,
) -> Result<StagedExternalFileDto, CommandError> {
    let relative_path =
        AppPaths::staged_import_relative_path(operation_id, extension(detected_kind));
    let ready_path = paths.resolve_relative(&relative_path);
    remove_file_if_exists(&ready_path)?;
    let now = unix_time_millis()?;
    let expires_at = now.saturating_add(IMPORT_TTL_MS);
    let insert = database.with_connection(|connection| {
        connection
            .execute(
                "INSERT INTO staged_external_imports (
                     id, operation_id, purpose, detected_kind, display_name,
                     media_type, byte_size, sha256, relative_path, state,
                     created_at, updated_at, expires_at
                 ) VALUES (?1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'staging', ?9, ?9, ?10)",
                params![
                    operation_id,
                    purpose_str(purpose),
                    kind_str(detected_kind),
                    display_name,
                    media_type(detected_kind),
                    byte_size,
                    sha256,
                    relative_path,
                    now,
                    expires_at,
                ],
            )
            .map_err(CommandError::database)?;
        Ok(())
    });
    if let Err(error) = insert {
        let _ = fs::remove_file(partial_path);
        if let Some(existing) = database.with_connection(|connection| {
            load_import_by_operation(connection, paths, operation_id)
        })? {
            return Ok(existing);
        }
        return Err(error);
    }
    if let Err(error) = fs::rename(partial_path, &ready_path) {
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
                "The selected file changed while it was being staged.",
            ));
        }
        Ok(())
    })?;
    Ok(StagedExternalFileDto {
        id: operation_id.to_string(),
        purpose,
        detected_kind,
        display_name,
        media_type: media_type(detected_kind).to_string(),
        byte_size,
        sha256,
        expires_at,
    })
}

#[cfg(target_os = "android")]
fn load_import_by_operation(
    connection: &Connection,
    paths: &AppPaths,
    operation_id: &str,
) -> Result<Option<StagedExternalFileDto>, CommandError> {
    let row: Option<(
        String,
        String,
        String,
        String,
        i64,
        String,
        String,
        i64,
        String,
    )> = connection
        .query_row(
            "SELECT id, purpose, detected_kind, display_name, byte_size, sha256,
                    relative_path, expires_at, state
             FROM staged_external_imports WHERE operation_id = ?1",
            [operation_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                ))
            },
        )
        .optional()
        .map_err(CommandError::database)?;
    let Some((id, purpose, kind, display_name, byte_size, sha256, relative, expires_at, state)) =
        row
    else {
        return Ok(None);
    };
    if !matches!(state.as_str(), "staged" | "consumed") {
        return Err(CommandError::new(
            "external_file_operation_conflict",
            "That external-file operation identifier is no longer reusable.",
        ));
    }
    verify_file(&paths.resolve_relative(&relative), byte_size, &sha256)?;
    let detected_kind = parse_kind(&kind)?;
    Ok(Some(StagedExternalFileDto {
        id,
        purpose: parse_purpose(&purpose)?,
        detected_kind,
        display_name,
        media_type: media_type(detected_kind).to_string(),
        byte_size,
        sha256,
        expires_at,
    }))
}

fn load_prepared_export(
    connection: &Connection,
    paths: &AppPaths,
    id: &str,
) -> Result<StoredPreparedExport, CommandError> {
    let row: Option<(String, String, i64, String, String, String)> = connection
        .query_row(
            "SELECT suggested_filename, media_type, byte_size, sha256, relative_path, state
             FROM prepared_exports WHERE id = ?1",
            [id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )
        .optional()
        .map_err(CommandError::database)?;
    let Some((suggested_filename, media_type, byte_size, sha256, relative_path, state)) = row
    else {
        return Err(CommandError::new(
            "prepared_export_not_found",
            "The prepared export is no longer available.",
        ));
    };
    if !matches!(state.as_str(), "prepared" | "published") {
        return Err(CommandError::new(
            "prepared_export_unavailable",
            "The prepared export is no longer available.",
        ));
    }
    verify_file(&paths.resolve_relative(&relative_path), byte_size, &sha256)?;
    Ok(StoredPreparedExport {
        id: id.to_string(),
        suggested_filename,
        media_type,
        relative_path,
    })
}

#[cfg(target_os = "android")]
fn detect_kind(
    path: &Path,
    purpose: ExternalImportPurpose,
) -> Result<ExternalFileKind, CommandError> {
    match purpose {
        ExternalImportPurpose::Image => {
            let mut file = File::open(path).map_err(CommandError::io)?;
            let mut header = [0_u8; 16];
            let read = file.read(&mut header).map_err(CommandError::io)?;
            let (mime, _) = detect_image_bytes(&header[..read])?;
            match mime {
                "image/png" => Ok(ExternalFileKind::Png),
                "image/jpeg" => Ok(ExternalFileKind::Jpeg),
                "image/gif" => Ok(ExternalFileKind::Gif),
                "image/webp" => Ok(ExternalFileKind::Webp),
                _ => Err(CommandError::new(
                    "unsupported_image",
                    "Use a supported image file.",
                )),
            }
        }
        ExternalImportPurpose::PdfDocument => {
            let mut file = File::open(path).map_err(CommandError::io)?;
            let mut signature = [0_u8; 5];
            file.read_exact(&mut signature).map_err(CommandError::io)?;
            if &signature == b"%PDF-" {
                Ok(ExternalFileKind::Pdf)
            } else {
                Err(CommandError::new(
                    "invalid_pdf",
                    "The selected file is not a PDF.",
                ))
            }
        }
        ExternalImportPurpose::StudyContent => detect_study_content(path),
        ExternalImportPurpose::LibraryBackup => {
            let format = sqlite_metadata(path, "backup_metadata", "format")?;
            if format.as_deref() == Some("ostudy-library-backup") {
                Ok(ExternalFileKind::Ostudybackup)
            } else {
                Err(CommandError::new(
                    "invalid_backup",
                    "The selected file is not an ostudy library backup.",
                ))
            }
        }
    }
}

#[cfg(target_os = "android")]
fn detect_study_content(path: &Path) -> Result<ExternalFileKind, CommandError> {
    let mut signature = [0_u8; 16];
    let read = File::open(path)
        .and_then(|mut file| file.read(&mut signature))
        .map_err(CommandError::io)?;
    if read == signature.len() && &signature == b"SQLite format 3\0" {
        let format = sqlite_metadata(path, "package_metadata", "format")?;
        return if format.as_deref() == Some("studydeck-sqlite") {
            Ok(ExternalFileKind::Studydeck)
        } else {
            Err(CommandError::new(
                "invalid_package",
                "The selected SQLite file is not an ostudy deck package.",
            ))
        };
    }
    let bytes = fs::read(path).map_err(CommandError::io)?;
    let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| {
        CommandError::new(
            "invalid_flashcard_import_json",
            "The selected content is neither an ostudy deck nor valid generated JSON.",
        )
    })?;
    if value.get("format").and_then(serde_json::Value::as_str) == Some("ostudy-flashcards")
        && value
            .get("formatVersion")
            .and_then(serde_json::Value::as_i64)
            == Some(1)
    {
        Ok(ExternalFileKind::GeneratedJson)
    } else {
        Err(CommandError::new(
            "unsupported_flashcard_import",
            "The selected JSON is not a supported ostudy generated-flashcard file.",
        ))
    }
}

#[cfg(target_os = "android")]
fn sqlite_metadata(path: &Path, table: &str, key: &str) -> Result<Option<String>, CommandError> {
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(CommandError::database)?;
    connection
        .pragma_update(None, "query_only", true)
        .map_err(CommandError::database)?;
    let sql = match table {
        "package_metadata" => "SELECT value FROM package_metadata WHERE key = ?1",
        "backup_metadata" => "SELECT value FROM backup_metadata WHERE key = ?1",
        _ => {
            return Err(CommandError::new(
                "invalid_external_file_metadata",
                "The external file metadata table is invalid.",
            ));
        }
    };
    connection
        .query_row(sql, [key], |row| row.get(0))
        .optional()
        .map_err(|_| {
            CommandError::new(
                "invalid_external_file_metadata",
                "The selected file does not contain valid ostudy metadata.",
            )
        })
}

#[cfg(target_os = "android")]
fn maximum_bytes(purpose: ExternalImportPurpose) -> u64 {
    match purpose {
        ExternalImportPurpose::Image => MAX_IMAGE_BYTES,
        ExternalImportPurpose::PdfDocument => MAX_PDF_BYTES,
        ExternalImportPurpose::StudyContent => MAX_STUDY_CONTENT_BYTES,
        ExternalImportPurpose::LibraryBackup => MAX_BACKUP_BYTES,
    }
}

#[cfg(target_os = "android")]
fn purpose_str(purpose: ExternalImportPurpose) -> &'static str {
    match purpose {
        ExternalImportPurpose::Image => "image",
        ExternalImportPurpose::PdfDocument => "pdf_document",
        ExternalImportPurpose::StudyContent => "study_content",
        ExternalImportPurpose::LibraryBackup => "library_backup",
    }
}

#[cfg(target_os = "android")]
fn parse_purpose(value: &str) -> Result<ExternalImportPurpose, CommandError> {
    match value {
        "image" => Ok(ExternalImportPurpose::Image),
        "pdf_document" => Ok(ExternalImportPurpose::PdfDocument),
        "study_content" => Ok(ExternalImportPurpose::StudyContent),
        "library_backup" => Ok(ExternalImportPurpose::LibraryBackup),
        _ => Err(CommandError::new(
            "invalid_external_file_purpose",
            "The staged file purpose is invalid.",
        )),
    }
}

#[cfg(target_os = "android")]
fn kind_str(kind: ExternalFileKind) -> &'static str {
    match kind {
        ExternalFileKind::Png => "png",
        ExternalFileKind::Jpeg => "jpeg",
        ExternalFileKind::Gif => "gif",
        ExternalFileKind::Webp => "webp",
        ExternalFileKind::Pdf => "pdf",
        ExternalFileKind::Studydeck => "studydeck",
        ExternalFileKind::GeneratedJson => "generated_json",
        ExternalFileKind::Ostudybackup => "ostudybackup",
    }
}

#[cfg(target_os = "android")]
fn parse_kind(value: &str) -> Result<ExternalFileKind, CommandError> {
    match value {
        "png" => Ok(ExternalFileKind::Png),
        "jpeg" => Ok(ExternalFileKind::Jpeg),
        "gif" => Ok(ExternalFileKind::Gif),
        "webp" => Ok(ExternalFileKind::Webp),
        "pdf" => Ok(ExternalFileKind::Pdf),
        "studydeck" => Ok(ExternalFileKind::Studydeck),
        "generated_json" => Ok(ExternalFileKind::GeneratedJson),
        "ostudybackup" => Ok(ExternalFileKind::Ostudybackup),
        _ => Err(CommandError::new(
            "invalid_external_file_kind",
            "The staged file type is invalid.",
        )),
    }
}

#[cfg(target_os = "android")]
fn extension(kind: ExternalFileKind) -> &'static str {
    match kind {
        ExternalFileKind::Png => "png",
        ExternalFileKind::Jpeg => "jpg",
        ExternalFileKind::Gif => "gif",
        ExternalFileKind::Webp => "webp",
        ExternalFileKind::Pdf => "pdf",
        ExternalFileKind::Studydeck => "studydeck",
        ExternalFileKind::GeneratedJson => "json",
        ExternalFileKind::Ostudybackup => "ostudybackup",
    }
}

#[cfg(target_os = "android")]
fn media_type(kind: ExternalFileKind) -> &'static str {
    match kind {
        ExternalFileKind::Png => "image/png",
        ExternalFileKind::Jpeg => "image/jpeg",
        ExternalFileKind::Gif => "image/gif",
        ExternalFileKind::Webp => "image/webp",
        ExternalFileKind::Pdf => "application/pdf",
        ExternalFileKind::Studydeck => "application/vnd.ostudy.studydeck",
        ExternalFileKind::GeneratedJson => "application/json",
        ExternalFileKind::Ostudybackup => "application/vnd.ostudy.backup",
    }
}

#[cfg(target_os = "android")]
fn validate_display_name(value: &str) -> Result<String, CommandError> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed.chars().count() > 255
        || trimmed
            .chars()
            .any(|character| character.is_control() || matches!(character, '/' | '\\' | '\0'))
    {
        return Err(CommandError::new(
            "invalid_external_file_name",
            "The selected file has an invalid name.",
        ));
    }
    Ok(trimmed.to_string())
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

#[cfg(target_os = "android")]
fn copy_bounded(source: &Path, destination: &Path, maximum_bytes: u64) -> Result<(), CommandError> {
    let mut input = File::open(source).map_err(CommandError::io)?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(CommandError::io)?;
    let mut copied = 0_u64;
    let mut buffer = vec![0_u8; COPY_BUFFER_BYTES];
    loop {
        let read = input.read(&mut buffer).map_err(CommandError::io)?;
        if read == 0 {
            break;
        }
        copied = copied.saturating_add(u64::try_from(read).unwrap_or(u64::MAX));
        if copied > maximum_bytes {
            drop(output);
            let _ = fs::remove_file(destination);
            return Err(CommandError::new(
                "external_file_size_invalid",
                "The selected file exceeds the supported size limit.",
            ));
        }
        output
            .write_all(&buffer[..read])
            .map_err(CommandError::io)?;
    }
    output.sync_all().map_err(CommandError::io)
}

fn hash_bounded_file(path: &Path, maximum_bytes: u64) -> Result<(i64, String), CommandError> {
    let metadata = fs::metadata(path).map_err(CommandError::io)?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > maximum_bytes {
        return Err(CommandError::new(
            "external_file_size_invalid",
            "The selected file is empty or exceeds the supported size limit.",
        ));
    }
    let mut file = File::open(path).map_err(CommandError::io)?;
    let mut hasher = Sha256::new();
    let mut total = 0_u64;
    let mut buffer = vec![0_u8; COPY_BUFFER_BYTES];
    loop {
        let read = file.read(&mut buffer).map_err(CommandError::io)?;
        if read == 0 {
            break;
        }
        total = total.saturating_add(u64::try_from(read).unwrap_or(u64::MAX));
        if total > maximum_bytes {
            return Err(CommandError::new(
                "external_file_size_invalid",
                "The selected file exceeds the supported size limit.",
            ));
        }
        hasher.update(&buffer[..read]);
    }
    Ok((
        i64::try_from(total).unwrap_or(i64::MAX),
        format!("{:x}", hasher.finalize()),
    ))
}

fn verify_file(path: &Path, expected_size: i64, expected_hash: &str) -> Result<(), CommandError> {
    let maximum = u64::try_from(expected_size).map_err(|_| {
        CommandError::new(
            "external_file_changed",
            "The managed transfer file has an invalid recorded size.",
        )
    })?;
    let (actual_size, actual_hash) = hash_bounded_file(path, maximum)?;
    if actual_size != expected_size || actual_hash != expected_hash {
        return Err(CommandError::new(
            "external_file_changed",
            "The managed transfer file no longer matches its recorded content.",
        ));
    }
    Ok(())
}

#[cfg(target_os = "android")]
fn create_parent(path: &Path) -> Result<(), CommandError> {
    let parent = path.parent().ok_or_else(|| {
        CommandError::new(
            "invalid_external_file_path",
            "The managed external-file path is invalid.",
        )
    })?;
    fs::create_dir_all(parent).map_err(CommandError::io)
}

#[cfg(target_os = "android")]
fn remove_file_if_exists(path: &Path) -> Result<(), CommandError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(CommandError::io(error)),
    }
}

#[cfg(target_os = "android")]
fn sync_file(path: &Path) -> Result<(), CommandError> {
    OpenOptions::new()
        .read(true)
        .open(path)
        .and_then(|file| file.sync_all())
        .map_err(CommandError::io)
}

#[cfg(target_os = "android")]
fn sync_directory(path: Option<&Path>) -> Result<(), CommandError> {
    let path = path.ok_or_else(|| {
        CommandError::new(
            "invalid_external_file_path",
            "The managed external-file directory is invalid.",
        )
    })?;
    OpenOptions::new()
        .read(true)
        .open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(CommandError::io)
}

#[cfg(target_os = "android")]
fn plugin_error(operation: &str, error: tauri_plugin_ostudy_platform::Error) -> CommandError {
    CommandError::new(
        "android_platform_operation_failed",
        format!("The Android {operation} operation failed: {error}"),
    )
}
