//! Canonical streaming external-file command boundary.
//!
//! The platform adapter owns selection and byte transfer. Shared Rust owns
//! validation, hashing, content detection, transfer ledgers, idempotency, and
//! publication receipts. This is the only import/export command layer exposed
//! to React; legacy path-selection commands remain unregistered while their
//! desktop callers are retired.

use std::{
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
};

use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, State};

use crate::{
    assets::detect_image_bytes,
    database::Database,
    document_domain::DocumentFormat,
    document_ingestion,
    error::CommandError,
    external_files::{
        ExternalFileKind, ExternalImportPurpose, PublishPreparedExportInput, PublishReceiptDto,
        StageExternalImportInput, StagedExternalFileDto,
    },
    paths::AppPaths,
    platform::{
        ExternalAcquireResult, ExternalExportSource, ExternalFileRequest, ExternalImportTarget,
        ExternalPublishResult, FileFilter, PlatformRuntime,
    },
    util::{unix_time_millis, validate_entity_id},
};

const IMPORT_TTL_MS: i64 = 7 * 24 * 60 * 60 * 1_000;
const COPY_BUFFER_BYTES: usize = 1024 * 1024;
const MAX_IMAGE_BYTES: u64 = 25 * 1024 * 1024;
const MAX_DOCUMENT_BYTES: u64 = 512 * 1024 * 1024;
const MAX_STUDY_CONTENT_BYTES: u64 = 640 * 1024 * 1024;
const MAX_BACKUP_BYTES: u64 = 8 * 1024 * 1024 * 1024;

#[derive(Debug)]
struct StoredPreparedTransfer {
    id: String,
    suggested_filename: String,
    media_type: String,
    byte_size: i64,
    relative_path: String,
}

#[derive(Debug)]
struct StagedImportRow {
    id: String,
    purpose: String,
    kind: String,
    display_name: String,
    byte_size: i64,
    sha256: String,
    relative_path: String,
    expires_at: i64,
    state: String,
}

#[derive(Debug)]
struct PreparedTransferRow {
    suggested_filename: String,
    media_type: String,
    byte_size: i64,
    sha256: String,
    relative_path: String,
    state: String,
}

#[tauri::command]
pub async fn stream_select_and_stage_external_import(
    app: AppHandle,
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    runtime: State<'_, PlatformRuntime>,
    input: StageExternalImportInput,
) -> Result<Option<StagedExternalFileDto>, CommandError> {
    validate_entity_id(&input.operation_id, "external file selection")?;
    if let Some(existing) = database.with_connection(|connection| {
        load_import_by_operation(connection, paths.inner(), &input.operation_id)
    })? {
        return Ok(Some(existing));
    }

    let partial_relative = AppPaths::staged_import_partial_relative_path(&input.operation_id);
    let partial_path = paths.resolve_relative(&partial_relative);
    create_parent(&partial_path)?;
    remove_file_if_exists(&partial_path)?;

    let request = import_request(input.purpose);
    let maximum_bytes = maximum_import_bytes(input.purpose);
    let acquired = runtime
        .external_files()
        .acquire_import(
            app,
            request,
            ExternalImportTarget {
                path: partial_path.clone(),
                maximum_bytes,
            },
        )
        .await;

    let acquired = match acquired {
        Ok(Some(acquired)) => acquired,
        Ok(None) => {
            remove_file_if_exists(&partial_path)?;
            return Ok(None);
        }
        Err(error) => {
            let _ = remove_file_if_exists(&partial_path);
            return Err(error);
        }
    };

    let staged = finalize_acquired_import(
        database.inner(),
        paths.inner(),
        input.operation_id,
        input.purpose,
        partial_path.clone(),
        acquired,
    );
    if staged.is_err() {
        let _ = remove_file_if_exists(&partial_path);
    }
    staged.map(Some)
}

#[tauri::command]
pub async fn stream_publish_prepared_export(
    app: AppHandle,
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    runtime: State<'_, PlatformRuntime>,
    input: PublishPreparedExportInput,
) -> Result<Option<PublishReceiptDto>, CommandError> {
    validate_entity_id(&input.prepared_export_id, "prepared export")?;
    let stored = database.with_connection(|connection| {
        load_prepared_transfer(connection, paths.inner(), &input.prepared_export_id)
    })?;
    let source_path = paths.resolve_relative(&stored.relative_path);

    let expected_bytes = u64::try_from(stored.byte_size).map_err(|_| {
        CommandError::new(
            "prepared_export_size_invalid",
            "The prepared export has an invalid recorded size.",
        )
    })?;
    let request = export_request(&stored.suggested_filename);
    let published = runtime
        .external_files()
        .publish_export(
            app,
            request,
            ExternalExportSource {
                path: source_path,
                display_name: stored.suggested_filename.clone(),
                media_type: stored.media_type,
                maximum_bytes: expected_bytes,
            },
        )
        .await?;

    let Some(published) = published else {
        return Ok(None);
    };
    let (display_name, destination_kind) = validate_publish_result(expected_bytes, &published)?;
    let completed_at = unix_time_millis()?;
    database.with_connection(|connection| {
        let changed = connection
            .execute(
                "UPDATE prepared_exports
                 SET state = 'published', published_at = ?1, updated_at = ?1
                 WHERE id = ?2 AND state IN ('prepared', 'published')",
                params![completed_at, stored.id],
            )
            .map_err(CommandError::database)?;
        if changed != 1 {
            return Err(CommandError::new(
                "prepared_export_state_changed",
                "The prepared export changed while it was being published.",
            ));
        }
        Ok(())
    })?;

    Ok(Some(PublishReceiptDto {
        prepared_export_id: input.prepared_export_id,
        display_name,
        destination_kind,
        completed_at,
    }))
}

fn finalize_acquired_import(
    database: &Database,
    paths: &AppPaths,
    operation_id: String,
    purpose: ExternalImportPurpose,
    partial_path: PathBuf,
    acquired: ExternalAcquireResult,
) -> Result<StagedExternalFileDto, CommandError> {
    let display_name = validate_display_name(&acquired.display_name)?;
    validate_reported_media_type(acquired.reported_media_type.as_deref())?;
    let (byte_size, sha256) = validate_acquired_file(
        &partial_path,
        maximum_import_bytes(purpose),
        acquired.copied_bytes,
    )?;
    let detected_kind = detect_kind(&partial_path, purpose, &display_name)?;
    let relative_path =
        AppPaths::staged_import_relative_path(&operation_id, detected_kind.extension());
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
    });
    if let Err(error) = insert {
        if let Some(existing) = database.with_connection(|connection| {
            load_import_by_operation(connection, paths, &operation_id)
        })? {
            remove_file_if_exists(&partial_path)?;
            return Ok(existing);
        }
        return Err(error);
    }

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
                "The selected file changed while it was being staged.",
            ));
        }
        Ok(())
    })?;

    Ok(StagedExternalFileDto {
        id: operation_id,
        purpose,
        detected_kind,
        display_name,
        media_type: detected_kind.media_type().to_string(),
        byte_size,
        sha256,
        expires_at,
    })
}

fn import_request(purpose: ExternalImportPurpose) -> ExternalFileRequest {
    match purpose {
        ExternalImportPurpose::Image => ExternalFileRequest {
            title: "Choose an image".to_string(),
            suggested_filename: None,
            filters: vec![FileFilter {
                name: "Images".to_string(),
                extensions: vec![
                    "png".to_string(),
                    "jpg".to_string(),
                    "jpeg".to_string(),
                    "gif".to_string(),
                    "webp".to_string(),
                ],
            }],
        },
        ExternalImportPurpose::PdfDocument => ExternalFileRequest {
            title: "Choose a PDF document".to_string(),
            suggested_filename: None,
            filters: vec![FileFilter {
                name: "PDF documents".to_string(),
                extensions: vec!["pdf".to_string()],
            }],
        },
        ExternalImportPurpose::Document => ExternalFileRequest {
            title: "Choose a document".to_string(),
            suggested_filename: None,
            filters: vec![FileFilter {
                name: "Supported documents".to_string(),
                extensions: [
                    "pdf", "doc", "docx", "docm", "ppt", "pps", "pot", "pptx", "pptm", "ppsx",
                    "potx", "odt", "odp", "rtf", "epub", "xls", "xlsx", "xlsm", "xlsb", "ods",
                    "csv",
                ]
                .into_iter()
                .map(str::to_string)
                .collect(),
            }],
        },
        ExternalImportPurpose::StudyContent => ExternalFileRequest {
            title: "Import flashcards".to_string(),
            suggested_filename: None,
            filters: vec![FileFilter {
                name: "ostudy, Anki deck, or generated JSON".to_string(),
                extensions: vec!["studydeck".to_string(), "apkg".to_string(), "json".to_string()],
            }],
        },
        ExternalImportPurpose::LibraryBackup => ExternalFileRequest {
            title: "Choose an ostudy library backup".to_string(),
            suggested_filename: None,
            filters: vec![FileFilter {
                name: "ostudy library backup".to_string(),
                extensions: vec!["ostudybackup".to_string()],
            }],
        },
    }
}

fn export_request(suggested_filename: &str) -> ExternalFileRequest {
    let extension = Path::new(suggested_filename)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_string();
    ExternalFileRequest {
        title: "Publish ostudy export".to_string(),
        suggested_filename: Some(suggested_filename.to_string()),
        filters: vec![FileFilter {
            name: "ostudy export".to_string(),
            extensions: if extension.is_empty() {
                Vec::new()
            } else {
                vec![extension]
            },
        }],
    }
}

fn maximum_import_bytes(purpose: ExternalImportPurpose) -> u64 {
    match purpose {
        ExternalImportPurpose::Image => MAX_IMAGE_BYTES,
        ExternalImportPurpose::PdfDocument | ExternalImportPurpose::Document => MAX_DOCUMENT_BYTES,
        ExternalImportPurpose::StudyContent => MAX_STUDY_CONTENT_BYTES,
        ExternalImportPurpose::LibraryBackup => MAX_BACKUP_BYTES,
    }
}

fn validate_acquired_file(
    path: &Path,
    maximum_bytes: u64,
    reported_bytes: u64,
) -> Result<(i64, String), CommandError> {
    let metadata = fs::metadata(path).map_err(CommandError::io)?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > maximum_bytes {
        return Err(CommandError::new(
            "external_file_size_invalid",
            "The selected file is empty or exceeds the supported size limit.",
        ));
    }
    if metadata.len() != reported_bytes {
        return Err(CommandError::new(
            "external_file_transfer_mismatch",
            "The platform file adapter reported a different byte count than the staged file.",
        ));
    }
    hash_file(path)
}

fn validate_publish_result(
    expected_bytes: u64,
    published: &ExternalPublishResult,
) -> Result<(String, String), CommandError> {
    if published.copied_bytes != expected_bytes {
        return Err(CommandError::new(
            "external_file_transfer_mismatch",
            "The platform file adapter reported an incomplete export.",
        ));
    }
    let display_name = validate_display_name(&published.display_name)?;
    let destination_kind = published.destination_kind.trim();
    if destination_kind.is_empty()
        || destination_kind.chars().count() > 80
        || destination_kind.chars().any(char::is_control)
    {
        return Err(CommandError::new(
            "invalid_export_destination_kind",
            "The platform export destination is invalid.",
        ));
    }
    Ok((display_name, destination_kind.to_string()))
}

fn validate_reported_media_type(value: Option<&str>) -> Result<(), CommandError> {
    let Some(value) = value else {
        return Ok(());
    };
    if value.chars().count() > 255 || value.chars().any(char::is_control) {
        return Err(CommandError::new(
            "invalid_external_media_type",
            "The selected file reported an invalid media type.",
        ));
    }
    Ok(())
}

fn load_import_by_operation(
    connection: &Connection,
    paths: &AppPaths,
    operation_id: &str,
) -> Result<Option<StagedExternalFileDto>, CommandError> {
    let row = connection
        .query_row(
            "SELECT id, purpose, detected_kind, display_name, byte_size, sha256,
                    relative_path, expires_at, state
             FROM staged_external_imports WHERE operation_id = ?1",
            [operation_id],
            |row| {
                Ok(StagedImportRow {
                    id: row.get(0)?,
                    purpose: row.get(1)?,
                    kind: row.get(2)?,
                    display_name: row.get(3)?,
                    byte_size: row.get(4)?,
                    sha256: row.get(5)?,
                    relative_path: row.get(6)?,
                    expires_at: row.get(7)?,
                    state: row.get(8)?,
                })
            },
        )
        .optional()
        .map_err(CommandError::database)?;
    let Some(row) = row else {
        return Ok(None);
    };
    if !matches!(row.state.as_str(), "staged" | "consumed") {
        return Err(CommandError::new(
            "external_file_operation_conflict",
            "That external-file operation identifier is no longer reusable.",
        ));
    }
    verify_file(
        &paths.resolve_relative(&row.relative_path),
        row.byte_size,
        &row.sha256,
    )?;
    let detected_kind = ExternalFileKind::parse(&row.kind)?;
    Ok(Some(StagedExternalFileDto {
        id: row.id,
        purpose: parse_purpose(&row.purpose)?,
        detected_kind,
        display_name: row.display_name,
        media_type: detected_kind.media_type().to_string(),
        byte_size: row.byte_size,
        sha256: row.sha256,
        expires_at: row.expires_at,
    }))
}

fn load_prepared_transfer(
    connection: &Connection,
    paths: &AppPaths,
    id: &str,
) -> Result<StoredPreparedTransfer, CommandError> {
    let row = connection
        .query_row(
            "SELECT suggested_filename, media_type, byte_size, sha256, relative_path, state
             FROM prepared_exports WHERE id = ?1",
            [id],
            |row| {
                Ok(PreparedTransferRow {
                    suggested_filename: row.get(0)?,
                    media_type: row.get(1)?,
                    byte_size: row.get(2)?,
                    sha256: row.get(3)?,
                    relative_path: row.get(4)?,
                    state: row.get(5)?,
                })
            },
        )
        .optional()
        .map_err(CommandError::database)?;
    let Some(row) = row else {
        return Err(CommandError::new(
            "prepared_export_not_found",
            "The prepared export is no longer available.",
        ));
    };
    if !matches!(row.state.as_str(), "prepared" | "published") {
        return Err(CommandError::new(
            "prepared_export_unavailable",
            "The prepared export is no longer available.",
        ));
    }
    verify_file(
        &paths.resolve_relative(&row.relative_path),
        row.byte_size,
        &row.sha256,
    )?;
    Ok(StoredPreparedTransfer {
        id: id.to_string(),
        suggested_filename: validate_display_name(&row.suggested_filename)?,
        media_type: validate_media_type(&row.media_type)?,
        byte_size: row.byte_size,
        relative_path: row.relative_path,
    })
}

fn detect_kind(
    path: &Path,
    purpose: ExternalImportPurpose,
    display_name: &str,
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
        ExternalImportPurpose::Document => detect_document_kind(path, display_name),
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

fn detect_document_kind(path: &Path, display_name: &str) -> Result<ExternalFileKind, CommandError> {
    let bytes = fs::read(path).map_err(CommandError::io)?;
    let format = document_ingestion::detected_document_format(&bytes, display_name)?;
    match format {
        DocumentFormat::Pdf => Ok(ExternalFileKind::Pdf),
        DocumentFormat::Doc => Ok(ExternalFileKind::Doc),
        DocumentFormat::Docx => Ok(ExternalFileKind::Docx),
        DocumentFormat::Ppt => Ok(ExternalFileKind::Ppt),
        DocumentFormat::Pptx => Ok(ExternalFileKind::Pptx),
        DocumentFormat::Odt => Ok(ExternalFileKind::Odt),
        DocumentFormat::Odp => Ok(ExternalFileKind::Odp),
        DocumentFormat::Rtf => Ok(ExternalFileKind::Rtf),
        DocumentFormat::Epub => Ok(ExternalFileKind::Epub),
        DocumentFormat::Excel => Ok(ExternalFileKind::Excel),
        DocumentFormat::Ods => Ok(ExternalFileKind::Ods),
        DocumentFormat::Csv => Ok(ExternalFileKind::Csv),
        _ => Err(CommandError::new(
            "unsupported_document_format",
            "The selected file is not a supported document type.",
        )),
    }
}

fn detect_study_content(path: &Path) -> Result<ExternalFileKind, CommandError> {
    let mut signature = [0_u8; 16];
    let read = File::open(path)
        .and_then(|mut file| file.read(&mut signature))
        .map_err(CommandError::io)?;
    if signature[..read].starts_with(b"PK\x03\x04") {
        // Staging identifies the container only. The Anki adapter validates all
        // members, collection versions, and content before importing anything.
        return Ok(ExternalFileKind::AnkiApkg);
    }
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

    let file = File::open(path).map_err(CommandError::io)?;
    let value: serde_json::Value = serde_json::from_reader(file).map_err(|_| {
        CommandError::new(
            "invalid_flashcard_import_json",
            "Select an ostudy deck, an Anki .apkg deck, or valid generated JSON.",
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

fn parse_purpose(value: &str) -> Result<ExternalImportPurpose, CommandError> {
    match value {
        "image" => Ok(ExternalImportPurpose::Image),
        "pdf_document" => Ok(ExternalImportPurpose::PdfDocument),
        "document" => Ok(ExternalImportPurpose::Document),
        "study_content" => Ok(ExternalImportPurpose::StudyContent),
        "library_backup" => Ok(ExternalImportPurpose::LibraryBackup),
        _ => Err(CommandError::new(
            "invalid_external_file_purpose",
            "The staged file purpose is invalid.",
        )),
    }
}

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

fn validate_media_type(value: &str) -> Result<String, CommandError> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.chars().count() > 255 || trimmed.chars().any(char::is_control)
    {
        return Err(CommandError::new(
            "invalid_external_media_type",
            "The prepared export has an invalid media type.",
        ));
    }
    Ok(trimmed.to_string())
}

fn hash_file(path: &Path) -> Result<(i64, String), CommandError> {
    let mut file = File::open(path).map_err(CommandError::io)?;
    let mut hasher = Sha256::new();
    let mut total = 0_u64;
    let mut buffer = vec![0_u8; COPY_BUFFER_BYTES];
    loop {
        let read = file.read(&mut buffer).map_err(CommandError::io)?;
        if read == 0 {
            break;
        }
        total = total.saturating_add(read as u64);
        hasher.update(&buffer[..read]);
    }
    let byte_size = i64::try_from(total).map_err(|_| {
        CommandError::new(
            "external_file_size_invalid",
            "The selected file exceeds the supported integer range.",
        )
    })?;
    Ok((byte_size, format!("{:x}", hasher.finalize())))
}

fn verify_file(path: &Path, expected_size: i64, expected_hash: &str) -> Result<(), CommandError> {
    if !path.is_file() {
        return Err(CommandError::new(
            "external_file_missing",
            "The managed transfer file is missing.",
        ));
    }
    let metadata = fs::metadata(path).map_err(CommandError::io)?;
    if i64::try_from(metadata.len()).unwrap_or(i64::MAX) != expected_size {
        return Err(CommandError::new(
            "external_file_changed",
            "The managed transfer file no longer matches its recorded size.",
        ));
    }
    let (_, actual_hash) = hash_file(path)?;
    if actual_hash != expected_hash {
        return Err(CommandError::new(
            "external_file_changed",
            "The managed transfer file no longer matches its recorded content.",
        ));
    }
    Ok(())
}

fn create_parent(path: &Path) -> Result<(), CommandError> {
    let parent = path.parent().ok_or_else(|| {
        CommandError::new(
            "invalid_external_file_path",
            "The managed transfer path has no parent folder.",
        )
    })?;
    fs::create_dir_all(parent).map_err(CommandError::io)
}

fn remove_file_if_exists(path: &Path) -> Result<(), CommandError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(CommandError::io(error)),
    }
}

fn sync_file(path: &Path) -> Result<(), CommandError> {
    File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(CommandError::io)
}

#[cfg(unix)]
fn sync_directory(path: Option<&Path>) -> Result<(), CommandError> {
    if let Some(path) = path {
        File::open(path)
            .and_then(|directory| directory.sync_all())
            .map_err(CommandError::io)?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn sync_directory(_path: Option<&Path>) -> Result<(), CommandError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };

    use super::*;

    static NEXT_FILE: AtomicU64 = AtomicU64::new(1);

    #[test]
    fn every_import_purpose_has_a_bounded_platform_request() {
        for purpose in [
            ExternalImportPurpose::Image,
            ExternalImportPurpose::PdfDocument,
            ExternalImportPurpose::Document,
            ExternalImportPurpose::StudyContent,
            ExternalImportPurpose::LibraryBackup,
        ] {
            let request = import_request(purpose);
            assert!(!request.title.is_empty());
            assert!(!request.filters.is_empty());
            assert!(maximum_import_bytes(purpose) > 0);
        }
    }

    #[test]
    fn anki_containers_are_staged_with_the_anki_kind_and_picker_extension() {
        let path = test_file(b"PK\x03\x04container");
        assert_eq!(detect_study_content(&path).unwrap(), ExternalFileKind::AnkiApkg);
        let request = import_request(ExternalImportPurpose::StudyContent);
        assert!(request.filters.iter().any(|filter| filter.extensions.iter().any(|extension| extension == "apkg")));
        assert_eq!(ExternalFileKind::parse("anki_apkg").unwrap().extension(), "apkg");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn generic_document_detection_is_content_authoritative() {
        let pdf = test_file(b"%PDF-1.7\n");
        assert_eq!(
            detect_kind(&pdf, ExternalImportPurpose::Document, "renamed.docx").unwrap(),
            ExternalFileKind::Pdf
        );
        let _ = fs::remove_file(pdf);

        let disguised = test_file(b"not an OOXML package");
        let error =
            detect_kind(&disguised, ExternalImportPurpose::Document, "renamed.docx").unwrap_err();
        assert_eq!(error.code, "unsupported_document_format");
        let _ = fs::remove_file(disguised);
    }

    #[test]
    fn csv_detection_uses_the_authorized_filename_hint() {
        let csv = test_file(b"name,value\na,1\n");
        assert_eq!(
            detect_kind(&csv, ExternalImportPurpose::Document, "table.csv").unwrap(),
            ExternalFileKind::Csv
        );
        let _ = fs::remove_file(csv);
    }

    #[test]
    fn rejects_adapter_byte_count_mismatches_before_ledger_commit() {
        let path = test_file(b"android-stream");
        let error = validate_acquired_file(&path, 1024, 3).unwrap_err();
        assert_eq!(error.code, "external_file_transfer_mismatch");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn rejects_incomplete_publication_receipts() {
        let result = ExternalPublishResult {
            display_name: "deck.studydeck".to_string(),
            copied_bytes: 9,
            destination_kind: "android_document_provider".to_string(),
        };
        let error = validate_publish_result(10, &result).unwrap_err();
        assert_eq!(error.code, "external_file_transfer_mismatch");
    }

    #[test]
    fn accepts_a_complete_publication_receipt() {
        let result = ExternalPublishResult {
            display_name: "deck.studydeck".to_string(),
            copied_bytes: 10,
            destination_kind: "android_document_provider".to_string(),
        };
        assert_eq!(
            validate_publish_result(10, &result).unwrap(),
            (
                "deck.studydeck".to_string(),
                "android_document_provider".to_string(),
            )
        );
    }

    fn test_file(bytes: &[u8]) -> PathBuf {
        let sequence = NEXT_FILE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "ostudy-external-stream-{}-{sequence}.tmp",
            std::process::id()
        ));
        fs::write(&path, bytes).unwrap();
        path
    }
}
