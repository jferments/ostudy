//! Additive release-v10 admission for the document marks cards came from.
//!
//! Migration 10 adds note_annotation_sources and its two indexes. Links are
//! made when a learner makes a card from a mark; an upgrade writes none, so
//! the table starts empty.

use std::sync::OnceLock;

use rusqlite::Connection;

use super::object_delta::{self, Delta};

const VERSION: i64 = 10;

pub(super) fn validate_schema(connection: &Connection, version: i64) -> Result<(), String> {
    static DELTA: OnceLock<Result<Delta, String>> = OnceLock::new();
    object_delta::validate(connection, version, VERSION, "card mark link", &DELTA)
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

    /// A schema-9 library as the previous release left it, with one document.
    fn schema_nine() -> Connection {
        let connection = release_library(9);
        crate::collections::test_support::seed_document(&connection, DOCUMENT, 2, 'a').unwrap();
        assert_eq!(validate_supported_input(&connection).unwrap(), 9);
        connection
    }

    fn count(connection: &Connection, sql: &str) -> i64 {
        connection.query_row(sql, [], |row| row.get(0)).unwrap()
    }

    #[test]
    fn schema_nine_upgrades_additively_with_no_links() {
        let mut connection = schema_nine();
        let before = preservation_receipt(&connection).unwrap();
        let changes = connection.total_changes();
        validate_backup_input(&connection).unwrap();
        assert_eq!(connection.total_changes(), changes);

        assert!(apply_migrations_through(&mut connection, VERSION).unwrap());
        assert_eq!(validate_supported_input(&connection).unwrap(), VERSION);
        assert_eq!(
            count(&connection, "SELECT COUNT(*) FROM note_annotation_sources"),
            0
        );
        assert_eq!(count(&connection, "SELECT COUNT(*) FROM documents"), 1);

        let comparison = connection.transaction().unwrap();
        comparison
            .execute_batch("DROP TABLE note_annotation_sources;")
            .unwrap();
        assert_eq!(preservation_receipt(&comparison).unwrap(), before);
        comparison.rollback().unwrap();
    }

    #[test]
    fn interrupted_tenth_migration_rolls_back_to_schema_nine_and_retries() {
        let mut connection = schema_nine();
        let before = preservation_receipt(&connection).unwrap();
        connection
            .execute_batch(
                "CREATE TEMP TRIGGER reject_mark_link_migration
                 BEFORE INSERT ON schema_migrations
                 WHEN NEW.version=10
                 BEGIN SELECT RAISE(ABORT,'injected failure'); END;",
            )
            .unwrap();
        assert!(apply_migrations(&mut connection).is_err());
        assert_eq!(validate_supported_input(&connection).unwrap(), 9);
        assert_eq!(preservation_receipt(&connection).unwrap(), before);
        validate_schema(&connection, 9).unwrap();

        connection
            .execute_batch("DROP TRIGGER reject_mark_link_migration;")
            .unwrap();
        assert!(apply_migrations(&mut connection).unwrap());
        let changes = connection.total_changes();
        assert!(!apply_migrations(&mut connection).unwrap());
        assert_eq!(connection.total_changes(), changes);
        validate_input(&connection).unwrap();
    }

    #[test]
    fn unregistered_missing_and_substituted_link_schemas_fail_read_only() {
        let mut connection = schema_nine();
        for unregistered in [
            migration_sql(VERSION),
            "CREATE INDEX note_annotation_sources_by_document ON documents(id);",
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
            "DROP TABLE note_annotation_sources;",
            "DROP INDEX note_annotation_sources_by_annotation;",
            "DROP INDEX note_annotation_sources_by_document;
             CREATE INDEX note_annotation_sources_by_document
                 ON note_annotation_sources(document_id);",
            // Same columns with a cascading document link is not the registered table.
            "DROP TABLE note_annotation_sources;
             CREATE TABLE note_annotation_sources(
                 note_id TEXT PRIMARY KEY REFERENCES notes(id) ON DELETE CASCADE,
                 document_id TEXT REFERENCES documents(id) ON DELETE CASCADE,
                 annotation_id TEXT REFERENCES document_annotations(id) ON DELETE SET NULL,
                 page_number INTEGER NOT NULL, quote TEXT NOT NULL,
                 created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL
             ) STRICT;
             CREATE INDEX note_annotation_sources_by_document
                 ON note_annotation_sources(document_id, page_number);
             CREATE INDEX note_annotation_sources_by_annotation
                 ON note_annotation_sources(annotation_id);",
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
