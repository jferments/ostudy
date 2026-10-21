//! Release-v14 admission for annotated PDF exports.
//!
//! Migration 14 rebuilds `prepared_exports` so a PDF can be prepared with its
//! annotations drawn in (`annotated_pdf`). Every prepared export is kept, and the
//! upgrade writes no new rows.

use std::sync::OnceLock;

use rusqlite::Connection;

use super::object_delta::{self, Delta};

const VERSION: i64 = 14;

pub(super) fn validate_schema(connection: &Connection, version: i64) -> Result<(), String> {
    static DELTA: OnceLock<Result<Delta, String>> = OnceLock::new();
    object_delta::validate(connection, version, VERSION, "annotated PDF export", &DELTA)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::release::{
        apply_migrations, apply_migrations_through, migration_sql, preservation_receipt,
        subjects_fixture::release_library, validate_backup_input, validate_input,
        validate_supported_input,
    };

    /// A schema-13 library as the previous release left it.
    fn schema_thirteen() -> Connection {
        let connection = release_library(13);
        assert_eq!(validate_supported_input(&connection).unwrap(), 13);
        connection
    }

    fn insert(connection: &Connection, id: char, purpose: &str) -> rusqlite::Result<usize> {
        connection.execute(
            "INSERT INTO prepared_exports(
                 id, operation_id, purpose, suggested_filename, media_type, byte_size,
                 sha256, relative_path, state, summary_json, created_at, updated_at,
                 expires_at, published_at, released_at
             ) VALUES (?1, ?1, ?2, 'out', 'application/octet-stream', 10, ?3, ?4,
                       'published', '{}', 1, 2, 3, 2, NULL)",
            rusqlite::params![
                id.to_string().repeat(32),
                purpose,
                "e".repeat(64),
                format!("transfers/exports/{id}")
            ],
        )
    }

    fn count(connection: &Connection, sql: &str) -> i64 {
        connection.query_row(sql, [], |row| row.get(0)).unwrap()
    }

    #[test]
    fn schema_thirteen_upgrades_keeping_every_export_and_accepts_annotated_pdfs() {
        let mut connection = schema_thirteen();
        insert(&connection, 'a', "study_guide_pdf").unwrap();
        insert(&connection, 'b', "studydeck").unwrap();
        insert(&connection, 'e', "lecture_pptx").unwrap();
        assert!(insert(&connection, 'c', "annotated_pdf").is_err());
        let before = preservation_receipt(&connection).unwrap();
        let changes = connection.total_changes();
        validate_backup_input(&connection).unwrap();
        assert_eq!(connection.total_changes(), changes);

        assert!(apply_migrations_through(&mut connection, VERSION).unwrap());
        assert_eq!(validate_supported_input(&connection).unwrap(), VERSION);
        assert_eq!(preservation_receipt(&connection).unwrap(), before);
        insert(&connection, 'c', "annotated_pdf").unwrap();
        assert!(insert(&connection, 'd', "slides").is_err());
        assert_eq!(
            count(
                &connection,
                "SELECT COUNT(*) FROM sqlite_schema
                 WHERE name = 'prepared_exports_v14'
                    OR (type = 'index' AND name = 'prepared_exports_state_expiry_idx')"
            ),
            1
        );
        // Later migrations keep the annotated PDFs.
        apply_migrations(&mut connection).unwrap();
        assert_eq!(
            count(&connection, "SELECT COUNT(*) FROM prepared_exports"),
            4
        );
        validate_input(&connection).unwrap();
    }

    #[test]
    fn interrupted_fourteenth_migration_rolls_back_to_schema_thirteen_and_retries() {
        let mut connection = schema_thirteen();
        insert(&connection, 'a', "library_backup").unwrap();
        let before = preservation_receipt(&connection).unwrap();
        connection
            .execute_batch(
                "CREATE TEMP TRIGGER reject_annotated_pdf_migration
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
            .execute_batch("DROP TRIGGER reject_annotated_pdf_migration;")
            .unwrap();
        assert!(apply_migrations(&mut connection).unwrap());
        let changes = connection.total_changes();
        assert!(!apply_migrations(&mut connection).unwrap());
        assert_eq!(connection.total_changes(), changes);
        validate_input(&connection).unwrap();
    }

    #[test]
    fn unregistered_missing_and_substituted_export_schemas_fail_read_only() {
        let mut connection = schema_thirteen();
        {
            let transaction = connection.transaction().unwrap();
            transaction.execute_batch(migration_sql(VERSION)).unwrap();
            let changes = transaction.total_changes();
            assert!(validate_supported_input(&transaction).is_err());
            assert_eq!(transaction.total_changes(), changes);
            transaction.rollback().unwrap();
        }

        apply_migrations(&mut connection).unwrap();
        for damage in [
            "DROP TABLE prepared_exports;",
            // The old purposes without annotated PDFs are not the registered table.
            "DROP TABLE prepared_exports;
             CREATE TABLE prepared_exports(
                 id TEXT PRIMARY KEY, operation_id TEXT NOT NULL UNIQUE,
                 purpose TEXT NOT NULL CHECK(purpose IN ('studydeck', 'library_backup', 'study_guide_pdf', 'lecture_pptx')),
                 suggested_filename TEXT NOT NULL, media_type TEXT NOT NULL,
                 byte_size INTEGER NOT NULL, sha256 TEXT NOT NULL,
                 relative_path TEXT NOT NULL UNIQUE, state TEXT NOT NULL,
                 summary_json TEXT NOT NULL, created_at INTEGER NOT NULL,
                 updated_at INTEGER NOT NULL, expires_at INTEGER NOT NULL,
                 published_at INTEGER, released_at INTEGER
             ) STRICT;
             CREATE INDEX prepared_exports_state_expiry_idx
                 ON prepared_exports(state, expires_at);",
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
