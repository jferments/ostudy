//! Import-operation journal for deck package asset staging.
//!
//! The package importer already writes every image through a unique temporary
//! file, flushes it, renames it into the managed namespace, and removes files on
//! ordinary errors. This module adds the durable operation identity needed for
//! crash recovery: an unfinished import remains visible in `asset_drafts`, while
//! startup reconciliation quarantines unowned temp/final files and preserves any
//! file that reached a durable note/template reference.

use rusqlite::params;

use crate::{
    asset_lifecycle,
    database::Database,
    error::CommandError,
    paths::AppPaths,
    util::{generate_id, unix_time_millis, validate_entity_id},
};

const IMPORT_OPERATION_TTL_MS: i64 = 24 * 60 * 60 * 1_000;

pub(crate) fn begin_import(database: &Database) -> Result<String, CommandError> {
    database.with_connection(|connection| {
        let now = unix_time_millis()?;
        let id = generate_id(connection)?;
        connection
            .execute(
                "INSERT INTO asset_drafts (
                     id, purpose, owner_id, created_at, updated_at, expires_at
                 ) VALUES (?1, 'import', NULL, ?2, ?2, ?3)",
                params![id, now, now.saturating_add(IMPORT_OPERATION_TTL_MS)],
            )
            .map_err(CommandError::database)?;
        Ok(id)
    })
}

pub(crate) fn finish_import(
    database: &Database,
    paths: &AppPaths,
    operation_id: &str,
    deck_id: &str,
) -> Result<(), CommandError> {
    validate_entity_id(operation_id, "asset import operation")?;
    validate_entity_id(deck_id, "deck")?;
    database.with_connection(|connection| {
        let now = unix_time_millis()?;
        let changed = connection
            .execute(
                "UPDATE asset_drafts
                 SET owner_id = ?1, updated_at = ?2, committed_at = ?2
                 WHERE id = ?3 AND purpose = 'import'
                   AND committed_at IS NULL AND canceled_at IS NULL",
                params![deck_id, now, operation_id],
            )
            .map_err(CommandError::database)?;
        if changed != 1 {
            return Err(CommandError::new(
                "asset_import_operation_changed",
                "The managed import operation could not be finalized.",
            ));
        }
        Ok(())
    })?;
    asset_lifecycle::reconcile_startup(database, paths)
}

pub(crate) fn abort_import(
    database: &Database,
    paths: &AppPaths,
    operation_id: &str,
    detail: &str,
) -> Result<(), CommandError> {
    validate_entity_id(operation_id, "asset import operation")?;
    database.with_connection(|connection| {
        let now = unix_time_millis()?;
        connection
            .execute(
                "UPDATE asset_drafts
                 SET updated_at = ?1, canceled_at = ?1, owner_id = ?2
                 WHERE id = ?3 AND purpose = 'import'
                   AND committed_at IS NULL AND canceled_at IS NULL",
                params![now, detail, operation_id],
            )
            .map_err(CommandError::database)?;
        Ok(())
    })?;
    asset_lifecycle::reconcile_startup(database, paths)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::AppPaths;

    fn temp_paths() -> AppPaths {
        let root = std::env::temp_dir().join(format!(
            "ostudy-transfer-lifecycle-{}-{}",
            std::process::id(),
            unix_time_millis().expect("clock")
        ));
        AppPaths::new(root).expect("paths should initialize")
    }

    #[test]
    fn import_operations_commit_or_cancel_explicitly() {
        let database = Database::open_in_memory().expect("database should open");
        let paths = temp_paths();
        let committed = begin_import(&database).expect("operation should begin");
        let canceled = begin_import(&database).expect("operation should begin");

        database
            .with_connection(|connection| {
                connection
                    .execute(
                        "INSERT INTO decks (
                             id, name, description, sort_order,
                             created_at, updated_at, revision
                         ) VALUES (?1, 'Imported', '', 0, 1, 1, 1)",
                        ["a".repeat(32)],
                    )
                    .map_err(CommandError::database)?;
                Ok(())
            })
            .expect("deck should insert");

        finish_import(&database, &paths, &committed, &"a".repeat(32))
            .expect("operation should commit");
        abort_import(&database, &paths, &canceled, "test failure")
            .expect("operation should cancel");

        database
            .with_connection(|connection| {
                let state: (Option<i64>, Option<i64>, Option<String>) = connection
                    .query_row(
                        "SELECT committed_at, canceled_at, owner_id
                         FROM asset_drafts WHERE id = ?1",
                        [committed.as_str()],
                        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                    )
                    .map_err(CommandError::database)?;
                assert!(state.0.is_some());
                assert_eq!(state.1, None);
                assert_eq!(state.2.as_deref(), Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"));

                let canceled_at: Option<i64> = connection
                    .query_row(
                        "SELECT canceled_at FROM asset_drafts WHERE id = ?1",
                        [canceled.as_str()],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                assert!(canceled_at.is_some());
                Ok(())
            })
            .expect("operation state should persist");
        let _ = std::fs::remove_dir_all(paths.app_data_dir());
    }
}
