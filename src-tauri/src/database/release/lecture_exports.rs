//! Release-v13 admission for lecture exports.
//!
//! Migration 13 rebuilds `prepared_exports` so a lesson can be prepared as a
//! lecture deck (`lecture_pptx`). Every prepared export is kept, and the
//! upgrade writes no new rows.

use std::sync::OnceLock;

use rusqlite::Connection;

use super::object_delta::{self, Delta};

const VERSION: i64 = 13;

pub(super) fn validate_schema(connection: &Connection, version: i64) -> Result<(), String> {
    static DELTA: OnceLock<Result<Delta, String>> = OnceLock::new();
    object_delta::validate(connection, version, VERSION, "lecture export", &DELTA)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::release::{
        apply_migrations, apply_migrations_through, migration_sql, preservation_receipt,
        subjects_fixture::release_library, validate_backup_input, validate_input,
        validate_supported_input,
    };

    /// A schema-12 library as the previous release left it.
    fn schema_twelve() -> Connection {
        let connection = release_library(12);
        assert_eq!(validate_supported_input(&connection).unwrap(), 12);
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
    fn schema_twelve_upgrades_keeping_every_export_and_accepts_lecture_decks() {
        let mut connection = schema_twelve();
        insert(&connection, 'a', "study_guide_pdf").unwrap();
        insert(&connection, 'b', "studydeck").unwrap();
        assert!(insert(&connection, 'c', "lecture_pptx").is_err());
        let before = preservation_receipt(&connection).unwrap();
        let changes = connection.total_changes();
        validate_backup_input(&connection).unwrap();
        assert_eq!(connection.total_changes(), changes);

        assert!(apply_migrations_through(&mut connection, VERSION).unwrap());
        assert_eq!(validate_supported_input(&connection).unwrap(), VERSION);
        assert_eq!(preservation_receipt(&connection).unwrap(), before);
        insert(&connection, 'c', "lecture_pptx").unwrap();
        assert!(insert(&connection, 'd', "slides").is_err());
        assert_eq!(
            count(
                &connection,
                "SELECT COUNT(*) FROM sqlite_schema
                 WHERE name = 'prepared_exports_v13'
                    OR (type = 'index' AND name = 'prepared_exports_state_expiry_idx')"
            ),
            1
        );
        validate_schema(&connection, VERSION).unwrap();
        apply_migrations(&mut connection).unwrap();
        validate_input(&connection).unwrap();
    }

    #[test]
    fn interrupted_thirteenth_migration_rolls_back_to_schema_twelve_and_retries() {
        let mut connection = schema_twelve();
        insert(&connection, 'a', "library_backup").unwrap();
        let before = preservation_receipt(&connection).unwrap();
        connection
            .execute_batch(
                "CREATE TEMP TRIGGER reject_lecture_migration
                 BEFORE INSERT ON schema_migrations
                 WHEN NEW.version=13
                 BEGIN SELECT RAISE(ABORT,'injected failure'); END;",
            )
            .unwrap();
        assert!(apply_migrations(&mut connection).is_err());
        assert_eq!(validate_supported_input(&connection).unwrap(), 12);
        assert_eq!(preservation_receipt(&connection).unwrap(), before);
        validate_schema(&connection, 12).unwrap();

        connection
            .execute_batch("DROP TRIGGER reject_lecture_migration;")
            .unwrap();
        assert!(apply_migrations(&mut connection).unwrap());
        let changes = connection.total_changes();
        assert!(!apply_migrations(&mut connection).unwrap());
        assert_eq!(connection.total_changes(), changes);
        validate_input(&connection).unwrap();
    }

    #[test]
    fn unregistered_missing_and_substituted_export_schemas_fail_read_only() {
        let mut connection = schema_twelve();
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
            // The old purposes without lecture decks are not the registered table.
            "DROP TABLE prepared_exports;
             CREATE TABLE prepared_exports(
                 id TEXT PRIMARY KEY, operation_id TEXT NOT NULL UNIQUE,
                 purpose TEXT NOT NULL CHECK(purpose IN ('studydeck', 'library_backup', 'study_guide_pdf')),
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
