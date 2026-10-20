//! Additive release-v14 admission for document figures.
//!
//! Migration 14 adds, for each PDF document, whether the learner chose to use
//! its images and the figures found in it with the passages that explain them.
//! Figures are found after import; an upgrade writes none, so the tables start
//! empty.

use std::sync::OnceLock;

use rusqlite::Connection;

use super::object_delta::{self, Delta};

const VERSION: i64 = 14;

pub(super) fn validate_schema(connection: &Connection, version: i64) -> Result<(), String> {
    static DELTA: OnceLock<Result<Delta, String>> = OnceLock::new();
    object_delta::validate(connection, version, VERSION, "document figures", &DELTA)
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
    const FIGURE_TABLES: [&str; 3] = [
        "document_figure_scans",
        "document_figures",
        "document_figure_passages",
    ];

    /// A schema-13 library as the previous release left it, with one document.
    fn schema_twelve() -> Connection {
        let connection = release_library(13);
        crate::collections::test_support::seed_document(&connection, DOCUMENT, 2, 'a').unwrap();
        assert_eq!(validate_supported_input(&connection).unwrap(), 13);
        connection
    }

    fn count(connection: &Connection, sql: &str) -> i64 {
        connection.query_row(sql, [], |row| row.get(0)).unwrap()
    }

    #[test]
    fn schema_twelve_upgrades_additively_with_no_figures() {
        let mut connection = schema_twelve();
        let before = preservation_receipt(&connection).unwrap();
        let changes = connection.total_changes();
        validate_backup_input(&connection).unwrap();
        assert_eq!(connection.total_changes(), changes);

        assert!(apply_migrations_through(&mut connection, VERSION).unwrap());
        assert_eq!(validate_supported_input(&connection).unwrap(), VERSION);
        for table in FIGURE_TABLES {
            assert_eq!(
                count(&connection, &format!("SELECT COUNT(*) FROM {table}")),
                0
            );
        }
        assert_eq!(count(&connection, "SELECT COUNT(*) FROM documents"), 1);

        let comparison = connection.transaction().unwrap();
        comparison
            .execute_batch(
                "DROP TABLE document_figure_passages;
                 DROP TABLE document_figures;
                 DROP TABLE document_figure_scans;",
            )
            .unwrap();
        assert_eq!(preservation_receipt(&comparison).unwrap(), before);
        comparison.rollback().unwrap();
    }

    #[test]
    fn interrupted_thirteenth_migration_rolls_back_to_schema_twelve_and_retries() {
        let mut connection = schema_twelve();
        let before = preservation_receipt(&connection).unwrap();
        connection
            .execute_batch(
                "CREATE TEMP TRIGGER reject_figures_migration
                 BEFORE INSERT ON schema_migrations
                 WHEN NEW.version=14
                 BEGIN SELECT RAISE(ABORT,'injected failure'); END;",
            )
            .unwrap();
        assert!(apply_migrations(&mut connection).is_err());
        assert_eq!(validate_supported_input(&connection).unwrap(), 13);
        assert_eq!(preservation_receipt(&connection).unwrap(), before);
        validate_schema(&connection, 13).unwrap();

        connection
            .execute_batch("DROP TRIGGER reject_figures_migration;")
            .unwrap();
        assert!(apply_migrations(&mut connection).unwrap());
        let changes = connection.total_changes();
        assert!(!apply_migrations(&mut connection).unwrap());
        assert_eq!(connection.total_changes(), changes);
        validate_input(&connection).unwrap();
    }

    #[test]
    fn unregistered_missing_and_substituted_figure_schemas_fail_read_only() {
        let mut connection = schema_twelve();
        for unregistered in [
            migration_sql(VERSION),
            "CREATE INDEX document_figures_in_order ON documents(id);",
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
            "DROP TABLE document_figure_passages;",
            "DROP INDEX document_figures_in_order;",
            "DROP INDEX document_figures_in_order;
             CREATE INDEX document_figures_in_order ON document_figures(document_id, ordinal);",
            // The same columns without the learner's import choice is not the registered table.
            "DROP TABLE document_figure_scans;
             CREATE TABLE document_figure_scans (
                 document_id TEXT PRIMARY KEY REFERENCES documents(id) ON DELETE CASCADE,
                 state TEXT NOT NULL, detector TEXT, scanned_at INTEGER,
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
