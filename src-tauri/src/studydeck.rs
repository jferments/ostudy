//! Canonical `.studydeck` package boundary.
//!
//! Version 2 writes core content, tags, generated-card metadata, and managed
//! images from one SQLite read snapshot. Version 1 packages remain readable
//! through the tolerant reader in this module, but no current export is built by
//! layering optional tables onto an already-finished file. Imports validate the
//! complete graph before staging files or mutating the library, then commit one
//! deck transaction with a retry-safe operation identity and fresh schedules.

mod export;
mod import;
mod read;

#[cfg(test)]
mod extended_tests;
#[cfg(test)]
mod tests;

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

use rusqlite::{params, Connection, OpenFlags, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tauri::State;

use crate::{
    assets::{detect_image_bytes, validate_original_filename},
    cards::{collect_asset_ids, validate_content},
    database::Database,
    decks::{get_deck_from_connection, validate_description, validate_name, DeckSummaryDto},
    error::CommandError,
    paths::AppPaths,
    util::{generate_id, unix_time_millis, validate_entity_id},
};

const PACKAGE_FORMAT: &str = "studydeck-sqlite";
const CURRENT_PACKAGE_VERSION: i64 = 2;
const LEGACY_PACKAGE_VERSION: i64 = 1;
const MAX_PACKAGE_ASSET_BYTES: usize = 512 * 1024 * 1024;
const MAX_PACKAGE_SINGLE_ASSET_BYTES: usize = 25 * 1024 * 1024;
const MAX_PACKAGE_NOTES: usize = 100_000;
const MAX_PACKAGE_CARDS: usize = 200_000;
const MAX_PACKAGE_TAGS: usize = 100_000;
const MAX_TAGS_PER_NOTE: usize = 32;
const MAX_CLOZE_SOURCE_CHARS: usize = 200_000;

const PACKAGE_SCHEMA: &str = r#"
PRAGMA foreign_keys = ON;
CREATE TABLE package_metadata (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
) STRICT;
CREATE TABLE deck (
    source_id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    sort_order INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    revision INTEGER NOT NULL
) STRICT;
CREATE TABLE notes (
    source_id TEXT PRIMARY KEY,
    source_deck_id TEXT NOT NULL,
    note_type TEXT NOT NULL CHECK(note_type IN ('basic', 'cloze')),
    content_format TEXT NOT NULL CHECK(content_format = 'tiptap-json'),
    content_schema_version INTEGER NOT NULL CHECK(content_schema_version = 1),
    front_doc TEXT NOT NULL,
    back_doc TEXT NOT NULL,
    front_text TEXT NOT NULL,
    back_text TEXT NOT NULL,
    duplicate_hash TEXT NOT NULL,
    reverse_enabled INTEGER NOT NULL CHECK(reverse_enabled IN (0, 1)),
    cloze_source TEXT NOT NULL,
    cloze_mode TEXT NOT NULL CHECK(cloze_mode IN ('one_at_a_time', 'all_together')),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    revision INTEGER NOT NULL
) STRICT;
CREATE TABLE cards (
    source_id TEXT PRIMARY KEY,
    source_note_id TEXT NOT NULL REFERENCES notes(source_id) ON DELETE CASCADE,
    direction TEXT NOT NULL CHECK(direction IN ('forward', 'reverse')),
    template_ordinal INTEGER NOT NULL CHECK(template_ordinal >= 0),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    UNIQUE(source_note_id, direction, template_ordinal)
) STRICT;
CREATE TABLE tags (
    source_id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    normalized_name TEXT NOT NULL UNIQUE,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
) STRICT;
CREATE TABLE note_tags (
    source_note_id TEXT NOT NULL REFERENCES notes(source_id) ON DELETE CASCADE,
    source_tag_id TEXT NOT NULL REFERENCES tags(source_id) ON DELETE CASCADE,
    PRIMARY KEY(source_note_id, source_tag_id)
) STRICT;
CREATE TABLE assets (
    source_id TEXT PRIMARY KEY,
    original_filename TEXT NOT NULL,
    mime_type TEXT NOT NULL,
    byte_size INTEGER NOT NULL CHECK(byte_size > 0),
    data BLOB NOT NULL
) STRICT;
CREATE TABLE note_assets (
    source_note_id TEXT NOT NULL REFERENCES notes(source_id) ON DELETE CASCADE,
    source_asset_id TEXT NOT NULL REFERENCES assets(source_id) ON DELETE RESTRICT,
    side TEXT NOT NULL CHECK(side IN ('front', 'back')),
    PRIMARY KEY(source_note_id, source_asset_id, side)
) STRICT;
"#;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PrepareDeckExportInput {
    pub deck_id: String,
    pub operation_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportDeckInput {
    pub operation_id: String,
    pub staged_file_id: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExportResultDto {
    pub(crate) destination_path: String,
    pub(crate) card_count: i64,
    pub(crate) asset_count: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResultDto {
    pub deck: DeckSummaryDto,
    pub card_count: i64,
    pub asset_count: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct DeckExportSummary {
    card_count: i64,
    asset_count: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedDeckExportDto {
    pub prepared_export: crate::external_files::PreparedExportDto,
    pub card_count: i64,
    pub asset_count: i64,
}

#[tauri::command]
pub fn prepare_deck_export(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: PrepareDeckExportInput,
) -> Result<PreparedDeckExportDto, CommandError> {
    validate_entity_id(&input.deck_id, "deck")?;
    validate_entity_id(&input.operation_id, "deck export operation")?;
    if let Some((prepared_export, summary)) = database.with_connection(|connection| {
        crate::external_files::load_prepared_export_summary::<DeckExportSummary>(
            connection,
            paths.inner(),
            &input.operation_id,
            "studydeck",
        )
    })? {
        return Ok(PreparedDeckExportDto {
            prepared_export,
            card_count: summary.card_count,
            asset_count: summary.asset_count,
        });
    }

    let relative = AppPaths::prepared_export_relative_path(&input.operation_id, "studydeck");
    let destination = paths.resolve_relative(&relative);
    let exported = database.with_connection(|connection| {
        export::export_deck_from_connection(connection, &paths, &input.deck_id, &destination)
    })?;
    let deck_name = database.with_connection(|connection| {
        Ok(get_deck_from_connection(connection, &input.deck_id)?.name)
    })?;
    let summary = DeckExportSummary {
        card_count: exported.card_count,
        asset_count: exported.asset_count,
    };
    let suggested_filename = format!("{}.studydeck", portable_filename(&deck_name));
    let prepared_export = database.with_connection(|connection| {
        crate::external_files::register_prepared_export(
            connection,
            paths.inner(),
            crate::external_files::RegisterPreparedExport {
                id: &input.operation_id,
                purpose: "studydeck",
                suggested_filename: &suggested_filename,
                media_type: "application/vnd.ostudy.studydeck",
                relative_path: &relative,
                summary: &summary,
            },
        )
    })?;
    Ok(PreparedDeckExportDto {
        prepared_export,
        card_count: summary.card_count,
        asset_count: summary.asset_count,
    })
}

#[tauri::command]
pub fn import_deck(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: ImportDeckInput,
) -> Result<ImportResultDto, CommandError> {
    validate_entity_id(&input.operation_id, "deck import operation")?;
    validate_entity_id(&input.staged_file_id, "staged deck")?;
    if let Some(result) = database.with_connection(|connection| {
        crate::external_files::consumed_import_result::<ImportResultDto>(
            connection,
            &input.staged_file_id,
        )
    })? {
        return Ok(result);
    }
    let staged = database.with_connection(|connection| {
        crate::external_files::resolve_staged_import(
            connection,
            paths.inner(),
            &input.staged_file_id,
            crate::external_files::ExternalImportPurpose::StudyContent,
            &[crate::external_files::ExternalFileKind::Studydeck],
        )
    })?;
    let fingerprint = hash_file(&staged.path)?;
    let package = read::read_package(&staged.path)?;
    let result = database.with_connection(|connection| {
        import::import_package(
            connection,
            &paths,
            &input.operation_id,
            &fingerprint,
            package,
        )
    })?;
    database.with_connection(|connection| {
        crate::external_files::mark_import_consumed(
            connection,
            &input.staged_file_id,
            &result,
            unix_time_millis()?,
        )
    })?;
    Ok(result)
}

fn portable_filename(value: &str) -> String {
    let normalized = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, ' ' | '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    let trimmed = normalized.trim().trim_matches('_');
    if trimmed.is_empty() {
        "ostudy-deck".to_string()
    } else {
        trimmed.chars().take(120).collect()
    }
}

#[derive(Clone)]
struct PackageDeck {
    source_id: String,
    name: String,
    description: String,
    sort_order: i64,
    created_at: i64,
    updated_at: i64,
    revision: i64,
}

#[derive(Clone)]
struct PackageNote {
    source_id: String,
    source_deck_id: String,
    note_type: String,
    content_format: String,
    content_schema_version: i64,
    front_doc: Value,
    back_doc: Value,
    front_text: String,
    back_text: String,
    duplicate_hash: String,
    reverse_enabled: bool,
    cloze_source: String,
    cloze_mode: String,
    created_at: i64,
    updated_at: i64,
    revision: i64,
}

#[derive(Clone)]
struct PackageCard {
    source_id: String,
    source_note_id: String,
    direction: String,
    template_ordinal: i64,
    created_at: i64,
    updated_at: i64,
}

#[derive(Clone)]
struct PackageTag {
    source_id: String,
    name: String,
    normalized_name: String,
    created_at: i64,
    updated_at: i64,
}

#[derive(Clone)]
struct PackageAsset {
    source_id: String,
    original_filename: String,
    mime_type: String,
    byte_size: i64,
    data: Vec<u8>,
}

struct PackageData {
    version: i64,
    deck: PackageDeck,
    notes: Vec<PackageNote>,
    cards: Vec<PackageCard>,
    tags: Vec<PackageTag>,
    note_tags: Vec<(String, String)>,
    assets: Vec<PackageAsset>,
    note_assets: Vec<(String, String, String)>,
}

struct TemporaryFileGuard {
    path: PathBuf,
    keep: bool,
}

impl TemporaryFileGuard {
    fn new(path: PathBuf) -> Self {
        Self { path, keep: false }
    }

    fn keep(mut self) {
        self.keep = true;
    }
}

impl Drop for TemporaryFileGuard {
    fn drop(&mut self) {
        if !self.keep {
            let _ = fs::remove_file(&self.path);
        }
    }
}

fn temporary_export_path(destination: &Path) -> PathBuf {
    let suffix = format!("{}.tmp", std::process::id());
    destination.with_extension(format!("studydeck.{suffix}"))
}

fn replace_export_file(temporary: &Path, destination: &Path) -> Result<(), CommandError> {
    if destination.exists() {
        let backup = destination.with_extension("studydeck.previous");
        let _ = fs::remove_file(&backup);
        fs::rename(destination, &backup).map_err(CommandError::io)?;
        match fs::rename(temporary, destination) {
            Ok(()) => {
                let _ = fs::remove_file(backup);
            }
            Err(error) => {
                let _ = fs::rename(&backup, destination);
                return Err(CommandError::io(error));
            }
        }
    } else {
        fs::rename(temporary, destination).map_err(CommandError::io)?;
    }
    sync_directory(destination.parent())
}

fn hash_file(path: &Path) -> Result<String, CommandError> {
    let mut file = File::open(path).map_err(CommandError::io)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(CommandError::io)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn metadata_value(connection: &Connection, key: &str) -> Result<String, CommandError> {
    connection
        .query_row(
            "SELECT value FROM package_metadata WHERE key = ?1",
            [key],
            |row| row.get(0),
        )
        .optional()
        .map_err(CommandError::database)?
        .ok_or_else(|| {
            CommandError::invalid_package(format!("The package is missing metadata '{key}'."))
        })
}

fn table_exists(connection: &Connection, table: &str) -> Result<bool, CommandError> {
    connection
        .query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1
             )",
            [table],
            |row| row.get::<_, i64>(0),
        )
        .map(|value| value == 1)
        .map_err(CommandError::database)
}

fn column_exists(connection: &Connection, table: &str, column: &str) -> Result<bool, CommandError> {
    let sql = format!(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('{}') WHERE name = ?1)",
        table.replace('\'', "''")
    );
    connection
        .query_row(&sql, [column], |row| row.get::<_, i64>(0))
        .map(|value| value == 1)
        .map_err(CommandError::database)
}

fn choose_id(
    connection: &Connection,
    table: &str,
    preferred: &str,
) -> Result<String, CommandError> {
    validate_entity_id(preferred, "package entity")?;
    let allowed = ["decks", "notes", "cards", "assets", "tags"];
    if !allowed.contains(&table) {
        return Err(CommandError::new(
            "invalid_entity_table",
            "The package requested an unsupported entity table.",
        ));
    }
    let sql = format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE id = ?1)");
    let exists: i64 = connection
        .query_row(&sql, [preferred], |row| row.get(0))
        .map_err(CommandError::database)?;
    if exists == 0 {
        Ok(preferred.to_string())
    } else {
        generate_id(connection)
    }
}

fn rewrite_asset_ids(
    value: &mut Value,
    asset_map: &HashMap<String, String>,
) -> Result<(), CommandError> {
    match value {
        Value::Array(values) => {
            for nested in values {
                rewrite_asset_ids(nested, asset_map)?;
            }
        }
        Value::Object(object) => {
            if object.get("type").and_then(Value::as_str) == Some("image") {
                let attrs = object
                    .get_mut("attrs")
                    .and_then(Value::as_object_mut)
                    .ok_or_else(|| {
                        CommandError::invalid_package("An imported image node has no attributes.")
                    })?;
                let source_id = attrs
                    .get("assetId")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        CommandError::invalid_package(
                            "An imported image is missing its asset identifier.",
                        )
                    })?;
                let mapped = asset_map.get(source_id).ok_or_else(|| {
                    CommandError::invalid_package(
                        "An imported card refers to an image not contained in the package.",
                    )
                })?;
                attrs.insert("assetId".to_string(), Value::String(mapped.clone()));
                attrs.insert(
                    "src".to_string(),
                    Value::String(format!("study-asset://{mapped}")),
                );
            }
            for nested in object.values_mut() {
                rewrite_asset_ids(nested, asset_map)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn validate_timestamp_pair(
    created_at: i64,
    updated_at: i64,
    label: &str,
) -> Result<(), CommandError> {
    if created_at < 0 || updated_at < created_at {
        return Err(CommandError::invalid_package(format!(
            "The package contains invalid {label} timestamps."
        )));
    }
    Ok(())
}

fn normalize_tag_name(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn duplicate_fingerprint(front: &str, back: &str) -> String {
    let front = front
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    let back = back
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in front
        .as_bytes()
        .iter()
        .copied()
        .chain(std::iter::once(0xff))
        .chain(back.as_bytes().iter().copied())
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn write_new_file(path: &Path, data: &[u8]) -> Result<(), CommandError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(CommandError::io)?;
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(CommandError::io)?;
    file.write_all(data).map_err(CommandError::io)?;
    file.sync_all().map_err(CommandError::io)
}

#[cfg(unix)]
fn sync_directory(path: Option<&Path>) -> Result<(), CommandError> {
    let path = path
        .ok_or_else(|| CommandError::new("invalid_path", "The parent directory is unavailable."))?;
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(CommandError::io)
}

#[cfg(not(unix))]
fn sync_directory(_path: Option<&Path>) -> Result<(), CommandError> {
    Ok(())
}
