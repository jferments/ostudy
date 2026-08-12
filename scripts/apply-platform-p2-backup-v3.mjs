#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const read = (path) => readFile(join(root, path), "utf8");
const write = (path, content) => writeFile(join(root, path), content, "utf8");

function replaceExact(text, oldValue, newValue, label) {
  if (!text.includes(oldValue)) throw new Error(`${label}: expected source was not found`);
  return text.replace(oldValue, newValue);
}

function replaceBetween(text, startMarker, endMarker, replacement, label) {
  const start = text.indexOf(startMarker);
  const end = text.indexOf(endMarker, start + startMarker.length);
  if (start < 0 || end < 0) throw new Error(`${label}: replacement range was not found`);
  return text.slice(0, start) + replacement + text.slice(end);
}

let backup = await read("src-tauri/src/library_backup.rs");
backup = replaceExact(
  backup,
  `                "SELECT COUNT(*) FROM asset_drafts
                 WHERE committed_at IS NULL AND canceled_at IS NULL
                   AND retention_mode = 'ephemeral'
                   AND retention_mode = 'ephemeral'
                   AND retention_mode = 'ephemeral'",`,
  `                "SELECT COUNT(*) FROM asset_drafts
                 WHERE committed_at IS NULL AND canceled_at IS NULL
                   AND retention_mode = 'ephemeral'",`,
  "deduplicate ephemeral backup blocker",
);

const exportAssetFiles = `fn export_asset_files(
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

`;
backup = replaceBetween(
  backup,
  "fn export_asset_files(\n",
  "fn validate_backup_package(\n",
  exportAssetFiles,
  "replace managed backup asset export",
);
backup = backup.replaceAll(
  "validate_backup_assets(&package, &snapshot, None)?;",
  "validate_backup_assets(&package, &snapshot, version, None)?;",
);
backup = backup.replaceAll(
  "validate_backup_assets(&package, &snapshot, Some(extraction_root))?;",
  "validate_backup_assets(&package, &snapshot, version, Some(extraction_root))?;",
);

const validateBackupAssets = `fn validate_backup_assets(
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

`;
backup = replaceBetween(
  backup,
  "fn validate_backup_assets(\n",
  "fn validate_snapshot_database(\n",
  validateBackupAssets,
  "replace managed backup asset validation",
);

const validateManagedPath = `fn validate_managed_asset_path(
    value: &str,
    lifecycle_state: &str,
) -> Result<(), CommandError> {
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

`;
backup = replaceBetween(
  backup,
  "fn validate_managed_asset_path(\n",
  "fn replace_file(\n",
  validateManagedPath,
  "replace managed backup path validation",
);
await write("src-tauri/src/library_backup.rs", backup);

let extended = await read("src-tauri/src/library_backup/extended_tests.rs");
const tests = `

#[test]
fn backup_v3_restores_active_editor_draft_and_its_staged_image() {
    let source_paths = temp_paths("draft-source");
    let source = Database::open(source_paths.database_path()).expect("source database");
    let (draft_id, asset_draft_id, asset_id, staged_relative, payload_json) = source
        .with_connection(|connection| {
            let now = unix_time_millis()?;
            let draft_id = generate_id(connection)?;
            let asset_draft_id = generate_id(connection)?;
            let asset_id = generate_id(connection)?;
            let operation_id = generate_id(connection)?;
            let staged_relative =
                AppPaths::staged_asset_relative_path(&asset_draft_id, &asset_id, "png");
            let final_relative = AppPaths::image_relative_path(&asset_id, "png");
            let payload_json = serde_json::to_string(&json!({
                "frontDoc": {"type":"doc","content":[{"type":"paragraph"}]},
                "backDoc": {"type":"doc","content":[{"type":"paragraph"}]},
                "frontText": "",
                "backText": "",
                "tags": []
            }))
            .map_err(CommandError::serialization)?;
            fs::write(source_paths.resolve_relative(&staged_relative), png_bytes())
                .map_err(CommandError::io)?;
            connection
                .execute(
                    "INSERT INTO asset_drafts (
                         id, purpose, owner_id, created_at, updated_at, expires_at,
                         retention_mode, editor_draft_id
                     ) VALUES (?1, 'card', NULL, ?2, ?2, ?3, 'persistent_editor', ?4)",
                    params![
                        asset_draft_id,
                        now,
                        now.saturating_add(7 * 24 * 60 * 60 * 1000),
                        draft_id,
                    ],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "INSERT INTO editor_drafts (
                         id, open_operation_id, draft_kind, target_entity_id,
                         context_entity_id, base_revision, payload_schema_version,
                         payload_json, payload_sha256, revision, state, asset_draft_id,
                         created_at, updated_at, last_opened_at
                     ) VALUES (
                         ?1, ?2, 'card', NULL,
                         NULL, NULL, 1,
                         ?3, ?4, 1, 'active', ?5,
                         ?6, ?6, ?6
                     )",
                    params![
                        draft_id,
                        operation_id,
                        payload_json,
                        "0".repeat(64),
                        asset_draft_id,
                        now,
                    ],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "INSERT INTO assets (
                         id, original_filename, mime_type, byte_size, relative_path,
                         created_at, lifecycle_state, draft_id, staged_relative_path,
                         updated_at
                     ) VALUES (
                         ?1, 'draft-diagram.png', 'image/png', ?2, ?3,
                         ?4, 'staged', ?5, ?6, ?4
                     )",
                    params![
                        asset_id,
                        png_bytes().len() as i64,
                        final_relative,
                        now,
                        asset_draft_id,
                        staged_relative,
                    ],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "INSERT INTO editor_draft_assets (draft_id, asset_id) VALUES (?1, ?2)",
                    params![draft_id, asset_id],
                )
                .map_err(CommandError::database)?;
            Ok((
                draft_id,
                asset_draft_id,
                asset_id,
                staged_relative,
                payload_json,
            ))
        })
        .expect("persistent draft fixture");

    let backup_path = source_paths.app_data_dir().join("draft-v3.ostudybackup");
    let exported = export_backup(&source, &source_paths, &backup_path).expect("v3 backup");
    assert_eq!(exported.asset_count, 0, "staged images are not canonical library assets");
    let package = Connection::open_with_flags(&backup_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .expect("backup package");
    assert_eq!(backup_version(&package).expect("version"), 3);
    let packaged: (i64, String) = package
        .query_row(
            "SELECT COUNT(*), MIN(relative_path) FROM asset_files",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("packaged staged asset");
    assert_eq!(packaged, (1, staged_relative.clone()));
    drop(package);
    drop(source);

    let target_paths = temp_paths("draft-target");
    let pending = pending_restore_path(&target_paths);
    copy_and_sync(&backup_path, &pending).expect("pending backup");
    let guard = apply_pending_restore(&target_paths)
        .expect("restore should prepare")
        .expect("restore should exist");
    let restored = Database::open(target_paths.database_path()).expect("restored database");
    crate::editor_drafts::reconcile_startup(&restored).expect("draft reconciliation");
    crate::asset_lifecycle::reconcile_startup(&restored, &target_paths)
        .expect("asset reconciliation");
    restored
        .with_connection(|connection| {
            let restored_row: (String, String, String, String, String, i64) = connection
                .query_row(
                    "SELECT ed.state, ed.payload_json, ad.retention_mode,
                            a.lifecycle_state, a.staged_relative_path,
                            (SELECT COUNT(*) FROM editor_draft_assets
                             WHERE draft_id = ed.id AND asset_id = a.id)
                     FROM editor_drafts ed
                     JOIN asset_drafts ad ON ad.id = ed.asset_draft_id
                     JOIN assets a ON a.draft_id = ad.id
                     WHERE ed.id = ?1 AND ad.id = ?2 AND a.id = ?3",
                    params![draft_id, asset_draft_id, asset_id],
                    |row| {
                        Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                            row.get(4)?,
                            row.get(5)?,
                        ))
                    },
                )
                .map_err(CommandError::database)?;
            assert_eq!(
                restored_row,
                (
                    "active".to_string(),
                    payload_json,
                    "persistent_editor".to_string(),
                    "staged".to_string(),
                    staged_relative.clone(),
                    1,
                )
            );
            Ok(())
        })
        .expect("restored draft ledger");
    assert_eq!(
        fs::read(target_paths.resolve_relative(&staged_relative)).expect("restored draft image"),
        png_bytes()
    );
    drop(restored);
    guard.commit();
    let _ = fs::remove_dir_all(source_paths.app_data_dir());
    let _ = fs::remove_dir_all(target_paths.app_data_dir());
}

#[test]
fn backup_versions_one_and_two_remain_restorable() {
    let source_paths = temp_paths("legacy-source");
    let source = Database::open(source_paths.database_path()).expect("source database");
    source
        .with_connection(|connection| {
            create_deck_in_connection(
                connection,
                CreateDeckInput {
                    name: "Legacy-compatible deck".to_string(),
                    description: None,
                },
            )?;
            Ok(())
        })
        .expect("legacy fixture");
    let current_path = source_paths.app_data_dir().join("current.ostudybackup");
    export_backup(&source, &source_paths, &current_path).expect("current backup");
    drop(source);

    for version in [1_i64, 2_i64] {
        let legacy_path = source_paths
            .app_data_dir()
            .join(format!("legacy-v{version}.ostudybackup"));
        copy_and_sync(&current_path, &legacy_path).expect("legacy package copy");
        {
            let package = Connection::open(&legacy_path).expect("writable legacy package");
            package
                .execute(
                    "UPDATE backup_metadata SET value = ?1 WHERE key = 'formatVersion'",
                    [version.to_string()],
                )
                .expect("legacy version metadata");
        }
        let summary = validate_backup_package(&legacy_path, None).expect("legacy package validation");
        assert_eq!(summary.deck_count, 1);

        let target_paths = temp_paths(&format!("legacy-target-v{version}"));
        copy_and_sync(&legacy_path, &pending_restore_path(&target_paths))
            .expect("pending legacy backup");
        let guard = apply_pending_restore(&target_paths)
            .expect("legacy restore should prepare")
            .expect("legacy restore should exist");
        let restored = Database::open(target_paths.database_path()).expect("legacy restored database");
        let deck_count = restored
            .with_connection(|connection| {
                connection
                    .query_row(
                        "SELECT COUNT(*) FROM decks WHERE deleted_at IS NULL",
                        [],
                        |row| row.get::<_, i64>(0),
                    )
                    .map_err(CommandError::database)
            })
            .expect("legacy deck count");
        assert_eq!(deck_count, 1);
        drop(restored);
        guard.commit();
        let _ = fs::remove_dir_all(target_paths.app_data_dir());
    }
    let _ = fs::remove_dir_all(source_paths.app_data_dir());
}
`;
if (extended.includes("backup_v3_restores_active_editor_draft_and_its_staged_image")) {
  throw new Error("The backup v3 draft round-trip test already exists.");
}
extended += tests;
await write("src-tauri/src/library_backup/extended_tests.rs", extended);

console.log("Applied backup v3 persistent-draft validation and compatibility tests.");
