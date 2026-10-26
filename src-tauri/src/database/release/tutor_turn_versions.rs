//! Additive release-v24 admission for replaced Tutor answers.
//!
//! Migration 24 adds `tutor_turn_replacements`: which turn a newer version
//! (Regenerate, or a question edited and sent again) replaced. An upgrade
//! writes none, so every saved turn stays current.

use std::sync::OnceLock;

use rusqlite::Connection;

use super::object_delta::{self, Delta};

const VERSION: i64 = 24;

pub(super) fn validate_schema(connection: &Connection, version: i64) -> Result<(), String> {
    static DELTA: OnceLock<Result<Delta, String>> = OnceLock::new();
    object_delta::validate(
        connection,
        version,
        VERSION,
        "Tutor answer versions",
        &DELTA,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::release::{
        apply_migrations, apply_migrations_through, migration_sql, preservation_receipt,
        subjects_fixture::release_library, validate_backup_input, validate_input,
        validate_supported_input,
    };

    /// A schema-23 library as the previous release left it.
    fn schema_twenty_three() -> Connection {
        let connection = release_library(23);
        assert_eq!(validate_supported_input(&connection).unwrap(), 23);
        connection
    }

    fn count(connection: &Connection, sql: &str) -> i64 {
        connection.query_row(sql, [], |row| row.get(0)).unwrap()
    }

    #[test]
    fn schema_twenty_three_upgrades_additively_with_every_turn_current() {
        let mut connection = schema_twenty_three();
        let before = preservation_receipt(&connection).unwrap();
        let changes = connection.total_changes();
        validate_backup_input(&connection).unwrap();
        assert_eq!(connection.total_changes(), changes);

        assert!(apply_migrations_through(&mut connection, VERSION).unwrap());
        assert_eq!(validate_supported_input(&connection).unwrap(), VERSION);
        assert_eq!(
            count(&connection, "SELECT COUNT(*) FROM tutor_turn_replacements"),
            0
        );

        let comparison = connection.transaction().unwrap();
        comparison
            .execute_batch("DROP TABLE tutor_turn_replacements;")
            .unwrap();
        assert_eq!(preservation_receipt(&comparison).unwrap(), before);
        comparison.rollback().unwrap();
    }

    #[test]
    fn interrupted_twenty_fourth_migration_rolls_back_to_schema_twenty_three_and_retries() {
        let mut connection = schema_twenty_three();
        let before = preservation_receipt(&connection).unwrap();
        connection
            .execute_batch(
                "CREATE TEMP TRIGGER reject_tutor_versions_migration
                 BEFORE INSERT ON schema_migrations
                 WHEN NEW.version=24
                 BEGIN SELECT RAISE(ABORT,'injected failure'); END;",
            )
            .unwrap();
        assert!(apply_migrations(&mut connection).is_err());
        assert_eq!(validate_supported_input(&connection).unwrap(), 23);
        assert_eq!(preservation_receipt(&connection).unwrap(), before);
        validate_schema(&connection, 23).unwrap();

        connection
            .execute_batch("DROP TRIGGER reject_tutor_versions_migration;")
            .unwrap();
        assert!(apply_migrations(&mut connection).unwrap());
        let changes = connection.total_changes();
        assert!(!apply_migrations(&mut connection).unwrap());
        assert_eq!(connection.total_changes(), changes);
        validate_input(&connection).unwrap();
    }

    #[test]
    fn unregistered_missing_and_substituted_version_schemas_fail_read_only() {
        let mut connection = schema_twenty_three();
        for unregistered in [
            migration_sql(VERSION),
            "CREATE TABLE tutor_turn_replacements(turn_id TEXT PRIMARY KEY) STRICT;",
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
            "DROP TABLE tutor_turn_replacements;",
            // Same columns without the cascades is not the registered table.
            "DROP TABLE tutor_turn_replacements;
             CREATE TABLE tutor_turn_replacements(
                 turn_id TEXT PRIMARY KEY,
                 replaced_by_turn_id TEXT NOT NULL UNIQUE,
                 created_at INTEGER NOT NULL
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
