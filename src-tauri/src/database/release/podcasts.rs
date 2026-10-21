//! Additive release-v17 admission for podcasts.
//!
//! Migration 17 adds podcasts: two hosts talking through some of a subject's
//! lessons. They are made in the app; an upgrade writes none, so the table
//! starts empty.

use std::sync::OnceLock;

use rusqlite::Connection;

use super::object_delta::{self, Delta};

const VERSION: i64 = 17;

pub(super) fn validate_schema(connection: &Connection, version: i64) -> Result<(), String> {
    static DELTA: OnceLock<Result<Delta, String>> = OnceLock::new();
    object_delta::validate(connection, version, VERSION, "podcast", &DELTA)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::release::{
        apply_migrations, apply_migrations_through, migration_sql, preservation_receipt,
        subjects_fixture::release_library, validate_backup_input, validate_input,
        validate_supported_input,
    };

    /// A schema-16 library as the previous release left it.
    fn schema_sixteen() -> Connection {
        let connection = release_library(16);
        assert_eq!(validate_supported_input(&connection).unwrap(), 16);
        connection
    }

    #[test]
    fn schema_sixteen_upgrades_additively_with_no_podcasts() {
        let mut connection = schema_sixteen();
        let before = preservation_receipt(&connection).unwrap();
        let changes = connection.total_changes();
        validate_backup_input(&connection).unwrap();
        assert_eq!(connection.total_changes(), changes);

        assert!(apply_migrations_through(&mut connection, VERSION).unwrap());
        assert_eq!(validate_supported_input(&connection).unwrap(), VERSION);
        let rows: i64 = connection
            .query_row("SELECT COUNT(*) FROM podcasts", [], |row| row.get(0))
            .unwrap();
        assert_eq!(rows, 0);

        let comparison = connection.transaction().unwrap();
        comparison.execute_batch("DROP TABLE podcasts;").unwrap();
        assert_eq!(preservation_receipt(&comparison).unwrap(), before);
        comparison.rollback().unwrap();
    }

    #[test]
    fn interrupted_seventeenth_migration_rolls_back_to_schema_sixteen_and_retries() {
        let mut connection = schema_sixteen();
        let before = preservation_receipt(&connection).unwrap();
        connection
            .execute_batch(
                "CREATE TEMP TRIGGER reject_podcast_migration
                 BEFORE INSERT ON schema_migrations
                 WHEN NEW.version=17
                 BEGIN SELECT RAISE(ABORT,'injected failure'); END;",
            )
            .unwrap();
        assert!(apply_migrations(&mut connection).is_err());
        assert_eq!(validate_supported_input(&connection).unwrap(), 16);
        assert_eq!(preservation_receipt(&connection).unwrap(), before);
        validate_schema(&connection, 16).unwrap();

        connection
            .execute_batch("DROP TRIGGER reject_podcast_migration;")
            .unwrap();
        assert!(apply_migrations(&mut connection).unwrap());
        let changes = connection.total_changes();
        assert!(!apply_migrations(&mut connection).unwrap());
        assert_eq!(connection.total_changes(), changes);
        validate_input(&connection).unwrap();
    }

    #[test]
    fn unregistered_missing_and_substituted_podcast_schemas_fail_read_only() {
        let mut connection = schema_sixteen();
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
            "DROP TABLE podcasts;",
            "DROP INDEX podcasts_by_subject;",
            // Same columns without the checks is not the registered table.
            "DROP TABLE podcasts;
             CREATE TABLE podcasts(
                 id TEXT PRIMARY KEY,
                 subject_id TEXT NOT NULL REFERENCES document_collections(id) ON DELETE CASCADE,
                 title TEXT NOT NULL, target_minutes INTEGER NOT NULL,
                 model_id TEXT NOT NULL, first_voice TEXT NOT NULL,
                 second_voice TEXT NOT NULL, script_json TEXT NOT NULL,
                 provenance_json TEXT NOT NULL, created_at INTEGER NOT NULL
             ) STRICT;
             CREATE INDEX podcasts_by_subject ON podcasts(subject_id, created_at);",
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
