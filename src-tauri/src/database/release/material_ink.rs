//! Additive release-v19 admission for ink on study materials.
//!
//! Migration 19 adds the marks a learner writes on a lesson's lecture slides
//! and on a document study guide's designed pages, with their indexes. An
//! upgrade writes none, so the table starts empty.

use std::sync::OnceLock;

use rusqlite::Connection;

use super::object_delta::{self, Delta};

const VERSION: i64 = 19;

pub(super) fn validate_schema(connection: &Connection, version: i64) -> Result<(), String> {
    static DELTA: OnceLock<Result<Delta, String>> = OnceLock::new();
    object_delta::validate(connection, version, VERSION, "material ink", &DELTA)
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

    /// A schema-18 library as the previous release left it, with one document.
    fn schema_eighteen() -> Connection {
        let connection = release_library(18);
        crate::collections::test_support::seed_document(&connection, DOCUMENT, 2, 'a').unwrap();
        assert_eq!(validate_supported_input(&connection).unwrap(), 18);
        connection
    }

    fn count(connection: &Connection, sql: &str) -> i64 {
        connection.query_row(sql, [], |row| row.get(0)).unwrap()
    }

    #[test]
    fn schema_eighteen_upgrades_additively_with_no_ink() {
        let mut connection = schema_eighteen();
        let before = preservation_receipt(&connection).unwrap();
        let changes = connection.total_changes();
        validate_backup_input(&connection).unwrap();
        assert_eq!(connection.total_changes(), changes);

        assert!(apply_migrations_through(&mut connection, VERSION).unwrap());
        assert_eq!(validate_supported_input(&connection).unwrap(), VERSION);
        assert_eq!(count(&connection, "SELECT COUNT(*) FROM material_ink"), 0);
        assert_eq!(count(&connection, "SELECT COUNT(*) FROM documents"), 1);

        let comparison = connection.transaction().unwrap();
        comparison
            .execute_batch("DROP TABLE material_ink;")
            .unwrap();
        assert_eq!(preservation_receipt(&comparison).unwrap(), before);
        comparison.rollback().unwrap();
    }

    #[test]
    fn interrupted_nineteenth_migration_rolls_back_to_schema_eighteen_and_retries() {
        let mut connection = schema_eighteen();
        let before = preservation_receipt(&connection).unwrap();
        connection
            .execute_batch(
                "CREATE TEMP TRIGGER reject_ink_migration
                 BEFORE INSERT ON schema_migrations
                 WHEN NEW.version=19
                 BEGIN SELECT RAISE(ABORT,'injected failure'); END;",
            )
            .unwrap();
        assert!(apply_migrations(&mut connection).is_err());
        assert_eq!(validate_supported_input(&connection).unwrap(), 18);
        assert_eq!(preservation_receipt(&connection).unwrap(), before);
        validate_schema(&connection, 18).unwrap();

        connection
            .execute_batch("DROP TRIGGER reject_ink_migration;")
            .unwrap();
        assert!(apply_migrations(&mut connection).unwrap());
        let changes = connection.total_changes();
        assert!(!apply_migrations(&mut connection).unwrap());
        assert_eq!(connection.total_changes(), changes);
        validate_input(&connection).unwrap();
    }

    #[test]
    fn unregistered_missing_and_substituted_ink_schemas_fail_read_only() {
        let mut connection = schema_eighteen();
        for unregistered in [
            migration_sql(VERSION),
            "CREATE INDEX material_ink_by_lesson ON documents(id);",
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
            "DROP TABLE material_ink;",
            "DROP INDEX material_ink_by_document;",
            // Same columns without the checks is not the registered table.
            "DROP TABLE material_ink;
             CREATE TABLE material_ink(
                 id TEXT PRIMARY KEY,
                 lesson_id TEXT REFERENCES lessons(id) ON DELETE CASCADE,
                 document_id TEXT REFERENCES documents(id) ON DELETE CASCADE,
                 page_key TEXT NOT NULL, kind TEXT NOT NULL, payload_json TEXT NOT NULL,
                 revision INTEGER NOT NULL, created_at INTEGER NOT NULL,
                 updated_at INTEGER NOT NULL
             ) STRICT;
             CREATE INDEX material_ink_by_lesson ON material_ink(lesson_id, created_at, id)
                 WHERE lesson_id IS NOT NULL;
             CREATE INDEX material_ink_by_document ON material_ink(document_id, created_at, id)
                 WHERE document_id IS NOT NULL;",
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
