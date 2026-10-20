//! Complete local-library backup and recoverable in-app restoration.
//!
//! Packages hold coherent SQLite snapshots and required managed file bytes.
//! `live` coordinates request admission, connection replacement and durable
//! completion; `activation` owns filesystem journaling and startup recovery.
//! Credentials stay in the native secret store and never enter the package.

use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use rusqlite::{backup::Backup, params, Connection, OpenFlags, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    database::{
        Database, CURRENT_SCHEMA_LINEAGE, CURRENT_SCHEMA_VERSION, LATEST_SCHEMA_VERSION,
        MIN_SUPPORTED_RELEASE_SCHEMA_VERSION,
    },
    document_backup,
    error::CommandError,
    media::inspect_managed_asset_bytes,
    paths::AppPaths,
    util::unix_time_millis,
};

const BACKUP_FORMAT: &str = "ostudy-library-backup";
const BACKUP_VERSION: i64 = 4;
const MIN_SUPPORTED_BACKUP_VERSION: i64 = 1;
const MAX_BACKUP_FILE_BYTES: u64 = 8 * 1024 * 1024 * 1024;
const MAX_DATABASE_SNAPSHOT_BYTES: usize = 1_500 * 1024 * 1024;
const MAX_SINGLE_ASSET_BYTES: usize = 25 * 1024 * 1024;
static BACKUP_VALIDATION_TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);
const PACKAGE_SCHEMA: &str = r#"
PRAGMA foreign_keys = ON;
CREATE TABLE backup_metadata (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
) STRICT;
CREATE TABLE database_snapshot (
    id INTEGER PRIMARY KEY CHECK(id = 1),
    data BLOB NOT NULL
) STRICT;
CREATE TABLE asset_files (
    asset_id TEXT PRIMARY KEY,
    relative_path TEXT NOT NULL UNIQUE,
    mime_type TEXT NOT NULL,
    byte_size INTEGER NOT NULL CHECK(byte_size > 0),
    data BLOB NOT NULL
) STRICT;
CREATE TABLE document_files (
    file_key TEXT PRIMARY KEY,
    document_id TEXT NOT NULL CHECK(length(document_id) = 32),
    page_number INTEGER CHECK(page_number IS NULL OR page_number >= 1),
    file_kind TEXT NOT NULL CHECK(file_kind IN (
        'source_pdf', 'source_document', 'page_render', 'source_asset'
    )),
    relative_path TEXT NOT NULL UNIQUE,
    mime_type TEXT NOT NULL CHECK(length(mime_type) BETWEEN 3 AND 160),
    byte_size INTEGER NOT NULL CHECK(byte_size > 0),
    sha256 TEXT NOT NULL CHECK(length(sha256) = 64),
    data BLOB NOT NULL,
    CHECK(
        (file_kind = 'page_render' AND page_number IS NOT NULL)
        OR
        (file_kind IN ('source_pdf', 'source_document', 'source_asset') AND page_number IS NULL)
    )
) STRICT;
"#;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PrepareLibraryBackupInput {
    pub operation_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PrepareLibraryRestoreInput {
    pub staged_file_id: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LibraryBackupResultDto {
    destination_path: String,
    deck_count: i64,
    template_count: i64,
    review_count: i64,
    asset_count: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct LibraryBackupSummaryDto {
    deck_count: i64,
    template_count: i64,
    review_count: i64,
    asset_count: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedLibraryBackupDto {
    pub prepared_export: crate::external_files::PreparedExportDto,
    pub deck_count: i64,
    pub template_count: i64,
    pub review_count: i64,
    pub asset_count: i64,
}

#[derive(Clone, Debug)]
struct BackupSummary {
    deck_count: i64,
    template_count: i64,
    review_count: i64,
    asset_count: i64,
}

#[path = "library_backup/activation.rs"]
mod activation;
#[path = "library_backup/live.rs"]
pub(crate) mod live;
pub(crate) use activation::{apply_pending_restore, checkpoint, meter};
#[path = "library_backup/package.rs"]
mod package;
#[cfg(test)]
use package::{backup_validation_temp_path, validate_backup_package};
use package::{export_backup, extract_backup_package};

#[tauri::command]
pub fn prepare_library_backup(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: PrepareLibraryBackupInput,
) -> Result<PreparedLibraryBackupDto, CommandError> {
    let _learning_guard = crate::knowledge::runtime::MaintenanceGuard::begin()?;
    crate::util::validate_entity_id(&input.operation_id, "library backup operation")?;
    if let Some((prepared_export, summary)) = database.with_connection(|connection| {
        crate::external_files::load_prepared_export_summary::<LibraryBackupSummaryDto>(
            connection,
            paths.inner(),
            &input.operation_id,
            "library_backup",
        )
    })? {
        return Ok(PreparedLibraryBackupDto {
            prepared_export,
            deck_count: summary.deck_count,
            template_count: summary.template_count,
            review_count: summary.review_count,
            asset_count: summary.asset_count,
        });
    }
    let relative = AppPaths::prepared_export_relative_path(&input.operation_id, "ostudybackup");
    let destination = paths.resolve_relative(&relative);
    let exported = export_backup(database.inner(), paths.inner(), &destination)?;
    let summary = LibraryBackupSummaryDto {
        deck_count: exported.deck_count,
        template_count: exported.template_count,
        review_count: exported.review_count,
        asset_count: exported.asset_count,
    };
    let created_at = unix_time_millis()?;
    let created_date = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(created_at)
        .map(|value| value.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "backup".to_string());
    let suggested_filename = format!("ostudy-library-{created_date}.ostudybackup");
    let prepared_export = database.with_connection(|connection| {
        crate::external_files::register_prepared_export(
            connection,
            paths.inner(),
            crate::external_files::RegisterPreparedExport {
                id: &input.operation_id,
                purpose: "library_backup",
                suggested_filename: &suggested_filename,
                media_type: "application/vnd.ostudy.backup",
                relative_path: &relative,
                summary: &summary,
            },
        )
    })?;
    Ok(PreparedLibraryBackupDto {
        prepared_export,
        deck_count: summary.deck_count,
        template_count: summary.template_count,
        review_count: summary.review_count,
        asset_count: summary.asset_count,
    })
}

pub(crate) fn validate_snapshot_database(connection: &Connection) -> Result<(), CommandError> {
    let integrity: String = connection
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(CommandError::database)?;
    if integrity != "ok" {
        return Err(CommandError::new(
            "invalid_backup",
            "The backup's SQLite library is damaged.",
        ));
    }
    crate::database::validate_backup_lineage(connection)?;
    let (lineage, version) = crate::database::schema_identity(connection)?;
    let supported = if lineage == CURRENT_SCHEMA_LINEAGE {
        (MIN_SUPPORTED_RELEASE_SCHEMA_VERSION..=CURRENT_SCHEMA_VERSION).contains(&version)
    } else if lineage == "ostudy-public-v1" {
        (10..=LATEST_SCHEMA_VERSION).contains(&version)
    } else {
        false
    };
    if !supported {
        return Err(CommandError::new(
            "unsupported_backup_schema",
            format!("This backup uses unsupported database lineage {lineage} schema {version}."),
        ));
    }
    Ok(())
}

fn snapshot_schema_version(connection: &Connection) -> Result<i64, CommandError> {
    crate::database::schema_identity(connection).map(|(_, version)| version)
}

fn snapshot_summary(connection: &Connection) -> Result<BackupSummary, CommandError> {
    Ok(BackupSummary {
        deck_count: count_table(connection, "decks", "deleted_at IS NULL")?,
        template_count: count_table(connection, "card_templates", "deleted_at IS NULL")?,
        review_count: count_table(connection, "review_events", "1 = 1")?,
        asset_count: count_table(
            connection,
            "assets",
            "lifecycle_state = 'ready' AND deleted_at IS NULL",
        )?,
    })
}

fn count_table(connection: &Connection, table: &str, condition: &str) -> Result<i64, CommandError> {
    let allowed_tables = ["decks", "card_templates", "review_events", "assets"];
    if !allowed_tables.contains(&table) {
        return Err(CommandError::new(
            "invalid_backup_query",
            "Invalid backup count table.",
        ));
    }
    let sql = format!("SELECT COUNT(*) FROM {table} WHERE {condition}");
    connection
        .query_row(&sql, [], |row| row.get(0))
        .map_err(CommandError::database)
}

fn backup_metadata(connection: &Connection, key: &str) -> Result<String, CommandError> {
    connection
        .query_row(
            "SELECT value FROM backup_metadata WHERE key = ?1",
            [key],
            |row| row.get(0),
        )
        .optional()
        .map_err(CommandError::database)?
        .ok_or_else(|| CommandError::new("invalid_backup", "The backup metadata is incomplete."))
}

fn backup_version(connection: &Connection) -> Result<i64, CommandError> {
    backup_metadata(connection, "formatVersion")?
        .parse::<i64>()
        .map_err(|_| CommandError::new("invalid_backup", "The backup version is invalid."))
}

fn pending_restore_path(paths: &AppPaths) -> PathBuf {
    paths
        .app_data_dir()
        .join("pending-library-restore.ostudybackup")
}

fn validate_managed_asset_path(value: &str, lifecycle_state: &str) -> Result<(), CommandError> {
    let managed_directory = match lifecycle_state {
        "ready" => "images",
        "staged" => "staging",
        _ => {
            return Err(CommandError::new(
                "invalid_backup",
                "A packaged asset has an unsupported lifecycle state.",
            ));
        }
    };
    let path = Path::new(value);
    let components = path.components().collect::<Vec<_>>();
    if components.len() != 3
        || components[0] != Component::Normal("assets".as_ref())
        || components[1] != Component::Normal(Path::new(managed_directory).as_os_str())
        || !matches!(components[2], Component::Normal(_))
    {
        return Err(CommandError::new(
            "invalid_backup",
            "A packaged asset path escapes its managed lifecycle directory.",
        ));
    }
    Ok(())
}

fn replace_file(temporary: &Path, destination: &Path) -> Result<(), CommandError> {
    if destination.exists() {
        let previous = destination.with_extension("previous");
        remove_file_if_exists(&previous).map_err(CommandError::io)?;
        fs::rename(destination, &previous).map_err(CommandError::io)?;
        if let Err(error) = fs::rename(temporary, destination) {
            let _ = fs::rename(&previous, destination);
            return Err(CommandError::io(error));
        }
        let _ = fs::remove_file(previous);
    } else {
        fs::rename(temporary, destination).map_err(CommandError::io)?;
    }
    sync_directory(destination.parent().ok_or_else(|| {
        CommandError::new(
            "invalid_backup_path",
            "The backup parent directory is invalid.",
        )
    })?)
}

fn copy_and_sync(source: &Path, destination: &Path) -> Result<(), CommandError> {
    let mut input = File::open(source).map_err(CommandError::io)?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(CommandError::io)?;
    let copied = std::io::copy(&mut input, &mut output).map_err(CommandError::io)?;
    activation::meter::advance(copied);
    output.sync_all().map_err(CommandError::io)?;
    sync_directory(destination.parent().ok_or_else(|| {
        CommandError::new(
            "invalid_backup_path",
            "The backup staging directory is invalid.",
        )
    })?)
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

fn remove_file_if_exists(path: &Path) -> std::io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn remove_dir_if_exists(path: &Path) -> std::io::Result<()> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<(), CommandError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(CommandError::io)
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> Result<(), CommandError> {
    Ok(())
}

#[cfg(test)]
mod extended_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        cards::validate_content,
        decks::{create_deck_in_connection, CreateDeckInput},
        scheduler::insert_new_schedule,
        util::generate_id,
    };
    use serde_json::json;

    fn temp_paths(label: &str) -> AppPaths {
        let root = std::env::temp_dir().join(format!(
            "ostudy-backup-{label}-{}-{}",
            std::process::id(),
            unix_time_millis().expect("clock")
        ));
        AppPaths::new(root).expect("paths")
    }

    #[test]
    fn backup_validation_temp_paths_do_not_collide_within_one_millisecond() {
        let first = backup_validation_temp_path(123);
        let second = backup_validation_temp_path(123);
        assert_ne!(first, second);
    }

    #[test]
    fn complete_backup_preserves_review_history_and_settings() {
        let paths = temp_paths("round-trip");
        let database = Database::open(paths.database_path()).expect("database");
        database
            .with_connection(|connection| {
                let deck = create_deck_in_connection(
                    connection,
                    CreateDeckInput {
                        name: "Backup deck".to_string(),
                        description: None,
                    },
                )?;
                let note_id = generate_id(connection)?;
                let card_id = generate_id(connection)?;
                let content = validate_content(
                    json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Question"}]}]}),
                    json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Answer"}]}]}),
                    "Question".to_string(),
                    "Answer".to_string(),
                )?;
                connection.execute(
                    "INSERT INTO notes (
                         id, deck_id, note_type, content_format, content_schema_version,
                         front_doc, back_doc, front_text, back_text, duplicate_hash,
                         created_at, updated_at, revision
                     ) VALUES (?1, ?2, 'basic', 'tiptap-json', 1, ?3, ?4, ?5, ?6, 'hash', 1, 1, 1)",
                    params![note_id, deck.id, content.front_doc_json, content.back_doc_json, content.front_text, content.back_text],
                ).map_err(CommandError::database)?;
                connection.execute(
                    "INSERT INTO cards (
                         id, note_id, deck_id, direction, template_ordinal, created_at, updated_at
                     ) VALUES (?1, ?2, ?3, 'forward', 0, 1, 1)",
                    params![card_id, note_id, deck.id],
                ).map_err(CommandError::database)?;
                insert_new_schedule(connection, &card_id, 1)?;
                connection.execute(
                    "INSERT INTO review_events (
                         id, submission_id, session_id, card_id, reviewed_at,
                         rating, answer_correct, confidence, response_time_ms,
                         state_before, state_after, due_before, due_after,
                         stability_before, stability_after,
                         difficulty_before, difficulty_after,
                         scheduled_days_before, scheduled_days_after,
                         scheduler_name, scheduler_version, created_at,
                         request_fingerprint, result_completed,
                         result_rated_count, result_correct_count
                     ) VALUES (
                         ?1, ?2, NULL, ?3, 2,
                         3, 1, 'somewhat_confident', 100,
                         0, 1, 1, 2,
                         0.0, 1.0, 0.0, 5.0,
                         0, 1, 'fsrs', 'test', 2,
                         ?4, 0, 1, 1
                     )",
                    params![generate_id(connection)?, generate_id(connection)?, card_id, "a".repeat(64)],
                ).map_err(CommandError::database)?;
                connection.execute(
                    "UPDATE app_settings SET value_json = '12' WHERE key = 'dailyNewLimit'",
                    [],
                ).map_err(CommandError::database)?;
                Ok(())
            })
            .expect("fixture");
        let destination = paths.app_data_dir().join("library.ostudybackup");
        let result = export_backup(&database, &paths, &destination).expect("backup");
        assert_eq!(result.deck_count, 1);
        assert_eq!(result.review_count, 1);
        let summary = validate_backup_package(&destination, None).expect("valid backup");
        assert_eq!(summary.review_count, 1);
        let _ = fs::remove_dir_all(paths.app_data_dir());
    }

    #[test]
    fn version_four_package_schema_accepts_generic_document_files() {
        let connection = Connection::open_in_memory().expect("package should open");
        connection
            .execute_batch(PACKAGE_SCHEMA)
            .expect("package schema should initialize");
        connection
            .execute(
                "INSERT INTO document_files (
                     file_key, document_id, page_number, file_kind, relative_path,
                     mime_type, byte_size, sha256, data
                 ) VALUES (
                     'source:doc', ?1, NULL, 'source_document', ?2,
                     'text/csv', 3, ?3, x'612C62'
                 )",
                params![
                    "a".repeat(32),
                    format!("documents/{}/source.csv", "a".repeat(32)),
                    "0".repeat(64),
                ],
            )
            .expect("generic source row should be admitted");
        connection
            .execute(
                "INSERT INTO document_files (
                     file_key, document_id, page_number, file_kind, relative_path,
                     mime_type, byte_size, sha256, data
                 ) VALUES (
                     'asset:doc:1', ?1, NULL, 'source_asset', ?2,
                     'image/png', 1, ?3, x'00'
                 )",
                params![
                    "a".repeat(32),
                    format!("documents/{}/assets/asset-000001.png", "a".repeat(32)),
                    "0".repeat(64),
                ],
            )
            .expect("generic source-asset row should be admitted");
    }
}

#[cfg(test)]
#[path = "library_backup/image_set_backup_tests.rs"]
mod image_set_backup_tests;

#[cfg(test)]
#[path = "library_backup/lesson_backup_tests.rs"]
mod lesson_backup_tests;

#[cfg(test)]
#[path = "library_backup/media_backup_tests.rs"]
mod media_backup_tests;

#[cfg(test)]
#[path = "library_backup/tutor_backup_tests.rs"]
mod tutor_backup_tests;
