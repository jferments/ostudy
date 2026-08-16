//! Conflict-aware device synchronization snapshots.
//!
//! The sync engine operates on a strict whitelist of durable user-data tables,
//! preserves stable entity identifiers, and performs a field-level three-way
//! merge against the last snapshot acknowledged by a trusted peer. It never
//! copies the live SQLite file and never uses last-writer-wins for concurrent
//! edits. Immutable review rows are unioned; delete-vs-edit and same-field
//! changes become explicit conflicts.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap, VecDeque},
    fs,
    io::Write,
    path::{Component, Path, PathBuf},
};

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use rusqlite::{
    params_from_iter,
    types::{Value as SqliteValue, ValueRef},
    Connection, OptionalExtension, Transaction, TransactionBehavior,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::State;

use crate::{
    database::Database,
    error::CommandError,
    external_files::{self, ExternalFileKind, ExternalImportPurpose},
    paths::AppPaths,
    util::{generate_id, unix_time_millis, validate_entity_id},
};

const SYNC_FORMAT: &str = "ostudy-device-sync";
const SYNC_SCHEMA_VERSION: i64 = 1;
const MAX_SYNC_BYTES: u64 = 600 * 1024 * 1024;
const MAX_SYNC_TABLES: usize = 32;
const MAX_SYNC_ROWS: usize = 500_000;
const MAX_SYNC_CONFLICTS: usize = 10_000;
const MAX_SYNC_ASSET_BYTES: u64 = 25 * 1024 * 1024;
const MAX_SYNC_TOTAL_ASSET_BYTES: u64 = 512 * 1024 * 1024;
const MAX_DEVICE_NAME_CHARS: usize = 100;

const USER_DATA_TABLES: &[&str] = &[
    "settings",
    "deck_folders",
    "decks",
    "tags",
    "assets",
    "card_templates",
    "card_template_fields",
    "card_template_rules",
    "notes",
    "cards",
    "note_tags",
    "note_assets",
    "schedules",
    "review_events",
    "practice_sessions",
    "ai_provider_profiles",
    "ai_task_defaults",
];

const APPEND_ONLY_TABLES: &[&str] = &["review_events"];
const VOLATILE_MERGE_COLUMNS: &[&str] = &["updated_at", "revision"];

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case", tag = "type", content = "value")]
pub enum SyncValue {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(String),
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SyncRow {
    pub key: String,
    pub values: BTreeMap<String, SyncValue>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SyncTable {
    pub name: String,
    pub columns: Vec<String>,
    pub primary_key: Vec<String>,
    pub rows: Vec<SyncRow>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SyncAssetPayload {
    pub asset_id: String,
    pub relative_path: String,
    pub byte_size: u64,
    pub sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_base64: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeviceSyncSnapshot {
    pub format: String,
    pub schema_version: i64,
    pub snapshot_id: String,
    pub source_device_id: String,
    pub source_device_name: String,
    pub created_at_utc_millis: i64,
    pub tables: Vec<SyncTable>,
    pub assets: Vec<SyncAssetPayload>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GetSyncIdentityInput {
    pub device_name: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncIdentityDto {
    pub device_id: String,
    pub device_name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PrepareDeviceSyncSnapshotInput {
    pub operation_id: String,
    pub peer_device_id: String,
    pub device_name: String,
    pub include_settings: bool,
    pub include_ai_profiles: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedDeviceSyncSnapshotDto {
    pub prepared_export: crate::external_files::PreparedExportDto,
    pub snapshot_id: String,
    pub table_count: usize,
    pub row_count: usize,
    pub asset_count: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreviewDeviceSyncInput {
    pub operation_id: String,
    pub staged_file_id: String,
    pub peer_device_id: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncConflictKind {
    ConcurrentFieldEdit,
    DeleteVsEdit,
    ConcurrentInsert,
    ImmutableRowMismatch,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SyncConflictDto {
    pub id: String,
    pub table: String,
    pub row_key: String,
    pub column: Option<String>,
    pub kind: SyncConflictKind,
    pub base_value: Option<SyncValue>,
    pub local_value: Option<SyncValue>,
    pub remote_value: Option<SyncValue>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceSyncPreviewDto {
    pub preview_id: String,
    pub remote_device_id: String,
    pub remote_device_name: String,
    pub upsert_count: usize,
    pub delete_count: usize,
    pub asset_write_count: usize,
    pub conflicts: Vec<SyncConflictDto>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncResolutionChoice {
    Local,
    Remote,
    Custom,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SyncConflictResolutionInput {
    pub conflict_id: String,
    pub choice: SyncResolutionChoice,
    pub custom_value: Option<SyncValue>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CommitDeviceSyncInput {
    pub preview_id: String,
    pub resolutions: Vec<SyncConflictResolutionInput>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitDeviceSyncResultDto {
    pub remote_device_id: String,
    pub upserted_rows: usize,
    pub deleted_rows: usize,
    pub written_assets: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredSyncPreview {
    preview_id: String,
    peer_device_id: String,
    remote_device_id: String,
    remote_device_name: String,
    local_fingerprint: String,
    base_snapshot: DeviceSyncSnapshot,
    local_snapshot: DeviceSyncSnapshot,
    remote_snapshot: DeviceSyncSnapshot,
    merged_snapshot: DeviceSyncSnapshot,
    conflicts: Vec<SyncConflictDto>,
    upsert_count: usize,
    delete_count: usize,
    asset_write_count: usize,
}

#[derive(Clone, Debug)]
struct TableSchema {
    name: String,
    columns: Vec<String>,
    primary_key: Vec<String>,
    foreign_parents: BTreeSet<String>,
}

#[derive(Clone, Debug)]
struct MergePlan {
    merged: DeviceSyncSnapshot,
    conflicts: Vec<SyncConflictDto>,
    upsert_count: usize,
    delete_count: usize,
    asset_write_count: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SyncExportSummary {
    snapshot_id: String,
    table_count: usize,
    row_count: usize,
    asset_count: usize,
}

#[tauri::command]
pub fn get_device_sync_identity(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: GetSyncIdentityInput,
) -> Result<SyncIdentityDto, CommandError> {
    let device_name = validate_device_name(input.device_name.unwrap_or_else(default_device_name))?;
    let identity_path = sync_root(paths.inner()).join("identity.json");
    if identity_path.is_file() {
        let bytes = read_bounded_file(&identity_path, 16 * 1024)?;
        let mut identity: SyncIdentityDto = serde_json::from_slice(&bytes).map_err(|_| {
            CommandError::new(
                "sync_identity_invalid",
                "The saved device synchronization identity is invalid.",
            )
        })?;
        if identity.device_name != device_name {
            identity.device_name = device_name;
            write_json_atomic(&identity_path, &identity)?;
        }
        return Ok(identity);
    }
    let device_id = database.with_connection(generate_id)?;
    let identity = SyncIdentityDto {
        device_id,
        device_name,
    };
    write_json_atomic(&identity_path, &identity)?;
    Ok(identity)
}

#[tauri::command]
pub fn prepare_device_sync_snapshot(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: PrepareDeviceSyncSnapshotInput,
) -> Result<PreparedDeviceSyncSnapshotDto, CommandError> {
    validate_entity_id(&input.operation_id, "device sync export operation")?;
    validate_entity_id(&input.peer_device_id, "paired device")?;
    let device_name = validate_device_name(input.device_name)?;
    let identity = get_or_create_identity(database.inner(), paths.inner(), device_name)?;
    let snapshot = database.with_connection(|connection| {
        capture_snapshot(
            connection,
            paths.inner(),
            &identity,
            input.include_settings,
            input.include_ai_profiles,
            true,
        )
    })?;
    let row_count = snapshot.tables.iter().map(|table| table.rows.len()).sum();
    let summary = SyncExportSummary {
        snapshot_id: snapshot.snapshot_id.clone(),
        table_count: snapshot.tables.len(),
        row_count,
        asset_count: snapshot.assets.len(),
    };
    let relative = AppPaths::prepared_export_relative_path(&input.operation_id, "json");
    let destination = paths.resolve_relative(&relative);
    write_json_atomic(&destination, &snapshot)?;
    let prepared_export = database.with_connection(|connection| {
        external_files::register_prepared_export(
            connection,
            paths.inner(),
            external_files::RegisterPreparedExport {
                id: &input.operation_id,
                purpose: "device_sync",
                suggested_filename: &format!(
                    "ostudy-sync-{}.json",
                    portable_filename(&identity.device_name)
                ),
                media_type: "application/vnd.ostudy.sync+json",
                relative_path: &relative,
                summary: &summary,
            },
        )
    })?;
    Ok(PreparedDeviceSyncSnapshotDto {
        prepared_export,
        snapshot_id: summary.snapshot_id,
        table_count: summary.table_count,
        row_count: summary.row_count,
        asset_count: summary.asset_count,
    })
}

#[tauri::command]
pub fn preview_device_sync(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: PreviewDeviceSyncInput,
) -> Result<DeviceSyncPreviewDto, CommandError> {
    validate_entity_id(&input.operation_id, "device sync preview")?;
    validate_entity_id(&input.staged_file_id, "staged synchronization snapshot")?;
    validate_entity_id(&input.peer_device_id, "paired device")?;
    let staged = database.with_connection(|connection| {
        external_files::resolve_staged_import(
            connection,
            paths.inner(),
            &input.staged_file_id,
            ExternalImportPurpose::StudyContent,
            &[ExternalFileKind::Json],
        )
    })?;
    let remote = read_snapshot(&staged.path)?;
    if remote.source_device_id != input.peer_device_id {
        return Err(CommandError::new(
            "sync_peer_mismatch",
            "This synchronization file belongs to a different paired device.",
        ));
    }
    let identity = get_or_create_identity(
        database.inner(),
        paths.inner(),
        default_device_name(),
    )?;
    let local = database.with_connection(|connection| {
        capture_snapshot(connection, paths.inner(), &identity, true, true, false)
    })?;
    let base = read_base_snapshot(paths.inner(), &input.peer_device_id)?
        .unwrap_or_else(|| empty_base_snapshot(&identity, &remote));
    let plan = merge_snapshots(&base, &local, &remote)?;
    let preview = StoredSyncPreview {
        preview_id: input.operation_id.clone(),
        peer_device_id: input.peer_device_id,
        remote_device_id: remote.source_device_id.clone(),
        remote_device_name: remote.source_device_name.clone(),
        local_fingerprint: snapshot_fingerprint(&local)?,
        base_snapshot: base,
        local_snapshot: local,
        remote_snapshot: remote,
        merged_snapshot: plan.merged,
        conflicts: plan.conflicts.clone(),
        upsert_count: plan.upsert_count,
        delete_count: plan.delete_count,
        asset_write_count: plan.asset_write_count,
    };
    write_json_atomic(&preview_path(paths.inner(), &preview.preview_id), &preview)?;
    Ok(DeviceSyncPreviewDto {
        preview_id: preview.preview_id,
        remote_device_id: preview.remote_device_id,
        remote_device_name: preview.remote_device_name,
        upsert_count: preview.upsert_count,
        delete_count: preview.delete_count,
        asset_write_count: preview.asset_write_count,
        conflicts: preview.conflicts,
    })
}

#[tauri::command]
pub fn commit_device_sync(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: CommitDeviceSyncInput,
) -> Result<CommitDeviceSyncResultDto, CommandError> {
    validate_entity_id(&input.preview_id, "device sync preview")?;
    let path = preview_path(paths.inner(), &input.preview_id);
    let bytes = read_bounded_file(&path, MAX_SYNC_BYTES)?;
    let mut preview: StoredSyncPreview = serde_json::from_slice(&bytes).map_err(|_| {
        CommandError::new(
            "sync_preview_invalid",
            "The saved synchronization preview is invalid.",
        )
    })?;
    let identity = get_or_create_identity(
        database.inner(),
        paths.inner(),
        default_device_name(),
    )?;
    let current_local = database.with_connection(|connection| {
        capture_snapshot(connection, paths.inner(), &identity, true, true, false)
    })?;
    if snapshot_fingerprint(&current_local)? != preview.local_fingerprint {
        return Err(CommandError::new(
            "sync_preview_stale",
            "The local library changed after this synchronization preview. Preview the merge again.",
        ));
    }
    apply_conflict_resolutions(&mut preview, &input.resolutions)?;
    let asset_guards = stage_remote_assets(
        paths.inner(),
        &preview.local_snapshot,
        &preview.merged_snapshot,
    )?;
    let apply_result = database.with_connection(|connection| {
        apply_merged_snapshot(connection, &preview.local_snapshot, &preview.merged_snapshot)
    });
    if let Err(error) = apply_result {
        cleanup_asset_guards(&asset_guards);
        return Err(error);
    }
    finalize_asset_guards(&asset_guards)?;
    let mut base = preview.merged_snapshot.clone();
    for asset in &mut base.assets {
        asset.data_base64 = None;
    }
    write_json_atomic(
        &base_path(paths.inner(), &preview.peer_device_id),
        &base,
    )?;
    let _ = fs::remove_file(path);
    Ok(CommitDeviceSyncResultDto {
        remote_device_id: preview.remote_device_id,
        upserted_rows: preview.upsert_count,
        deleted_rows: preview.delete_count,
        written_assets: preview.asset_write_count,
    })
}

fn capture_snapshot(
    connection: &mut Connection,
    paths: &AppPaths,
    identity: &SyncIdentityDto,
    include_settings: bool,
    include_ai_profiles: bool,
    include_asset_data: bool,
) -> Result<DeviceSyncSnapshot, CommandError> {
    let schemas = load_sync_schemas(connection, include_settings, include_ai_profiles)?;
    if schemas.len() > MAX_SYNC_TABLES {
        return Err(CommandError::new(
            "sync_snapshot_too_large",
            "The synchronization snapshot contains too many table groups.",
        ));
    }
    let order = topological_table_order(&schemas);
    let mut tables = Vec::with_capacity(order.len());
    let mut total_rows = 0_usize;
    for table_name in order {
        let schema = schemas.get(&table_name).ok_or_else(|| {
            CommandError::new("sync_schema_invalid", "A synchronization table disappeared.")
        })?;
        let table = capture_table(connection, schema)?;
        total_rows = total_rows.saturating_add(table.rows.len());
        if total_rows > MAX_SYNC_ROWS {
            return Err(CommandError::new(
                "sync_snapshot_too_large",
                "The synchronization snapshot contains too many rows.",
            ));
        }
        tables.push(table);
    }
    sanitize_ai_profile_metadata(&mut tables);
    let assets = capture_asset_payloads(paths, &tables, include_asset_data)?;
    let snapshot_id = generate_id(connection)?;
    Ok(DeviceSyncSnapshot {
        format: SYNC_FORMAT.to_string(),
        schema_version: SYNC_SCHEMA_VERSION,
        snapshot_id,
        source_device_id: identity.device_id.clone(),
        source_device_name: identity.device_name.clone(),
        created_at_utc_millis: unix_time_millis()?,
        tables,
        assets,
    })
}

fn load_sync_schemas(
    connection: &Connection,
    include_settings: bool,
    include_ai_profiles: bool,
) -> Result<BTreeMap<String, TableSchema>, CommandError> {
    let available = connection
        .prepare(
            "SELECT name FROM sqlite_schema
             WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
        )
        .and_then(|mut statement| {
            statement
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<Result<BTreeSet<_>, _>>()
        })
        .map_err(CommandError::database)?;
    let mut schemas = BTreeMap::new();
    for name in USER_DATA_TABLES {
        if !available.contains(*name) {
            continue;
        }
        if *name == "settings" && !include_settings {
            continue;
        }
        if matches!(*name, "ai_provider_profiles" | "ai_task_defaults") && !include_ai_profiles {
            continue;
        }
        let schema = load_table_schema(connection, name)?;
        if !schema.primary_key.is_empty() {
            schemas.insert((*name).to_string(), schema);
        }
    }
    Ok(schemas)
}

fn load_table_schema(
    connection: &Connection,
    table_name: &str,
) -> Result<TableSchema, CommandError> {
    let pragma = format!("PRAGMA table_info({})", quote_identifier(table_name));
    let mut columns = Vec::new();
    let mut primary = Vec::<(i64, String)>::new();
    connection
        .prepare(&pragma)
        .and_then(|mut statement| {
            let rows = statement.query_map([], |row| {
                Ok((row.get::<_, String>(1)?, row.get::<_, i64>(5)?))
            })?;
            for row in rows {
                let (name, primary_order) = row?;
                columns.push(name.clone());
                if primary_order > 0 {
                    primary.push((primary_order, name));
                }
            }
            Ok(())
        })
        .map_err(CommandError::database)?;
    primary.sort_by_key(|entry| entry.0);
    let foreign_pragma = format!("PRAGMA foreign_key_list({})", quote_identifier(table_name));
    let foreign_parents = connection
        .prepare(&foreign_pragma)
        .and_then(|mut statement| {
            statement
                .query_map([], |row| row.get::<_, String>(2))?
                .collect::<Result<BTreeSet<_>, _>>()
        })
        .map_err(CommandError::database)?;
    Ok(TableSchema {
        name: table_name.to_string(),
        columns,
        primary_key: primary.into_iter().map(|entry| entry.1).collect(),
        foreign_parents,
    })
}

fn capture_table(
    connection: &Connection,
    schema: &TableSchema,
) -> Result<SyncTable, CommandError> {
    let columns_sql = schema
        .columns
        .iter()
        .map(|column| quote_identifier(column))
        .collect::<Vec<_>>()
        .join(", ");
    let where_clause = if schema.name == "assets" {
        " WHERE lifecycle_state = 'committed'"
    } else {
        ""
    };
    let order_sql = schema
        .primary_key
        .iter()
        .map(|column| quote_identifier(column))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT {columns_sql} FROM {}{where_clause} ORDER BY {order_sql}",
        quote_identifier(&schema.name)
    );
    let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
    let mut query = statement.query([]).map_err(CommandError::database)?;
    let mut rows = Vec::new();
    while let Some(row) = query.next().map_err(CommandError::database)? {
        let mut values = BTreeMap::new();
        for (index, column) in schema.columns.iter().enumerate() {
            let mut value = sync_value(row.get_ref(index).map_err(CommandError::database)?)?;
            if schema.name == "ai_provider_profiles" && column == "credential_storage" {
                value = SyncValue::Text("none".to_string());
            }
            values.insert(column.clone(), value);
        }
        let key = row_key(&schema.primary_key, &values)?;
        rows.push(SyncRow { key, values });
    }
    Ok(SyncTable {
        name: schema.name.clone(),
        columns: schema.columns.clone(),
        primary_key: schema.primary_key.clone(),
        rows,
    })
}

fn sync_value(value: ValueRef<'_>) -> Result<SyncValue, CommandError> {
    Ok(match value {
        ValueRef::Null => SyncValue::Null,
        ValueRef::Integer(value) => SyncValue::Integer(value),
        ValueRef::Real(value) if value.is_finite() => SyncValue::Real(value),
        ValueRef::Real(_) => {
            return Err(CommandError::new(
                "sync_value_invalid",
                "A synchronization row contains a non-finite number.",
            ))
        }
        ValueRef::Text(value) => SyncValue::Text(
            std::str::from_utf8(value)
                .map_err(|_| {
                    CommandError::new(
                        "sync_value_invalid",
                        "A synchronization text value is not valid UTF-8.",
                    )
                })?
                .to_string(),
        ),
        ValueRef::Blob(value) => SyncValue::Blob(BASE64_STANDARD.encode(value)),
    })
}

fn row_key(
    primary_key: &[String],
    values: &BTreeMap<String, SyncValue>,
) -> Result<String, CommandError> {
    let selected = primary_key
        .iter()
        .map(|column| {
            values.get(column).cloned().ok_or_else(|| {
                CommandError::new(
                    "sync_schema_invalid",
                    "A synchronization row is missing a primary-key value.",
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    serde_json::to_string(&selected).map_err(CommandError::serialization)
}

fn capture_asset_payloads(
    paths: &AppPaths,
    tables: &[SyncTable],
    include_data: bool,
) -> Result<Vec<SyncAssetPayload>, CommandError> {
    let Some(table) = tables.iter().find(|table| table.name == "assets") else {
        return Ok(Vec::new());
    };
    let mut total = 0_u64;
    let mut payloads = Vec::new();
    for row in &table.rows {
        let asset_id = text_column(row, "id")?;
        let relative_path = text_column(row, "relative_path")?;
        validate_relative_asset_path(&relative_path)?;
        let path = paths.resolve_relative(&relative_path);
        let metadata = fs::metadata(&path).map_err(CommandError::io)?;
        if !metadata.is_file() || metadata.len() == 0 || metadata.len() > MAX_SYNC_ASSET_BYTES {
            return Err(CommandError::new(
                "sync_asset_invalid",
                "A synchronized card image is missing or exceeds the per-file limit.",
            ));
        }
        total = total.saturating_add(metadata.len());
        if total > MAX_SYNC_TOTAL_ASSET_BYTES {
            return Err(CommandError::new(
                "sync_snapshot_too_large",
                "The synchronized card images exceed the total transfer limit.",
            ));
        }
        let data = fs::read(&path).map_err(CommandError::io)?;
        let sha256 = sha256_hex(&data);
        payloads.push(SyncAssetPayload {
            asset_id,
            relative_path,
            byte_size: metadata.len(),
            sha256,
            data_base64: include_data.then(|| BASE64_STANDARD.encode(data)),
        });
    }
    payloads.sort_by(|left, right| left.asset_id.cmp(&right.asset_id));
    Ok(payloads)
}

fn sanitize_ai_profile_metadata(tables: &mut [SyncTable]) {
    let Some(table) = tables
        .iter_mut()
        .find(|table| table.name == "ai_provider_profiles")
    else {
        return;
    };
    for row in &mut table.rows {
        if row.values.contains_key("credential_storage") {
            row.values.insert(
                "credential_storage".to_string(),
                SyncValue::Text("none".to_string()),
            );
        }
    }
}

fn merge_snapshots(
    base: &DeviceSyncSnapshot,
    local: &DeviceSyncSnapshot,
    remote: &DeviceSyncSnapshot,
) -> Result<MergePlan, CommandError> {
    validate_snapshot(base, false)?;
    validate_snapshot(local, false)?;
    validate_snapshot(remote, true)?;
    let table_names = base
        .tables
        .iter()
        .chain(&local.tables)
        .chain(&remote.tables)
        .map(|table| table.name.clone())
        .collect::<BTreeSet<_>>();
    let mut merged_tables = Vec::new();
    let mut conflicts = Vec::new();
    let mut upsert_count = 0_usize;
    let mut delete_count = 0_usize;
    for table_name in table_names {
        let base_table = find_table(base, &table_name);
        let local_table = find_table(local, &table_name);
        let remote_table = find_table(remote, &table_name);
        let schema = compatible_table_schema(base_table, local_table, remote_table)?;
        let base_rows = rows_by_key(base_table);
        let local_rows = rows_by_key(local_table);
        let remote_rows = rows_by_key(remote_table);
        let row_keys = base_rows
            .keys()
            .chain(local_rows.keys())
            .chain(remote_rows.keys())
            .cloned()
            .collect::<BTreeSet<_>>();
        let mut merged_rows = Vec::new();
        for key in row_keys {
            let base_row = base_rows.get(&key).copied();
            let local_row = local_rows.get(&key).copied();
            let remote_row = remote_rows.get(&key).copied();
            let before = local_row.cloned();
            let merged = merge_row(
                &table_name,
                &key,
                base_row,
                local_row,
                remote_row,
                &mut conflicts,
            )?;
            if merged != before {
                if merged.is_some() {
                    upsert_count = upsert_count.saturating_add(1);
                } else if before.is_some() {
                    delete_count = delete_count.saturating_add(1);
                }
            }
            if let Some(row) = merged {
                merged_rows.push(row);
            }
        }
        merged_tables.push(SyncTable {
            name: table_name,
            columns: schema.0,
            primary_key: schema.1,
            rows: merged_rows,
        });
    }
    if conflicts.len() > MAX_SYNC_CONFLICTS {
        return Err(CommandError::new(
            "sync_conflict_limit",
            "This merge produced too many conflicts to review safely.",
        ));
    }
    let merged_assets = merge_assets(base, local, remote)?;
    let local_asset_hashes = local
        .assets
        .iter()
        .map(|asset| (asset.asset_id.as_str(), asset.sha256.as_str()))
        .collect::<HashMap<_, _>>();
    let asset_write_count = merged_assets
        .iter()
        .filter(|asset| {
            local_asset_hashes.get(asset.asset_id.as_str()).copied()
                != Some(asset.sha256.as_str())
        })
        .count();
    Ok(MergePlan {
        merged: DeviceSyncSnapshot {
            format: SYNC_FORMAT.to_string(),
            schema_version: SYNC_SCHEMA_VERSION,
            snapshot_id: remote.snapshot_id.clone(),
            source_device_id: local.source_device_id.clone(),
            source_device_name: local.source_device_name.clone(),
            created_at_utc_millis: remote
                .created_at_utc_millis
                .max(local.created_at_utc_millis),
            tables: merged_tables,
            assets: merged_assets,
        },
        conflicts,
        upsert_count,
        delete_count,
        asset_write_count,
    })
}

fn merge_row(
    table: &str,
    row_key: &str,
    base: Option<&SyncRow>,
    local: Option<&SyncRow>,
    remote: Option<&SyncRow>,
    conflicts: &mut Vec<SyncConflictDto>,
) -> Result<Option<SyncRow>, CommandError> {
    if local == remote {
        return Ok(local.cloned());
    }
    if local == base {
        return Ok(remote.cloned());
    }
    if remote == base {
        return Ok(local.cloned());
    }
    match (base, local, remote) {
        (None, Some(local), Some(remote)) => {
            if APPEND_ONLY_TABLES.contains(&table) {
                conflicts.push(conflict(
                    table,
                    row_key,
                    None,
                    SyncConflictKind::ImmutableRowMismatch,
                    None,
                    None,
                    None,
                ));
                return Ok(Some(local.clone()));
            }
            merge_present_rows(table, row_key, None, local, remote, conflicts).map(Some)
        }
        (Some(base), None, Some(remote)) => {
            conflicts.push(conflict(
                table,
                row_key,
                None,
                SyncConflictKind::DeleteVsEdit,
                None,
                None,
                None,
            ));
            Ok(Some(remote.clone()))
        }
        (Some(base), Some(local), None) => {
            conflicts.push(conflict(
                table,
                row_key,
                None,
                SyncConflictKind::DeleteVsEdit,
                None,
                None,
                None,
            ));
            Ok(Some(local.clone()))
        }
        (Some(base), Some(local), Some(remote)) => {
            if APPEND_ONLY_TABLES.contains(&table) {
                conflicts.push(conflict(
                    table,
                    row_key,
                    None,
                    SyncConflictKind::ImmutableRowMismatch,
                    None,
                    None,
                    None,
                ));
                return Ok(Some(local.clone()));
            }
            merge_present_rows(table, row_key, Some(base), local, remote, conflicts).map(Some)
        }
        (None, Some(local), None) => Ok(Some(local.clone())),
        (None, None, Some(remote)) => Ok(Some(remote.clone())),
        (_, None, None) => Ok(None),
    }
}

fn merge_present_rows(
    table: &str,
    row_key: &str,
    base: Option<&SyncRow>,
    local: &SyncRow,
    remote: &SyncRow,
    conflicts: &mut Vec<SyncConflictDto>,
) -> Result<SyncRow, CommandError> {
    let columns = local
        .values
        .keys()
        .chain(remote.values.keys())
        .chain(base.into_iter().flat_map(|row| row.values.keys()))
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut values = BTreeMap::new();
    for column in columns {
        let base_value = base.and_then(|row| row.values.get(&column));
        let local_value = local.values.get(&column);
        let remote_value = remote.values.get(&column);
        let selected = if VOLATILE_MERGE_COLUMNS.contains(&column.as_str()) {
            merge_volatile_column(&column, base_value, local_value, remote_value)
        } else if local_value == remote_value {
            local_value.cloned()
        } else if local_value == base_value {
            remote_value.cloned()
        } else if remote_value == base_value {
            local_value.cloned()
        } else {
            conflicts.push(conflict(
                table,
                row_key,
                Some(column.clone()),
                if base.is_none() {
                    SyncConflictKind::ConcurrentInsert
                } else {
                    SyncConflictKind::ConcurrentFieldEdit
                },
                base_value.cloned(),
                local_value.cloned(),
                remote_value.cloned(),
            ));
            local_value.cloned()
        };
        if let Some(selected) = selected {
            values.insert(column, selected);
        }
    }
    Ok(SyncRow {
        key: row_key.to_string(),
        values,
    })
}

fn merge_volatile_column(
    column: &str,
    base: Option<&SyncValue>,
    local: Option<&SyncValue>,
    remote: Option<&SyncValue>,
) -> Option<SyncValue> {
    if column == "revision" {
        let maximum = [base, local, remote]
            .into_iter()
            .flatten()
            .filter_map(integer_value)
            .max()?;
        return Some(SyncValue::Integer(maximum.saturating_add(1)));
    }
    if column == "updated_at" {
        let maximum = [base, local, remote]
            .into_iter()
            .flatten()
            .filter_map(integer_value)
            .max()?;
        return Some(SyncValue::Integer(maximum));
    }
    local.cloned().or_else(|| remote.cloned()).or_else(|| base.cloned())
}

fn conflict(
    table: &str,
    row_key: &str,
    column: Option<String>,
    kind: SyncConflictKind,
    base_value: Option<SyncValue>,
    local_value: Option<SyncValue>,
    remote_value: Option<SyncValue>,
) -> SyncConflictDto {
    let mut hasher = Sha256::new();
    hasher.update(table.as_bytes());
    hasher.update([0]);
    hasher.update(row_key.as_bytes());
    hasher.update([0]);
    hasher.update(column.as_deref().unwrap_or("<row>").as_bytes());
    SyncConflictDto {
        id: format!("{:x}", hasher.finalize())[..32].to_string(),
        table: table.to_string(),
        row_key: row_key.to_string(),
        column,
        kind,
        base_value,
        local_value,
        remote_value,
    }
}

fn merge_assets(
    base: &DeviceSyncSnapshot,
    local: &DeviceSyncSnapshot,
    remote: &DeviceSyncSnapshot,
) -> Result<Vec<SyncAssetPayload>, CommandError> {
    let base_map = asset_map(base);
    let local_map = asset_map(local);
    let remote_map = asset_map(remote);
    let ids = base_map
        .keys()
        .chain(local_map.keys())
        .chain(remote_map.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut merged = Vec::new();
    for id in ids {
        let base = base_map.get(&id).copied();
        let local = local_map.get(&id).copied();
        let remote = remote_map.get(&id).copied();
        let selected = if same_asset(local, remote) {
            local.or(remote)
        } else if same_asset(local, base) {
            remote
        } else if same_asset(remote, base) {
            local
        } else {
            local.or(remote)
        };
        if let Some(asset) = selected {
            merged.push(asset.clone());
        }
    }
    merged.sort_by(|left, right| left.asset_id.cmp(&right.asset_id));
    Ok(merged)
}

fn apply_conflict_resolutions(
    preview: &mut StoredSyncPreview,
    resolutions: &[SyncConflictResolutionInput],
) -> Result<(), CommandError> {
    let supplied = resolutions
        .iter()
        .map(|resolution| (resolution.conflict_id.as_str(), resolution))
        .collect::<HashMap<_, _>>();
    for conflict in &preview.conflicts {
        let resolution = supplied.get(conflict.id.as_str()).ok_or_else(|| {
            CommandError::new(
                "sync_conflicts_unresolved",
                "Choose a resolution for every synchronization conflict.",
            )
        })?;
        let selected = match resolution.choice {
            SyncResolutionChoice::Local => conflict.local_value.clone(),
            SyncResolutionChoice::Remote => conflict.remote_value.clone(),
            SyncResolutionChoice::Custom => resolution.custom_value.clone(),
        };
        if let Some(column) = &conflict.column {
            let row = find_row_mut(
                &mut preview.merged_snapshot,
                &conflict.table,
                &conflict.row_key,
            )
            .ok_or_else(|| {
                CommandError::new(
                    "sync_preview_invalid",
                    "A field conflict no longer has a merged row.",
                )
            })?;
            match selected {
                Some(value) => {
                    row.values.insert(column.clone(), value);
                }
                None => {
                    row.values.remove(column);
                }
            }
        } else {
            let selected_row = match resolution.choice {
                SyncResolutionChoice::Local => find_row(
                    &preview.local_snapshot,
                    &conflict.table,
                    &conflict.row_key,
                ),
                SyncResolutionChoice::Remote => find_row(
                    &preview.remote_snapshot,
                    &conflict.table,
                    &conflict.row_key,
                ),
                SyncResolutionChoice::Custom => {
                    return Err(CommandError::new(
                        "sync_resolution_invalid",
                        "Whole-row conflicts cannot use a custom scalar value.",
                    ))
                }
            }
            .cloned();
            replace_row(
                &mut preview.merged_snapshot,
                &conflict.table,
                &conflict.row_key,
                selected_row,
            )?;
        }
    }
    Ok(())
}

fn apply_merged_snapshot(
    connection: &mut Connection,
    local: &DeviceSyncSnapshot,
    merged: &DeviceSyncSnapshot,
) -> Result<(), CommandError> {
    validate_snapshot(merged, false)?;
    let schemas = load_sync_schemas(connection, true, true)?;
    let order = topological_table_order(&schemas);
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    transaction
        .execute_batch("PRAGMA defer_foreign_keys = ON;")
        .map_err(CommandError::database)?;
    for table_name in order.iter().rev() {
        let Some(schema) = schemas.get(table_name) else {
            continue;
        };
        let local_rows = rows_by_key(find_table(local, table_name));
        let merged_rows = rows_by_key(find_table(merged, table_name));
        for key in local_rows.keys() {
            if !merged_rows.contains_key(key) {
                delete_row(&transaction, schema, local_rows[key])?;
            }
        }
    }
    for table_name in order {
        let Some(schema) = schemas.get(&table_name) else {
            continue;
        };
        let local_rows = rows_by_key(find_table(local, &table_name));
        let merged_rows = rows_by_key(find_table(merged, &table_name));
        for (key, row) in merged_rows {
            if local_rows.get(key).copied() != Some(row) {
                upsert_row(&transaction, schema, row)?;
            }
        }
    }
    transaction.commit().map_err(CommandError::database)
}

fn upsert_row(
    transaction: &Transaction<'_>,
    schema: &TableSchema,
    row: &SyncRow,
) -> Result<(), CommandError> {
    let columns = schema
        .columns
        .iter()
        .filter(|column| row.values.contains_key(*column))
        .cloned()
        .collect::<Vec<_>>();
    let placeholders = (1..=columns.len())
        .map(|index| format!("?{index}"))
        .collect::<Vec<_>>()
        .join(", ");
    let update_columns = columns
        .iter()
        .filter(|column| !schema.primary_key.contains(column))
        .map(|column| {
            format!(
                "{} = excluded.{}",
                quote_identifier(column),
                quote_identifier(column)
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    let conflict = schema
        .primary_key
        .iter()
        .map(|column| quote_identifier(column))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = if update_columns.is_empty() {
        format!(
            "INSERT OR IGNORE INTO {} ({}) VALUES ({placeholders})",
            quote_identifier(&schema.name),
            columns
                .iter()
                .map(|column| quote_identifier(column))
                .collect::<Vec<_>>()
                .join(", ")
        )
    } else {
        format!(
            "INSERT INTO {} ({}) VALUES ({placeholders})
             ON CONFLICT ({conflict}) DO UPDATE SET {update_columns}",
            quote_identifier(&schema.name),
            columns
                .iter()
                .map(|column| quote_identifier(column))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    let values = columns
        .iter()
        .map(|column| sqlite_value(&row.values[column]))
        .collect::<Result<Vec<_>, _>>()?;
    transaction
        .execute(&sql, params_from_iter(values))
        .map_err(CommandError::database)?;
    Ok(())
}

fn delete_row(
    transaction: &Transaction<'_>,
    schema: &TableSchema,
    row: &SyncRow,
) -> Result<(), CommandError> {
    let predicates = schema
        .primary_key
        .iter()
        .enumerate()
        .map(|(index, column)| format!("{} = ?{}", quote_identifier(column), index + 1))
        .collect::<Vec<_>>()
        .join(" AND ");
    let values = schema
        .primary_key
        .iter()
        .map(|column| {
            row.values.get(column).ok_or_else(|| {
                CommandError::new(
                    "sync_schema_invalid",
                    "A deleted synchronization row is missing its primary key.",
                )
            })
        })
        .map(|value| value.and_then(sqlite_value))
        .collect::<Result<Vec<_>, _>>()?;
    transaction
        .execute(
            &format!(
                "DELETE FROM {} WHERE {predicates}",
                quote_identifier(&schema.name)
            ),
            params_from_iter(values),
        )
        .map_err(CommandError::database)?;
    Ok(())
}

fn sqlite_value(value: &SyncValue) -> Result<SqliteValue, CommandError> {
    Ok(match value {
        SyncValue::Null => SqliteValue::Null,
        SyncValue::Integer(value) => SqliteValue::Integer(*value),
        SyncValue::Real(value) if value.is_finite() => SqliteValue::Real(*value),
        SyncValue::Real(_) => {
            return Err(CommandError::new(
                "sync_value_invalid",
                "A synchronization value contains a non-finite number.",
            ))
        }
        SyncValue::Text(value) => SqliteValue::Text(value.clone()),
        SyncValue::Blob(value) => SqliteValue::Blob(
            BASE64_STANDARD.decode(value).map_err(|_| {
                CommandError::new(
                    "sync_value_invalid",
                    "A synchronization blob is not valid base64.",
                )
            })?,
        ),
    })
}

#[derive(Clone, Debug)]
struct AssetWriteGuard {
    temporary: PathBuf,
    destination: PathBuf,
    existed: bool,
}

fn stage_remote_assets(
    paths: &AppPaths,
    local: &DeviceSyncSnapshot,
    merged: &DeviceSyncSnapshot,
) -> Result<Vec<AssetWriteGuard>, CommandError> {
    let local_hashes = local
        .assets
        .iter()
        .map(|asset| (asset.asset_id.as_str(), asset.sha256.as_str()))
        .collect::<HashMap<_, _>>();
    let staging = sync_root(paths).join("asset-staging");
    fs::create_dir_all(&staging).map_err(CommandError::io)?;
    let mut guards = Vec::new();
    for asset in &merged.assets {
        if local_hashes.get(asset.asset_id.as_str()).copied() == Some(asset.sha256.as_str()) {
            continue;
        }
        validate_relative_asset_path(&asset.relative_path)?;
        if asset.byte_size == 0 || asset.byte_size > MAX_SYNC_ASSET_BYTES {
            cleanup_asset_guards(&guards);
            return Err(CommandError::new(
                "sync_asset_invalid",
                "A synchronized image has an invalid byte size.",
            ));
        }
        let encoded = asset.data_base64.as_deref().ok_or_else(|| {
            CommandError::new(
                "sync_asset_missing",
                "A changed synchronized image does not contain its file payload.",
            )
        })?;
        let data = BASE64_STANDARD.decode(encoded).map_err(|_| {
            CommandError::new(
                "sync_asset_invalid",
                "A synchronized image payload is not valid base64.",
            )
        })?;
        if data.len() as u64 != asset.byte_size || sha256_hex(&data) != asset.sha256 {
            cleanup_asset_guards(&guards);
            return Err(CommandError::new(
                "sync_asset_invalid",
                "A synchronized image failed its size or digest check.",
            ));
        }
        let destination = paths.resolve_relative(&asset.relative_path);
        let temporary = staging.join(format!("{}.part", asset.asset_id));
        if let Some(parent) = temporary.parent() {
            fs::create_dir_all(parent).map_err(CommandError::io)?;
        }
        write_new_file(&temporary, &data)?;
        guards.push(AssetWriteGuard {
            temporary,
            existed: destination.exists(),
            destination,
        });
    }
    Ok(guards)
}

fn finalize_asset_guards(guards: &[AssetWriteGuard]) -> Result<(), CommandError> {
    for guard in guards {
        if let Some(parent) = guard.destination.parent() {
            fs::create_dir_all(parent).map_err(CommandError::io)?;
        }
        if guard.existed {
            let backup = guard.destination.with_extension("sync-previous");
            let _ = fs::remove_file(&backup);
            fs::rename(&guard.destination, &backup).map_err(CommandError::io)?;
            if let Err(error) = fs::rename(&guard.temporary, &guard.destination) {
                let _ = fs::rename(&backup, &guard.destination);
                return Err(CommandError::io(error));
            }
            let _ = fs::remove_file(backup);
        } else {
            fs::rename(&guard.temporary, &guard.destination).map_err(CommandError::io)?;
        }
        sync_parent(&guard.destination)?;
    }
    Ok(())
}

fn cleanup_asset_guards(guards: &[AssetWriteGuard]) {
    for guard in guards {
        let _ = fs::remove_file(&guard.temporary);
    }
}

fn validate_snapshot(snapshot: &DeviceSyncSnapshot, require_payloads: bool) -> Result<(), CommandError> {
    if snapshot.format != SYNC_FORMAT || snapshot.schema_version != SYNC_SCHEMA_VERSION {
        return Err(CommandError::new(
            "sync_format_unsupported",
            "This synchronization snapshot uses an unsupported format version.",
        ));
    }
    validate_entity_id(&snapshot.snapshot_id, "synchronization snapshot")?;
    validate_entity_id(&snapshot.source_device_id, "synchronization source device")?;
    validate_device_name(snapshot.source_device_name.clone())?;
    if snapshot.tables.len() > MAX_SYNC_TABLES {
        return Err(CommandError::new(
            "sync_snapshot_too_large",
            "The synchronization snapshot contains too many table groups.",
        ));
    }
    let mut total_rows = 0_usize;
    let mut seen_tables = BTreeSet::new();
    for table in &snapshot.tables {
        if !USER_DATA_TABLES.contains(&table.name.as_str()) || !seen_tables.insert(&table.name) {
            return Err(CommandError::new(
                "sync_table_unsupported",
                "The synchronization snapshot contains an unsupported table.",
            ));
        }
        total_rows = total_rows.saturating_add(table.rows.len());
        if total_rows > MAX_SYNC_ROWS {
            return Err(CommandError::new(
                "sync_snapshot_too_large",
                "The synchronization snapshot contains too many rows.",
            ));
        }
        let mut seen_rows = BTreeSet::new();
        for row in &table.rows {
            if !seen_rows.insert(&row.key) {
                return Err(CommandError::new(
                    "sync_snapshot_invalid",
                    "The synchronization snapshot contains a duplicate row identity.",
                ));
            }
        }
    }
    let mut total_assets = 0_u64;
    let mut seen_assets = BTreeSet::new();
    for asset in &snapshot.assets {
        validate_entity_id(&asset.asset_id, "synchronized asset")?;
        validate_relative_asset_path(&asset.relative_path)?;
        if !seen_assets.insert(&asset.asset_id) || asset.byte_size > MAX_SYNC_ASSET_BYTES {
            return Err(CommandError::new(
                "sync_asset_invalid",
                "The synchronization snapshot contains an invalid image record.",
            ));
        }
        total_assets = total_assets.saturating_add(asset.byte_size);
        if total_assets > MAX_SYNC_TOTAL_ASSET_BYTES {
            return Err(CommandError::new(
                "sync_snapshot_too_large",
                "The synchronization snapshot contains too many image bytes.",
            ));
        }
        if require_payloads && asset.data_base64.is_none() {
            return Err(CommandError::new(
                "sync_asset_missing",
                "The imported synchronization snapshot omits an image payload.",
            ));
        }
    }
    Ok(())
}

fn read_snapshot(path: &Path) -> Result<DeviceSyncSnapshot, CommandError> {
    let bytes = read_bounded_file(path, MAX_SYNC_BYTES)?;
    let snapshot = serde_json::from_slice::<DeviceSyncSnapshot>(&bytes).map_err(|_| {
        CommandError::new(
            "sync_snapshot_invalid",
            "The selected synchronization file is not a valid ostudy snapshot.",
        )
    })?;
    validate_snapshot(&snapshot, true)?;
    Ok(snapshot)
}

fn read_base_snapshot(
    paths: &AppPaths,
    peer_device_id: &str,
) -> Result<Option<DeviceSyncSnapshot>, CommandError> {
    let path = base_path(paths, peer_device_id);
    if !path.is_file() {
        return Ok(None);
    }
    let bytes = read_bounded_file(&path, MAX_SYNC_BYTES)?;
    let snapshot = serde_json::from_slice::<DeviceSyncSnapshot>(&bytes).map_err(|_| {
        CommandError::new(
            "sync_base_invalid",
            "The saved synchronization base is invalid. Remove and pair the device again.",
        )
    })?;
    validate_snapshot(&snapshot, false)?;
    Ok(Some(snapshot))
}

fn empty_base_snapshot(
    local_identity: &SyncIdentityDto,
    remote: &DeviceSyncSnapshot,
) -> DeviceSyncSnapshot {
    DeviceSyncSnapshot {
        format: SYNC_FORMAT.to_string(),
        schema_version: SYNC_SCHEMA_VERSION,
        snapshot_id: remote.snapshot_id.clone(),
        source_device_id: local_identity.device_id.clone(),
        source_device_name: local_identity.device_name.clone(),
        created_at_utc_millis: 0,
        tables: remote
            .tables
            .iter()
            .map(|table| SyncTable {
                name: table.name.clone(),
                columns: table.columns.clone(),
                primary_key: table.primary_key.clone(),
                rows: Vec::new(),
            })
            .collect(),
        assets: Vec::new(),
    }
}

fn compatible_table_schema(
    base: Option<&SyncTable>,
    local: Option<&SyncTable>,
    remote: Option<&SyncTable>,
) -> Result<(Vec<String>, Vec<String>), CommandError> {
    let candidates = [base, local, remote].into_iter().flatten().collect::<Vec<_>>();
    let first = candidates.first().ok_or_else(|| {
        CommandError::new("sync_schema_invalid", "A synchronization table has no schema.")
    })?;
    for candidate in &candidates[1..] {
        if candidate.columns != first.columns || candidate.primary_key != first.primary_key {
            return Err(CommandError::new(
                "sync_schema_mismatch",
                "The devices use incompatible synchronization table schemas.",
            ));
        }
    }
    Ok((first.columns.clone(), first.primary_key.clone()))
}

fn topological_table_order(schemas: &BTreeMap<String, TableSchema>) -> Vec<String> {
    let mut indegree = schemas
        .keys()
        .map(|name| (name.clone(), 0_usize))
        .collect::<BTreeMap<_, _>>();
    let mut children = BTreeMap::<String, BTreeSet<String>>::new();
    for (name, schema) in schemas {
        for parent in &schema.foreign_parents {
            if schemas.contains_key(parent) && parent != name {
                *indegree.entry(name.clone()).or_default() += 1;
                children.entry(parent.clone()).or_default().insert(name.clone());
            }
        }
    }
    let mut queue = indegree
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(name, _)| name.clone())
        .collect::<VecDeque<_>>();
    let mut order = Vec::new();
    while let Some(name) = queue.pop_front() {
        order.push(name.clone());
        if let Some(next) = children.get(&name) {
            for child in next {
                let degree = indegree.get_mut(child).expect("known child");
                *degree = degree.saturating_sub(1);
                if *degree == 0 {
                    queue.push_back(child.clone());
                }
            }
        }
    }
    if order.len() != schemas.len() {
        for name in schemas.keys() {
            if !order.contains(name) {
                order.push(name.clone());
            }
        }
    }
    order
}

fn get_or_create_identity(
    database: &Database,
    paths: &AppPaths,
    device_name: String,
) -> Result<SyncIdentityDto, CommandError> {
    let name = validate_device_name(device_name)?;
    let path = sync_root(paths).join("identity.json");
    if path.is_file() {
        let bytes = read_bounded_file(&path, 16 * 1024)?;
        let mut identity: SyncIdentityDto = serde_json::from_slice(&bytes).map_err(|_| {
            CommandError::new("sync_identity_invalid", "The saved sync identity is invalid.")
        })?;
        if identity.device_name != name {
            identity.device_name = name;
            write_json_atomic(&path, &identity)?;
        }
        return Ok(identity);
    }
    let device_id = database.with_connection(generate_id)?;
    let identity = SyncIdentityDto {
        device_id,
        device_name: name,
    };
    write_json_atomic(&path, &identity)?;
    Ok(identity)
}

fn sync_root(paths: &AppPaths) -> PathBuf {
    paths.app_data_dir().join("device-sync")
}

fn base_path(paths: &AppPaths, peer_device_id: &str) -> PathBuf {
    sync_root(paths)
        .join("bases")
        .join(format!("{peer_device_id}.json"))
}

fn preview_path(paths: &AppPaths, preview_id: &str) -> PathBuf {
    sync_root(paths)
        .join("previews")
        .join(format!("{preview_id}.json"))
}

fn write_json_atomic(path: &Path, value: &impl Serialize) -> Result<(), CommandError> {
    let bytes = serde_json::to_vec(value).map_err(CommandError::serialization)?;
    if bytes.len() as u64 > MAX_SYNC_BYTES {
        return Err(CommandError::new(
            "sync_snapshot_too_large",
            "The synchronization snapshot exceeds the file-size limit.",
        ));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(CommandError::io)?;
    }
    let temporary = path.with_extension("json.part");
    let _ = fs::remove_file(&temporary);
    write_new_file(&temporary, &bytes)?;
    if path.exists() {
        let backup = path.with_extension("json.previous");
        let _ = fs::remove_file(&backup);
        fs::rename(path, &backup).map_err(CommandError::io)?;
        if let Err(error) = fs::rename(&temporary, path) {
            let _ = fs::rename(&backup, path);
            return Err(CommandError::io(error));
        }
        let _ = fs::remove_file(backup);
    } else {
        fs::rename(&temporary, path).map_err(CommandError::io)?;
    }
    sync_parent(path)
}

fn write_new_file(path: &Path, bytes: &[u8]) -> Result<(), CommandError> {
    let mut options = fs::OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path).map_err(CommandError::io)?;
    file.write_all(bytes).map_err(CommandError::io)?;
    file.sync_all().map_err(CommandError::io)
}

fn read_bounded_file(path: &Path, maximum: u64) -> Result<Vec<u8>, CommandError> {
    let metadata = fs::metadata(path).map_err(CommandError::io)?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > maximum {
        return Err(CommandError::new(
            "sync_file_invalid",
            "The synchronization file is empty or exceeds its size limit.",
        ));
    }
    fs::read(path).map_err(CommandError::io)
}

fn sync_parent(path: &Path) -> Result<(), CommandError> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    let directory = fs::File::open(parent).map_err(CommandError::io)?;
    directory.sync_all().map_err(CommandError::io)
}

fn snapshot_fingerprint(snapshot: &DeviceSyncSnapshot) -> Result<String, CommandError> {
    let bytes = serde_json::to_vec(snapshot).map_err(CommandError::serialization)?;
    Ok(sha256_hex(&bytes))
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn quote_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

fn validate_relative_asset_path(value: &str) -> Result<(), CommandError> {
    let path = Path::new(value);
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(CommandError::new(
            "sync_asset_path_invalid",
            "A synchronized image path escapes managed application storage.",
        ));
    }
    Ok(())
}

fn validate_device_name(value: String) -> Result<String, CommandError> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > MAX_DEVICE_NAME_CHARS {
        return Err(CommandError::new(
            "sync_device_name_invalid",
            "Device names must contain between 1 and 100 characters.",
        ));
    }
    Ok(value.to_string())
}

fn default_device_name() -> String {
    std::env::var("HOSTNAME")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "ostudy device".to_string())
}

fn portable_filename(value: &str) -> String {
    let value = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    let trimmed = value.trim_matches('-');
    if trimmed.is_empty() {
        "device".to_string()
    } else {
        trimmed.chars().take(80).collect()
    }
}

fn text_column(row: &SyncRow, column: &str) -> Result<String, CommandError> {
    match row.values.get(column) {
        Some(SyncValue::Text(value)) => Ok(value.clone()),
        _ => Err(CommandError::new(
            "sync_schema_invalid",
            format!("A synchronized row is missing text column '{column}'."),
        )),
    }
}

fn integer_value(value: &SyncValue) -> Option<i64> {
    match value {
        SyncValue::Integer(value) => Some(*value),
        _ => None,
    }
}

fn find_table<'a>(snapshot: &'a DeviceSyncSnapshot, name: &str) -> Option<&'a SyncTable> {
    snapshot.tables.iter().find(|table| table.name == name)
}

fn rows_by_key(table: Option<&SyncTable>) -> BTreeMap<&str, &SyncRow> {
    table
        .into_iter()
        .flat_map(|table| &table.rows)
        .map(|row| (row.key.as_str(), row))
        .collect()
}

fn asset_map(snapshot: &DeviceSyncSnapshot) -> BTreeMap<String, &SyncAssetPayload> {
    snapshot
        .assets
        .iter()
        .map(|asset| (asset.asset_id.clone(), asset))
        .collect()
}

fn same_asset(left: Option<&SyncAssetPayload>, right: Option<&SyncAssetPayload>) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(left), Some(right)) => {
            left.asset_id == right.asset_id
                && left.relative_path == right.relative_path
                && left.byte_size == right.byte_size
                && left.sha256 == right.sha256
        }
        _ => false,
    }
}

fn find_row<'a>(
    snapshot: &'a DeviceSyncSnapshot,
    table: &str,
    key: &str,
) -> Option<&'a SyncRow> {
    find_table(snapshot, table)?
        .rows
        .iter()
        .find(|row| row.key == key)
}

fn find_row_mut<'a>(
    snapshot: &'a mut DeviceSyncSnapshot,
    table: &str,
    key: &str,
) -> Option<&'a mut SyncRow> {
    snapshot
        .tables
        .iter_mut()
        .find(|candidate| candidate.name == table)?
        .rows
        .iter_mut()
        .find(|row| row.key == key)
}

fn replace_row(
    snapshot: &mut DeviceSyncSnapshot,
    table: &str,
    key: &str,
    replacement: Option<SyncRow>,
) -> Result<(), CommandError> {
    let table = snapshot
        .tables
        .iter_mut()
        .find(|candidate| candidate.name == table)
        .ok_or_else(|| {
            CommandError::new(
                "sync_preview_invalid",
                "A synchronization conflict references a missing table.",
            )
        })?;
    table.rows.retain(|row| row.key != key);
    if let Some(replacement) = replacement {
        table.rows.push(replacement);
        table.rows.sort_by(|left, right| left.key.cmp(&right.key));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(key: &str, title: &str, description: &str, revision: i64) -> SyncRow {
        SyncRow {
            key: key.to_string(),
            values: BTreeMap::from([
                ("id".to_string(), SyncValue::Text(key.to_string())),
                ("name".to_string(), SyncValue::Text(title.to_string())),
                (
                    "description".to_string(),
                    SyncValue::Text(description.to_string()),
                ),
                ("revision".to_string(), SyncValue::Integer(revision)),
            ]),
        }
    }

    #[test]
    fn disjoint_field_edits_merge_without_conflict() {
        let base = row("deck", "Base", "Base description", 1);
        let local = row("deck", "Local title", "Base description", 2);
        let remote = row("deck", "Base", "Remote description", 2);
        let mut conflicts = Vec::new();
        let merged = merge_row(
            "decks",
            "deck",
            Some(&base),
            Some(&local),
            Some(&remote),
            &mut conflicts,
        )
        .unwrap()
        .unwrap();
        assert!(conflicts.is_empty());
        assert_eq!(
            merged.values.get("name"),
            Some(&SyncValue::Text("Local title".to_string()))
        );
        assert_eq!(
            merged.values.get("description"),
            Some(&SyncValue::Text("Remote description".to_string()))
        );
    }

    #[test]
    fn same_field_edit_becomes_explicit_conflict() {
        let base = row("deck", "Base", "Description", 1);
        let local = row("deck", "Local", "Description", 2);
        let remote = row("deck", "Remote", "Description", 2);
        let mut conflicts = Vec::new();
        let merged = merge_row(
            "decks",
            "deck",
            Some(&base),
            Some(&local),
            Some(&remote),
            &mut conflicts,
        )
        .unwrap()
        .unwrap();
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].column.as_deref(), Some("name"));
        assert_eq!(
            merged.values.get("name"),
            Some(&SyncValue::Text("Local".to_string()))
        );
    }

    #[test]
    fn delete_vs_edit_preserves_data_until_user_resolves() {
        let base = row("deck", "Base", "Description", 1);
        let remote = row("deck", "Remote", "Description", 2);
        let mut conflicts = Vec::new();
        let merged = merge_row(
            "decks",
            "deck",
            Some(&base),
            None,
            Some(&remote),
            &mut conflicts,
        )
        .unwrap();
        assert_eq!(conflicts.len(), 1);
        assert!(merged.is_some());
    }

    #[test]
    fn review_rows_union_when_identifiers_are_distinct() {
        let local = row("review-a", "Local", "", 1);
        let remote = row("review-b", "Remote", "", 1);
        let mut conflicts = Vec::new();
        assert!(merge_row(
            "review_events",
            "review-a",
            None,
            Some(&local),
            None,
            &mut conflicts,
        )
        .unwrap()
        .is_some());
        assert!(merge_row(
            "review_events",
            "review-b",
            None,
            None,
            Some(&remote),
            &mut conflicts,
        )
        .unwrap()
        .is_some());
        assert!(conflicts.is_empty());
    }

    #[test]
    fn topological_order_places_parent_before_child() {
        let schemas = BTreeMap::from([
            (
                "decks".to_string(),
                TableSchema {
                    name: "decks".to_string(),
                    columns: vec!["id".to_string()],
                    primary_key: vec!["id".to_string()],
                    foreign_parents: BTreeSet::new(),
                },
            ),
            (
                "notes".to_string(),
                TableSchema {
                    name: "notes".to_string(),
                    columns: vec!["id".to_string(), "deck_id".to_string()],
                    primary_key: vec!["id".to_string()],
                    foreign_parents: BTreeSet::from(["decks".to_string()]),
                },
            ),
        ]);
        assert_eq!(topological_table_order(&schemas), vec!["decks", "notes"]);
    }
}
