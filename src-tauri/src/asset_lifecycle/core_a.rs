
use std::{
    collections::{BTreeSet, HashSet},
    fs::{self, File, OpenOptions},
    io::Read,
    path::{Path, PathBuf},
};

use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    assets::{detect_image_bytes, validate_original_filename, AssetDto},
    database::Database,
    error::CommandError,
    external_files::{self, ExternalFileKind, ExternalImportPurpose},
    media::inspect_media_bytes,
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
    pub staged_file_id: String,
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
    let retained = normalize_asset_ids(input.asset_ids)?
        .into_iter()
        .collect::<BTreeSet<_>>();
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

/// Stages a PNG oStudy drew itself, such as a region clipped from a PDF
/// page, into a draft, as `stage_image` stages a file the learner chose. The
/// bytes pass through a scratch file outside the asset store, which is
/// removed whether or not staging succeeds.
pub(crate) fn stage_generated_png(
    database: &Database,
    paths: &AppPaths,
    draft_id: &str,
    png: &[u8],
    display_name: &str,
) -> Result<AssetDto, CommandError> {
    validate_entity_id(draft_id, "asset draft")?;
    let scratch = std::env::temp_dir().join(format!(
        "ostudy-generated-{draft_id}-{}-{}.png",
        std::process::id(),
        unix_time_millis()?
    ));
    fs::write(&scratch, png).map_err(CommandError::io)?;
    let staged = database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        let asset = stage_source_named_in_connection(
            &transaction,
            paths,
            &scratch,
            draft_id,
            Some(display_name),
            IngestFaults::default(),
        )?;
        transaction.commit().map_err(CommandError::database)?;
        Ok(asset)
    });
    let _ = fs::remove_file(&scratch);
    staged
}

pub(crate) fn stage_image(
    database: &Database,
    paths: &AppPaths,
    input: ImportImageInput,
) -> Result<AssetDto, CommandError> {
    validate_entity_id(&input.draft_id, "asset draft")?;
    validate_entity_id(&input.staged_file_id, "staged image")?;
    database.with_connection(|connection| {
        if let Some(asset) =
            external_files::consumed_import_result::<AssetDto>(connection, &input.staged_file_id)?
        {
            return Ok(asset);
        }
        let staged = external_files::resolve_staged_import(
            connection,
            paths,
            &input.staged_file_id,
            ExternalImportPurpose::Image,
            &[
                ExternalFileKind::Png,
                ExternalFileKind::Jpeg,
                ExternalFileKind::Gif,
                ExternalFileKind::Webp,
            ],
        )?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        let asset = stage_source_named_in_connection(
            &transaction,
            paths,
            &staged.path,
            &input.draft_id,
            Some(&staged.display_name),
            IngestFaults::default(),
        )?;
        let extension = match asset.mime_type.as_str() {
            "image/png" => "png",
            "image/jpeg" => "jpg",
            "image/gif" => "gif",
            "image/webp" => "webp",
            _ => "bin",
        };
        let staged_asset_path = paths.resolve_relative(&AppPaths::staged_asset_relative_path(
            &input.draft_id,
            &asset.id,
            extension,
        ));
        if let Err(error) = external_files::mark_import_consumed(
            &transaction,
            &input.staged_file_id,
            &asset,
            unix_time_millis()?,
        ) {
            drop(transaction);
            let _ = fs::remove_file(staged_asset_path);
            return Err(error);
        }
        if let Err(error) = transaction.commit() {
            let _ = fs::remove_file(staged_asset_path);
            return Err(CommandError::database(error));
        }
        Ok(asset)
    })
}

