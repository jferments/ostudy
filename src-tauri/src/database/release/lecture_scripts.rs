//! Additive release-v15 admission for lecture scripts.
//!
//! Migration 15 adds the kept teacher's lecture of a lesson revision. Scripts
//! are written in the app; an upgrade writes none, so the table starts empty.

use std::sync::OnceLock;

use rusqlite::Connection;

use super::object_delta::{self, Delta};

const VERSION: i64 = 15;

pub(super) fn validate_schema(connection: &Connection, version: i64) -> Result<(), String> {
    static DELTA: OnceLock<Result<Delta, String>> = OnceLock::new();
    object_delta::validate(connection, version, VERSION, "lecture script", &DELTA)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::release::{
        apply_migrations, apply_migrations_through, migration_sql, preservation_receipt,
        subjects_fixture::release_library, validate_backup_input, validate_input,
        validate_supported_input,
    };

    /// A schema-14 library as the previous release left it.
    fn schema_fourteen() -> Connection {
        let connection = release_library(14);
        assert_eq!(validate_supported_input(&connection).unwrap(), 14);
        connection
    }

    #[test]
    fn schema_fourteen_upgrades_additively_with_no_scripts() {
        let mut connection = schema_fourteen();
        let before = preservation_receipt(&connection).unwrap();
        let changes = connection.total_changes();
        validate_backup_input(&connection).unwrap();
        assert_eq!(connection.total_changes(), changes);

        assert!(apply_migrations_through(&mut connection, VERSION).unwrap());
        assert_eq!(validate_supported_input(&connection).unwrap(), VERSION);
        let scripts: i64 = connection
            .query_row("SELECT COUNT(*) FROM lecture_scripts", [], |row| row.get(0))
            .unwrap();
        assert_eq!(scripts, 0);

        let comparison = connection.transaction().unwrap();
        comparison
            .execute_batch("DROP TABLE lecture_scripts;")
            .unwrap();
        assert_eq!(preservation_receipt(&comparison).unwrap(), before);
        comparison.rollback().unwrap();
    }

    #[test]
    fn interrupted_fifteenth_migration_rolls_back_to_schema_fourteen_and_retries() {
        let mut connection = schema_fourteen();
        let before = preservation_receipt(&connection).unwrap();
        connection
            .execute_batch(
                "CREATE TEMP TRIGGER reject_script_migration
                 BEFORE INSERT ON schema_migrations
                 WHEN NEW.version=15
                 BEGIN SELECT RAISE(ABORT,'injected failure'); END;",
            )
            .unwrap();
        assert!(apply_migrations(&mut connection).is_err());
        assert_eq!(validate_supported_input(&connection).unwrap(), 14);
        assert_eq!(preservation_receipt(&connection).unwrap(), before);
        validate_schema(&connection, 14).unwrap();

        connection
            .execute_batch("DROP TRIGGER reject_script_migration;")
            .unwrap();
        assert!(apply_migrations(&mut connection).unwrap());
        let changes = connection.total_changes();
        assert!(!apply_migrations(&mut connection).unwrap());
        assert_eq!(connection.total_changes(), changes);
        validate_input(&connection).unwrap();
    }

    #[test]
    fn unregistered_missing_and_substituted_script_schemas_fail_read_only() {
        let mut connection = schema_fourteen();
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
            "DROP TABLE lecture_scripts;",
            // Same columns without the checks is not the registered table.
            "DROP TABLE lecture_scripts;
             CREATE TABLE lecture_scripts(
                 lesson_id TEXT NOT NULL, revision_number INTEGER NOT NULL,
                 script_json TEXT NOT NULL, provenance_json TEXT NOT NULL,
                 created_at INTEGER NOT NULL,
                 PRIMARY KEY(lesson_id, revision_number),
                 FOREIGN KEY(lesson_id, revision_number)
                     REFERENCES lesson_revisions(lesson_id, revision_number) ON DELETE CASCADE
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
