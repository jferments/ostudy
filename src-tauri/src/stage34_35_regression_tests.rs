use rusqlite::params;

use crate::{asset_lifecycle, error::CommandError, paths::AppPaths};

#[test]
fn canonical_database_reopen_does_not_mutate_application_rows() {
    let root = std::env::temp_dir().join(format!(
        "ostudy-no-op-startup-{}-{}",
        std::process::id(),
        crate::util::unix_time_millis().expect("clock")
    ));
    std::fs::create_dir_all(&root).expect("temp directory");
    let database_path = root.join("library.sqlite3");

    {
        let database =
            crate::database::Database::open(&database_path).expect("database should open");
        database
            .with_connection(|connection| {
                connection
                    .execute(
                        "INSERT INTO decks (
                             id, name, description, sort_order,
                             created_at, updated_at, revision
                         ) VALUES (?1, 'Canonical', '', 0, 111, 222, 7)",
                        ["c".repeat(32)],
                    )
                    .map_err(CommandError::database)?;
                Ok(())
            })
            .expect("fixture should write");
    }

    let before = std::fs::metadata(&database_path)
        .expect("database metadata")
        .len();
    let database = crate::database::Database::open(&database_path).expect("database should reopen");
    database
        .with_connection(|connection| {
            assert_eq!(connection.total_changes(), 0);
            let deck: (i64, i64) = connection
                .query_row(
                    "SELECT updated_at, revision FROM decks WHERE id = ?1",
                    ["c".repeat(32)],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .map_err(CommandError::database)?;
            assert_eq!(deck, (222, 7));
            let repair_ledger: (i64, i64) = connection
                .query_row(
                    "SELECT COUNT(*), COALESCE(SUM(changed_rows), 0) FROM data_repairs",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .map_err(CommandError::database)?;
            assert_eq!(repair_ledger, (3, 0));
            Ok(())
        })
        .expect("canonical startup should be read-only");
    drop(database);
    let after = std::fs::metadata(&database_path)
        .expect("database metadata")
        .len();
    assert_eq!(before, after);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn startup_returns_an_interrupted_import_rename_to_its_active_draft() {
    let root = std::env::temp_dir().join(format!(
        "ostudy-import-reconcile-{}-{}",
        std::process::id(),
        crate::util::unix_time_millis().expect("clock")
    ));
    let paths = AppPaths::new(root).expect("paths should initialize");
    let database =
        crate::database::Database::open(paths.database_path()).expect("database should open");
    let draft_id = "d".repeat(32);
    let asset_id = "e".repeat(32);
    let relative_path = AppPaths::image_relative_path(&asset_id, "png");
    let staged_relative_path = AppPaths::staged_asset_relative_path(&draft_id, &asset_id, "png");
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
                    params![asset_id, relative_path, now, draft_id, staged_relative_path,],
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
