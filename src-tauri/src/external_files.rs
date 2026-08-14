//! Opaque external-file staging and prepared-export publication.
//!
//! Platform-specific paths, content URIs, and security-scoped references stop at
//! `ExternalFilePort`. Shared use cases receive only managed transfer IDs and
//! validated app-private files.

use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
};

use rusqlite::{params, Connection, OptionalExtension};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::State;

use crate::{
    database::Database,
    error::CommandError,
    paths::AppPaths,
    util::{unix_time_millis, validate_entity_id},
};

const EXPORT_TTL_MS: i64 = 7 * 24 * 60 * 60 * 1_000;
const COPY_BUFFER_BYTES: usize = 1024 * 1024;

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
    pub path: PathBuf,
    pub display_name: String,
}

#[derive(Clone, Debug)]
struct StoredPreparedExport {
    dto: PreparedExportDto,
    summary_json: String,
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
            &AppPaths::staged_import_partial_relative_path(&input.staged_file_id),
        ))?;
    }
    Ok(())
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
    let expected_relative =
        AppPaths::staged_import_relative_path(staged_file_id, stored.0.detected_kind.extension());
    if stored.2 != expected_relative {
        return Err(CommandError::new(
            "invalid_external_file_path",
            "The staged file path does not match its transfer identity.",
        ));
    }
    let path = paths.resolve_relative(&stored.2);
    verify_file(&path, stored.0.byte_size, &stored.0.sha256)?;
    Ok(ResolvedStagedImport {
        path,
        display_name: stored.0.display_name,
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
    let summary =
        serde_json::from_str(&stored.summary_json).map_err(CommandError::serialization)?;
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

pub(crate) fn reconcile_startup(database: &Database, paths: &AppPaths) -> Result<(), CommandError> {
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
            let partial =
                paths.resolve_relative(&AppPaths::staged_import_partial_relative_path(&id));
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

    remove_unknown_transfer_files(&paths.transfer_imports_dir(), &referenced)?;
    remove_unknown_transfer_files(&paths.transfer_exports_dir(), &referenced)?;
    Ok(())
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
        summary_json: row.7,
    })
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

fn remove_file_if_exists(path: &Path) -> Result<(), CommandError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(CommandError::io(error)),
    }
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
