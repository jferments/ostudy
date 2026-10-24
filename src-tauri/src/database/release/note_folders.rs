//! Additive release-v23 admission for Notes folders and notebook pins.
//!
//! Migration 23 adds the learner's Notes folders and each notebook's place in
//! them. An upgrade writes none, so every notebook starts at the top.

use std::sync::OnceLock;

use rusqlite::Connection;

use super::object_delta::{self, Delta};

const VERSION: i64 = 23;

pub(super) fn validate_schema(connection: &Connection, version: i64) -> Result<(), String> {
    static DELTA: OnceLock<Result<Delta, String>> = OnceLock::new();
    object_delta::validate(connection, version, VERSION, "Notes folders", &DELTA)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::release::{
        apply_migrations, apply_migrations_through, migration_sql, preservation_receipt,
        subjects_fixture::release_library, validate_backup_input, validate_input,
        validate_supported_input,
    };

    const NOTEBOOK: &str = "11111111111111111111111111111111";
    const FOLDER: &str = "22222222222222222222222222222222";
    const INNER: &str = "33333333333333333333333333333333";

    /// A schema-22 library as the previous release left it, with one notebook.
    fn schema_twenty_two() -> Connection {
        let connection = release_library(22);
        connection
            .execute_batch(&format!(
                "INSERT INTO notebooks(id,subject_id,title,paper,page_size,revision,created_at,updated_at)
                 VALUES('{NOTEBOOK}',NULL,'Questions','lined','letter',1,1,1);"
            ))
            .unwrap();
        assert_eq!(validate_supported_input(&connection).unwrap(), 22);
        connection
    }

    fn count(connection: &Connection, sql: &str) -> i64 {
        connection.query_row(sql, [], |row| row.get(0)).unwrap()
    }

    #[test]
    fn schema_twenty_two_upgrades_additively_with_every_notebook_at_the_top() {
        let mut connection = schema_twenty_two();
        let before = preservation_receipt(&connection).unwrap();
        let changes = connection.total_changes();
        validate_backup_input(&connection).unwrap();
        assert_eq!(connection.total_changes(), changes);

        assert!(apply_migrations_through(&mut connection, VERSION).unwrap());
        assert_eq!(validate_supported_input(&connection).unwrap(), VERSION);
        assert_eq!(count(&connection, "SELECT COUNT(*) FROM note_folders"), 0);
        assert_eq!(count(&connection, "SELECT COUNT(*) FROM notebook_shelf"), 0);
        assert_eq!(count(&connection, "SELECT COUNT(*) FROM notebooks"), 1);

        let comparison = connection.transaction().unwrap();
        comparison
            .execute_batch("DROP TABLE notebook_shelf; DROP TABLE note_folders;")
            .unwrap();
        assert_eq!(preservation_receipt(&comparison).unwrap(), before);
        comparison.rollback().unwrap();
    }

    #[test]
    fn deleting_a_folder_deletes_its_folders_and_returns_their_notebooks_to_the_top() {
        let mut connection = schema_twenty_two();
        apply_migrations(&mut connection).unwrap();
        connection
            .execute_batch(&format!(
                "INSERT INTO note_folders(id,parent_id,name,sort_order,created_at,updated_at)
                 VALUES('{FOLDER}',NULL,'Physiology',0,1,1),('{INNER}','{FOLDER}','GI',0,1,1);
                 INSERT INTO notebook_shelf(notebook_id,folder_id,pinned_at,updated_at)
                 VALUES('{NOTEBOOK}','{INNER}',5,5);
                 DELETE FROM note_folders WHERE id='{FOLDER}';"
            ))
            .unwrap();
        assert_eq!(count(&connection, "SELECT COUNT(*) FROM note_folders"), 0);
        assert_eq!(
            count(
                &connection,
                "SELECT COUNT(*) FROM notebook_shelf WHERE folder_id IS NULL AND pinned_at = 5"
            ),
            1,
            "the notebook keeps its pin at the top"
        );
        connection
            .execute_batch(&format!("DELETE FROM notebooks WHERE id='{NOTEBOOK}'"))
            .unwrap();
        assert_eq!(count(&connection, "SELECT COUNT(*) FROM notebook_shelf"), 0);
        assert!(connection
            .execute_batch(&format!(
                "INSERT INTO note_folders(id,parent_id,name,sort_order,created_at,updated_at)
                 VALUES('{FOLDER}','{FOLDER}','Loop',0,1,1)"
            ))
            .is_err());
    }

    #[test]
    fn interrupted_twenty_third_migration_rolls_back_to_schema_twenty_two_and_retries() {
        let mut connection = schema_twenty_two();
        let before = preservation_receipt(&connection).unwrap();
        connection
            .execute_batch(
                "CREATE TEMP TRIGGER reject_note_folders_migration
                 BEFORE INSERT ON schema_migrations
                 WHEN NEW.version=23
                 BEGIN SELECT RAISE(ABORT,'injected failure'); END;",
            )
            .unwrap();
        assert!(apply_migrations(&mut connection).is_err());
        assert_eq!(validate_supported_input(&connection).unwrap(), 22);
        assert_eq!(preservation_receipt(&connection).unwrap(), before);
        validate_schema(&connection, 22).unwrap();

        connection
            .execute_batch("DROP TRIGGER reject_note_folders_migration;")
            .unwrap();
        assert!(apply_migrations(&mut connection).unwrap());
        let changes = connection.total_changes();
        assert!(!apply_migrations(&mut connection).unwrap());
        assert_eq!(connection.total_changes(), changes);
        validate_input(&connection).unwrap();
    }

    #[test]
    fn unregistered_missing_and_substituted_folder_schemas_fail_read_only() {
        let mut connection = schema_twenty_two();
        for unregistered in [
            migration_sql(VERSION),
            "CREATE TABLE note_folders(id TEXT PRIMARY KEY) STRICT;",
        ] {
            let transaction = connection.transaction().unwrap();
            transaction.execute_batch(unregistered).unwrap();
            let changes = transaction.total_changes();
            assert!(validate_supported_input(&transaction).is_err());
            assert_eq!(transaction.total_changes(), changes);
            transaction.rollback().unwrap();
        }

        apply_migrations(&mut connection).unwrap();
        for damage in [
            "DROP INDEX notebook_shelf_by_folder;",
            // Same columns without the cascade is not the registered table.
            "DROP TABLE notebook_shelf;
             CREATE TABLE notebook_shelf(
                 notebook_id TEXT PRIMARY KEY,
                 folder_id TEXT,
                 pinned_at INTEGER,
                 updated_at INTEGER NOT NULL
             ) STRICT;",
        ] {
            let transaction = connection.transaction().unwrap();
            transaction.execute_batch(damage).unwrap();
            let changes = transaction.total_changes();
            assert!(validate_backup_input(&transaction).is_err(), "{damage}");
            assert_eq!(transaction.total_changes(), changes);
            transaction.rollback().unwrap();
        }
        validate_input(&connection).unwrap();
    }
}
