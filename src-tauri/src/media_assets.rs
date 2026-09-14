//! Native audio/video acquisition into the existing managed-asset lifecycle.
//!
//! The platform streaming port owns file selection and byte transfer. This
//! command inspects the acquired bytes, then stages the result into the same
//! draft-owned asset ledger used by card images. No donor path or URL persists.

use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::Path,
};

use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::Deserialize;
use tauri::{AppHandle, State};

use crate::{
    assets::{validate_original_filename, AssetDto},
    database::Database,
    error::CommandError,
    media::{inspect_media_bytes, MediaKind},
    paths::AppPaths,
    platform::{ExternalFileRequest, ExternalImportTarget, FileFilter, PlatformRuntime},
    util::{unix_time_millis, validate_entity_id},
};

const MAX_MEDIA_BYTES: u64 = 25 * 1024 * 1024;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportMediaInput {
    pub operation_id: String,
    pub draft_id: String,
}

#[tauri::command]
pub async fn import_media(
    app: AppHandle,
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    runtime: State<'_, PlatformRuntime>,
    input: ImportMediaInput,
) -> Result<Option<AssetDto>, CommandError> {
    validate_entity_id(&input.operation_id, "media import operation")?;
    validate_entity_id(&input.draft_id, "asset draft")?;
    if let Some(existing) = database.with_connection(|connection| {
        load_existing(connection, &input.operation_id, &input.draft_id)
    })? {
        return Ok(Some(existing));
    }
    database.with_connection(|connection| ensure_active_draft(connection, &input.draft_id))?;

    let partial_relative = AppPaths::staged_import_partial_relative_path(&input.operation_id);
    let partial_path = paths.resolve_relative(&partial_relative);
    create_parent(&partial_path)?;
    remove_file_if_exists(&partial_path)?;
    let request = ExternalFileRequest {
        title: "Choose audio or video".to_string(),
        suggested_filename: None,
        filters: vec![FileFilter {
            name: "Audio and video".to_string(),
            extensions: ["mp3", "aac", "wav", "ogg", "oga", "m4a", "mp4"]
                .into_iter()
                .map(str::to_string)
                .collect(),
        }],
    };
    let acquired = runtime
        .external_files()
        .acquire_import(
            app,
            request,
            ExternalImportTarget {
                path: partial_path.clone(),
                maximum_bytes: MAX_MEDIA_BYTES,
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
    let metadata = fs::metadata(&partial_path).map_err(CommandError::io)?;
    if !metadata.is_file()
        || metadata.len() == 0
        || metadata.len() > MAX_MEDIA_BYTES
        || metadata.len() != acquired.copied_bytes
    {
        let _ = remove_file_if_exists(&partial_path);
        return Err(CommandError::new(
            "media_transfer_invalid",
            "The selected media file was empty, too large, or incompletely transferred.",
        ));
    }
    let bytes = match fs::read(&partial_path) {
        Ok(bytes) => bytes,
        Err(error) => {
            let _ = remove_file_if_exists(&partial_path);
            return Err(CommandError::io(error));
        }
    };
    let inspection = match inspect_media_bytes(&bytes) {
        Ok(inspection) => inspection,
        Err(error) => {
            let _ = remove_file_if_exists(&partial_path);
            return Err(error);
        }
    };
    let original_filename = match validate_original_filename(&acquired.display_name) {
        Ok(name) => name,
        Err(error) => {
            let _ = remove_file_if_exists(&partial_path);
            return Err(error);
        }
    };
    let relative_path = AppPaths::image_relative_path(&input.operation_id, inspection.extension);
    let staged_relative_path = AppPaths::staged_asset_relative_path(
        &input.draft_id,
        &input.operation_id,
        inspection.extension,
    );
    let staged_path = paths.resolve_relative(&staged_relative_path);
    create_parent(&staged_path)?;
    remove_file_if_exists(&staged_path)?;
    write_new_file(&staged_path, &bytes)?;
    remove_file_if_exists(&partial_path)?;

    let now = unix_time_millis()?;
    let byte_size = i64::try_from(bytes.len()).unwrap_or(i64::MAX);
    let event_detail = match inspection.kind {
        MediaKind::Audio => "editor audio staged",
        MediaKind::Video => "editor video staged",
    };
    let result = database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        ensure_active_draft(&transaction, &input.draft_id)?;
        if let Some(existing) = load_existing(&transaction, &input.operation_id, &input.draft_id)? {
            return Ok(existing);
        }
        transaction
            .execute(
                "INSERT INTO assets (
                     id, original_filename, mime_type, byte_size, relative_path,
                     created_at, lifecycle_state, draft_id, staged_relative_path, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'staged', ?7, ?8, ?6)",
                params![
                    input.operation_id,
                    original_filename,
                    inspection.mime_type,
                    byte_size,
                    relative_path,
                    now,
                    input.draft_id,
                    staged_relative_path,
                ],
            )
            .map_err(CommandError::database)?;
        transaction
            .execute(
                "INSERT INTO asset_reconciliation_events (
                     id, asset_id, action, from_path, to_path, detail, occurred_at
                 ) VALUES (lower(hex(randomblob(16))), ?1, 'staged', NULL, ?2, ?3, ?4)",
                params![input.operation_id, staged_relative_path, event_detail, now],
            )
            .map_err(CommandError::database)?;
        let asset = AssetDto {
            id: input.operation_id.clone(),
            original_filename,
            mime_type: inspection.mime_type.to_string(),
            byte_size,
            created_at: now,
        };
        transaction.commit().map_err(CommandError::database)?;
        Ok(asset)
    });
    if result.is_err() {
        let _ = remove_file_if_exists(&staged_path);
    }
    result.map(Some)
}

fn load_existing(
    connection: &rusqlite::Connection,
    asset_id: &str,
    draft_id: &str,
) -> Result<Option<AssetDto>, CommandError> {
    let row = connection
        .query_row(
            "SELECT original_filename, mime_type, byte_size, created_at, draft_id,
                    lifecycle_state, deleted_at
             FROM assets WHERE id = ?1",
            [asset_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, Option<i64>>(6)?,
                ))
            },
        )
        .optional()
        .map_err(CommandError::database)?;
    let Some((original_filename, mime_type, byte_size, created_at, owner, state, deleted_at)) = row
    else {
        return Ok(None);
    };
    if deleted_at.is_some()
        || owner.as_deref() != Some(draft_id)
        || !matches!(state.as_str(), "staged" | "ready")
    {
        return Err(CommandError::new(
            "idempotency_conflict",
            "That media import operation identifier already belongs to another asset.",
        ));
    }
    Ok(Some(AssetDto {
        id: asset_id.to_string(),
        original_filename,
        mime_type,
        byte_size,
        created_at,
    }))
}

fn ensure_active_draft(
    connection: &rusqlite::Connection,
    draft_id: &str,
) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    let row: Option<(i64, Option<i64>, Option<i64>, String)> = connection
        .query_row(
            "SELECT expires_at, committed_at, canceled_at, retention_mode
             FROM asset_drafts WHERE id = ?1",
            [draft_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()
        .map_err(CommandError::database)?;
    match row {
        None => Err(CommandError::new(
            "asset_draft_not_found",
            "This asset draft no longer exists.",
        )),
        Some((_, Some(_), _, _)) => Err(CommandError::new(
            "asset_draft_committed",
            "This asset draft has already been saved.",
        )),
        Some((_, _, Some(_), _)) => Err(CommandError::new(
            "asset_draft_canceled",
            "This asset draft has already been canceled.",
        )),
        Some((expires_at, None, None, retention_mode))
            if retention_mode == "ephemeral" && expires_at < now =>
        {
            Err(CommandError::new(
                "asset_draft_expired",
                "This asset draft expired. Reopen the editor and add the media again.",
            ))
        }
        Some(_) => Ok(()),
    }
}

fn create_parent(path: &Path) -> Result<(), CommandError> {
    let parent = path.parent().ok_or_else(|| {
        CommandError::new(
            "invalid_asset_path",
            "The managed media path has no parent.",
        )
    })?;
    fs::create_dir_all(parent).map_err(CommandError::io)
}

fn write_new_file(path: &Path, bytes: &[u8]) -> Result<(), CommandError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(CommandError::io)?;
    file.write_all(bytes).map_err(CommandError::io)?;
    file.sync_all().map_err(CommandError::io)?;
    sync_directory(path.parent())
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
