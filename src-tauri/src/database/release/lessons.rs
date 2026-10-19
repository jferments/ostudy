//! Additive release-v9 admission for lessons.
//!
//! Migration 9 adds lessons, their immutable revisions, the knowledge units
//! each revision explains, and lesson progress. Lessons are written in the
//! app; an upgrade writes none, so every table starts empty.

use std::sync::OnceLock;

use rusqlite::Connection;

use super::object_delta::{self, Delta};

const VERSION: i64 = 9;

pub(super) fn validate_schema(connection: &Connection, version: i64) -> Result<(), String> {
    static DELTA: OnceLock<Result<Delta, String>> = OnceLock::new();
    object_delta::validate(connection, version, VERSION, "lesson", &DELTA)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::release::{
        apply_migrations, apply_migrations_through, migration_sql, preservation_receipt,
        subjects_fixture::release_library, validate_backup_input, validate_input,
        validate_supported_input,
    };

    const LESSON_TABLES: [&str; 4] = [
        "lessons",
        "lesson_revisions",
        "lesson_revision_units",
        "lesson_progress",
    ];

    /// A schema-8 library as the previous release left it, with one subject.
    fn schema_eight() -> Connection {
        let connection = release_library(8);
        connection
            .execute(
                "INSERT INTO document_collections(id, name, sort_order, created_at, updated_at, revision)
                 VALUES (?1, 'Renal physiology', 1000, 1, 1, 1)",
                ["5".repeat(32)],
            )
            .unwrap();
        assert_eq!(validate_supported_input(&connection).unwrap(), 8);
        connection
    }

    fn count(connection: &Connection, sql: &str) -> i64 {
        connection.query_row(sql, [], |row| row.get(0)).unwrap()
    }

    #[test]
    fn schema_eight_upgrades_additively_with_no_lessons() {
        let mut connection = schema_eight();
        let before = preservation_receipt(&connection).unwrap();
        let changes = connection.total_changes();
        validate_backup_input(&connection).unwrap();
        assert_eq!(connection.total_changes(), changes);

        assert!(apply_migrations_through(&mut connection, VERSION).unwrap());
        assert_eq!(validate_supported_input(&connection).unwrap(), VERSION);
        for table in LESSON_TABLES {
            assert_eq!(
                count(&connection, &format!("SELECT COUNT(*) FROM {table}")),
                0
            );
        }
        assert_eq!(
            count(&connection, "SELECT COUNT(*) FROM document_collections"),
            1
        );

        let comparison = connection.transaction().unwrap();
        comparison
            .execute_batch(
                "DROP TABLE lesson_progress;
                 DROP TABLE lesson_revision_units;
                 DROP TABLE lesson_revisions;
                 DROP TABLE lessons;",
            )
            .unwrap();
        assert_eq!(preservation_receipt(&comparison).unwrap(), before);
        comparison.rollback().unwrap();
    }

    #[test]
    fn interrupted_ninth_migration_rolls_back_to_schema_eight_and_retries() {
        let mut connection = schema_eight();
        let before = preservation_receipt(&connection).unwrap();
        connection
            .execute_batch(
                "CREATE TEMP TRIGGER reject_lesson_migration
                 BEFORE INSERT ON schema_migrations
                 WHEN NEW.version=9
                 BEGIN SELECT RAISE(ABORT,'injected failure'); END;",
            )
            .unwrap();
        assert!(apply_migrations(&mut connection).is_err());
        assert_eq!(validate_supported_input(&connection).unwrap(), 8);
        assert_eq!(preservation_receipt(&connection).unwrap(), before);
        validate_schema(&connection, 8).unwrap();

        connection
            .execute_batch("DROP TRIGGER reject_lesson_migration;")
            .unwrap();
        assert!(apply_migrations(&mut connection).unwrap());
        let changes = connection.total_changes();
        assert!(!apply_migrations(&mut connection).unwrap());
        assert_eq!(connection.total_changes(), changes);
        validate_input(&connection).unwrap();
    }

    #[test]
    fn unregistered_missing_and_substituted_lesson_schemas_fail_read_only() {
        let mut connection = schema_eight();
        for unregistered in [
            migration_sql(VERSION),
            "CREATE INDEX lessons_by_subject ON document_collections(id);",
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
            "DROP TABLE lesson_progress;",
            "DROP INDEX lesson_revision_units_by_unit;",
            "DROP INDEX lessons_by_subject;
             CREATE INDEX lessons_by_subject ON lessons(subject_id);",
            // Same columns without the checks is not the registered table.
            "DROP TABLE lesson_progress;
             CREATE TABLE lesson_progress(
                 lesson_id TEXT PRIMARY KEY REFERENCES lessons(id) ON DELETE CASCADE,
                 revision_number INTEGER NOT NULL, current_step INTEGER NOT NULL,
                 furthest_step INTEGER NOT NULL, checks_json TEXT NOT NULL,
                 revision INTEGER NOT NULL, started_at INTEGER NOT NULL,
                 updated_at INTEGER NOT NULL, finished_at INTEGER,
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
