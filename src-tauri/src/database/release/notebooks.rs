//! Additive release-v15 admission for notebooks.
//!
//! Migration 15 adds notebooks, their pages and the marks on those pages, with
//! their indexes. Learners make notebooks in Library's Notes page; an upgrade
//! writes none, so the tables start empty.

use std::sync::OnceLock;

use rusqlite::Connection;

use super::object_delta::{self, Delta};

const VERSION: i64 = 15;

pub(super) fn validate_schema(connection: &Connection, version: i64) -> Result<(), String> {
    static DELTA: OnceLock<Result<Delta, String>> = OnceLock::new();
    object_delta::validate(connection, version, VERSION, "notebook", &DELTA)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::release::{
        apply_migrations, apply_migrations_through, migration_sql, preservation_receipt,
        subjects_fixture::release_library, validate_backup_input, validate_input,
        validate_supported_input,
    };

    const DOCUMENT: &str = "11111111111111111111111111111111";
    const NOTEBOOK_TABLES: [&str; 3] = ["notebooks", "notebook_pages", "notebook_items"];

    /// A schema-14 library as the previous release left it, with one document.
    fn schema_fourteen() -> Connection {
        let connection = release_library(14);
        crate::collections::test_support::seed_document(&connection, DOCUMENT, 2, 'a').unwrap();
        assert_eq!(validate_supported_input(&connection).unwrap(), 14);
        connection
    }

    fn count(connection: &Connection, sql: &str) -> i64 {
        connection.query_row(sql, [], |row| row.get(0)).unwrap()
    }

    #[test]
    fn schema_fourteen_upgrades_additively_with_no_notebooks() {
        let mut connection = schema_fourteen();
        let before = preservation_receipt(&connection).unwrap();
        let changes = connection.total_changes();
        validate_backup_input(&connection).unwrap();
        assert_eq!(connection.total_changes(), changes);

        assert!(apply_migrations_through(&mut connection, VERSION).unwrap());
        assert_eq!(validate_supported_input(&connection).unwrap(), VERSION);
        for table in NOTEBOOK_TABLES {
            assert_eq!(
                count(&connection, &format!("SELECT COUNT(*) FROM {table}")),
                0
            );
        }
        assert_eq!(count(&connection, "SELECT COUNT(*) FROM documents"), 1);

        let comparison = connection.transaction().unwrap();
        comparison
            .execute_batch(
                "DROP TABLE notebook_items;
                 DROP TABLE notebook_pages;
                 DROP TABLE notebooks;",
            )
            .unwrap();
        assert_eq!(preservation_receipt(&comparison).unwrap(), before);
        comparison.rollback().unwrap();
    }

    #[test]
    fn interrupted_fifteenth_migration_rolls_back_to_schema_fourteen_and_retries() {
        let mut connection = schema_fourteen();
        let before = preservation_receipt(&connection).unwrap();
        connection
            .execute_batch(
                "CREATE TEMP TRIGGER reject_notebook_migration
                 BEFORE INSERT ON schema_migrations
                 WHEN NEW.version=15
                 BEGIN SELECT RAISE(ABORT,'injected failure'); END;",
            )
            .unwrap();
        assert!(apply_migrations(&mut connection).is_err());
        assert_eq!(validate_supported_input(&connection).unwrap(), 14);
        assert_eq!(preservation_receipt(&connection).unwrap(), before);
        validate_schema(&connection, 14).unwrap();

        connection
            .execute_batch("DROP TRIGGER reject_notebook_migration;")
            .unwrap();
        assert!(apply_migrations(&mut connection).unwrap());
        let changes = connection.total_changes();
        assert!(!apply_migrations(&mut connection).unwrap());
        assert_eq!(connection.total_changes(), changes);
        validate_input(&connection).unwrap();
    }

    #[test]
    fn unregistered_missing_and_substituted_notebook_schemas_fail_read_only() {
        let mut connection = schema_fourteen();
        for unregistered in [
            migration_sql(VERSION),
            "CREATE INDEX notebook_items_by_page ON documents(id);",
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
            "DROP TABLE notebook_items;",
            "DROP INDEX notebooks_by_subject;",
            "DROP INDEX notebook_items_by_page;
             CREATE INDEX notebook_items_by_page ON notebook_items(page_id);",
            // Same columns without the checks is not the registered table.
            "DROP TABLE notebook_items;
             CREATE TABLE notebook_items(
                 id TEXT PRIMARY KEY,
                 page_id TEXT NOT NULL REFERENCES notebook_pages(id) ON DELETE CASCADE,
                 kind TEXT NOT NULL, payload_json TEXT NOT NULL,
                 revision INTEGER NOT NULL, created_at INTEGER NOT NULL,
                 updated_at INTEGER NOT NULL
             ) STRICT;
             CREATE INDEX notebook_items_by_page
                 ON notebook_items(page_id, created_at, id);",
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
