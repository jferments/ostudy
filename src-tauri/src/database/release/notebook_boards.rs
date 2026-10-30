//! Additive release-v26 admission for notebook boards.
//!
//! Migration 26 adds `notebook_boards`: which notebook pages are boards, the
//! large sheets learners pan and zoom around. An upgrade writes none, so
//! every existing page stays a page.

use std::sync::OnceLock;

use rusqlite::Connection;

use super::object_delta::{self, Delta};

const VERSION: i64 = 26;

pub(super) fn validate_schema(connection: &Connection, version: i64) -> Result<(), String> {
    static DELTA: OnceLock<Result<Delta, String>> = OnceLock::new();
    object_delta::validate(connection, version, VERSION, "notebook boards", &DELTA)
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
    const PAGE: &str = "22222222222222222222222222222222";

    /// A schema-25 library as the previous release left it, with one notebook page.
    fn schema_twenty_five() -> Connection {
        let connection = release_library(25);
        connection
            .execute_batch(&format!(
                "INSERT INTO notebooks(id,subject_id,title,paper,page_size,revision,created_at,updated_at)
                 VALUES('{NOTEBOOK}',NULL,'Cardiac cycle','dots','letter',1,1,1);
                 INSERT INTO notebook_pages(id,notebook_id,position,paper,width,height,created_at)
                 VALUES('{PAGE}','{NOTEBOOK}',1,'dots',612,792,1);"
            ))
            .unwrap();
        assert_eq!(validate_supported_input(&connection).unwrap(), 25);
        connection
    }

    fn count(connection: &Connection, sql: &str) -> i64 {
        connection.query_row(sql, [], |row| row.get(0)).unwrap()
    }

    #[test]
    fn schema_twenty_five_upgrades_additively_and_pages_stay_pages() {
        let mut connection = schema_twenty_five();
        let before = preservation_receipt(&connection).unwrap();
        let changes = connection.total_changes();
        validate_backup_input(&connection).unwrap();
        assert_eq!(connection.total_changes(), changes);

        assert!(apply_migrations_through(&mut connection, VERSION).unwrap());
        assert_eq!(validate_supported_input(&connection).unwrap(), VERSION);
        assert_eq!(
            count(&connection, "SELECT COUNT(*) FROM notebook_boards"),
            0
        );
        assert_eq!(count(&connection, "SELECT COUNT(*) FROM notebook_pages"), 1);

        let comparison = connection.transaction().unwrap();
        comparison
            .execute_batch("DROP TABLE notebook_boards;")
            .unwrap();
        assert_eq!(preservation_receipt(&comparison).unwrap(), before);
        comparison.rollback().unwrap();
    }

    #[test]
    fn a_board_goes_with_its_page() {
        let mut connection = schema_twenty_five();
        apply_migrations(&mut connection).unwrap();
        connection
            .execute_batch(&format!(
                "PRAGMA foreign_keys = ON;
                 INSERT INTO notebook_boards(page_id, created_at) VALUES('{PAGE}', 2);"
            ))
            .unwrap();
        assert!(connection
            .execute(
                "INSERT INTO notebook_boards(page_id, created_at) VALUES('33333333333333333333333333333333', 2)",
                [],
            )
            .is_err());
        connection
            .execute("DELETE FROM notebooks WHERE id = ?1", [NOTEBOOK])
            .unwrap();
        assert_eq!(
            count(&connection, "SELECT COUNT(*) FROM notebook_boards"),
            0
        );
    }

    #[test]
    fn interrupted_twenty_sixth_migration_rolls_back_to_schema_twenty_five_and_retries() {
        let mut connection = schema_twenty_five();
        let before = preservation_receipt(&connection).unwrap();
        connection
            .execute_batch(
                "CREATE TEMP TRIGGER reject_notebook_boards_migration
                 BEFORE INSERT ON schema_migrations
                 WHEN NEW.version=26
                 BEGIN SELECT RAISE(ABORT,'injected failure'); END;",
            )
            .unwrap();
        assert!(apply_migrations(&mut connection).is_err());
        assert_eq!(validate_supported_input(&connection).unwrap(), 25);
        assert_eq!(preservation_receipt(&connection).unwrap(), before);
        validate_schema(&connection, 25).unwrap();

        connection
            .execute_batch("DROP TRIGGER reject_notebook_boards_migration;")
            .unwrap();
        assert!(apply_migrations(&mut connection).unwrap());
        let changes = connection.total_changes();
        assert!(!apply_migrations(&mut connection).unwrap());
        assert_eq!(connection.total_changes(), changes);
        validate_input(&connection).unwrap();
    }

    #[test]
    fn unregistered_missing_and_substituted_board_schemas_fail_read_only() {
        let mut connection = schema_twenty_five();
        for unregistered in [
            migration_sql(VERSION),
            "CREATE TABLE notebook_boards(page_id TEXT PRIMARY KEY) STRICT;",
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
            "DROP TABLE notebook_boards;",
            // Same columns without the page reference is not the registered table.
            "DROP TABLE notebook_boards;
             CREATE TABLE notebook_boards(
                 page_id TEXT PRIMARY KEY,
                 created_at INTEGER NOT NULL CHECK(created_at >= 0)
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
