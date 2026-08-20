//! Complete local-library backup and restart-safe restoration.
//!
//! A backup contains one coherent SQLite snapshot, every ready managed card
//! image, and every immutable managed PDF source and rendered document page.
//! Credentials remain outside SQLite and are never part of the package. Restore
//! is prepared while ostudy is running and applied before the database opens on
//! the next launch, allowing an atomic filesystem rollback if the restored
//! library cannot be opened, migrated, or reconciled.

use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Component, Path, PathBuf},
    time::Duration,
};

use rusqlite::{backup::Backup, params, Connection, OpenFlags, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    assets::detect_image_bytes,
    database::{Database, LATEST_SCHEMA_VERSION},
    document_backup,
    error::CommandError,
    paths::AppPaths,
    util::unix_time_millis,
};

const BACKUP_FORMAT: &str = "ostudy-library-backup";
const BACKUP_VERSION: i64 = 3;
const MIN_SUPPORTED_BACKUP_VERSION: i64 = 1;
const MAX_BACKUP_FILE_BYTES: u64 = 8 * 1024 * 1024 * 1024;
const MAX_DATABASE_SNAPSHOT_BYTES: usize = 1_500 * 1024 * 1024;
const MAX_SINGLE_ASSET_BYTES: usize = 25 * 1024 * 1024;
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
    file_kind TEXT NOT NULL CHECK(file_kind IN ('source_pdf', 'page_render')),
    relative_path TEXT NOT NULL UNIQUE,
    mime_type TEXT NOT NULL CHECK(mime_type IN ('application/pdf', 'image/png')),
    byte_size INTEGER NOT NULL CHECK(byte_size > 0),
    sha256 TEXT NOT NULL CHECK(length(sha256) = 64),
    data BLOB NOT NULL,
    CHECK((file_kind = 'source_pdf') = (page_number IS NULL))
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
pub struct LibraryRestoreResultDto {
    pub deck_count: i64,
    pub template_count: i64,
    pub review_count: i64,
    pub asset_count: i64,
    pub restart_required: bool,
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

pub(crate) struct PendingRestoreGuard {
    database_path: PathBuf,
    assets_path: PathBuf,
    documents_path: PathBuf,
    rollback_database: Option<PathBuf>,
    rollback_assets: Option<PathBuf>,
    rollback_documents: Option<PathBuf>,
    pending_package: PathBuf,
    staging_root: PathBuf,
    committed: bool,
}

impl PendingRestoreGuard {
    pub(crate) fn commit(mut self) {
        // Once the restored database has opened and reconciled successfully, a
        // cleanup failure must never invoke Drop's rollback against that live
        // connection. Mark the guard committed first and clean conservatively.
        self.committed = true;
        if let Some(path) = self.rollback_database.take() {
            let _ = remove_file_if_exists(&path);
        }
        if let Some(path) = self.rollback_assets.take() {
            let _ = remove_dir_if_exists(&path);
        }
        if let Some(path) = self.rollback_documents.take() {
            let _ = remove_dir_if_exists(&path);
        }
        let _ = remove_file_if_exists(&self.pending_package);
        let _ = remove_dir_if_exists(&self.staging_root);
    }
}

impl Drop for PendingRestoreGuard {
    fn drop(&mut self) {
        if self.committed {
            return;
        }
        let _ = remove_file_if_exists(&self.database_path);
        let _ = remove_dir_if_exists(&self.assets_path);
        let _ = remove_dir_if_exists(&self.documents_path);
        if let Some(path) = self.rollback_database.take() {
            let _ = fs::rename(path, &self.database_path);
        }
        if let Some(path) = self.rollback_assets.take() {
            let _ = fs::rename(path, &self.assets_path);
        }
        if let Some(path) = self.rollback_documents.take() {
            let _ = fs::rename(path, &self.documents_path);
        }
    }
}

#[tauri::command]
pub fn prepare_library_backup(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: PrepareLibraryBackupInput,
) -> Result<PreparedLibraryBackupDto, CommandError> {
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

#[tauri::command]
pub fn prepare_library_restore(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: PrepareLibraryRestoreInput,
) -> Result<LibraryRestoreResultDto, CommandError> {
    crate::util::validate_entity_id(&input.staged_file_id, "staged library backup")?;
    if let Some(result) = database.with_connection(|connection| {
        crate::external_files::consumed_import_result::<LibraryRestoreResultDto>(
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
            crate::external_files::ExternalImportPurpose::LibraryBackup,
            &[crate::external_files::ExternalFileKind::Ostudybackup],
        )
    })?;
    let summary = validate_backup_package(&staged.path, None)?;
    let pending = pending_restore_path(&paths);
    let temporary = pending.with_extension("ostudybackup.tmp");
    remove_file_if_exists(&temporary).map_err(CommandError::io)?;
    copy_and_sync(&staged.path, &temporary)?;
    replace_file(&temporary, &pending)?;
    let result = LibraryRestoreResultDto {
        deck_count: summary.deck_count,
        template_count: summary.template_count,
        review_count: summary.review_count,
        asset_count: summary.asset_count,
        restart_required: true,
    };
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

pub(crate) fn apply_pending_restore(
    paths: &AppPaths,
) -> Result<Option<PendingRestoreGuard>, String> {
    let pending = pending_restore_path(paths);
    if !pending.is_file() {
        return Ok(None);
    }
    let staging_root = paths
        .app_data_dir()
        .join(format!("restore-staging-{}", std::process::id()));
    remove_dir_if_exists(&staging_root)
        .map_err(|error| format!("Could not clear restore staging: {error}"))?;
    fs::create_dir_all(&staging_root)
        .map_err(|error| format!("Could not create restore staging: {error}"))?;
    let staged_database = staging_root.join("library.sqlite3");
    let staged_assets = staging_root.join("assets");
    let staged_documents = staging_root.join("documents");
    extract_backup_package(&pending, &staged_database, &staged_assets)
        .map_err(|error| error.message)?;

    let database_path = paths.database_path().to_path_buf();
    let assets_path = paths.app_data_dir().join("assets");
    let documents_path = paths.app_data_dir().join("documents");
    let rollback_database = paths.app_data_dir().join("library.sqlite3.pre-restore");
    let rollback_assets = paths.app_data_dir().join("assets.pre-restore");
    let rollback_documents = paths.app_data_dir().join("documents.pre-restore");
    remove_file_if_exists(&rollback_database)
        .map_err(|error| format!("Could not clear database rollback path: {error}"))?;
    remove_dir_if_exists(&rollback_assets)
        .map_err(|error| format!("Could not clear image rollback path: {error}"))?;
    remove_dir_if_exists(&rollback_documents)
        .map_err(|error| format!("Could not clear document rollback path: {error}"))?;

    let had_database = database_path.exists();
    let had_assets = assets_path.exists();
    let had_documents = documents_path.exists();
    if had_database {
        fs::rename(&database_path, &rollback_database).map_err(|error| {
            format!("Could not stage the current database for rollback: {error}")
        })?;
    }
    if had_assets {
        if let Err(error) = fs::rename(&assets_path, &rollback_assets) {
            if had_database {
                let _ = fs::rename(&rollback_database, &database_path);
            }
            return Err(format!(
                "Could not stage current images for rollback: {error}"
            ));
        }
    }
    if had_documents {
        if let Err(error) = fs::rename(&documents_path, &rollback_documents) {
            if had_assets {
                let _ = fs::rename(&rollback_assets, &assets_path);
            }
            if had_database {
                let _ = fs::rename(&rollback_database, &database_path);
            }
            return Err(format!(
                "Could not stage current documents for rollback: {error}"
            ));
        }
    }

    if let Err(error) = fs::rename(&staged_database, &database_path) {
        restore_previous_filesystem(
            &database_path,
            &assets_path,
            &documents_path,
            had_database.then_some(&rollback_database),
            had_assets.then_some(&rollback_assets),
            had_documents.then_some(&rollback_documents),
        );
        return Err(format!("Could not install the restored database: {error}"));
    }
    if let Err(error) = fs::rename(&staged_assets, &assets_path) {
        restore_previous_filesystem(
            &database_path,
            &assets_path,
            &documents_path,
            had_database.then_some(&rollback_database),
            had_assets.then_some(&rollback_assets),
            had_documents.then_some(&rollback_documents),
        );
        return Err(format!("Could not install the restored images: {error}"));
    }
    if let Err(error) = fs::rename(&staged_documents, &documents_path) {
        restore_previous_filesystem(
            &database_path,
            &assets_path,
            &documents_path,
            had_database.then_some(&rollback_database),
            had_assets.then_some(&rollback_assets),
            had_documents.then_some(&rollback_documents),
        );
        return Err(format!("Could not install the restored documents: {error}"));
    }
    sync_directory(paths.app_data_dir()).map_err(|error| error.message)?;

    Ok(Some(PendingRestoreGuard {
        database_path,
        assets_path,
        documents_path,
        rollback_database: had_database.then_some(rollback_database),
        rollback_assets: had_assets.then_some(rollback_assets),
        rollback_documents: had_documents.then_some(rollback_documents),
        pending_package: pending,
        staging_root,
        committed: false,
    }))
}

fn restore_previous_filesystem(
    database_path: &Path,
    assets_path: &Path,
    documents_path: &Path,
    rollback_database: Option<&PathBuf>,
    rollback_assets: Option<&PathBuf>,
    rollback_documents: Option<&PathBuf>,
) {
    let _ = remove_file_if_exists(database_path);
    let _ = remove_dir_if_exists(assets_path);
    let _ = remove_dir_if_exists(documents_path);
    if let Some(path) = rollback_database {
        let _ = fs::rename(path, database_path);
    }
    if let Some(path) = rollback_assets {
        let _ = fs::rename(path, assets_path);
    }
    if let Some(path) = rollback_documents {
        let _ = fs::rename(path, documents_path);
    }
}

fn export_backup(
    database: &Database,
    paths: &AppPaths,
    destination: &Path,
) -> Result<LibraryBackupResultDto, CommandError> {
    let parent = destination
        .parent()
        .ok_or_else(|| CommandError::new("invalid_backup_path", "Choose a valid backup folder."))?;
    if !parent.is_dir() {
        return Err(CommandError::new(
            "invalid_backup_path",
            "The selected backup folder no longer exists.",
        ));
    }
    let (active_drafts, processing_documents) = database.with_connection(|connection| {
        let active_drafts = connection
            .query_row(
                "SELECT COUNT(*) FROM asset_drafts
                 WHERE committed_at IS NULL AND canceled_at IS NULL
                   AND retention_mode = 'ephemeral'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map_err(CommandError::database)?;
        let processing_documents = if document_backup::table_exists(connection, "documents")? {
            connection
                .query_row(
                    "SELECT COUNT(*) FROM documents WHERE lifecycle_state = 'processing'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .map_err(CommandError::database)?
        } else {
            0
        };
        Ok((active_drafts, processing_documents))
    })?;
    if active_drafts > 0 {
        return Err(CommandError::new(
            "active_asset_draft",
            "Finish or cancel the open card, template, or import before backing up the library.",
        ));
    }
    if processing_documents > 0 {
        return Err(CommandError::new(
            "document_processing_active",
            "Finish PDF extraction and flashcard generation before backing up the library.",
        ));
    }

    let scratch = destination.with_extension(format!("ostudybackup.{}.work", std::process::id()));
    remove_dir_if_exists(&scratch).map_err(CommandError::io)?;
    fs::create_dir_all(&scratch).map_err(CommandError::io)?;
    let snapshot_path = scratch.join("library.sqlite3");
    database.with_connection(|source| {
        let mut target = Connection::open(&snapshot_path).map_err(CommandError::database)?;
        let backup = Backup::new(source, &mut target).map_err(CommandError::database)?;
        backup
            .run_to_completion(128, Duration::from_millis(10), None)
            .map_err(CommandError::database)?;
        Ok(())
    })?;
    let snapshot = Connection::open(&snapshot_path).map_err(CommandError::database)?;
    validate_snapshot_database(&snapshot)?;
    let summary = snapshot_summary(&snapshot)?;
    // The SQLite backup API can leave the destination in WAL mode. Queries on
    // the live connection see WAL-resident pages, but the backup package embeds
    // only the main database file. Checkpoint and switch to DELETE mode before
    // reading raw bytes so the embedded snapshot contains every committed page.
    snapshot
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode = DELETE;")
        .map_err(CommandError::database)?;
    drop(snapshot);
    let snapshot = Connection::open_with_flags(&snapshot_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(CommandError::database)?;

    let temporary = destination.with_extension(format!("ostudybackup.{}.tmp", std::process::id()));
    remove_file_if_exists(&temporary).map_err(CommandError::io)?;
    let mut package = Connection::open(&temporary).map_err(CommandError::database)?;
    package
        .execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA journal_mode = DELETE;
             PRAGMA synchronous = FULL;
             PRAGMA trusted_schema = OFF;",
        )
        .map_err(CommandError::database)?;
    package
        .execute_batch(PACKAGE_SCHEMA)
        .map_err(CommandError::database)?;
    let transaction = package.transaction().map_err(CommandError::database)?;
    let created_at = unix_time_millis()?;
    for (key, value) in [
        ("format", BACKUP_FORMAT.to_string()),
        ("formatVersion", BACKUP_VERSION.to_string()),
        ("createdAt", created_at.to_string()),
        ("applicationVersion", env!("CARGO_PKG_VERSION").to_string()),
        (
            "schemaVersion",
            snapshot_schema_version(&snapshot)?.to_string(),
        ),
        ("deckCount", summary.deck_count.to_string()),
        ("templateCount", summary.template_count.to_string()),
        ("reviewCount", summary.review_count.to_string()),
        ("assetCount", summary.asset_count.to_string()),
    ] {
        transaction
            .execute(
                "INSERT INTO backup_metadata (key, value) VALUES (?1, ?2)",
                params![key, value],
            )
            .map_err(CommandError::database)?;
    }
    let database_bytes = fs::read(&snapshot_path).map_err(CommandError::io)?;
    if database_bytes.is_empty() || database_bytes.len() > MAX_DATABASE_SNAPSHOT_BYTES {
        return Err(CommandError::new(
            "database_backup_too_large",
            "The SQLite library is too large for the current single-file backup format.",
        ));
    }
    transaction
        .execute(
            "INSERT INTO database_snapshot (id, data) VALUES (1, ?1)",
            [database_bytes],
        )
        .map_err(CommandError::database)?;

    export_asset_files(&snapshot, paths, &transaction)?;
    let document_file_count =
        document_backup::export_document_files(&snapshot, paths, &transaction)?;
    transaction
        .execute(
            "INSERT INTO backup_metadata (key, value) VALUES ('documentFileCount', ?1)",
            [document_file_count.to_string()],
        )
        .map_err(CommandError::database)?;
    transaction.commit().map_err(CommandError::database)?;

    let integrity: String = package
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(CommandError::database)?;
    if integrity != "ok" {
        return Err(CommandError::new(
            "backup_validation_failed",
            "The completed backup failed its integrity check.",
        ));
    }
    drop(package);
    let package_size = fs::metadata(&temporary).map_err(CommandError::io)?.len();
    if package_size == 0 || package_size > MAX_BACKUP_FILE_BYTES {
        return Err(CommandError::new(
            "backup_too_large",
            "The completed backup exceeds ostudy's supported size limit.",
        ));
    }
    OpenOptions::new()
        .read(true)
        .open(&temporary)
        .and_then(|file| file.sync_all())
        .map_err(CommandError::io)?;
    replace_file(&temporary, destination)?;
    remove_dir_if_exists(&scratch).map_err(CommandError::io)?;
    Ok(LibraryBackupResultDto {
        destination_path: destination.to_string_lossy().into_owned(),
        deck_count: summary.deck_count,
        template_count: summary.template_count,
        review_count: summary.review_count,
        asset_count: summary.asset_count,
    })
}

fn export_asset_files(
    snapshot: &Connection,
    paths: &AppPaths,
    transaction: &rusqlite::Transaction<'_>,
) -> Result<(), CommandError> {
    let mut statement = snapshot
        .prepare(
            "SELECT a.id,
                    CASE
                      WHEN a.lifecycle_state = 'ready' THEN a.relative_path
                      ELSE a.staged_relative_path
                    END,
                    a.mime_type, a.byte_size, a.lifecycle_state
             FROM assets a
             LEFT JOIN asset_drafts ad ON ad.id = a.draft_id
             LEFT JOIN editor_drafts ed ON ed.id = ad.editor_draft_id
             WHERE a.deleted_at IS NULL
               AND (
                 a.lifecycle_state = 'ready'
                 OR (
                   a.lifecycle_state = 'staged'
                   AND ad.retention_mode = 'persistent_editor'
                   AND ad.committed_at IS NULL AND ad.canceled_at IS NULL
                   AND ed.state IN ('active', 'conflicted')
                 )
               )
             ORDER BY a.id",
        )
        .map_err(CommandError::database)?;
    let assets = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, String>(4)?,
            ))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    for (asset_id, relative_path, mime_type, byte_size, lifecycle_state) in assets {
        validate_managed_asset_path(&relative_path, &lifecycle_state)?;
        let data = fs::read(paths.resolve_relative(&relative_path)).map_err(|_| {
            CommandError::new(
                "missing_asset",
                "A managed image is missing. Repair it before creating a complete backup.",
            )
        })?;
        if data.is_empty()
            || data.len() > MAX_SINGLE_ASSET_BYTES
            || i64::try_from(data.len()).unwrap_or(i64::MAX) != byte_size
        {
            return Err(CommandError::new(
                "invalid_asset",
                "A managed image does not match its database size record.",
            ));
        }
        let (detected_mime, _) = detect_image_bytes(&data)?;
        if detected_mime != mime_type {
            return Err(CommandError::new(
                "invalid_asset",
                "A managed image does not match its database file type.",
            ));
        }
        transaction
            .execute(
                "INSERT INTO asset_files (
                     asset_id, relative_path, mime_type, byte_size, data
                 ) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![asset_id, relative_path, mime_type, byte_size, data],
            )
            .map_err(CommandError::database)?;
    }
    Ok(())
}

fn validate_backup_package(
    source: &Path,
    extract_database_to: Option<&Path>,
) -> Result<BackupSummary, CommandError> {
    if !source.is_file() {
        return Err(CommandError::new(
            "invalid_backup",
            "Select an ostudy library backup file.",
        ));
    }
    let metadata = fs::metadata(source).map_err(CommandError::io)?;
    if metadata.len() == 0 || metadata.len() > MAX_BACKUP_FILE_BYTES {
        return Err(CommandError::new(
            "invalid_backup",
            "The backup is empty or exceeds the supported size limit.",
        ));
    }
    let package =
        Connection::open_with_flags(source, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|error| {
            CommandError::new(
                "invalid_backup",
                format!("Could not open the backup: {error}"),
            )
        })?;
    package
        .execute_batch("PRAGMA trusted_schema = OFF; PRAGMA query_only = ON;")
        .map_err(CommandError::database)?;
    let integrity: String = package
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(CommandError::database)?;
    let version = backup_version(&package)?;
    if integrity != "ok"
        || backup_metadata(&package, "format")? != BACKUP_FORMAT
        || !(MIN_SUPPORTED_BACKUP_VERSION..=BACKUP_VERSION).contains(&version)
    {
        return Err(CommandError::new(
            "invalid_backup",
            "This is not a valid supported ostudy library backup.",
        ));
    }
    let database_bytes: Vec<u8> = package
        .query_row(
            "SELECT data FROM database_snapshot WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    if database_bytes.is_empty() || database_bytes.len() > MAX_DATABASE_SNAPSHOT_BYTES {
        return Err(CommandError::new(
            "invalid_backup",
            "The backup contains an invalid SQLite snapshot.",
        ));
    }
    let temporary = extract_database_to
        .map(Path::to_path_buf)
        .unwrap_or_else(|| {
            std::env::temp_dir().join(format!(
                "ostudy-backup-validation-{}-{}.sqlite3",
                std::process::id(),
                unix_time_millis().unwrap_or_default()
            ))
        });
    remove_file_if_exists(&temporary).map_err(CommandError::io)?;
    write_new_file(&temporary, &database_bytes)?;
    let snapshot = Connection::open_with_flags(&temporary, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(CommandError::database)?;
    validate_snapshot_database(&snapshot)?;
    validate_backup_assets(&package, &snapshot, version, None)?;
    if version >= 2 {
        document_backup::validate_document_files(&package, &snapshot, None)?;
    } else if document_backup::table_exists(&snapshot, "documents")?
        && snapshot
            .query_row("SELECT COUNT(*) FROM documents", [], |row| {
                row.get::<_, i64>(0)
            })
            .map_err(CommandError::database)?
            > 0
    {
        return Err(CommandError::new(
            "invalid_backup",
            "This legacy backup version cannot contain managed PDF documents.",
        ));
    }
    let summary = snapshot_summary(&snapshot)?;
    drop(snapshot);
    if extract_database_to.is_none() {
        remove_file_if_exists(&temporary).map_err(CommandError::io)?;
    }
    Ok(summary)
}

fn extract_backup_package(
    source: &Path,
    database_path: &Path,
    assets_path: &Path,
) -> Result<BackupSummary, CommandError> {
    let summary = validate_backup_package(source, Some(database_path))?;
    let package = Connection::open_with_flags(source, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(CommandError::database)?;
    let version = backup_version(&package)?;
    let snapshot = Connection::open_with_flags(database_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(CommandError::database)?;
    let extraction_root = assets_path.parent().ok_or_else(|| {
        CommandError::new(
            "invalid_restore_path",
            "The restore staging path is invalid.",
        )
    })?;
    let documents_path = extraction_root.join("documents");
    remove_dir_if_exists(assets_path).map_err(CommandError::io)?;
    remove_dir_if_exists(&documents_path).map_err(CommandError::io)?;
    fs::create_dir_all(assets_path.join("images")).map_err(CommandError::io)?;
    fs::create_dir_all(assets_path.join("staging")).map_err(CommandError::io)?;
    fs::create_dir_all(assets_path.join("quarantine")).map_err(CommandError::io)?;
    fs::create_dir_all(documents_path.join("staging")).map_err(CommandError::io)?;
    validate_backup_assets(&package, &snapshot, version, Some(extraction_root))?;
    if version >= 2 {
        document_backup::validate_document_files(&package, &snapshot, Some(extraction_root))?;
    }
    sync_directory(assets_path)?;
    sync_directory(&documents_path)?;
    Ok(summary)
}

fn validate_backup_assets(
    package: &Connection,
    snapshot: &Connection,
    version: i64,
    extraction_root: Option<&Path>,
) -> Result<(), CommandError> {
    let expected: i64 = if version >= 3 {
        snapshot
            .query_row(
                "SELECT COUNT(*)
                 FROM assets a
                 LEFT JOIN asset_drafts ad ON ad.id = a.draft_id
                 LEFT JOIN editor_drafts ed ON ed.id = ad.editor_draft_id
                 WHERE a.deleted_at IS NULL
                   AND (
                     a.lifecycle_state = 'ready'
                     OR (
                       a.lifecycle_state = 'staged'
                       AND ad.retention_mode = 'persistent_editor'
                       AND ad.committed_at IS NULL AND ad.canceled_at IS NULL
                       AND ed.state IN ('active', 'conflicted')
                     )
                   )",
                [],
                |row| row.get(0),
            )
            .map_err(CommandError::database)?
    } else {
        snapshot
            .query_row(
                "SELECT COUNT(*) FROM assets
                 WHERE lifecycle_state = 'ready' AND deleted_at IS NULL",
                [],
                |row| row.get(0),
            )
            .map_err(CommandError::database)?
    };
    let actual: i64 = package
        .query_row("SELECT COUNT(*) FROM asset_files", [], |row| row.get(0))
        .map_err(CommandError::database)?;
    if expected != actual {
        return Err(CommandError::new(
            "invalid_backup",
            "The backup does not contain every managed image required by the library and its recoverable drafts.",
        ));
    }
    let mut statement = package
        .prepare(
            "SELECT asset_id, relative_path, mime_type, byte_size, data
             FROM asset_files ORDER BY asset_id",
        )
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, Vec<u8>>(4)?,
            ))
        })
        .map_err(CommandError::database)?;
    for row in rows {
        let (asset_id, relative_path, mime_type, byte_size, data) =
            row.map_err(CommandError::database)?;
        let stored: Option<(String, String, i64, String)> = if version >= 3 {
            snapshot
                .query_row(
                    "SELECT CASE
                              WHEN a.lifecycle_state = 'ready' THEN a.relative_path
                              ELSE a.staged_relative_path
                            END,
                            a.mime_type, a.byte_size, a.lifecycle_state
                     FROM assets a
                     LEFT JOIN asset_drafts ad ON ad.id = a.draft_id
                     LEFT JOIN editor_drafts ed ON ed.id = ad.editor_draft_id
                     WHERE a.id = ?1 AND a.deleted_at IS NULL
                       AND (
                         a.lifecycle_state = 'ready'
                         OR (
                           a.lifecycle_state = 'staged'
                           AND ad.retention_mode = 'persistent_editor'
                           AND ad.committed_at IS NULL AND ad.canceled_at IS NULL
                           AND ed.state IN ('active', 'conflicted')
                         )
                       )",
                    [asset_id.as_str()],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .optional()
                .map_err(CommandError::database)?
        } else {
            snapshot
                .query_row(
                    "SELECT relative_path, mime_type, byte_size, lifecycle_state
                     FROM assets
                     WHERE id = ?1 AND lifecycle_state = 'ready' AND deleted_at IS NULL",
                    [asset_id.as_str()],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .optional()
                .map_err(CommandError::database)?
        };
        let Some((stored_path, stored_mime, stored_size, lifecycle_state)) = stored else {
            return Err(CommandError::new(
                "invalid_backup",
                "A packaged image is not owned by the SQLite asset ledger.",
            ));
        };
        validate_managed_asset_path(&relative_path, &lifecycle_state)?;
        if stored_path != relative_path
            || stored_mime != mime_type
            || stored_size != byte_size
            || data.is_empty()
            || data.len() > MAX_SINGLE_ASSET_BYTES
            || i64::try_from(data.len()).unwrap_or(i64::MAX) != byte_size
        {
            return Err(CommandError::new(
                "invalid_backup",
                "A packaged image does not match the SQLite asset ledger.",
            ));
        }
        let (detected_mime, _) = detect_image_bytes(&data)?;
        if detected_mime != mime_type {
            return Err(CommandError::new(
                "invalid_backup",
                "A packaged image has an invalid file signature.",
            ));
        }
        if let Some(root) = extraction_root {
            let destination = root.join(&relative_path);
            write_new_file(&destination, &data)?;
            sync_directory(destination.parent().ok_or_else(|| {
                CommandError::new("invalid_restore_path", "A restored image path is invalid.")
            })?)?;
        }
    }
    Ok(())
}

fn validate_snapshot_database(connection: &Connection) -> Result<(), CommandError> {
    let integrity: String = connection
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(CommandError::database)?;
    if integrity != "ok" {
        return Err(CommandError::new(
            "invalid_backup",
            "The backup's SQLite library is damaged.",
        ));
    }
    let version = snapshot_schema_version(connection)?;
    if version != LATEST_SCHEMA_VERSION {
        return Err(CommandError::new(
            "unsupported_backup_schema",
            format!("This backup uses unsupported database schema {version}."),
        ));
    }
    Ok(())
}

fn snapshot_schema_version(connection: &Connection) -> Result<i64, CommandError> {
    connection
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |row| row.get(0),
        )
        .map_err(CommandError::database)
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
                "A packaged image has an unsupported lifecycle state.",
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
            "A packaged image path escapes its managed lifecycle directory.",
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
    std::io::copy(&mut input, &mut output).map_err(CommandError::io)?;
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
}
