#!/usr/bin/env python3
"""Repair the active-import rename crash window and add regression coverage."""

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def write(path: str, content: str) -> None:
    (ROOT / path).write_text(content, encoding="utf-8")


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one match, found {count}")
    return text.replace(old, new, 1)


path = "src-tauri/src/asset_lifecycle.rs"
text = read(path)
old = '''        "staged" => {
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
            } else {'''
new = '''        "staged" => {
            let draft_active = asset
                .draft_id
                .as_deref()
                .is_some_and(|draft_id| ensure_active_draft(connection, draft_id, now).is_ok());
            if draft_active {
                if staged_exists {
                    return Ok(());
                }
                if final_exists {
                    let staged = staged_path.as_ref().ok_or_else(|| {
                        CommandError::new(
                            "asset_stage_missing",
                            "A staged asset has no managed staging path.",
                        )
                    })?;
                    create_parent(staged)?;
                    fs::rename(&final_path, staged).map_err(CommandError::io)?;
                    sync_directory(final_path.parent())?;
                    sync_directory(staged.parent())?;
                    record_event(
                        connection,
                        Some(&asset.id),
                        "restored",
                        Some(&asset.relative_path),
                        asset.staged_relative_path.as_deref(),
                        "returned interrupted import rename to active staging",
                        now,
                    )?;
                    return Ok(());
                }
            }
            if staged_exists {
                cleanup.push(staged_path.expect("checked staged path"));
            }
            if final_exists {
                quarantine_asset_file(connection, paths, &asset, &final_path, now)?;
            } else {'''
text = replace_once(text, old, new, "active import staged reconciliation")
write(path, text)

path = "src-tauri/src/stage34_35_regression_tests.rs"
text = read(path)
text = replace_once(
    text,
    '''use crate::{
    data_repairs::{apply_required_repairs, verify_repair_ledger},
    error::CommandError,
};''',
    '''use crate::{
    asset_lifecycle,
    data_repairs::{apply_required_repairs, verify_repair_ledger},
    error::CommandError,
    paths::AppPaths,
};''',
    "crash regression imports",
)
text += r'''

#[test]
fn startup_returns_an_interrupted_import_rename_to_its_active_draft() {
    let root = std::env::temp_dir().join(format!(
        "ostudy-import-reconcile-{}-{}",
        std::process::id(),
        crate::util::unix_time_millis().expect("clock")
    ));
    let paths = AppPaths::new(root).expect("paths should initialize");
    let database = crate::database::Database::open(paths.database_path())
        .expect("database should open");
    let draft_id = "d".repeat(32);
    let asset_id = "e".repeat(32);
    let relative_path = AppPaths::image_relative_path(&asset_id, "png");
    let staged_relative_path =
        AppPaths::staged_asset_relative_path(&draft_id, &asset_id, "png");
    let final_path = paths.resolve_relative(&relative_path);
    std::fs::write(
        &final_path,
        [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 0],
    )
    .expect("interrupted final file should exist");

    database
        .with_connection(|connection| {
            let now = crate::util::unix_time_millis()?;
            connection
                .execute(
                    "INSERT INTO asset_drafts (
                         id, purpose, owner_id, created_at, updated_at, expires_at
                     ) VALUES (?1, 'import', ?2, ?3, ?3, ?4)",
                    params![draft_id, "f".repeat(32), now, now + 60_000],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "INSERT INTO assets (
                         id, original_filename, mime_type, byte_size,
                         relative_path, created_at, lifecycle_state,
                         draft_id, staged_relative_path, updated_at
                     ) VALUES (
                         ?1, 'diagram.png', 'image/png', 12,
                         ?2, ?3, 'staged', ?4, ?5, ?3
                     )",
                    params![
                        asset_id,
                        relative_path,
                        now,
                        draft_id,
                        staged_relative_path,
                    ],
                )
                .map_err(CommandError::database)?;
            Ok(())
        })
        .expect("crash fixture should write");

    asset_lifecycle::reconcile_startup(&database, &paths)
        .expect("startup reconciliation should recover the draft");

    assert!(!final_path.exists());
    assert!(paths.resolve_relative(&staged_relative_path).is_file());
    database
        .with_connection(|connection| {
            let state: (String, Option<String>, Option<String>) = connection
                .query_row(
                    "SELECT lifecycle_state, draft_id, staged_relative_path
                     FROM assets WHERE id = ?1",
                    [asset_id.as_str()],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .map_err(CommandError::database)?;
            assert_eq!(state.0, "staged");
            assert_eq!(state.1.as_deref(), Some(draft_id.as_str()));
            assert_eq!(state.2.as_deref(), Some(staged_relative_path.as_str()));
            let recoveries: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM asset_reconciliation_events
                     WHERE asset_id = ?1 AND action = 'restored'",
                    [asset_id.as_str()],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            assert_eq!(recoveries, 1);
            Ok(())
        })
        .expect("recovered state should persist");
    let _ = std::fs::remove_dir_all(paths.app_data_dir());
}
'''
write(path, text)

print("Applied active import crash reconciliation.")
