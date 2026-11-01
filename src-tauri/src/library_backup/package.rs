//! Versioned complete-backup encoding, validation, and staged extraction.

use super::*;

pub(super) fn export_backup(
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
            "Finish document extraction and flashcard generation before backing up the library.",
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
    // Windows refuses to remove the scratch folder while this file is open.
    drop(snapshot);
    let package_size = fs::metadata(&temporary).map_err(CommandError::io)?.len();
    if package_size == 0 || package_size > MAX_BACKUP_FILE_BYTES {
        return Err(CommandError::new(
            "backup_too_large",
            "The completed backup exceeds ostudy's supported size limit.",
        ));
    }
    crate::util::durable_fs::sync_file(&temporary).map_err(CommandError::io)?;
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
                "A managed asset is missing. Repair it before creating a complete backup.",
            )
        })?;
        if data.is_empty()
            || data.len() > MAX_SINGLE_ASSET_BYTES
            || i64::try_from(data.len()).unwrap_or(i64::MAX) != byte_size
        {
            return Err(CommandError::new(
                "invalid_asset",
                "A managed asset does not match its database size record.",
            ));
        }
        let (detected_mime, _) = inspect_managed_asset_bytes(&data)?;
        if detected_mime != mime_type {
            return Err(CommandError::new(
                "invalid_asset",
                "A managed asset does not match its database file type.",
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

pub(super) fn backup_validation_temp_path(timestamp_ms: i64) -> PathBuf {
    std::env::temp_dir().join(format!(
        "ostudy-backup-validation-{}-{timestamp_ms}-{}.sqlite3",
        std::process::id(),
        BACKUP_VALIDATION_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ))
}

pub(super) fn validate_backup_package(
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
        .unwrap_or_else(|| backup_validation_temp_path(unix_time_millis().unwrap_or_default()));
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
            "This legacy backup version cannot contain managed documents.",
        ));
    }
    let summary = snapshot_summary(&snapshot)?;
    drop(snapshot);
    if extract_database_to.is_none() {
        remove_file_if_exists(&temporary).map_err(CommandError::io)?;
    }
    Ok(summary)
}

pub(super) fn extract_backup_package(
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
            "The backup does not contain every managed asset required by the library and its recoverable drafts.",
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
                "A packaged asset is not owned by the SQLite asset ledger.",
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
                "A packaged asset does not match the SQLite asset ledger.",
            ));
        }
        let (detected_mime, _) = inspect_managed_asset_bytes(&data)?;
        if detected_mime != mime_type {
            return Err(CommandError::new(
                "invalid_backup",
                "A packaged asset has an invalid file signature.",
            ));
        }
        if let Some(root) = extraction_root {
            let destination = root.join(&relative_path);
            write_new_file(&destination, &data)?;
            sync_directory(destination.parent().ok_or_else(|| {
                CommandError::new("invalid_restore_path", "A restored asset path is invalid.")
            })?)?;
        }
    }
    Ok(())
}
