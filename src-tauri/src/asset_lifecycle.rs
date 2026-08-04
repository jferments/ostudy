//! Durable staged image ingest and conservative filesystem reconciliation.
//!
//! SQLite is the authoritative asset ledger. Files are first copied into a
//! draft-owned staging path, flushed, and then promoted by the same transaction
//! that creates a note, template, or imported deck. Every filesystem boundary is
//! represented in the ledger so startup can repair a crash without guessing or
//! deleting a referenced image.

use std::{
    collections::{BTreeSet, HashSet},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    assets::{detect_image_bytes, validate_original_filename, AssetDto},
    database::Database,
    error::CommandError,
    paths::AppPaths,
    util::{generate_id, unix_time_millis, validate_entity_id},
};

const MAX_IMAGE_BYTES: u64 = 25 * 1024 * 1024;
const DRAFT_TTL_MS: i64 = 24 * 60 * 60 * 1_000;
const RECOVERY_GRACE_MS: i64 = 24 * 60 * 60 * 1_000;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BeginAssetDraftInput {
    pub purpose: String,
    pub owner_id: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetDraftDto {
    pub id: String,
    pub purpose: String,
    pub expires_at: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportImageInput {
    pub file_path: String,
    pub draft_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SyncAssetDraftInput {
    pub draft_id: String,
    #[serde(default)]
    pub asset_ids: Vec<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct DraftFinalization {
    cleanup_paths: Vec<PathBuf>,
}

impl DraftFinalization {
    pub(crate) fn cleanup(self) {
        remove_paths_best_effort(&self.cleanup_paths);
    }
}

#[derive(Clone, Debug)]
struct AssetRecord {
    id: String,
    original_filename: String,
    mime_type: String,
    byte_size: i64,
    relative_path: String,
    lifecycle_state: String,
    draft_id: Option<String>,
    staged_relative_path: Option<String>,
    quarantine_relative_path: Option<String>,
    created_at: i64,
    updated_at: i64,
}

#[derive(Clone, Copy, Debug, Default)]
struct IngestFaults {
    fail_after_copy: bool,
    fail_after_sync: bool,
    fail_before_insert: bool,
}

#[derive(Clone, Copy, Debug, Default)]
struct PromotionFaults {
    fail_after_rename: bool,
}

#[tauri::command]
pub fn begin_asset_draft(
    database: State<'_, Database>,
    input: BeginAssetDraftInput,
) -> Result<AssetDraftDto, CommandError> {
    let purpose = validate_draft_purpose(&input.purpose)?;
    if let Some(owner_id) = input.owner_id.as_deref() {
        validate_entity_id(owner_id, "asset draft owner")?;
    }
    database.with_connection(|connection| {
        begin_draft_in_connection(connection, purpose, input.owner_id.as_deref())
    })
}

#[tauri::command]
pub fn cancel_asset_draft(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    draft_id: String,
) -> Result<(), CommandError> {
    validate_entity_id(&draft_id, "asset draft")?;
    let cleanup = database.with_connection(|connection| {
        abandon_draft_in_connection(connection, &paths, &draft_id, "draft canceled")
    })?;
    remove_paths_best_effort(&cleanup);
    Ok(())
}

#[tauri::command]
pub fn sync_asset_draft(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: SyncAssetDraftInput,
) -> Result<(), CommandError> {
    validate_entity_id(&input.draft_id, "asset draft")?;
    let retained = normalize_asset_ids(input.asset_ids)?;
    let cleanup = database.with_connection(|connection| {
        let now = unix_time_millis()?;
        ensure_active_draft(connection, &input.draft_id, now)?;
        let transaction = connection.transaction().map_err(CommandError::database)?;
        let cleanup = retire_unused_draft_assets(
            &transaction,
            &paths,
            &input.draft_id,
            &retained,
            now,
            "removed before save",
        )?;
        transaction
            .execute(
                "UPDATE asset_drafts SET updated_at = ?1 WHERE id = ?2",
                params![now, input.draft_id],
            )
            .map_err(CommandError::database)?;
        transaction.commit().map_err(CommandError::database)?;
        Ok(cleanup)
    })?;
    remove_paths_best_effort(&cleanup);
    Ok(())
}

pub(crate) fn stage_image(
    database: &Database,
    paths: &AppPaths,
    input: ImportImageInput,
) -> Result<AssetDto, CommandError> {
    validate_entity_id(&input.draft_id, "asset draft")?;
    let source_path = fs::canonicalize(&input.file_path).map_err(CommandError::io)?;
    database.with_connection(|connection| {
        stage_source_in_connection(
            connection,
            paths,
            &source_path,
            &input.draft_id,
            IngestFaults::default(),
        )
    })
}

pub(crate) fn begin_import_draft(
    connection: &mut Connection,
) -> Result<AssetDraftDto, CommandError> {
    begin_draft_in_connection(connection, "import", None)
}

pub(crate) fn stage_import_asset(
    connection: &Connection,
    paths: &AppPaths,
    draft_id: &str,
    asset_id: &str,
    original_filename: &str,
    mime_type: &str,
    data: &[u8],
) -> Result<AssetDto, CommandError> {
    validate_entity_id(draft_id, "asset draft")?;
    validate_entity_id(asset_id, "asset")?;
    ensure_active_draft(connection, draft_id, unix_time_millis()?)?;
    let original_filename = validate_original_filename(original_filename)?;
    if data.is_empty() || data.len() as u64 > MAX_IMAGE_BYTES {
        return Err(CommandError::new(
            "invalid_image",
            "An imported image is empty or exceeds 25 MB.",
        ));
    }
    let (detected_mime, extension) = detect_image_bytes(data)?;
    if detected_mime != mime_type {
        return Err(CommandError::new(
            "asset_type_mismatch",
            "An imported image does not match its declared file type.",
        ));
    }
    let now = unix_time_millis()?;
    let relative_path = AppPaths::image_relative_path(asset_id, extension);
    let staged_relative_path = AppPaths::staged_asset_relative_path(draft_id, asset_id, extension);
    let staged_path = paths.resolve_relative(&staged_relative_path);
    create_parent(&staged_path)?;
    write_new_file(&staged_path, data)?;
    sync_directory(staged_path.parent())?;

    if let Err(error) = connection.execute(
        "INSERT INTO assets (
             id, original_filename, mime_type, byte_size, relative_path, created_at,
             lifecycle_state, draft_id, staged_relative_path, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'staged', ?7, ?8, ?6)",
        params![
            asset_id,
            original_filename,
            mime_type,
            i64::try_from(data.len()).unwrap_or(i64::MAX),
            relative_path,
            now,
            draft_id,
            staged_relative_path,
        ],
    ) {
        let _ = fs::remove_file(&staged_path);
        return Err(CommandError::database(error));
    }
    record_event(
        connection,
        Some(asset_id),
        "staged",
        None,
        Some(&staged_relative_path),
        "import asset staged",
        now,
    )?;
    Ok(AssetDto {
        id: asset_id.to_string(),
        original_filename,
        mime_type: mime_type.to_string(),
        byte_size: i64::try_from(data.len()).unwrap_or(i64::MAX),
        file_path: staged_path.to_string_lossy().into_owned(),
        created_at: now,
    })
}

pub(crate) fn finalize_assets_for_owner(
    transaction: &Transaction<'_>,
    paths: &AppPaths,
    asset_ids: &[String],
    draft_id: Option<&str>,
    owner_id: Option<&str>,
) -> Result<DraftFinalization, CommandError> {
    finalize_assets_with_faults(
        transaction,
        paths,
        asset_ids,
        draft_id,
        owner_id,
        PromotionFaults::default(),
    )
}

pub(crate) fn ensure_assets_available(
    connection: &Connection,
    paths: &AppPaths,
    asset_ids: &[String],
    draft_id: Option<&str>,
) -> Result<(), CommandError> {
    for asset_id in normalize_asset_ids(asset_ids.to_vec())? {
        let asset = load_asset(connection, &asset_id)?;
        match asset.lifecycle_state.as_str() {
            "ready" => {
                let final_path = paths.resolve_relative(&asset.relative_path);
                if !final_path.is_file() {
                    return Err(CommandError::new(
                        "asset_missing",
                        format!(
                            "The image '{}' is missing from the local library.",
                            asset.original_filename
                        ),
                    ));
                }
            }
            "staged" if asset.draft_id.as_deref() == draft_id => {
                let staged = asset.staged_relative_path.as_deref().ok_or_else(|| {
                    CommandError::new("asset_stage_missing", "A staged image has no managed path.")
                })?;
                if !paths.resolve_relative(staged).is_file() {
                    return Err(CommandError::new(
                        "asset_stage_missing",
                        "A staged image is no longer available. Add it again.",
                    ));
                }
            }
            "staged" => {
                return Err(CommandError::new(
                    "asset_draft_mismatch",
                    "A staged image belongs to another editor draft.",
                ));
            }
            "missing" => {
                return Err(CommandError::new(
                    "asset_missing",
                    format!(
                        "The image '{}' is missing from the local library.",
                        asset.original_filename
                    ),
                ));
            }
            "quarantined" => {
                return Err(CommandError::new(
                    "asset_quarantined",
                    "A referenced image was quarantined and must be restored before saving.",
                ));
            }
            _ => {
                return Err(CommandError::new(
                    "asset_not_found",
                    "One of the images used by this content is no longer available.",
                ));
            }
        }
    }
    Ok(())
}

pub(crate) fn reconcile_startup(database: &Database, paths: &AppPaths) -> Result<(), CommandError> {
    let cleanup = database.with_connection(|connection| {
        reconcile_in_connection(connection, paths, unix_time_millis()?)
    })?;
    remove_paths_best_effort(&cleanup);
    Ok(())
}

fn validate_draft_purpose(value: &str) -> Result<&str, CommandError> {
    match value {
        "card" | "template" | "import" => Ok(value),
        _ => Err(CommandError::new(
            "invalid_asset_draft_purpose",
            "Choose a supported image draft purpose.",
        )),
    }
}

fn begin_draft_in_connection(
    connection: &mut Connection,
    purpose: &str,
    owner_id: Option<&str>,
) -> Result<AssetDraftDto, CommandError> {
    let purpose = validate_draft_purpose(purpose)?;
    let now = unix_time_millis()?;
    let id = generate_id(connection)?;
    let expires_at = now.saturating_add(DRAFT_TTL_MS);
    connection
        .execute(
            "INSERT INTO asset_drafts (
                 id, purpose, owner_id, created_at, updated_at, expires_at
             ) VALUES (?1, ?2, ?3, ?4, ?4, ?5)",
            params![id, purpose, owner_id, now, expires_at],
        )
        .map_err(CommandError::database)?;
    Ok(AssetDraftDto {
        id,
        purpose: purpose.to_string(),
        expires_at,
    })
}

fn ensure_active_draft(
    connection: &Connection,
    draft_id: &str,
    now: i64,
) -> Result<String, CommandError> {
    let row: Option<(String, i64, Option<i64>, Option<i64>)> = connection
        .query_row(
            "SELECT purpose, expires_at, committed_at, canceled_at
             FROM asset_drafts WHERE id = ?1",
            [draft_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()
        .map_err(CommandError::database)?;
    match row {
        None => Err(CommandError::new(
            "asset_draft_not_found",
            "This image draft no longer exists.",
        )),
        Some((_, _, Some(_), _)) => Err(CommandError::new(
            "asset_draft_committed",
            "This image draft has already been saved.",
        )),
        Some((_, _, _, Some(_))) => Err(CommandError::new(
            "asset_draft_canceled",
            "This image draft has already been canceled.",
        )),
        Some((_, expires_at, None, None)) if expires_at < now => Err(CommandError::new(
            "asset_draft_expired",
            "This image draft expired. Reopen the editor and add the image again.",
        )),
        Some((purpose, _, None, None)) => Ok(purpose),
    }
}

fn stage_source_in_connection(
    connection: &mut Connection,
    paths: &AppPaths,
    source_path: &Path,
    draft_id: &str,
    faults: IngestFaults,
) -> Result<AssetDto, CommandError> {
    ensure_active_draft(connection, draft_id, unix_time_millis()?)?;
    if !source_path.is_file() {
        return Err(CommandError::new(
            "invalid_image",
            "Select an image file, not a folder.",
        ));
    }
    let metadata = fs::metadata(source_path).map_err(CommandError::io)?;
    if metadata.len() == 0 {
        return Err(CommandError::new(
            "invalid_image",
            "The selected image is empty.",
        ));
    }
    if metadata.len() > MAX_IMAGE_BYTES {
        return Err(CommandError::new(
            "image_too_large",
            "Images cannot exceed 25 MB.",
        ));
    }
    let mut header_file = File::open(source_path).map_err(CommandError::io)?;
    let mut header = [0_u8; 16];
    let bytes_read = header_file.read(&mut header).map_err(CommandError::io)?;
    let (mime_type, extension) = detect_image_bytes(&header[..bytes_read])?;
    let original_filename = validate_original_filename(
        source_path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("image"),
    )?;
    let now = unix_time_millis()?;
    let id = generate_id(connection)?;
    let relative_path = AppPaths::image_relative_path(&id, extension);
    let staged_relative_path = AppPaths::staged_asset_relative_path(draft_id, &id, extension);
    let staged_path = paths.resolve_relative(&staged_relative_path);
    create_parent(&staged_path)?;

    let copy_result = (|| {
        let mut source = File::open(source_path).map_err(CommandError::io)?;
        let mut destination = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staged_path)
            .map_err(CommandError::io)?;
        std::io::copy(&mut source, &mut destination).map_err(CommandError::io)?;
        if faults.fail_after_copy {
            return Err(CommandError::new(
                "asset_test_copy_failure",
                "Injected failure after the staged copy.",
            ));
        }
        destination.sync_all().map_err(CommandError::io)?;
        if faults.fail_after_sync {
            return Err(CommandError::new(
                "asset_test_sync_failure",
                "Injected failure after the staged file flush.",
            ));
        }
        sync_directory(staged_path.parent())?;
        Ok(())
    })();
    if let Err(error) = copy_result {
        let _ = fs::remove_file(&staged_path);
        return Err(error);
    }
    if faults.fail_before_insert {
        let _ = fs::remove_file(&staged_path);
        return Err(CommandError::new(
            "asset_test_database_failure",
            "Injected failure before the asset ledger insert.",
        ));
    }

    let insert = connection.execute(
        "INSERT INTO assets (
             id, original_filename, mime_type, byte_size, relative_path, created_at,
             lifecycle_state, draft_id, staged_relative_path, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'staged', ?7, ?8, ?6)",
        params![
            id,
            original_filename,
            mime_type,
            i64::try_from(metadata.len()).unwrap_or(i64::MAX),
            relative_path,
            now,
            draft_id,
            staged_relative_path,
        ],
    );
    if let Err(error) = insert {
        let _ = fs::remove_file(&staged_path);
        return Err(CommandError::database(error));
    }
    record_event(
        connection,
        Some(&id),
        "staged",
        None,
        Some(&staged_relative_path),
        "editor image staged",
        now,
    )?;
    Ok(AssetDto {
        id,
        original_filename,
        mime_type: mime_type.to_string(),
        byte_size: i64::try_from(metadata.len()).unwrap_or(i64::MAX),
        file_path: staged_path.to_string_lossy().into_owned(),
        created_at: now,
    })
}

fn finalize_assets_with_faults(
    transaction: &Transaction<'_>,
    paths: &AppPaths,
    asset_ids: &[String],
    draft_id: Option<&str>,
    owner_id: Option<&str>,
    faults: PromotionFaults,
) -> Result<DraftFinalization, CommandError> {
    let normalized = normalize_asset_ids(asset_ids.to_vec())?;
    let now = unix_time_millis()?;
    if let Some(draft_id) = draft_id {
        validate_entity_id(draft_id, "asset draft")?;
        ensure_active_draft(transaction, draft_id, now)?;
    }
    ensure_assets_available(transaction, paths, &normalized, draft_id)?;

    for asset_id in &normalized {
        let asset = load_asset(transaction, asset_id)?;
        if asset.lifecycle_state != "staged" {
            continue;
        }
        let staged_relative = asset.staged_relative_path.as_deref().ok_or_else(|| {
            CommandError::new("asset_stage_missing", "A staged image has no managed path.")
        })?;
        let staged_path = paths.resolve_relative(staged_relative);
        let final_path = paths.resolve_relative(&asset.relative_path);
        create_parent(&final_path)?;
        if final_path.exists() {
            return Err(CommandError::new(
                "asset_path_conflict",
                "A managed image path is already occupied.",
            ));
        }
        fs::rename(&staged_path, &final_path).map_err(CommandError::io)?;
        sync_directory(staged_path.parent())?;
        sync_directory(final_path.parent())?;
        if faults.fail_after_rename {
            return Err(CommandError::new(
                "asset_test_post_rename_crash",
                "Injected failure after the managed rename.",
            ));
        }
        transaction
            .execute(
                "UPDATE assets
                 SET lifecycle_state = 'ready', draft_id = NULL,
                     staged_relative_path = NULL, quarantine_relative_path = NULL,
                     ready_at = COALESCE(ready_at, ?1), verified_at = ?1,
                     updated_at = ?1, lifecycle_reason = '', deleted_at = NULL
                 WHERE id = ?2 AND lifecycle_state = 'staged'",
                params![now, asset_id],
            )
            .map_err(CommandError::database)?;
        record_event(
            transaction,
            Some(asset_id),
            "promoted",
            Some(staged_relative),
            Some(&asset.relative_path),
            "asset promoted with durable owner",
            now,
        )?;
    }

    let cleanup_paths = if let Some(draft_id) = draft_id {
        let retained = normalized.iter().cloned().collect::<BTreeSet<_>>();
        let cleanup = retire_unused_draft_assets(
            transaction,
            paths,
            draft_id,
            &retained,
            now,
            "unused when owner committed",
        )?;
        transaction
            .execute(
                "UPDATE asset_drafts
                 SET owner_id = COALESCE(?1, owner_id), updated_at = ?2, committed_at = ?2
                 WHERE id = ?3 AND committed_at IS NULL AND canceled_at IS NULL",
                params![owner_id, now, draft_id],
            )
            .map_err(CommandError::database)?;
        cleanup
    } else {
        Vec::new()
    };

    Ok(DraftFinalization { cleanup_paths })
}

fn retire_unused_draft_assets(
    connection: &Connection,
    paths: &AppPaths,
    draft_id: &str,
    retained: &BTreeSet<String>,
    now: i64,
    reason: &str,
) -> Result<Vec<PathBuf>, CommandError> {
    let assets = load_assets_for_draft(connection, draft_id)?;
    let mut cleanup = Vec::new();
    for asset in assets {
        if retained.contains(&asset.id) || reference_count(connection, &asset.id)? > 0 {
            continue;
        }
        if let Some(staged) = asset.staged_relative_path.as_deref() {
            cleanup.push(paths.resolve_relative(staged));
        }
        if paths.resolve_relative(&asset.relative_path).exists() {
            cleanup.push(paths.resolve_relative(&asset.relative_path));
        }
        connection
            .execute(
                "UPDATE assets
                 SET lifecycle_state = 'deleted', draft_id = NULL,
                     deleted_at = COALESCE(deleted_at, ?1), updated_at = ?1,
                     lifecycle_reason = ?2
                 WHERE id = ?3",
                params![now, reason, asset.id],
            )
            .map_err(CommandError::database)?;
        record_event(
            connection,
            Some(&asset.id),
            "deleted",
            asset.staged_relative_path.as_deref(),
            None,
            reason,
            now,
        )?;
    }
    Ok(cleanup)
}

fn abandon_draft_in_connection(
    connection: &mut Connection,
    paths: &AppPaths,
    draft_id: &str,
    reason: &str,
) -> Result<Vec<PathBuf>, CommandError> {
    let now = unix_time_millis()?;
    let row: Option<(Option<i64>, Option<i64>)> = connection
        .query_row(
            "SELECT committed_at, canceled_at FROM asset_drafts WHERE id = ?1",
            [draft_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(CommandError::database)?;
    let Some((committed_at, canceled_at)) = row else {
        return Ok(Vec::new());
    };
    if committed_at.is_some() || canceled_at.is_some() {
        return Ok(Vec::new());
    }
    let transaction = connection.transaction().map_err(CommandError::database)?;
    let cleanup =
        retire_unused_draft_assets(&transaction, paths, draft_id, &BTreeSet::new(), now, reason)?;
    transaction
        .execute(
            "UPDATE asset_drafts SET updated_at = ?1, canceled_at = ?1 WHERE id = ?2",
            params![now, draft_id],
        )
        .map_err(CommandError::database)?;
    transaction.commit().map_err(CommandError::database)?;
    Ok(cleanup)
}

fn reconcile_in_connection(
    connection: &mut Connection,
    paths: &AppPaths,
    now: i64,
) -> Result<Vec<PathBuf>, CommandError> {
    let mut cleanup = Vec::new();
    expire_drafts(connection, paths, now, &mut cleanup)?;
    let asset_ids = {
        let mut statement = connection
            .prepare("SELECT id FROM assets ORDER BY created_at, id")
            .map_err(CommandError::database)?;
        statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?
    };
    for asset_id in asset_ids {
        reconcile_asset(connection, paths, &asset_id, now, &mut cleanup)?;
    }
    quarantine_unknown_files(connection, paths, now)?;
    Ok(cleanup)
}

fn expire_drafts(
    connection: &mut Connection,
    paths: &AppPaths,
    now: i64,
    cleanup: &mut Vec<PathBuf>,
) -> Result<(), CommandError> {
    let draft_ids = {
        let mut statement = connection
            .prepare(
                "SELECT id FROM asset_drafts
                 WHERE committed_at IS NULL AND canceled_at IS NULL AND expires_at < ?1",
            )
            .map_err(CommandError::database)?;
        statement
            .query_map([now], |row| row.get::<_, String>(0))
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?
    };
    for draft_id in draft_ids {
        cleanup.extend(abandon_draft_in_connection(
            connection,
            paths,
            &draft_id,
            "draft expired",
        )?);
    }
    Ok(())
}

fn reconcile_asset(
    connection: &mut Connection,
    paths: &AppPaths,
    asset_id: &str,
    now: i64,
    cleanup: &mut Vec<PathBuf>,
) -> Result<(), CommandError> {
    let asset = load_asset(connection, asset_id)?;
    let references = reference_count(connection, asset_id)?;
    let final_path = paths.resolve_relative(&asset.relative_path);
    let staged_path = asset
        .staged_relative_path
        .as_deref()
        .map(|value| paths.resolve_relative(value));
    let quarantine_path = asset
        .quarantine_relative_path
        .as_deref()
        .map(|value| paths.resolve_relative(value));
    let final_exists = final_path.is_file();
    let staged_exists = staged_path.as_ref().is_some_and(|path| path.is_file());
    let quarantine_exists = quarantine_path.as_ref().is_some_and(|path| path.is_file());

    if references > 0 {
        if final_exists {
            verify_asset_file(&asset, &final_path)?;
            transition_asset(connection, &asset, "ready", None, None, "", now, "restored")?;
            return Ok(());
        }
        if staged_exists {
            let staged = staged_path.as_ref().expect("checked staged path");
            create_parent(&final_path)?;
            fs::rename(staged, &final_path).map_err(CommandError::io)?;
            sync_directory(staged.parent())?;
            sync_directory(final_path.parent())?;
            verify_asset_file(&asset, &final_path)?;
            transition_asset(
                connection,
                &asset,
                "ready",
                None,
                None,
                "recovered referenced staged file",
                now,
                "restored",
            )?;
            return Ok(());
        }
        if quarantine_exists {
            let quarantined = quarantine_path.as_ref().expect("checked quarantine path");
            create_parent(&final_path)?;
            fs::rename(quarantined, &final_path).map_err(CommandError::io)?;
            sync_directory(quarantined.parent())?;
            sync_directory(final_path.parent())?;
            verify_asset_file(&asset, &final_path)?;
            transition_asset(
                connection,
                &asset,
                "ready",
                None,
                None,
                "restored referenced quarantine file",
                now,
                "restored",
            )?;
            return Ok(());
        }
        transition_asset(
            connection,
            &asset,
            "missing",
            asset.staged_relative_path.as_deref(),
            asset.quarantine_relative_path.as_deref(),
            "referenced file absent",
            now,
            "missing",
        )?;
        return Ok(());
    }

    match asset.lifecycle_state.as_str() {
        "staged" => {
            if staged_exists {
                let draft_active = asset
                    .draft_id
                    .as_deref()
                    .is_some_and(|draft_id| ensure_active_draft(connection, draft_id, now).is_ok());
                if draft_active {
                    return Ok(());
                }
                cleanup.push(staged_path.expect("checked staged path"));
            }
            if final_exists {
                quarantine_asset_file(connection, paths, &asset, &final_path, now)?;
            } else {
                transition_asset(
                    connection,
                    &asset,
                    "deleted",
                    asset.staged_relative_path.as_deref(),
                    None,
                    "unreferenced staged asset expired",
                    now,
                    "deleted",
                )?;
            }
        }
        "ready" => {
            if !final_exists {
                transition_asset(
                    connection,
                    &asset,
                    "deleted",
                    None,
                    None,
                    "unreferenced ready file absent",
                    now,
                    "deleted",
                )?;
            } else if now.saturating_sub(asset.updated_at.max(asset.created_at))
                >= RECOVERY_GRACE_MS
            {
                quarantine_asset_file(connection, paths, &asset, &final_path, now)?;
            }
        }
        "missing" => {
            if final_exists {
                verify_asset_file(&asset, &final_path)?;
                transition_asset(
                    connection,
                    &asset,
                    "ready",
                    None,
                    None,
                    "unreferenced file returned",
                    now,
                    "restored",
                )?;
            } else {
                transition_asset(
                    connection,
                    &asset,
                    "deleted",
                    None,
                    None,
                    "unreferenced missing asset retired",
                    now,
                    "deleted",
                )?;
            }
        }
        "quarantined" => {
            if quarantine_exists
                && now.saturating_sub(asset.updated_at.max(asset.created_at)) >= RECOVERY_GRACE_MS
            {
                cleanup.push(quarantine_path.expect("checked quarantine path"));
                transition_asset(
                    connection,
                    &asset,
                    "deleted",
                    None,
                    asset.quarantine_relative_path.as_deref(),
                    "quarantine grace elapsed",
                    now,
                    "deleted",
                )?;
            }
        }
        "deleted" => {
            if let Some(path) = staged_path.filter(|path| path.exists()) {
                cleanup.push(path);
            }
            if final_exists {
                cleanup.push(final_path);
            }
            if let Some(path) = quarantine_path.filter(|path| path.exists()) {
                cleanup.push(path);
            }
        }
        _ => {}
    }
    Ok(())
}

fn quarantine_asset_file(
    connection: &Connection,
    paths: &AppPaths,
    asset: &AssetRecord,
    source: &Path,
    now: i64,
) -> Result<(), CommandError> {
    let suffix = now.to_string();
    let relative = AppPaths::quarantined_asset_relative_path(&asset.id, &suffix);
    let destination = paths.resolve_relative(&relative);
    create_parent(&destination)?;
    fs::rename(source, &destination).map_err(CommandError::io)?;
    sync_directory(source.parent())?;
    sync_directory(destination.parent())?;
    transition_asset(
        connection,
        asset,
        "quarantined",
        None,
        Some(&relative),
        "unreferenced managed file quarantined",
        now,
        "quarantined",
    )
}

fn quarantine_unknown_files(
    connection: &Connection,
    paths: &AppPaths,
    now: i64,
) -> Result<(), CommandError> {
    let mut known = HashSet::new();
    let mut statement = connection
        .prepare("SELECT relative_path, staged_relative_path, quarantine_relative_path FROM assets")
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })
        .map_err(CommandError::database)?;
    for row in rows {
        let (ready, staged, quarantined) = row.map_err(CommandError::database)?;
        known.insert(ready);
        if let Some(value) = staged {
            known.insert(value);
        }
        if let Some(value) = quarantined {
            known.insert(value);
        }
    }

    for directory in [paths.images_dir(), paths.asset_staging_dir()] {
        for entry in fs::read_dir(directory).map_err(CommandError::io)? {
            let entry = entry.map_err(CommandError::io)?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let relative = relative_to_app_data(paths, &path)?;
            if known.contains(&relative) {
                continue;
            }
            let orphan_id = generate_id(connection)?;
            let destination_relative =
                AppPaths::quarantined_asset_relative_path(&orphan_id, &now.to_string());
            let destination = paths.resolve_relative(&destination_relative);
            fs::rename(&path, &destination).map_err(CommandError::io)?;
            sync_directory(path.parent())?;
            sync_directory(destination.parent())?;
            record_event(
                connection,
                None,
                "orphan_quarantined",
                Some(&relative),
                Some(&destination_relative),
                "unknown managed file moved out of active namespace",
                now,
            )?;
        }
    }
    Ok(())
}

fn transition_asset(
    connection: &Connection,
    asset: &AssetRecord,
    state: &str,
    staged_relative_path: Option<&str>,
    quarantine_relative_path: Option<&str>,
    reason: &str,
    now: i64,
    event: &str,
) -> Result<(), CommandError> {
    let changed = asset.lifecycle_state != state
        || asset.staged_relative_path.as_deref() != staged_relative_path
        || asset.quarantine_relative_path.as_deref() != quarantine_relative_path
        || (state == "ready" && asset.draft_id.is_some());
    connection
        .execute(
            "UPDATE assets
             SET lifecycle_state = ?1,
                 draft_id = CASE WHEN ?1 = 'staged' THEN draft_id ELSE NULL END,
                 staged_relative_path = ?2,
                 quarantine_relative_path = ?3,
                 ready_at = CASE WHEN ?1 = 'ready' THEN COALESCE(ready_at, ?4) ELSE ready_at END,
                 verified_at = CASE WHEN ?1 = 'ready' THEN ?4 ELSE verified_at END,
                 deleted_at = CASE WHEN ?1 = 'deleted' THEN COALESCE(deleted_at, ?4) ELSE NULL END,
                 updated_at = CASE WHEN ?5 THEN ?4 ELSE updated_at END,
                 lifecycle_reason = ?6
             WHERE id = ?7",
            params![
                state,
                staged_relative_path,
                quarantine_relative_path,
                now,
                changed,
                reason,
                asset.id,
            ],
        )
        .map_err(CommandError::database)?;
    if changed {
        record_event(
            connection,
            Some(&asset.id),
            event,
            asset
                .staged_relative_path
                .as_deref()
                .or(asset.quarantine_relative_path.as_deref())
                .or(Some(&asset.relative_path)),
            staged_relative_path
                .or(quarantine_relative_path)
                .or((state == "ready").then_some(asset.relative_path.as_str())),
            reason,
            now,
        )?;
    }
    Ok(())
}

fn load_asset(connection: &Connection, asset_id: &str) -> Result<AssetRecord, CommandError> {
    connection
        .query_row(
            "SELECT id, original_filename, mime_type, byte_size, relative_path,
                    lifecycle_state, draft_id, staged_relative_path,
                    quarantine_relative_path, created_at, updated_at
             FROM assets WHERE id = ?1",
            [asset_id],
            |row| {
                Ok(AssetRecord {
                    id: row.get(0)?,
                    original_filename: row.get(1)?,
                    mime_type: row.get(2)?,
                    byte_size: row.get(3)?,
                    relative_path: row.get(4)?,
                    lifecycle_state: row.get(5)?,
                    draft_id: row.get(6)?,
                    staged_relative_path: row.get(7)?,
                    quarantine_relative_path: row.get(8)?,
                    created_at: row.get(9)?,
                    updated_at: row.get(10)?,
                })
            },
        )
        .optional()
        .map_err(CommandError::database)?
        .ok_or_else(|| {
            CommandError::new(
                "asset_not_found",
                "One of the images used by this content is no longer available.",
            )
        })
}

fn load_assets_for_draft(
    connection: &Connection,
    draft_id: &str,
) -> Result<Vec<AssetRecord>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT id, original_filename, mime_type, byte_size, relative_path,
                    lifecycle_state, draft_id, staged_relative_path,
                    quarantine_relative_path, created_at, updated_at
             FROM assets WHERE draft_id = ?1 ORDER BY created_at, id",
        )
        .map_err(CommandError::database)?;
    statement
        .query_map([draft_id], |row| {
            Ok(AssetRecord {
                id: row.get(0)?,
                original_filename: row.get(1)?,
                mime_type: row.get(2)?,
                byte_size: row.get(3)?,
                relative_path: row.get(4)?,
                lifecycle_state: row.get(5)?,
                draft_id: row.get(6)?,
                staged_relative_path: row.get(7)?,
                quarantine_relative_path: row.get(8)?,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)
}

fn reference_count(connection: &Connection, asset_id: &str) -> Result<i64, CommandError> {
    connection
        .query_row(
            "SELECT
                 (SELECT COUNT(*) FROM note_assets WHERE asset_id = ?1) +
                 (SELECT COUNT(*) FROM card_template_assets WHERE asset_id = ?1)",
            [asset_id],
            |row| row.get(0),
        )
        .map_err(CommandError::database)
}

fn normalize_asset_ids(values: Vec<String>) -> Result<Vec<String>, CommandError> {
    let mut unique = BTreeSet::new();
    for value in values {
        validate_entity_id(&value, "asset")?;
        unique.insert(value);
    }
    Ok(unique.into_iter().collect())
}

fn record_event(
    connection: &Connection,
    asset_id: Option<&str>,
    action: &str,
    from_path: Option<&str>,
    to_path: Option<&str>,
    detail: &str,
    occurred_at: i64,
) -> Result<(), CommandError> {
    let id = generate_id(connection)?;
    connection
        .execute(
            "INSERT INTO asset_reconciliation_events (
                 id, asset_id, action, from_path, to_path, detail, occurred_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                id,
                asset_id,
                action,
                from_path,
                to_path,
                detail,
                occurred_at
            ],
        )
        .map_err(CommandError::database)?;
    Ok(())
}

fn verify_asset_file(asset: &AssetRecord, path: &Path) -> Result<(), CommandError> {
    let metadata = fs::metadata(path).map_err(CommandError::io)?;
    if i64::try_from(metadata.len()).unwrap_or(i64::MAX) != asset.byte_size {
        return Err(CommandError::new(
            "asset_size_mismatch",
            format!(
                "The image '{}' does not match its database size record.",
                asset.original_filename
            ),
        ));
    }
    let mut file = File::open(path).map_err(CommandError::io)?;
    let mut header = [0_u8; 16];
    let bytes_read = file.read(&mut header).map_err(CommandError::io)?;
    let (mime, _) = detect_image_bytes(&header[..bytes_read])?;
    if mime != asset.mime_type {
        return Err(CommandError::new(
            "asset_type_mismatch",
            format!(
                "The image '{}' does not match its database file type.",
                asset.original_filename
            ),
        ));
    }
    Ok(())
}

fn write_new_file(path: &Path, data: &[u8]) -> Result<(), CommandError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(CommandError::io)?;
    file.write_all(data).map_err(CommandError::io)?;
    file.sync_all().map_err(CommandError::io)
}

fn create_parent(path: &Path) -> Result<(), CommandError> {
    let parent = path.parent().ok_or_else(|| {
        CommandError::new("invalid_asset_path", "The managed image path is invalid.")
    })?;
    fs::create_dir_all(parent).map_err(CommandError::io)
}

#[cfg(unix)]
fn sync_directory(path: Option<&Path>) -> Result<(), CommandError> {
    let path = path.ok_or_else(|| {
        CommandError::new(
            "invalid_asset_path",
            "The managed image directory is invalid.",
        )
    })?;
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(CommandError::io)
}

#[cfg(not(unix))]
fn sync_directory(_path: Option<&Path>) -> Result<(), CommandError> {
    Ok(())
}

fn relative_to_app_data(paths: &AppPaths, path: &Path) -> Result<String, CommandError> {
    path.strip_prefix(paths.app_data_dir())
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
        .map_err(|_| {
            CommandError::new(
                "invalid_asset_path",
                "A managed image escaped the application data directory.",
            )
        })
}

fn remove_paths_best_effort(paths: &[PathBuf]) {
    for path in paths {
        if let Err(error) = fs::remove_file(path) {
            if error.kind() != std::io::ErrorKind::NotFound {
                crate::logging::error(
                    "asset_cleanup_failed",
                    format!("path={} error={error}", path.display()),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        cards::validate_content,
        decks::{create_deck_in_connection, CreateDeckInput},
        scheduler::insert_new_schedule,
    };
    use serde_json::json;

    fn temp_paths(label: &str) -> AppPaths {
        let root = std::env::temp_dir().join(format!(
            "ostudy-{label}-{}-{}",
            std::process::id(),
            unix_time_millis().expect("clock")
        ));
        AppPaths::new(root).expect("paths should initialize")
    }

    fn png_bytes() -> Vec<u8> {
        vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 0]
    }

    fn source_image(paths: &AppPaths, name: &str) -> PathBuf {
        let path = paths.app_data_dir().join(name);
        fs::write(&path, png_bytes()).expect("source should write");
        path
    }

    #[test]
    fn staged_ingest_cleans_copy_sync_and_database_failures() {
        let database = Database::open_in_memory().expect("database should open");
        let paths = temp_paths("ingest-failures");
        let source = source_image(&paths, "source.png");
        database
            .with_connection(|connection| {
                let draft = begin_draft_in_connection(connection, "card", None)?;
                for faults in [
                    IngestFaults {
                        fail_after_copy: true,
                        ..Default::default()
                    },
                    IngestFaults {
                        fail_after_sync: true,
                        ..Default::default()
                    },
                    IngestFaults {
                        fail_before_insert: true,
                        ..Default::default()
                    },
                ] {
                    let error =
                        stage_source_in_connection(connection, &paths, &source, &draft.id, faults)
                            .expect_err("fault should fail");
                    assert!(error.code.starts_with("asset_test_"));
                }
                let count: i64 = connection
                    .query_row("SELECT COUNT(*) FROM assets", [], |row| row.get(0))
                    .map_err(CommandError::database)?;
                assert_eq!(count, 0);
                assert_eq!(fs::read_dir(paths.asset_staging_dir()).unwrap().count(), 0);
                Ok(())
            })
            .expect("failure cleanup should be durable");
        let _ = fs::remove_dir_all(paths.app_data_dir());
    }

    #[test]
    fn post_rename_failure_is_reconciled_without_losing_a_reference() {
        let database = Database::open_in_memory().expect("database should open");
        let paths = temp_paths("post-rename");
        let source = source_image(&paths, "source.png");
        database
            .with_connection(|connection| {
                let deck = create_deck_in_connection(
                    connection,
                    CreateDeckInput {
                        name: "Assets".to_string(),
                        description: None,
                    },
                )?;
                let draft = begin_draft_in_connection(connection, "card", None)?;
                let asset = stage_source_in_connection(
                    connection,
                    &paths,
                    &source,
                    &draft.id,
                    IngestFaults::default(),
                )?;
                let transaction = connection.transaction().map_err(CommandError::database)?;
                let content = validate_content(
                    json!({"type":"doc","content":[{"type":"image","attrs":{"assetId":asset.id,"src":format!("study-asset://{}", asset.id)}}]}),
                    json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Answer"}]}]}),
                    String::new(),
                    "Answer".to_string(),
                )?;
                let note_id = generate_id(&transaction)?;
                let card_id = generate_id(&transaction)?;
                transaction.execute(
                    "INSERT INTO notes (
                         id, deck_id, note_type, content_format, content_schema_version,
                         front_doc, back_doc, front_text, back_text, duplicate_hash,
                         created_at, updated_at, revision
                     ) VALUES (?1, ?2, 'basic', 'tiptap-json', 1, ?3, ?4, ?5, ?6, '', 1, 1, 1)",
                    params![note_id, deck.id, content.front_doc_json, content.back_doc_json, content.front_text, content.back_text],
                ).map_err(CommandError::database)?;
                transaction.execute(
                    "INSERT INTO cards (id, note_id, deck_id, direction, template_ordinal, created_at, updated_at)
                     VALUES (?1, ?2, ?3, 'forward', 0, 1, 1)",
                    params![card_id, note_id, deck.id],
                ).map_err(CommandError::database)?;
                insert_new_schedule(&transaction, &card_id, 1)?;
                transaction.execute(
                    "INSERT INTO note_assets (note_id, asset_id, side) VALUES (?1, ?2, 'front')",
                    params![note_id, asset.id],
                ).map_err(CommandError::database)?;
                let error = finalize_assets_with_faults(
                    &transaction,
                    &paths,
                    std::slice::from_ref(&asset.id),
                    Some(&draft.id),
                    Some(&note_id),
                    PromotionFaults { fail_after_rename: true },
                ).expect_err("post-rename crash should be injected");
                assert_eq!(error.code, "asset_test_post_rename_crash");
                drop(transaction);

                // Recreate the durable reference to model a commit that completed
                // while the final lifecycle update was interrupted.
                connection.execute(
                    "INSERT INTO notes (
                         id, deck_id, note_type, content_format, content_schema_version,
                         front_doc, back_doc, front_text, back_text, duplicate_hash,
                         created_at, updated_at, revision
                     ) VALUES (?1, ?2, 'basic', 'tiptap-json', 1, ?3, ?4, ?5, ?6, '', 2, 2, 1)",
                    params![note_id, deck.id, content.front_doc_json, content.back_doc_json, content.front_text, content.back_text],
                ).map_err(CommandError::database)?;
                connection.execute(
                    "INSERT INTO note_assets (note_id, asset_id, side) VALUES (?1, ?2, 'front')",
                    params![note_id, asset.id],
                ).map_err(CommandError::database)?;
                let cleanup = reconcile_in_connection(connection, &paths, unix_time_millis()?)?;
                remove_paths_best_effort(&cleanup);
                let state: String = connection.query_row(
                    "SELECT lifecycle_state FROM assets WHERE id = ?1",
                    [asset.id.as_str()],
                    |row| row.get(0),
                ).map_err(CommandError::database)?;
                assert_eq!(state, "ready");
                assert!(paths.resolve_relative(&AppPaths::image_relative_path(&asset.id, "png")).exists());
                Ok(())
            })
            .expect("reconciliation should restore the final file");
        let _ = fs::remove_dir_all(paths.app_data_dir());
    }

    #[test]
    fn cancel_and_sync_remove_only_unreferenced_draft_assets() {
        let database = Database::open_in_memory().expect("database should open");
        let paths = temp_paths("draft-cleanup");
        let source = source_image(&paths, "source.png");
        database
            .with_connection(|connection| {
                let draft = begin_draft_in_connection(connection, "card", None)?;
                let kept = stage_source_in_connection(
                    connection,
                    &paths,
                    &source,
                    &draft.id,
                    Default::default(),
                )?;
                let removed = stage_source_in_connection(
                    connection,
                    &paths,
                    &source,
                    &draft.id,
                    Default::default(),
                )?;
                let retained = BTreeSet::from([kept.id.clone()]);
                let transaction = connection.transaction().map_err(CommandError::database)?;
                let cleanup = retire_unused_draft_assets(
                    &transaction,
                    &paths,
                    &draft.id,
                    &retained,
                    unix_time_millis()?,
                    "test sync",
                )?;
                transaction.commit().map_err(CommandError::database)?;
                remove_paths_best_effort(&cleanup);
                assert!(Path::new(&kept.file_path).exists());
                assert!(!Path::new(&removed.file_path).exists());

                let cleanup =
                    abandon_draft_in_connection(connection, &paths, &draft.id, "test cancel")?;
                remove_paths_best_effort(&cleanup);
                assert!(!Path::new(&kept.file_path).exists());
                Ok(())
            })
            .expect("draft cleanup should succeed");
        let _ = fs::remove_dir_all(paths.app_data_dir());
    }

    #[test]
    fn repeated_reconciliation_is_idempotent_and_protects_shared_references() {
        let database = Database::open_in_memory().expect("database should open");
        let paths = temp_paths("reconcile-idempotent");
        database
            .with_connection(|connection| {
                let now = unix_time_millis()?;
                let asset_id = generate_id(connection)?;
                let relative = AppPaths::image_relative_path(&asset_id, "png");
                fs::write(paths.resolve_relative(&relative), png_bytes()).map_err(CommandError::io)?;
                connection.execute(
                    "INSERT INTO assets (
                         id, original_filename, mime_type, byte_size, relative_path, created_at,
                         lifecycle_state, ready_at, verified_at, updated_at
                     ) VALUES (?1, 'shared.png', 'image/png', ?2, ?3, ?4, 'ready', ?4, ?4, ?4)",
                    params![asset_id, png_bytes().len() as i64, relative, now],
                ).map_err(CommandError::database)?;
                let deck = create_deck_in_connection(
                    connection,
                    CreateDeckInput { name: "Shared".to_string(), description: None },
                )?;
                for ordinal in 0..2 {
                    let note_id = generate_id(connection)?;
                    connection.execute(
                        "INSERT INTO notes (
                             id, deck_id, note_type, content_format, content_schema_version,
                             front_doc, back_doc, front_text, back_text, duplicate_hash,
                             created_at, updated_at, revision
                         ) VALUES (?1, ?2, 'basic', 'tiptap-json', 1, '{}', '{}', 'Q', 'A', '', ?3, ?3, 1)",
                        params![note_id, deck.id, now + ordinal],
                    ).map_err(CommandError::database)?;
                    connection.execute(
                        "INSERT INTO note_assets (note_id, asset_id, side) VALUES (?1, ?2, 'front')",
                        params![note_id, asset_id],
                    ).map_err(CommandError::database)?;
                }
                let first = reconcile_in_connection(connection, &paths, now + 1)?;
                let event_count: i64 = connection.query_row(
                    "SELECT COUNT(*) FROM asset_reconciliation_events",
                    [],
                    |row| row.get(0),
                ).map_err(CommandError::database)?;
                let second = reconcile_in_connection(connection, &paths, now + 2)?;
                let repeated_count: i64 = connection.query_row(
                    "SELECT COUNT(*) FROM asset_reconciliation_events",
                    [],
                    |row| row.get(0),
                ).map_err(CommandError::database)?;
                assert!(first.is_empty());
                assert!(second.is_empty());
                assert_eq!(event_count, repeated_count);
                assert!(paths.resolve_relative(&relative).exists());
                Ok(())
            })
            .expect("shared referenced asset must remain ready");
        let _ = fs::remove_dir_all(paths.app_data_dir());
    }

    #[test]
    fn unknown_and_unreferenced_files_are_quarantined_conservatively() {
        let database = Database::open_in_memory().expect("database should open");
        let paths = temp_paths("orphan-quarantine");
        let orphan = paths.images_dir().join("unknown.png");
        fs::write(&orphan, png_bytes()).expect("orphan should write");
        database
            .with_connection(|connection| {
                reconcile_in_connection(connection, &paths, unix_time_millis()?)?;
                assert!(!orphan.exists());
                assert!(fs::read_dir(paths.asset_quarantine_dir()).unwrap().count() >= 1);
                let logged: i64 = connection.query_row(
                    "SELECT COUNT(*) FROM asset_reconciliation_events WHERE action = 'orphan_quarantined'",
                    [],
                    |row| row.get(0),
                ).map_err(CommandError::database)?;
                assert_eq!(logged, 1);
                Ok(())
            })
            .expect("orphan should be quarantined");
        let _ = fs::remove_dir_all(paths.app_data_dir());
    }
}
