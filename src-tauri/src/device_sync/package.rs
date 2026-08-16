use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Component, Path, PathBuf},
    time::Duration,
};

use rusqlite::{backup::Backup, params, Connection, OpenFlags};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{
    assets::detect_image_bytes,
    database::{Database, LATEST_SCHEMA_VERSION},
    error::CommandError,
    paths::AppPaths,
    util::unix_time_millis,
};

const PACKAGE_FORMAT: &str = "ostudy-device-sync";
const PACKAGE_VERSION: i64 = 1;
const MAX_PACKAGE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_DATABASE_BYTES: usize = 1_500 * 1024 * 1024;
const MAX_SINGLE_ASSET_BYTES: usize = 25 * 1024 * 1024;
const PACKAGE_SCHEMA: &str = r#"
PRAGMA foreign_keys = ON;
CREATE TABLE package_metadata (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
) STRICT;
CREATE TABLE database_snapshot (
    id INTEGER PRIMARY KEY CHECK(id = 1),
    data BLOB NOT NULL
) STRICT;
CREATE TABLE asset_files (
    asset_id TEXT PRIMARY KEY CHECK(length(asset_id) = 32),
    relative_path TEXT NOT NULL UNIQUE,
    mime_type TEXT NOT NULL,
    byte_size INTEGER NOT NULL CHECK(byte_size > 0),
    sha256 TEXT NOT NULL CHECK(length(sha256) = 64),
    data BLOB NOT NULL
) STRICT;
"#;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PackageSummary {
    pub database_bytes: u64,
    pub asset_count: usize,
    pub asset_bytes: u64,
    pub package_bytes: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct ExtractedAsset {
    pub asset_id: String,
    pub relative_path: String,
    pub mime_type: String,
    pub byte_size: u64,
    pub sha256: String,
    pub staged_path: PathBuf,
}

#[derive(Clone, Debug)]
pub(crate) struct ExtractedPackage {
    pub database_path: PathBuf,
    pub assets: Vec<ExtractedAsset>,
}

pub(crate) fn create_package(
    database: &Database,
    paths: &AppPaths,
    destination: &Path,
) -> Result<PackageSummary, CommandError> {
    let parent = destination.parent().ok_or_else(|| {
        CommandError::new(
            "device_sync_package_path_invalid",
            "The synchronization package path is invalid.",
        )
    })?;
    fs::create_dir_all(parent).map_err(CommandError::io)?;
    let scratch = parent.join(format!(
        ".device-sync-package-{}-{}",
        std::process::id(),
        unix_time_millis()?
    ));
    remove_dir_if_exists(&scratch)?;
    fs::create_dir_all(&scratch).map_err(CommandError::io)?;
    let snapshot_path = scratch.join("library.sqlite3");
    let result = (|| {
        database.with_connection(|source| {
            let mut target = Connection::open(&snapshot_path).map_err(CommandError::database)?;
            let backup = Backup::new(source, &mut target).map_err(CommandError::database)?;
            backup
                .run_to_completion(128, Duration::from_millis(10), None)
                .map_err(CommandError::database)
        })?;
        sanitize_snapshot(&snapshot_path)?;
        let database_bytes = fs::read(&snapshot_path).map_err(CommandError::io)?;
        if database_bytes.is_empty() || database_bytes.len() > MAX_DATABASE_BYTES {
            return Err(CommandError::new(
                "device_sync_database_too_large",
                "The local SQLite library is too large for device synchronization.",
            ));
        }

        let temporary = destination.with_extension(format!("sync.{}.tmp", std::process::id()));
        remove_file_if_exists(&temporary)?;
        let mut package = Connection::open(&temporary).map_err(CommandError::database)?;
        package
            .execute_batch(
                "PRAGMA journal_mode = DELETE;
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
            ("format", PACKAGE_FORMAT.to_string()),
            ("formatVersion", PACKAGE_VERSION.to_string()),
            ("schemaVersion", LATEST_SCHEMA_VERSION.to_string()),
            ("createdAt", created_at.to_string()),
            ("applicationVersion", env!("CARGO_PKG_VERSION").to_string()),
        ] {
            transaction
                .execute(
                    "INSERT INTO package_metadata (key, value) VALUES (?1, ?2)",
                    params![key, value],
                )
                .map_err(CommandError::database)?;
        }
        transaction
            .execute(
                "INSERT INTO database_snapshot (id, data) VALUES (1, ?1)",
                [database_bytes.as_slice()],
            )
            .map_err(CommandError::database)?;

        let snapshot = Connection::open_with_flags(&snapshot_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(CommandError::database)?;
        let (asset_count, asset_bytes) = export_assets(&snapshot, paths, &transaction)?;
        transaction.commit().map_err(CommandError::database)?;
        let integrity: String = package
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))
            .map_err(CommandError::database)?;
        if integrity != "ok" {
            return Err(CommandError::new(
                "device_sync_package_invalid",
                "The completed synchronization package failed its integrity check.",
            ));
        }
        drop(package);
        sync_file(&temporary)?;
        replace_file(&temporary, destination)?;
        let package_bytes = fs::metadata(destination).map_err(CommandError::io)?.len();
        if package_bytes == 0 || package_bytes > MAX_PACKAGE_BYTES {
            return Err(CommandError::new(
                "device_sync_package_too_large",
                "The synchronization package exceeds the supported size limit.",
            ));
        }
        Ok(PackageSummary {
            database_bytes: u64::try_from(database_bytes.len()).unwrap_or(u64::MAX),
            asset_count,
            asset_bytes,
            package_bytes,
        })
    })();
    let _ = fs::remove_dir_all(&scratch);
    result
}

pub(crate) fn extract_package(
    source: &Path,
    staging_root: &Path,
) -> Result<ExtractedPackage, CommandError> {
    let metadata = fs::metadata(source).map_err(CommandError::io)?;
    if metadata.len() == 0 || metadata.len() > MAX_PACKAGE_BYTES {
        return Err(invalid_package(
            "The synchronization package is empty or exceeds the supported size limit.",
        ));
    }
    remove_dir_if_exists(staging_root)?;
    fs::create_dir_all(staging_root).map_err(CommandError::io)?;
    let package = Connection::open_with_flags(source, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|error| invalid_package(format!("The synchronization package could not open: {error}")))?;
    package
        .execute_batch("PRAGMA trusted_schema = OFF; PRAGMA query_only = ON;")
        .map_err(CommandError::database)?;
    let integrity: String = package
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(CommandError::database)?;
    if integrity != "ok"
        || metadata_value(&package, "format")? != PACKAGE_FORMAT
        || metadata_value(&package, "formatVersion")? != PACKAGE_VERSION.to_string()
    {
        return Err(invalid_package(
            "This is not a supported ostudy device synchronization package.",
        ));
    }
    let schema_version = metadata_value(&package, "schemaVersion")?
        .parse::<i64>()
        .map_err(|_| invalid_package("The package schema version is invalid."))?;
    if schema_version > LATEST_SCHEMA_VERSION {
        return Err(invalid_package(
            "The sending device uses a newer ostudy database schema. Update this device first.",
        ));
    }

    let database_bytes: Vec<u8> = package
        .query_row(
            "SELECT data FROM database_snapshot WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    if database_bytes.is_empty() || database_bytes.len() > MAX_DATABASE_BYTES {
        return Err(invalid_package(
            "The package contains an invalid SQLite snapshot.",
        ));
    }
    let database_path = staging_root.join("library.sqlite3");
    write_new_file(&database_path, &database_bytes)?;
    // Opening through the application database boundary applies any supported
    // forward migrations before the merge engine inspects rows.
    let migrated = Database::open(&database_path).map_err(|message| {
        invalid_package(format!("The synchronized SQLite snapshot is invalid: {message}"))
    })?;
    drop(migrated);

    let assets_root = staging_root.join("assets");
    fs::create_dir_all(&assets_root).map_err(CommandError::io)?;
    let mut statement = package
        .prepare(
            "SELECT asset_id, relative_path, mime_type, byte_size, sha256, data
             FROM asset_files
             ORDER BY asset_id",
        )
        .map_err(CommandError::database)?;
    let mut rows = statement.query([]).map_err(CommandError::database)?;
    let mut assets = Vec::new();
    let mut aggregate_bytes = 0_u64;
    while let Some(row) = rows.next().map_err(CommandError::database)? {
        let asset_id: String = row.get(0).map_err(CommandError::database)?;
        let relative_path: String = row.get(1).map_err(CommandError::database)?;
        let mime_type: String = row.get(2).map_err(CommandError::database)?;
        let byte_size: i64 = row.get(3).map_err(CommandError::database)?;
        let expected_sha: String = row.get(4).map_err(CommandError::database)?;
        let data: Vec<u8> = row.get(5).map_err(CommandError::database)?;
        crate::util::validate_entity_id(&asset_id, "synchronized asset")?;
        validate_asset_path(&relative_path, &asset_id)?;
        if data.is_empty()
            || data.len() > MAX_SINGLE_ASSET_BYTES
            || i64::try_from(data.len()).unwrap_or(i64::MAX) != byte_size
            || expected_sha.len() != 64
        {
            return Err(invalid_package(
                "A synchronized managed image has an invalid size record.",
            ));
        }
        let actual_sha = format!("{:x}", Sha256::digest(&data));
        if actual_sha != expected_sha {
            return Err(invalid_package(
                "A synchronized managed image failed its content hash check.",
            ));
        }
        let (detected_mime, _) = detect_image_bytes(&data)?;
        if detected_mime != mime_type {
            return Err(invalid_package(
                "A synchronized managed image has an invalid file type.",
            ));
        }
        aggregate_bytes = aggregate_bytes.saturating_add(u64::try_from(data.len()).unwrap_or(u64::MAX));
        if aggregate_bytes > MAX_PACKAGE_BYTES {
            return Err(invalid_package(
                "The synchronized image payload exceeds the supported size limit.",
            ));
        }
        let staged_path = assets_root.join(format!("{asset_id}.image"));
        write_new_file(&staged_path, &data)?;
        assets.push(ExtractedAsset {
            asset_id,
            relative_path,
            mime_type,
            byte_size: u64::try_from(byte_size).map_err(|_| invalid_package("An asset size is invalid."))?,
            sha256: actual_sha,
            staged_path,
        });
    }
    Ok(ExtractedPackage {
        database_path,
        assets,
    })
}

pub(crate) fn install_assets(
    paths: &AppPaths,
    assets: &[ExtractedAsset],
) -> Result<(), CommandError> {
    for asset in assets {
        let target = paths.resolve_relative(&asset.relative_path);
        let parent = target.parent().ok_or_else(|| invalid_package("An asset path is invalid."))?;
        fs::create_dir_all(parent).map_err(CommandError::io)?;
        if target.is_file() {
            let existing = fs::read(&target).map_err(CommandError::io)?;
            let existing_sha = format!("{:x}", Sha256::digest(&existing));
            if existing_sha == asset.sha256 {
                continue;
            }
            return Err(CommandError::new(
                "device_sync_asset_identity_conflict",
                "A local managed image has the same identity but different bytes.",
            ));
        }
        let temporary = target.with_extension(format!("sync.{}.tmp", std::process::id()));
        remove_file_if_exists(&temporary)?;
        fs::copy(&asset.staged_path, &temporary).map_err(CommandError::io)?;
        sync_file(&temporary)?;
        replace_file(&temporary, &target)?;
    }
    Ok(())
}

fn sanitize_snapshot(path: &Path) -> Result<(), CommandError> {
    let connection = Connection::open(path).map_err(CommandError::database)?;
    connection
        .execute_batch(
            "UPDATE ai_provider_profiles
             SET credential_storage = 'none', credential_epoch = 0;
             DELETE FROM ai_provider_probe_results;
             DELETE FROM ai_provider_capabilities;",
        )
        .map_err(CommandError::database)?;
    let integrity: String = connection
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(CommandError::database)?;
    if integrity != "ok" {
        return Err(CommandError::new(
            "device_sync_snapshot_invalid",
            "The local synchronization snapshot failed its integrity check.",
        ));
    }
    Ok(())
}

fn export_assets(
    snapshot: &Connection,
    paths: &AppPaths,
    transaction: &rusqlite::Transaction<'_>,
) -> Result<(usize, u64), CommandError> {
    let mut statement = snapshot
        .prepare(
            "SELECT id, relative_path, mime_type, byte_size
             FROM assets
             WHERE lifecycle_state = 'ready' AND deleted_at IS NULL
             ORDER BY id",
        )
        .map_err(CommandError::database)?;
    let mut rows = statement.query([]).map_err(CommandError::database)?;
    let mut count = 0_usize;
    let mut aggregate = 0_u64;
    while let Some(row) = rows.next().map_err(CommandError::database)? {
        let asset_id: String = row.get(0).map_err(CommandError::database)?;
        let relative_path: String = row.get(1).map_err(CommandError::database)?;
        let mime_type: String = row.get(2).map_err(CommandError::database)?;
        let expected_size: i64 = row.get(3).map_err(CommandError::database)?;
        validate_asset_path(&relative_path, &asset_id)?;
        let data = fs::read(paths.resolve_relative(&relative_path)).map_err(|_| {
            CommandError::new(
                "device_sync_asset_missing",
                "A managed image is missing. Repair it before synchronizing devices.",
            )
        })?;
        if data.is_empty()
            || data.len() > MAX_SINGLE_ASSET_BYTES
            || i64::try_from(data.len()).unwrap_or(i64::MAX) != expected_size
        {
            return Err(CommandError::new(
                "device_sync_asset_invalid",
                "A managed image does not match its database size record.",
            ));
        }
        let (detected_mime, _) = detect_image_bytes(&data)?;
        if detected_mime != mime_type {
            return Err(CommandError::new(
                "device_sync_asset_invalid",
                "A managed image does not match its database file type.",
            ));
        }
        let sha256 = format!("{:x}", Sha256::digest(&data));
        transaction
            .execute(
                "INSERT INTO asset_files (
                     asset_id, relative_path, mime_type, byte_size, sha256, data
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![asset_id, relative_path, mime_type, expected_size, sha256, data],
            )
            .map_err(CommandError::database)?;
        count = count.saturating_add(1);
        aggregate = aggregate.saturating_add(u64::try_from(expected_size).unwrap_or(u64::MAX));
        if aggregate > MAX_PACKAGE_BYTES {
            return Err(CommandError::new(
                "device_sync_package_too_large",
                "The managed image library is too large for device synchronization.",
            ));
        }
    }
    Ok((count, aggregate))
}

fn metadata_value(connection: &Connection, key: &str) -> Result<String, CommandError> {
    connection
        .query_row(
            "SELECT value FROM package_metadata WHERE key = ?1",
            [key],
            |row| row.get(0),
        )
        .map_err(|_| invalid_package(format!("The package is missing metadata '{key}'.")))
}

fn validate_asset_path(relative: &str, asset_id: &str) -> Result<(), CommandError> {
    let path = Path::new(relative);
    if path.is_absolute()
        || path.components().any(|component| !matches!(component, Component::Normal(_)))
        || path.components().count() != 3
        || !relative.starts_with("assets/images/")
        || !path
            .file_name()
            .and_then(|value| value.to_str())
            .is_some_and(|name| name.starts_with(&format!("{asset_id}.")))
    {
        return Err(invalid_package(
            "A synchronized managed image path is invalid.",
        ));
    }
    Ok(())
}

fn write_new_file(path: &Path, data: &[u8]) -> Result<(), CommandError> {
    let parent = path.parent().ok_or_else(|| invalid_package("A package path is invalid."))?;
    fs::create_dir_all(parent).map_err(CommandError::io)?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    let mut file = options.open(path).map_err(CommandError::io)?;
    file.write_all(data).map_err(CommandError::io)?;
    file.sync_all().map_err(CommandError::io)
}

fn sync_file(path: &Path) -> Result<(), CommandError> {
    OpenOptions::new()
        .read(true)
        .open(path)
        .and_then(|file| file.sync_all())
        .map_err(CommandError::io)
}

fn replace_file(temporary: &Path, destination: &Path) -> Result<(), CommandError> {
    if destination.exists() {
        fs::remove_file(destination).map_err(CommandError::io)?;
    }
    fs::rename(temporary, destination).map_err(CommandError::io)?;
    if let Some(parent) = destination.parent() {
        File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(CommandError::io)?;
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

fn remove_dir_if_exists(path: &Path) -> Result<(), CommandError> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(CommandError::io(error)),
    }
}

fn invalid_package(message: impl Into<String>) -> CommandError {
    CommandError::new("device_sync_package_invalid", message)
}
