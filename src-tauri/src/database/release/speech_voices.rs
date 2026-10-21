//! Additive release-v16 admission for saved voices.
//!
//! Migration 16 adds the voices a learner tried and saved, and the voice each
//! lesson is spoken in. Both are made in the app; an upgrade writes none, so
//! the tables start empty.

use std::sync::OnceLock;

use rusqlite::Connection;

use super::object_delta::{self, Delta};

const VERSION: i64 = 16;

pub(super) fn validate_schema(connection: &Connection, version: i64) -> Result<(), String> {
    static DELTA: OnceLock<Result<Delta, String>> = OnceLock::new();
    object_delta::validate(connection, version, VERSION, "saved voice", &DELTA)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::release::{
        apply_migrations, apply_migrations_through, migration_sql, preservation_receipt,
        subjects_fixture::release_library, validate_backup_input, validate_input,
        validate_supported_input,
    };

    /// A schema-15 library as the previous release left it.
    fn schema_fifteen() -> Connection {
        let connection = release_library(15);
        assert_eq!(validate_supported_input(&connection).unwrap(), 15);
        connection
    }

    #[test]
    fn schema_fifteen_upgrades_additively_with_no_voices() {
        let mut connection = schema_fifteen();
        let before = preservation_receipt(&connection).unwrap();
        let changes = connection.total_changes();
        validate_backup_input(&connection).unwrap();
        assert_eq!(connection.total_changes(), changes);

        assert!(apply_migrations_through(&mut connection, VERSION).unwrap());
        assert_eq!(validate_supported_input(&connection).unwrap(), VERSION);
        let rows: i64 = connection
            .query_row(
                "SELECT (SELECT COUNT(*) FROM speech_voices) + (SELECT COUNT(*) FROM lesson_voices)",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(rows, 0);

        let comparison = connection.transaction().unwrap();
        comparison
            .execute_batch("DROP TABLE lesson_voices; DROP TABLE speech_voices;")
            .unwrap();
        assert_eq!(preservation_receipt(&comparison).unwrap(), before);
        comparison.rollback().unwrap();
    }

    #[test]
    fn interrupted_sixteenth_migration_rolls_back_to_schema_fifteen_and_retries() {
        let mut connection = schema_fifteen();
        let before = preservation_receipt(&connection).unwrap();
        connection
            .execute_batch(
                "CREATE TEMP TRIGGER reject_voice_migration
                 BEFORE INSERT ON schema_migrations
                 WHEN NEW.version=16
                 BEGIN SELECT RAISE(ABORT,'injected failure'); END;",
            )
            .unwrap();
        assert!(apply_migrations(&mut connection).is_err());
        assert_eq!(validate_supported_input(&connection).unwrap(), 15);
        assert_eq!(preservation_receipt(&connection).unwrap(), before);
        validate_schema(&connection, 15).unwrap();

        connection
            .execute_batch("DROP TRIGGER reject_voice_migration;")
            .unwrap();
        assert!(apply_migrations(&mut connection).unwrap());
        let changes = connection.total_changes();
        assert!(!apply_migrations(&mut connection).unwrap());
        assert_eq!(connection.total_changes(), changes);
        validate_input(&connection).unwrap();
    }

    #[test]
    fn unregistered_missing_and_substituted_voice_schemas_fail_read_only() {
        let mut connection = schema_fifteen();
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
            "DROP TABLE lesson_voices;",
            // Same columns without the checks is not the registered table.
            "DROP TABLE speech_voices;
             CREATE TABLE speech_voices(
                 id TEXT PRIMARY KEY, model_id TEXT NOT NULL, name TEXT NOT NULL,
                 state TEXT NOT NULL, sample_text TEXT NOT NULL,
                 asset_id TEXT NOT NULL UNIQUE REFERENCES assets(id),
                 created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL
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
