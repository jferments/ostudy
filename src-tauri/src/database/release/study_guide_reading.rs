//! Additive release-v11 admission for study guide reading.
//!
//! Migration 11 adds the sections a learner marked as read in each document's
//! guide and the section they last opened, with their indexes. Both are written
//! in the reader; an upgrade writes none, so the tables start empty.

use std::sync::OnceLock;

use rusqlite::Connection;

use super::object_delta::{self, Delta};

const VERSION: i64 = 11;

pub(super) fn validate_schema(connection: &Connection, version: i64) -> Result<(), String> {
    static DELTA: OnceLock<Result<Delta, String>> = OnceLock::new();
    object_delta::validate(connection, version, VERSION, "study guide reading", &DELTA)
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
    const READING_TABLES: [&str; 2] = [
        "document_study_guide_section_reads",
        "document_study_guide_positions",
    ];

    /// A schema-10 library as the previous release left it, with one document.
    fn schema_ten() -> Connection {
        let connection = release_library(10);
        crate::collections::test_support::seed_document(&connection, DOCUMENT, 2, 'a').unwrap();
        assert_eq!(validate_supported_input(&connection).unwrap(), 10);
        connection
    }

    fn count(connection: &Connection, sql: &str) -> i64 {
        connection.query_row(sql, [], |row| row.get(0)).unwrap()
    }

    #[test]
    fn schema_ten_upgrades_additively_with_nothing_read() {
        let mut connection = schema_ten();
        let before = preservation_receipt(&connection).unwrap();
        let changes = connection.total_changes();
        validate_backup_input(&connection).unwrap();
        assert_eq!(connection.total_changes(), changes);

        assert!(apply_migrations_through(&mut connection, VERSION).unwrap());
        assert_eq!(validate_supported_input(&connection).unwrap(), VERSION);
        for table in READING_TABLES {
            assert_eq!(
                count(&connection, &format!("SELECT COUNT(*) FROM {table}")),
                0
            );
        }
        assert_eq!(count(&connection, "SELECT COUNT(*) FROM documents"), 1);

        let comparison = connection.transaction().unwrap();
        comparison
            .execute_batch(
                "DROP TABLE document_study_guide_section_reads;
                 DROP TABLE document_study_guide_positions;",
            )
            .unwrap();
        assert_eq!(preservation_receipt(&comparison).unwrap(), before);
        comparison.rollback().unwrap();
    }

    #[test]
    fn interrupted_eleventh_migration_rolls_back_to_schema_ten_and_retries() {
        let mut connection = schema_ten();
        let before = preservation_receipt(&connection).unwrap();
        connection
            .execute_batch(
                "CREATE TEMP TRIGGER reject_reading_migration
                 BEFORE INSERT ON schema_migrations
                 WHEN NEW.version=11
                 BEGIN SELECT RAISE(ABORT,'injected failure'); END;",
            )
            .unwrap();
        assert!(apply_migrations(&mut connection).is_err());
        assert_eq!(validate_supported_input(&connection).unwrap(), 10);
        assert_eq!(preservation_receipt(&connection).unwrap(), before);
        validate_schema(&connection, 10).unwrap();

        connection
            .execute_batch("DROP TRIGGER reject_reading_migration;")
            .unwrap();
        assert!(apply_migrations(&mut connection).unwrap());
        let changes = connection.total_changes();
        assert!(!apply_migrations(&mut connection).unwrap());
        assert_eq!(connection.total_changes(), changes);
        validate_input(&connection).unwrap();
    }

    #[test]
    fn unregistered_missing_and_substituted_reading_schemas_fail_read_only() {
        let mut connection = schema_ten();
        for unregistered in [
            migration_sql(VERSION),
            "CREATE INDEX document_study_guide_positions_by_opened ON documents(id);",
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
            "DROP TABLE document_study_guide_positions;",
            "DROP INDEX document_study_guide_section_reads_by_topic;",
            "DROP INDEX document_study_guide_positions_by_opened;
             CREATE INDEX document_study_guide_positions_by_opened
                 ON document_study_guide_positions(document_id, opened_at);",
            // Same columns keyed by guide revision is not the registered table.
            "DROP TABLE document_study_guide_section_reads;
             CREATE TABLE document_study_guide_section_reads(
                 document_id TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
                 topic_id TEXT NOT NULL REFERENCES document_learning_topics(id) ON DELETE CASCADE,
                 content_hash TEXT NOT NULL, read_at INTEGER NOT NULL,
                 PRIMARY KEY(topic_id, document_id)
             ) STRICT;
             CREATE INDEX document_study_guide_section_reads_by_topic
                 ON document_study_guide_section_reads(topic_id);",
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
