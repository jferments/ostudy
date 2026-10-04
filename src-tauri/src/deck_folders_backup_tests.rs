use std::time::Duration;

use rusqlite::{backup::Backup, params, Connection};

use crate::{database::Database, error::CommandError};

/// Deck folders are subjects, so a snapshot carries a deck's subject and order.
#[test]
fn database_snapshot_preserves_folder_membership_and_ordering() {
    let database = Database::open_in_memory().expect("database should open");
    database
        .with_connection(|source| {
            let folder_id = "f".repeat(32);
            let deck_id = "d".repeat(32);
            source
                .execute(
                    "INSERT INTO document_collections (
                         id, name, sort_order, created_at, updated_at, revision
                     ) VALUES (?1, 'Nursing', 1000, 1, 1, 1)",
                    [folder_id.as_str()],
                )
                .map_err(CommandError::database)?;
            source
                .execute(
                    "INSERT INTO decks (
                         id, name, description, sort_order, created_at, updated_at, revision,
                         subject_id
                     ) VALUES (?1, 'Pharmacology', '', 2000, 1, 1, 1, ?2)",
                    params![deck_id, folder_id],
                )
                .map_err(CommandError::database)?;

            let mut restored = Connection::open_in_memory().map_err(CommandError::database)?;
            {
                let backup = Backup::new(source, &mut restored).map_err(CommandError::database)?;
                backup
                    .run_to_completion(128, Duration::from_millis(1), None)
                    .map_err(CommandError::database)?;
            }

            let restored_folder: (String, i64) = restored
                .query_row(
                    "SELECT name, sort_order FROM document_collections WHERE id = ?1",
                    [folder_id.as_str()],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .map_err(CommandError::database)?;
            assert_eq!(restored_folder, ("Nursing".to_string(), 1000));

            let restored_deck: (Option<String>, i64) = restored
                .query_row(
                    "SELECT subject_id, sort_order FROM decks WHERE id = ?1",
                    [deck_id.as_str()],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .map_err(CommandError::database)?;
            assert_eq!(restored_deck, (Some(folder_id), 2000));

            let violation_count: i64 = restored
                .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
                    row.get(0)
                })
                .map_err(CommandError::database)?;
            assert_eq!(violation_count, 0);
            Ok(())
        })
        .expect("snapshot should preserve folder organization");
}
