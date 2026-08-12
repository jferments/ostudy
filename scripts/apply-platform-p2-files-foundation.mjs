#!/usr/bin/env node

import { createHash } from "node:crypto";
import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const read = (...parts) => readFile(join(root, ...parts), "utf8");
const write = async (path, content) => {
  const target = join(root, path);
  await mkdir(dirname(target), { recursive: true });
  await writeFile(target, content, "utf8");
};
const replace = async (path, from, to) => {
  const source = await read(path);
  if (!source.includes(from)) throw new Error(`${path}: source fragment not found`);
  await write(path, source.replace(from, to));
};

const migration = `-- Opaque external-file acquisition and prepared export publication.
--
-- Native platform references are never durable application values. Selected
-- bytes are copied into app-private staging and exports are prepared privately
-- before the operating system is asked where to publish them.

CREATE TABLE staged_external_imports (
    id              TEXT PRIMARY KEY CHECK(length(id) = 32),
    operation_id    TEXT NOT NULL UNIQUE CHECK(length(operation_id) = 32),
    purpose         TEXT NOT NULL CHECK(purpose IN (
                        'image', 'pdf_document', 'study_content', 'library_backup'
                    )),
    detected_kind   TEXT NOT NULL CHECK(detected_kind IN (
                        'png', 'jpeg', 'gif', 'webp', 'pdf',
                        'studydeck', 'generated_json', 'ostudybackup'
                    )),
    display_name    TEXT NOT NULL,
    media_type      TEXT NOT NULL,
    byte_size       INTEGER NOT NULL CHECK(byte_size > 0),
    sha256          TEXT NOT NULL CHECK(length(sha256) = 64),
    relative_path   TEXT NOT NULL UNIQUE,
    state           TEXT NOT NULL CHECK(state IN (
                        'staging', 'staged', 'consumed', 'released', 'failed'
                    )),
    result_json     TEXT NOT NULL DEFAULT '',
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL,
    expires_at      INTEGER NOT NULL,
    consumed_at     INTEGER,
    released_at     INTEGER,
    CHECK(expires_at >= created_at),
    CHECK(consumed_at IS NULL OR consumed_at >= created_at),
    CHECK(released_at IS NULL OR released_at >= created_at)
) STRICT;

CREATE INDEX staged_external_imports_state_expiry_idx
    ON staged_external_imports(state, expires_at);

CREATE TABLE prepared_exports (
    id                  TEXT PRIMARY KEY CHECK(length(id) = 32),
    operation_id        TEXT NOT NULL UNIQUE CHECK(length(operation_id) = 32),
    purpose             TEXT NOT NULL CHECK(purpose IN ('studydeck', 'library_backup')),
    suggested_filename  TEXT NOT NULL,
    media_type          TEXT NOT NULL,
    byte_size           INTEGER NOT NULL CHECK(byte_size > 0),
    sha256              TEXT NOT NULL CHECK(length(sha256) = 64),
    relative_path       TEXT NOT NULL UNIQUE,
    state               TEXT NOT NULL CHECK(state IN (
                            'prepared', 'published', 'released', 'failed'
                        )),
    summary_json        TEXT NOT NULL,
    created_at          INTEGER NOT NULL,
    updated_at          INTEGER NOT NULL,
    expires_at          INTEGER NOT NULL,
    published_at        INTEGER,
    released_at         INTEGER,
    CHECK(expires_at >= created_at),
    CHECK(published_at IS NULL OR published_at >= created_at),
    CHECK(released_at IS NULL OR released_at >= created_at)
) STRICT;

CREATE INDEX prepared_exports_state_expiry_idx
    ON prepared_exports(state, expires_at);
`;
await write("src-tauri/migrations/0024_external_file_transfers.sql", migration);
const migrationChecksum = createHash("sha256").update(migration).digest("hex");

await write("src-tauri/src/external_files.rs", String.raw`//! Opaque external-file staging and prepared-export publication.
//!
//! Platform-specific paths, content URIs, and security-scoped references stop at
//! `ExternalFilePort`. Shared use cases receive only managed transfer IDs and
//! validated app-private files.

use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};

use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, State};

use crate::{
    assets::detect_image_bytes,
    database::Database,
    error::CommandError,
    paths::AppPaths,
    platform::{ExternalFileRequest, FileFilter, PlatformRuntime},
    util::{unix_time_millis, validate_entity_id},
};

const IMPORT_TTL_MS: i64 = 7 * 24 * 60 * 60 * 1_000;
const EXPORT_TTL_MS: i64 = 7 * 24 * 60 * 60 * 1_000;
const COPY_BUFFER_BYTES: usize = 1024 * 1024;
const MAX_IMAGE_BYTES: u64 = 25 * 1024 * 1024;
const MAX_PDF_BYTES: u64 = 512 * 1024 * 1024;
const MAX_STUDY_CONTENT_BYTES: u64 = 640 * 1024 * 1024;
const MAX_BACKUP_BYTES: u64 = 8 * 1024 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ExternalImportPurpose {
    Image,
    PdfDocument,
    StudyContent,
    LibraryBackup,
}

impl ExternalImportPurpose {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::PdfDocument => "pdf_document",
            Self::StudyContent => "study_content",
            Self::LibraryBackup => "library_backup",
        }
    }

    fn maximum_bytes(self) -> u64 {
        match self {
            Self::Image => MAX_IMAGE_BYTES,
            Self::PdfDocument => MAX_PDF_BYTES,
            Self::StudyContent => MAX_STUDY_CONTENT_BYTES,
            Self::LibraryBackup => MAX_BACKUP_BYTES,
        }
    }

    fn request(self) -> ExternalFileRequest {
        match self {
            Self::Image => ExternalFileRequest {
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
            Self::PdfDocument => ExternalFileRequest {
                title: "Choose a PDF document".to_string(),
                suggested_filename: None,
                filters: vec![FileFilter {
                    name: "PDF documents".to_string(),
                    extensions: vec!["pdf".to_string()],
                }],
            },
            Self::StudyContent => ExternalFileRequest {
                title: "Import ostudy content".to_string(),
                suggested_filename: None,
                filters: vec![FileFilter {
                    name: "ostudy deck or generated JSON".to_string(),
                    extensions: vec!["studydeck".to_string(), "json".to_string()],
                }],
            },
            Self::LibraryBackup => ExternalFileRequest {
                title: "Choose an ostudy library backup".to_string(),
                suggested_filename: None,
                filters: vec![FileFilter {
                    name: "ostudy library backup".to_string(),
                    extensions: vec!["ostudybackup".to_string()],
                }],
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ExternalFileKind {
    Png,
    Jpeg,
    Gif,
    Webp,
    Pdf,
    Studydeck,
    GeneratedJson,
    Ostudybackup,
}

impl ExternalFileKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpeg",
            Self::Gif => "gif",
            Self::Webp => "webp",
            Self::Pdf => "pdf",
            Self::Studydeck => "studydeck",
            Self::GeneratedJson => "generated_json",
            Self::Ostudybackup => "ostudybackup",
        }
    }

    fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::Gif => "gif",
            Self::Webp => "webp",
            Self::Pdf => "pdf",
            Self::Studydeck => "studydeck",
            Self::GeneratedJson => "json",
            Self::Ostudybackup => "ostudybackup",
        }
    }

    fn media_type(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Gif => "image/gif",
            Self::Webp => "image/webp",
            Self::Pdf => "application/pdf",
            Self::Studydeck => "application/vnd.ostudy.studydeck",
            Self::GeneratedJson => "application/json",
            Self::Ostudybackup => "application/vnd.ostudy.backup",
        }
    }

    fn parse(value: &str) -> Result<Self, CommandError> {
        match value {
            "png" => Ok(Self::Png),
            "jpeg" => Ok(Self::Jpeg),
            "gif" => Ok(Self::Gif),
            "webp" => Ok(Self::Webp),
            "pdf" => Ok(Self::Pdf),
            "studydeck" => Ok(Self::Studydeck),
            "generated_json" => Ok(Self::GeneratedJson),
            "ostudybackup" => Ok(Self::Ostudybackup),
            _ => Err(CommandError::new(
                "invalid_external_file_kind",
                "The staged file type is invalid.",
            )),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StageExternalImportInput {
    pub operation_id: String,
    pub purpose: ExternalImportPurpose,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StagedExternalFileDto {
    pub id: String,
    pub purpose: ExternalImportPurpose,
    pub detected_kind: ExternalFileKind,
    pub display_name: String,
    pub media_type: String,
    pub byte_size: i64,
    pub sha256: String,
    pub expires_at: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReleaseExternalImportInput {
    pub staged_file_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PublishPreparedExportInput {
    pub prepared_export_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReleasePreparedExportInput {
    pub prepared_export_id: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishReceiptDto {
    pub prepared_export_id: String,
    pub display_name: String,
    pub destination_kind: String,
    pub completed_at: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedExportDto {
    pub id: String,
    pub purpose: String,
    pub suggested_filename: String,
    pub media_type: String,
    pub byte_size: i64,
    pub sha256: String,
    pub expires_at: i64,
}

#[derive(Clone, Debug)]
pub(crate) struct ResolvedStagedImport {
    pub id: String,
    pub path: PathBuf,
    pub display_name: String,
    pub detected_kind: ExternalFileKind,
    pub byte_size: i64,
    pub sha256: String,
    pub state: String,
    pub result_json: String,
}

#[derive(Clone, Debug)]
struct StoredPreparedExport {
    dto: PreparedExportDto,
    relative_path: String,
    state: String,
    summary_json: String,
}

#[tauri::command]
pub fn select_and_stage_external_import(
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

    let Some(source) = runtime
        .external_files()
        .select_import(&app, &input.purpose.request())?
    else {
        return Ok(None);
    };
    let source_path = fs::canonicalize(&source.path).map_err(CommandError::io)?;
    if !source_path.is_file() {
        return Err(CommandError::new(
            "external_file_missing",
            "The selected file is no longer available.",
        ));
    }
    let display_name = validate_display_name(&source.display_name)?;
    let partial_relative = AppPaths::staged_import_partial_relative_path(&input.operation_id);
    let partial_path = paths.resolve_relative(&partial_relative);
    create_parent(&partial_path)?;
    remove_file_if_exists(&partial_path)?;
    let (byte_size, sha256) = copy_bounded_with_hash(
        &source_path,
        &partial_path,
        input.purpose.maximum_bytes(),
    )?;
    let detected_kind = detect_kind(&partial_path, input.purpose)?;
    let relative_path = AppPaths::staged_import_relative_path(
        &input.operation_id,
        detected_kind.extension(),
    );
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
                    input.operation_id,
                    input.purpose.as_str(),
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
        let _ = fs::remove_file(&partial_path);
        if let Some(existing) = database.with_connection(|connection| {
            load_import_by_operation(connection, paths.inner(), &input.operation_id)
        })? {
            return Ok(Some(existing));
        }
        return Err(error);
    }

    if let Err(error) = fs::rename(&partial_path, &ready_path) {
        let _ = database.with_connection(|connection| {
            connection
                .execute(
                    "UPDATE staged_external_imports SET state = 'failed', updated_at = ?1
                     WHERE id = ?2 AND state = 'staging'",
                    params![unix_time_millis().unwrap_or_default(), input.operation_id],
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
                params![unix_time_millis()?, input.operation_id],
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

    Ok(Some(StagedExternalFileDto {
        id: input.operation_id,
        purpose: input.purpose,
        detected_kind,
        display_name,
        media_type: detected_kind.media_type().to_string(),
        byte_size,
        sha256,
        expires_at,
    }))
}

#[tauri::command]
pub fn release_staged_external_import(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: ReleaseExternalImportInput,
) -> Result<(), CommandError> {
    validate_entity_id(&input.staged_file_id, "staged file")?;
    let relative = database.with_connection(|connection| {
        let row: Option<(String, String)> = connection
            .query_row(
                "SELECT relative_path, state FROM staged_external_imports WHERE id = ?1",
                [input.staged_file_id.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(CommandError::database)?;
        let Some((relative, state)) = row else {
            return Ok(None);
        };
        if state == "consumed" {
            return Ok(None);
        }
        connection
            .execute(
                "UPDATE staged_external_imports
                 SET state = 'released', released_at = ?1, updated_at = ?1
                 WHERE id = ?2 AND state <> 'consumed'",
                params![unix_time_millis()?, input.staged_file_id],
            )
            .map_err(CommandError::database)?;
        Ok(Some(relative))
    })?;
    if let Some(relative) = relative {
        remove_file_if_exists(&paths.resolve_relative(&relative))?;
        remove_file_if_exists(&paths.resolve_relative(
            AppPaths::staged_import_partial_relative_path(&input.staged_file_id),
        ))?;
    }
    Ok(())
}

#[tauri::command]
pub fn publish_prepared_export(
    app: AppHandle,
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    runtime: State<'_, PlatformRuntime>,
    input: PublishPreparedExportInput,
) -> Result<Option<PublishReceiptDto>, CommandError> {
    validate_entity_id(&input.prepared_export_id, "prepared export")?;
    let stored = database.with_connection(|connection| {
        load_prepared_export(connection, paths.inner(), &input.prepared_export_id)
    })?;
    let extension = Path::new(&stored.dto.suggested_filename)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_string();
    let request = ExternalFileRequest {
        title: "Publish ostudy export".to_string(),
        suggested_filename: Some(stored.dto.suggested_filename.clone()),
        filters: vec![FileFilter {
            name: "ostudy export".to_string(),
            extensions: if extension.is_empty() {
                Vec::new()
            } else {
                vec![extension]
            },
        }],
    };
    let Some(destination) = runtime.external_files().select_export(&app, &request)? else {
        return Ok(None);
    };
    let source = paths.resolve_relative(&stored.relative_path);
    verify_file(&source, stored.dto.byte_size, &stored.dto.sha256)?;
    publish_atomic(&source, &destination.path)?;
    let completed_at = unix_time_millis()?;
    database.with_connection(|connection| {
        connection
            .execute(
                "UPDATE prepared_exports
                 SET state = 'published', published_at = ?1, updated_at = ?1
                 WHERE id = ?2 AND state IN ('prepared', 'published')",
                params![completed_at, input.prepared_export_id],
            )
            .map_err(CommandError::database)?;
        Ok(())
    })?;
    Ok(Some(PublishReceiptDto {
        prepared_export_id: stored.dto.id,
        display_name: destination.display_name,
        destination_kind: "desktop_file".to_string(),
        completed_at,
    }))
}

#[tauri::command]
pub fn release_prepared_export(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: ReleasePreparedExportInput,
) -> Result<(), CommandError> {
    validate_entity_id(&input.prepared_export_id, "prepared export")?;
    let relative = database.with_connection(|connection| {
        let relative: Option<String> = connection
            .query_row(
                "SELECT relative_path FROM prepared_exports WHERE id = ?1",
                [input.prepared_export_id.as_str()],
                |row| row.get(0),
            )
            .optional()
            .map_err(CommandError::database)?;
        if relative.is_some() {
            connection
                .execute(
                    "UPDATE prepared_exports
                     SET state = 'released', released_at = ?1, updated_at = ?1
                     WHERE id = ?2",
                    params![unix_time_millis()?, input.prepared_export_id],
                )
                .map_err(CommandError::database)?;
        }
        Ok(relative)
    })?;
    if let Some(relative) = relative {
        remove_file_if_exists(&paths.resolve_relative(&relative))?;
    }
    Ok(())
}

pub(crate) fn resolve_staged_import(
    connection: &Connection,
    paths: &AppPaths,
    staged_file_id: &str,
    expected_purpose: ExternalImportPurpose,
    allowed_kinds: &[ExternalFileKind],
) -> Result<ResolvedStagedImport, CommandError> {
    validate_entity_id(staged_file_id, "staged file")?;
    let stored = load_staged_import(connection, staged_file_id)?;
    if stored.1 != expected_purpose.as_str() {
        return Err(CommandError::new(
            "external_file_purpose_mismatch",
            "The staged file was selected for a different operation.",
        ));
    }
    if !allowed_kinds.contains(&stored.0.detected_kind) {
        return Err(CommandError::new(
            "external_file_type_mismatch",
            "The staged file is not a supported type for this operation.",
        ));
    }
    if !matches!(stored.0.state.as_str(), "staged" | "consumed") {
        return Err(CommandError::new(
            "external_file_unavailable",
            "The staged file is no longer available.",
        ));
    }
    let expected_relative = AppPaths::staged_import_relative_path(
        staged_file_id,
        stored.0.detected_kind.extension(),
    );
    if stored.2 != expected_relative {
        return Err(CommandError::new(
            "invalid_external_file_path",
            "The staged file path does not match its transfer identity.",
        ));
    }
    let path = paths.resolve_relative(&stored.2);
    verify_file(&path, stored.0.byte_size, &stored.0.sha256)?;
    Ok(ResolvedStagedImport {
        id: staged_file_id.to_string(),
        path,
        display_name: stored.0.display_name,
        detected_kind: stored.0.detected_kind,
        byte_size: stored.0.byte_size,
        sha256: stored.0.sha256,
        state: stored.0.state,
        result_json: stored.3,
    })
}

pub(crate) fn consumed_import_result<T: DeserializeOwned>(
    connection: &Connection,
    staged_file_id: &str,
) -> Result<Option<T>, CommandError> {
    let row: Option<(String, String)> = connection
        .query_row(
            "SELECT state, result_json FROM staged_external_imports WHERE id = ?1",
            [staged_file_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(CommandError::database)?;
    let Some((state, json)) = row else {
        return Ok(None);
    };
    if state != "consumed" || json.is_empty() {
        return Ok(None);
    }
    serde_json::from_str(&json)
        .map(Some)
        .map_err(CommandError::serialization)
}

pub(crate) fn mark_import_consumed<T: Serialize>(
    connection: &Connection,
    staged_file_id: &str,
    result: &T,
    now: i64,
) -> Result<(), CommandError> {
    let result_json = serde_json::to_string(result).map_err(CommandError::serialization)?;
    let changed = connection
        .execute(
            "UPDATE staged_external_imports
             SET state = 'consumed', result_json = ?1, consumed_at = COALESCE(consumed_at, ?2),
                 updated_at = ?2
             WHERE id = ?3 AND state IN ('staged', 'consumed')",
            params![result_json, now, staged_file_id],
        )
        .map_err(CommandError::database)?;
    if changed != 1 {
        return Err(CommandError::new(
            "external_file_state_changed",
            "The staged file changed before its import could be committed.",
        ));
    }
    Ok(())
}

pub(crate) fn load_prepared_export_summary<T: DeserializeOwned>(
    connection: &Connection,
    paths: &AppPaths,
    operation_id: &str,
    purpose: &str,
) -> Result<Option<(PreparedExportDto, T)>, CommandError> {
    let row: Option<String> = connection
        .query_row(
            "SELECT id FROM prepared_exports WHERE operation_id = ?1 AND purpose = ?2",
            params![operation_id, purpose],
            |row| row.get(0),
        )
        .optional()
        .map_err(CommandError::database)?;
    let Some(id) = row else {
        return Ok(None);
    };
    let stored = load_prepared_export(connection, paths, &id)?;
    let summary = serde_json::from_str(&stored.summary_json)
        .map_err(CommandError::serialization)?;
    Ok(Some((stored.dto, summary)))
}

pub(crate) struct RegisterPreparedExport<'a, T> {
    pub id: &'a str,
    pub purpose: &'a str,
    pub suggested_filename: &'a str,
    pub media_type: &'a str,
    pub relative_path: &'a str,
    pub summary: &'a T,
}

pub(crate) fn register_prepared_export<T: Serialize>(
    connection: &Connection,
    paths: &AppPaths,
    input: RegisterPreparedExport<'_, T>,
) -> Result<PreparedExportDto, CommandError> {
    validate_entity_id(input.id, "prepared export")?;
    let path = paths.resolve_relative(input.relative_path);
    let (byte_size, sha256) = hash_file(&path)?;
    if byte_size <= 0 {
        return Err(CommandError::new(
            "empty_prepared_export",
            "The prepared export is empty.",
        ));
    }
    let now = unix_time_millis()?;
    let expires_at = now.saturating_add(EXPORT_TTL_MS);
    let summary_json = serde_json::to_string(input.summary).map_err(CommandError::serialization)?;
    connection
        .execute(
            "INSERT INTO prepared_exports (
                 id, operation_id, purpose, suggested_filename, media_type,
                 byte_size, sha256, relative_path, state, summary_json,
                 created_at, updated_at, expires_at
             ) VALUES (?1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, 'prepared', ?8, ?9, ?9, ?10)",
            params![
                input.id,
                input.purpose,
                input.suggested_filename,
                input.media_type,
                byte_size,
                sha256,
                input.relative_path,
                summary_json,
                now,
                expires_at,
            ],
        )
        .map_err(CommandError::database)?;
    Ok(PreparedExportDto {
        id: input.id.to_string(),
        purpose: input.purpose.to_string(),
        suggested_filename: input.suggested_filename.to_string(),
        media_type: input.media_type.to_string(),
        byte_size,
        sha256,
        expires_at,
    })
}

pub(crate) fn reconcile_startup(
    database: &Database,
    paths: &AppPaths,
) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    let mut referenced = BTreeSet::<PathBuf>::new();
    database.with_connection(|connection| {
        let imports = {
            let mut statement = connection
                .prepare(
                    "SELECT id, detected_kind, relative_path, state, expires_at
                     FROM staged_external_imports
                     WHERE state <> 'released'",
                )
                .map_err(CommandError::database)?;
            statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, i64>(4)?,
                    ))
                })
                .map_err(CommandError::database)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(CommandError::database)?
        };
        for (id, kind, relative, state, expires_at) in imports {
            let kind = ExternalFileKind::parse(&kind)?;
            let expected = AppPaths::staged_import_relative_path(&id, kind.extension());
            if relative != expected {
                connection
                    .execute(
                        "UPDATE staged_external_imports SET state = 'failed', updated_at = ?1
                         WHERE id = ?2",
                        params![now, id],
                    )
                    .map_err(CommandError::database)?;
                continue;
            }
            let ready = paths.resolve_relative(&relative);
            let partial = paths.resolve_relative(AppPaths::staged_import_partial_relative_path(&id));
            referenced.insert(ready.clone());
            referenced.insert(partial.clone());
            if state == "staging" && !ready.exists() && partial.is_file() {
                fs::rename(&partial, &ready).map_err(CommandError::io)?;
                sync_directory(ready.parent())?;
                connection
                    .execute(
                        "UPDATE staged_external_imports SET state = 'staged', updated_at = ?1
                         WHERE id = ?2 AND state = 'staging'",
                        params![now, id],
                    )
                    .map_err(CommandError::database)?;
            } else if !ready.is_file() {
                connection
                    .execute(
                        "UPDATE staged_external_imports SET state = 'failed', updated_at = ?1
                         WHERE id = ?2",
                        params![now, id],
                    )
                    .map_err(CommandError::database)?;
            }
            if expires_at < now && state != "consumed" {
                let _ = fs::remove_file(&ready);
                let _ = fs::remove_file(&partial);
                connection
                    .execute(
                        "UPDATE staged_external_imports
                         SET state = 'released', released_at = ?1, updated_at = ?1
                         WHERE id = ?2 AND state <> 'consumed'",
                        params![now, id],
                    )
                    .map_err(CommandError::database)?;
            }
        }

        let exports = {
            let mut statement = connection
                .prepare(
                    "SELECT id, relative_path, state, expires_at FROM prepared_exports
                     WHERE state <> 'released'",
                )
                .map_err(CommandError::database)?;
            statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                })
                .map_err(CommandError::database)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(CommandError::database)?
        };
        for (id, relative, _state, expires_at) in exports {
            let path = paths.resolve_relative(&relative);
            referenced.insert(path.clone());
            if !path.is_file() {
                connection
                    .execute(
                        "UPDATE prepared_exports SET state = 'failed', updated_at = ?1
                         WHERE id = ?2",
                        params![now, id],
                    )
                    .map_err(CommandError::database)?;
            }
            if expires_at < now {
                let _ = fs::remove_file(&path);
                connection
                    .execute(
                        "UPDATE prepared_exports
                         SET state = 'released', released_at = ?1, updated_at = ?1
                         WHERE id = ?2",
                        params![now, id],
                    )
                    .map_err(CommandError::database)?;
            }
        }
        Ok(())
    })?;

    remove_unknown_transfer_files(paths.transfer_imports_dir(), &referenced)?;
    remove_unknown_transfer_files(paths.transfer_exports_dir(), &referenced)?;
    Ok(())
}

fn load_import_by_operation(
    connection: &Connection,
    paths: &AppPaths,
    operation_id: &str,
) -> Result<Option<StagedExternalFileDto>, CommandError> {
    let id: Option<String> = connection
        .query_row(
            "SELECT id FROM staged_external_imports WHERE operation_id = ?1",
            [operation_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(CommandError::database)?;
    let Some(id) = id else {
        return Ok(None);
    };
    let stored = load_staged_import(connection, &id)?;
    if !matches!(stored.0.state.as_str(), "staged" | "consumed") {
        return Err(CommandError::new(
            "external_file_operation_conflict",
            "That external-file operation identifier is no longer reusable.",
        ));
    }
    verify_file(
        &paths.resolve_relative(&stored.2),
        stored.0.byte_size,
        &stored.0.sha256,
    )?;
    Ok(Some(StagedExternalFileDto {
        id,
        purpose: parse_purpose(&stored.1)?,
        detected_kind: stored.0.detected_kind,
        display_name: stored.0.display_name,
        media_type: stored.0.detected_kind.media_type().to_string(),
        byte_size: stored.0.byte_size,
        sha256: stored.0.sha256,
        expires_at: stored.4,
    }))
}

struct LoadedImport {
    detected_kind: ExternalFileKind,
    display_name: String,
    byte_size: i64,
    sha256: String,
    state: String,
}

fn load_staged_import(
    connection: &Connection,
    id: &str,
) -> Result<(LoadedImport, String, String, String, i64), CommandError> {
    connection
        .query_row(
            "SELECT detected_kind, display_name, byte_size, sha256, state,
                    purpose, relative_path, result_json, expires_at
             FROM staged_external_imports WHERE id = ?1",
            [id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, i64>(8)?,
                ))
            },
        )
        .optional()
        .map_err(CommandError::database)?
        .ok_or_else(|| {
            CommandError::new(
                "external_file_not_found",
                "The staged file is no longer available.",
            )
        })
        .and_then(|row| {
            Ok((
                LoadedImport {
                    detected_kind: ExternalFileKind::parse(&row.0)?,
                    display_name: row.1,
                    byte_size: row.2,
                    sha256: row.3,
                    state: row.4,
                },
                row.5,
                row.6,
                row.7,
                row.8,
            ))
        })
}

fn load_prepared_export(
    connection: &Connection,
    paths: &AppPaths,
    id: &str,
) -> Result<StoredPreparedExport, CommandError> {
    let row = connection
        .query_row(
            "SELECT purpose, suggested_filename, media_type, byte_size, sha256,
                    relative_path, state, summary_json, expires_at
             FROM prepared_exports WHERE id = ?1",
            [id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, i64>(8)?,
                ))
            },
        )
        .optional()
        .map_err(CommandError::database)?
        .ok_or_else(|| {
            CommandError::new(
                "prepared_export_not_found",
                "The prepared export is no longer available.",
            )
        })?;
    if !matches!(row.6.as_str(), "prepared" | "published") {
        return Err(CommandError::new(
            "prepared_export_unavailable",
            "The prepared export is no longer available.",
        ));
    }
    verify_file(&paths.resolve_relative(&row.5), row.3, &row.4)?;
    Ok(StoredPreparedExport {
        dto: PreparedExportDto {
            id: id.to_string(),
            purpose: row.0,
            suggested_filename: row.1,
            media_type: row.2,
            byte_size: row.3,
            sha256: row.4,
            expires_at: row.8,
        },
        relative_path: row.5,
        state: row.6,
        summary_json: row.7,
    })
}

fn detect_kind(path: &Path, purpose: ExternalImportPurpose) -> Result<ExternalFileKind, CommandError> {
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
                _ => Err(CommandError::new("unsupported_image", "Use a supported image file.")),
            }
        }
        ExternalImportPurpose::PdfDocument => {
            let mut file = File::open(path).map_err(CommandError::io)?;
            let mut signature = [0_u8; 5];
            file.read_exact(&mut signature).map_err(CommandError::io)?;
            if &signature == b"%PDF-" {
                Ok(ExternalFileKind::Pdf)
            } else {
                Err(CommandError::new("invalid_pdf", "The selected file is not a PDF."))
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
        && value.get("formatVersion").and_then(serde_json::Value::as_i64) == Some(1)
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

fn copy_bounded_with_hash(
    source: &Path,
    destination: &Path,
    maximum_bytes: u64,
) -> Result<(i64, String), CommandError> {
    let metadata = fs::metadata(source).map_err(CommandError::io)?;
    if metadata.len() == 0 || metadata.len() > maximum_bytes {
        return Err(CommandError::new(
            "external_file_size_invalid",
            "The selected file is empty or exceeds the supported size limit.",
        ));
    }
    let mut input = File::open(source).map_err(CommandError::io)?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(CommandError::io)?;
    let mut hasher = Sha256::new();
    let mut total = 0_u64;
    let mut buffer = vec![0_u8; COPY_BUFFER_BYTES];
    loop {
        let read = input.read(&mut buffer).map_err(CommandError::io)?;
        if read == 0 {
            break;
        }
        total = total.saturating_add(read as u64);
        if total > maximum_bytes {
            let _ = fs::remove_file(destination);
            return Err(CommandError::new(
                "external_file_size_invalid",
                "The selected file exceeds the supported size limit.",
            ));
        }
        hasher.update(&buffer[..read]);
        output.write_all(&buffer[..read]).map_err(CommandError::io)?;
    }
    output.sync_all().map_err(CommandError::io)?;
    Ok((
        i64::try_from(total).unwrap_or(i64::MAX),
        format!("{:x}", hasher.finalize()),
    ))
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
    Ok((
        i64::try_from(total).unwrap_or(i64::MAX),
        format!("{:x}", hasher.finalize()),
    ))
}

fn verify_file(path: &Path, expected_size: i64, expected_hash: &str) -> Result<(), CommandError> {
    ensure_managed_path(path)?;
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

fn publish_atomic(source: &Path, destination: &Path) -> Result<(), CommandError> {
    let parent = destination.parent().ok_or_else(|| {
        CommandError::new(
            "invalid_export_destination",
            "Choose a valid export destination.",
        )
    })?;
    if !parent.is_dir() {
        return Err(CommandError::new(
            "invalid_export_destination",
            "The selected export folder is no longer available.",
        ));
    }
    let temporary = destination.with_extension(format!("ostudy-partial-{}", std::process::id()));
    remove_file_if_exists(&temporary)?;
    fs::copy(source, &temporary).map_err(CommandError::io)?;
    sync_file(&temporary)?;
    if destination.exists() {
        let backup = destination.with_extension(format!("ostudy-previous-{}", std::process::id()));
        remove_file_if_exists(&backup)?;
        fs::rename(destination, &backup).map_err(CommandError::io)?;
        match fs::rename(&temporary, destination) {
            Ok(()) => {
                let _ = fs::remove_file(backup);
            }
            Err(error) => {
                let _ = fs::rename(backup, destination);
                return Err(CommandError::io(error));
            }
        }
    } else {
        fs::rename(&temporary, destination).map_err(CommandError::io)?;
    }
    sync_directory(Some(parent))
}

fn remove_unknown_transfer_files(
    directory: &Path,
    referenced: &BTreeSet<PathBuf>,
) -> Result<(), CommandError> {
    if !directory.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(directory).map_err(CommandError::io)? {
        let entry = entry.map_err(CommandError::io)?;
        let path = entry.path();
        if path.is_file() && !referenced.contains(&path) {
            fs::remove_file(path).map_err(CommandError::io)?;
        }
    }
    Ok(())
}

fn ensure_managed_path(path: &Path) -> Result<(), CommandError> {
    if path.components().any(|component| {
        matches!(component, Component::ParentDir | Component::RootDir | Component::Prefix(_))
    }) {
        return Err(CommandError::new(
            "invalid_external_file_path",
            "The managed transfer path is invalid.",
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
`);

await write("src-tauri/src/asset_protocol.rs", String.raw`//! Asset-ID-only WebView protocol.

use std::{fs, path::{Component, Path}};

use rusqlite::OptionalExtension;
use tauri::{
    http::{header, Request, Response, StatusCode},
    Manager, Runtime, UriSchemeContext,
};

use crate::{
    database::Database,
    paths::AppPaths,
    util::validate_entity_id,
};

pub fn respond<R: Runtime>(
    context: UriSchemeContext<'_, R>,
    request: Request<Vec<u8>>,
) -> Response<Vec<u8>> {
    if context.webview_label() != "main" {
        return response(StatusCode::FORBIDDEN, "text/plain", b"forbidden".to_vec());
    }
    let asset_id = request.uri().path().trim_matches('/');
    if validate_entity_id(asset_id, "asset").is_err() {
        return response(StatusCode::BAD_REQUEST, "text/plain", b"invalid asset".to_vec());
    }
    let database = context.app_handle().state::<Database>();
    let paths = context.app_handle().state::<AppPaths>();
    let resolved = database.with_connection(|connection| {
        connection
            .query_row(
                "SELECT a.mime_type, a.byte_size,
                        CASE
                          WHEN a.lifecycle_state = 'ready' THEN a.relative_path
                          WHEN a.lifecycle_state = 'staged'
                               AND EXISTS (
                                 SELECT 1 FROM asset_drafts d
                                 WHERE d.id = a.draft_id
                                   AND d.committed_at IS NULL
                                   AND d.canceled_at IS NULL
                               )
                          THEN a.staged_relative_path
                          ELSE NULL
                        END
                 FROM assets a
                 WHERE a.id = ?1 AND a.deleted_at IS NULL",
                [asset_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(crate::error::CommandError::database)
    });
    let Ok(Some((mime_type, expected_size, Some(relative)))) = resolved else {
        return response(StatusCode::NOT_FOUND, "text/plain", b"asset not found".to_vec());
    };
    if !safe_relative_path(&relative) {
        return response(StatusCode::BAD_REQUEST, "text/plain", b"invalid asset path".to_vec());
    }
    let path = paths.resolve_relative(&relative);
    let Ok(bytes) = fs::read(&path) else {
        return response(StatusCode::NOT_FOUND, "text/plain", b"asset missing".to_vec());
    };
    if i64::try_from(bytes.len()).unwrap_or(i64::MAX) != expected_size {
        return response(StatusCode::GONE, "text/plain", b"asset changed".to_vec());
    }
    response(StatusCode::OK, &mime_type, bytes)
}

fn response(status: StatusCode, mime_type: &str, body: Vec<u8>) -> Response<Vec<u8>> {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, mime_type)
        .header(header::CACHE_CONTROL, "private, max-age=300")
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
        .body(body)
        .unwrap_or_else(|_| Response::new(Vec::new()))
}

fn safe_relative_path(value: &str) -> bool {
    let path = Path::new(value);
    !path.is_absolute()
        && !path.components().any(|component| {
            matches!(component, Component::ParentDir | Component::RootDir | Component::Prefix(_))
        })
        && (value.starts_with("assets/images/") || value.starts_with("assets/staging/"))
}
`);

let paths = await read("src-tauri/src/paths.rs");
let pathsWithDirs = paths.replace(
  "        let document_staging_dir = app_data_dir.join(\"documents\").join(\"staging\");\n        for directory in [\n            &images_dir,\n            &staged_assets_dir,\n            &quarantine_dir,\n            &documents_dir,\n            &document_staging_dir,\n        ] {",
  "        let document_staging_dir = app_data_dir.join(\"documents\").join(\"staging\");\n        let transfer_imports_dir = app_data_dir.join(\"transfers\").join(\"imports\");\n        let transfer_exports_dir = app_data_dir.join(\"transfers\").join(\"exports\");\n        for directory in [\n            &images_dir,\n            &staged_assets_dir,\n            &quarantine_dir,\n            &documents_dir,\n            &document_staging_dir,\n            &transfer_imports_dir,\n            &transfer_exports_dir,\n        ] {",
);
if (pathsWithDirs === paths) throw new Error("paths.rs: directory block not found");
const pathMethods = `

    pub fn transfer_imports_dir(&self) -> PathBuf {
        self.app_data_dir.join("transfers").join("imports")
    }

    pub fn transfer_exports_dir(&self) -> PathBuf {
        self.app_data_dir.join("transfers").join("exports")
    }

    pub fn staged_import_partial_relative_path(id: &str) -> String {
        format!("transfers/imports/{id}.partial")
    }

    pub fn staged_import_relative_path(id: &str, extension: &str) -> String {
        format!("transfers/imports/{id}.{extension}")
    }

    pub fn prepared_export_relative_path(id: &str, extension: &str) -> String {
        format!("transfers/exports/{id}.{extension}")
    }
`;
await write(
  "src-tauri/src/paths.rs",
  pathsWithDirs.replace("\n}\n\nfn validate_component", `${pathMethods}\n}\n\nfn validate_component`),
);

let database = await read("src-tauri/src/database.rs");
const registryAnchor = `    Migration {
        version: 23,
        name: "document_provenance_tag_cleanup",
        checksum: "ec70e81cd4f7c5db36793e1ced79d646ab1981ffd2eaee90244908fbc4b71892",
        sql: include_str!("../migrations/0023_document_provenance_tag_cleanup.sql"),
    },
];`;
const registryReplacement = `    Migration {
        version: 23,
        name: "document_provenance_tag_cleanup",
        checksum: "ec70e81cd4f7c5db36793e1ced79d646ab1981ffd2eaee90244908fbc4b71892",
        sql: include_str!("../migrations/0023_document_provenance_tag_cleanup.sql"),
    },
    Migration {
        version: 24,
        name: "external_file_transfers",
        checksum: "${migrationChecksum}",
        sql: include_str!("../migrations/0024_external_file_transfers.sql"),
    },
];`;
if (!database.includes(registryAnchor)) throw new Error("database.rs: migration anchor not found");
await write(
  "src-tauri/src/database.rs",
  database
    .replace(registryAnchor, registryReplacement)
    .replace("LATEST_SCHEMA_VERSION: i64 = 23", "LATEST_SCHEMA_VERSION: i64 = 24"),
);

let lib = await read("src-tauri/src/lib.rs");
let updatedLib = lib
  .replace("mod asset_lifecycle;\n", "mod asset_lifecycle;\nmod asset_protocol;\n")
  .replace("mod error;\n", "mod error;\nmod external_files;\n")
  .replace(
    "    tauri::Builder::default()\n        .plugin(tauri_plugin_dialog::init())",
    "    tauri::Builder::default()\n        .register_uri_scheme_protocol(\"ostudy-asset\", asset_protocol::respond)\n        .plugin(tauri_plugin_dialog::init())",
  )
  .replace(
    "            deletion::finalize_expired_deletions(&database)",
    "            external_files::reconcile_startup(&database, &paths)\n                .map_err(|error| io::Error::other(error.message))?;\n            deletion::finalize_expired_deletions(&database)",
  )
  .replace(
    "            platform::get_platform_capabilities,",
    "            platform::get_platform_capabilities,\n            external_files::select_and_stage_external_import,\n            external_files::release_staged_external_import,\n            external_files::publish_prepared_export,\n            external_files::release_prepared_export,",
  );
if (updatedLib === lib) throw new Error("lib.rs: external file integration failed");
await write("src-tauri/src/lib.rs", updatedLib);

await write("src/ipc/externalFiles.ts", String.raw`import { invoke } from "@tauri-apps/api/core";
import { createOperationId } from "./common";

export type ExternalImportPurpose =
  | "image"
  | "pdf_document"
  | "study_content"
  | "library_backup";

export type ExternalFileKind =
  | "png"
  | "jpeg"
  | "gif"
  | "webp"
  | "pdf"
  | "studydeck"
  | "generated_json"
  | "ostudybackup";

export interface StagedExternalFile {
  id: string;
  purpose: ExternalImportPurpose;
  detectedKind: ExternalFileKind;
  displayName: string;
  mediaType: string;
  byteSize: number;
  sha256: string;
  expiresAt: number;
}

export interface PublishReceipt {
  preparedExportId: string;
  displayName: string;
  destinationKind: string;
  completedAt: number;
}

export async function selectAndStageExternalImport(
  purpose: ExternalImportPurpose,
  operationId = createOperationId(),
): Promise<StagedExternalFile | null> {
  return invoke<StagedExternalFile | null>("select_and_stage_external_import", {
    input: { operationId, purpose },
  });
}

export async function releaseStagedExternalImport(stagedFileId: string): Promise<void> {
  return invoke<void>("release_staged_external_import", { input: { stagedFileId } });
}

export async function publishPreparedExport(
  preparedExportId: string,
): Promise<PublishReceipt | null> {
  return invoke<PublishReceipt | null>("publish_prepared_export", {
    input: { preparedExportId },
  });
}

export async function releasePreparedExport(preparedExportId: string): Promise<void> {
  return invoke<void>("release_prepared_export", { input: { preparedExportId } });
}
`);

await write("src/platform/files.ts", String.raw`import {
  publishPreparedExport,
  selectAndStageExternalImport,
  type ExternalImportPurpose,
  type PublishReceipt,
  type StagedExternalFile,
} from "../ipc/externalFiles";

export function selectExternalImport(
  purpose: ExternalImportPurpose,
): Promise<StagedExternalFile | null> {
  return selectAndStageExternalImport(purpose);
}

export function publishExport(preparedExportId: string): Promise<PublishReceipt | null> {
  return publishPreparedExport(preparedExportId);
}
`);

await write("src/platform/assets.ts", String.raw`import { convertFileSrc } from "@tauri-apps/api/core";

export function managedAssetUrl(assetId: string): string {
  return convertFileSrc(assetId, "ostudy-asset");
}
`);

let tauriConfig = JSON.parse(await read("src-tauri/tauri.conf.json"));
tauriConfig.app.security.csp = tauriConfig.app.security.csp.replace(
  "img-src 'self' asset: http://asset.localhost data:;",
  "img-src 'self' asset: http://asset.localhost ostudy-asset: http://ostudy-asset.localhost https://ostudy-asset.localhost data:;",
);
await write("src-tauri/tauri.conf.json", `${JSON.stringify(tauriConfig, null, 2)}\n`);

await rm(join(root, "scripts", "apply-platform-p2-files-foundation.mjs"), { force: true });
await rm(join(root, ".github", "workflows", "apply-platform-p2-files-foundation.yml"), { force: true });
console.log("Applied P2 external-file foundation.");
