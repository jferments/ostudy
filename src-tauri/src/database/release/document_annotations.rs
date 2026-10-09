//! Additive release-v8 admission for document annotations.
//!
//! Migration 8 adds the document_annotations table and its page index.
//! Learners create annotations in the annotator; an upgrade writes none, so
//! the table starts empty.

use std::sync::OnceLock;

use rusqlite::Connection;

use super::object_delta::{self, Delta};

const VERSION: i64 = 8;

pub(super) fn validate_schema(connection: &Connection, version: i64) -> Result<(), String> {
    static DELTA: OnceLock<Result<Delta, String>> = OnceLock::new();
    object_delta::validate(connection, version, VERSION, "document annotation", &DELTA)
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

    /// A schema-7 library as the previous release left it, with one document.
    fn schema_seven() -> Connection {
        let connection = release_library(7);
        crate::collections::test_support::seed_document(&connection, DOCUMENT, 2, 'a').unwrap();
        assert_eq!(validate_supported_input(&connection).unwrap(), 7);
        connection
    }

    fn count(connection: &Connection, sql: &str) -> i64 {
        connection.query_row(sql, [], |row| row.get(0)).unwrap()
    }

    #[test]
    fn schema_seven_upgrades_additively_with_no_annotations() {
        let mut connection = schema_seven();
        let before = preservation_receipt(&connection).unwrap();
        let changes = connection.total_changes();
        validate_backup_input(&connection).unwrap();
        assert_eq!(connection.total_changes(), changes);

        assert!(apply_migrations_through(&mut connection, VERSION).unwrap());
        assert_eq!(validate_supported_input(&connection).unwrap(), VERSION);
        assert_eq!(
            count(&connection, "SELECT COUNT(*) FROM document_annotations"),
            0
        );
        assert_eq!(count(&connection, "SELECT COUNT(*) FROM documents"), 1);

        let comparison = connection.transaction().unwrap();
        comparison
            .execute_batch("DROP TABLE document_annotations;")
            .unwrap();
        assert_eq!(preservation_receipt(&comparison).unwrap(), before);
        comparison.rollback().unwrap();
    }

    #[test]
    fn interrupted_eighth_migration_rolls_back_to_schema_seven_and_retries() {
        let mut connection = schema_seven();
        let before = preservation_receipt(&connection).unwrap();
        connection
            .execute_batch(
                "CREATE TEMP TRIGGER reject_annotation_migration
                 BEFORE INSERT ON schema_migrations
                 WHEN NEW.version=8
                 BEGIN SELECT RAISE(ABORT,'injected failure'); END;",
            )
            .unwrap();
        assert!(apply_migrations(&mut connection).is_err());
        assert_eq!(validate_supported_input(&connection).unwrap(), 7);
        assert_eq!(preservation_receipt(&connection).unwrap(), before);
        validate_schema(&connection, 7).unwrap();

        connection
            .execute_batch("DROP TRIGGER reject_annotation_migration;")
            .unwrap();
        assert!(apply_migrations(&mut connection).unwrap());
        let changes = connection.total_changes();
        assert!(!apply_migrations(&mut connection).unwrap());
        assert_eq!(connection.total_changes(), changes);
        validate_input(&connection).unwrap();
    }

    #[test]
    fn unregistered_missing_and_substituted_annotation_schemas_fail_read_only() {
        let mut connection = schema_seven();
        for unregistered in [
            migration_sql(VERSION),
            "CREATE INDEX document_annotations_by_page ON documents(id);",
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
            "DROP TABLE document_annotations;",
            "DROP INDEX document_annotations_by_page;",
            "DROP INDEX document_annotations_by_page;
             CREATE INDEX document_annotations_by_page
                 ON document_annotations(document_id);",
            // Same columns without the checks is not the registered table.
            "DROP TABLE document_annotations;
             CREATE TABLE document_annotations(
                 id TEXT PRIMARY KEY,
                 document_id TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
                 page_number INTEGER NOT NULL, kind TEXT NOT NULL,
                 payload_json TEXT NOT NULL, revision INTEGER NOT NULL,
                 created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL
             ) STRICT;
             CREATE INDEX document_annotations_by_page
                 ON document_annotations(document_id, page_number, created_at, id);",
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
