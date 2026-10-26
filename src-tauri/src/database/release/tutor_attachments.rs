//! Additive release-v25 admission for Tutor message attachments.
//!
//! Migration 25 adds `tutor_turn_attachments`: the pictures and text files a
//! learner attached to a Tutor message. An upgrade writes none.

use std::sync::OnceLock;

use rusqlite::Connection;

use super::object_delta::{self, Delta};

const VERSION: i64 = 25;

pub(super) fn validate_schema(connection: &Connection, version: i64) -> Result<(), String> {
    static DELTA: OnceLock<Result<Delta, String>> = OnceLock::new();
    object_delta::validate(connection, version, VERSION, "Tutor attachments", &DELTA)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::release::{
        apply_migrations, apply_migrations_through, migration_sql, preservation_receipt,
        subjects_fixture::release_library, validate_backup_input, validate_input,
        validate_supported_input,
    };

    /// A schema-24 library as the previous release left it.
    fn schema_twenty_four() -> Connection {
        let connection = release_library(24);
        assert_eq!(validate_supported_input(&connection).unwrap(), 24);
        connection
    }

    fn count(connection: &Connection, sql: &str) -> i64 {
        connection.query_row(sql, [], |row| row.get(0)).unwrap()
    }

    #[test]
    fn schema_twenty_four_upgrades_additively_with_no_attachments() {
        let mut connection = schema_twenty_four();
        let before = preservation_receipt(&connection).unwrap();
        let changes = connection.total_changes();
        validate_backup_input(&connection).unwrap();
        assert_eq!(connection.total_changes(), changes);

        assert!(apply_migrations_through(&mut connection, VERSION).unwrap());
        assert_eq!(validate_supported_input(&connection).unwrap(), VERSION);
        assert_eq!(
            count(&connection, "SELECT COUNT(*) FROM tutor_turn_attachments"),
            0
        );

        let comparison = connection.transaction().unwrap();
        comparison
            .execute_batch("DROP TABLE tutor_turn_attachments;")
            .unwrap();
        assert_eq!(preservation_receipt(&comparison).unwrap(), before);
        comparison.rollback().unwrap();
    }

    #[test]
    fn interrupted_twenty_fifth_migration_rolls_back_to_schema_twenty_four_and_retries() {
        let mut connection = schema_twenty_four();
        let before = preservation_receipt(&connection).unwrap();
        connection
            .execute_batch(
                "CREATE TEMP TRIGGER reject_tutor_attachments_migration
                 BEFORE INSERT ON schema_migrations
                 WHEN NEW.version=25
                 BEGIN SELECT RAISE(ABORT,'injected failure'); END;",
            )
            .unwrap();
        assert!(apply_migrations(&mut connection).is_err());
        assert_eq!(validate_supported_input(&connection).unwrap(), 24);
        assert_eq!(preservation_receipt(&connection).unwrap(), before);
        validate_schema(&connection, 24).unwrap();

        connection
            .execute_batch("DROP TRIGGER reject_tutor_attachments_migration;")
            .unwrap();
        assert!(apply_migrations(&mut connection).unwrap());
        let changes = connection.total_changes();
        assert!(!apply_migrations(&mut connection).unwrap());
        assert_eq!(connection.total_changes(), changes);
        validate_input(&connection).unwrap();
    }

    #[test]
    fn unregistered_missing_and_substituted_attachment_schemas_fail_read_only() {
        let mut connection = schema_twenty_four();
        for unregistered in [
            migration_sql(VERSION),
            "CREATE TABLE tutor_turn_attachments(turn_id TEXT PRIMARY KEY) STRICT;",
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
            "DROP INDEX tutor_turn_attachments_by_asset;",
            // Same columns without the asset reference is not the registered table.
            "DROP TABLE tutor_turn_attachments;
             CREATE TABLE tutor_turn_attachments(
                 turn_id TEXT NOT NULL REFERENCES tutor_turns(id) ON DELETE CASCADE,
                 position INTEGER NOT NULL,
                 kind TEXT NOT NULL,
                 name TEXT NOT NULL,
                 asset_id TEXT,
                 text TEXT,
                 PRIMARY KEY (turn_id, position)
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
