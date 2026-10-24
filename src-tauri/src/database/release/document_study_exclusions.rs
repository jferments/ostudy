//! Additive release-v22 admission for documents left out of a subject's study set.
//!
//! Migration 22 adds the documents a learner leaves out of their subject's
//! study materials, with triggers that let a moved document back in and mark
//! the subject's knowledge view stale. An upgrade writes none, so the table
//! starts empty.

use std::sync::OnceLock;

use rusqlite::Connection;

use super::object_delta::{self, Delta};

const VERSION: i64 = 22;

pub(super) fn validate_schema(connection: &Connection, version: i64) -> Result<(), String> {
    static DELTA: OnceLock<Result<Delta, String>> = OnceLock::new();
    object_delta::validate(
        connection,
        version,
        VERSION,
        "document study exclusions",
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

    const DOCUMENT: &str = "11111111111111111111111111111111";
    const SUBJECT: &str = "22222222222222222222222222222222";
    const OTHER_SUBJECT: &str = "33333333333333333333333333333333";

    /// A schema-21 library as the previous release left it, with one document
    /// filed in a subject.
    fn schema_twenty_one() -> Connection {
        let connection = release_library(21);
        crate::collections::test_support::seed_document(&connection, DOCUMENT, 2, 'a').unwrap();
        connection
            .execute_batch(&format!(
                "INSERT INTO document_collections(id,name,sort_order,created_at,updated_at,revision)
                 VALUES('{SUBJECT}','Physiology',0,1,1,1),('{OTHER_SUBJECT}','Pharmacology',1,1,1,1);
                 UPDATE documents SET subject_id='{SUBJECT}' WHERE id='{DOCUMENT}';"
            ))
            .unwrap();
        assert_eq!(validate_supported_input(&connection).unwrap(), 21);
        connection
    }

    fn count(connection: &Connection, sql: &str) -> i64 {
        connection.query_row(sql, [], |row| row.get(0)).unwrap()
    }

    #[test]
    fn schema_twenty_one_upgrades_additively_with_every_document_in() {
        let mut connection = schema_twenty_one();
        let before = preservation_receipt(&connection).unwrap();
        let changes = connection.total_changes();
        validate_backup_input(&connection).unwrap();
        assert_eq!(connection.total_changes(), changes);

        assert!(apply_migrations_through(&mut connection, VERSION).unwrap());
        assert_eq!(validate_supported_input(&connection).unwrap(), VERSION);
        assert_eq!(
            count(
                &connection,
                "SELECT COUNT(*) FROM document_study_exclusions"
            ),
            0
        );
        assert_eq!(count(&connection, "SELECT COUNT(*) FROM documents"), 1);

        let comparison = connection.transaction().unwrap();
        comparison
            .execute_batch(
                "DROP TRIGGER document_study_exclusions_follow_filing;
                 DROP TRIGGER knowledge_collection_view_study_exclusion_insert;
                 DROP TRIGGER knowledge_collection_view_study_exclusion_delete;
                 DROP TABLE document_study_exclusions;",
            )
            .unwrap();
        assert_eq!(preservation_receipt(&comparison).unwrap(), before);
        comparison.rollback().unwrap();
    }

    #[test]
    fn a_left_out_document_comes_back_when_it_changes_subject_or_goes() {
        let mut connection = schema_twenty_one();
        apply_migrations(&mut connection).unwrap();
        let exclude = format!(
            "INSERT INTO document_study_exclusions(document_id,created_at) VALUES('{DOCUMENT}',2)"
        );
        let excluded = "SELECT COUNT(*) FROM document_study_exclusions";

        connection.execute_batch(&exclude).unwrap();
        connection
            .execute_batch(&format!(
                "UPDATE documents SET title='Renamed' WHERE id='{DOCUMENT}'"
            ))
            .unwrap();
        assert_eq!(
            count(&connection, excluded),
            1,
            "other edits keep the choice"
        );

        connection
            .execute_batch(&format!(
                "UPDATE documents SET subject_id='{OTHER_SUBJECT}' WHERE id='{DOCUMENT}'"
            ))
            .unwrap();
        assert_eq!(count(&connection, excluded), 0, "a move lets it back in");

        connection.execute_batch(&exclude).unwrap();
        connection
            .execute_batch(&format!(
                "UPDATE documents SET subject_id=NULL WHERE id='{DOCUMENT}'"
            ))
            .unwrap();
        assert_eq!(
            count(&connection, excluded),
            0,
            "leaving the subject lets it back in"
        );

        connection
            .execute_batch(&format!(
                "UPDATE documents SET subject_id='{SUBJECT}' WHERE id='{DOCUMENT}'; {exclude};
                 DELETE FROM documents WHERE id='{DOCUMENT}';"
            ))
            .unwrap();
        assert_eq!(
            count(&connection, excluded),
            0,
            "deleting the document deletes the choice"
        );
    }

    #[test]
    fn interrupted_twenty_second_migration_rolls_back_to_schema_twenty_one_and_retries() {
        let mut connection = schema_twenty_one();
        let before = preservation_receipt(&connection).unwrap();
        connection
            .execute_batch(
                "CREATE TEMP TRIGGER reject_exclusions_migration
                 BEFORE INSERT ON schema_migrations
                 WHEN NEW.version=22
                 BEGIN SELECT RAISE(ABORT,'injected failure'); END;",
            )
            .unwrap();
        assert!(apply_migrations(&mut connection).is_err());
        assert_eq!(validate_supported_input(&connection).unwrap(), 21);
        assert_eq!(preservation_receipt(&connection).unwrap(), before);
        validate_schema(&connection, 21).unwrap();

        connection
            .execute_batch("DROP TRIGGER reject_exclusions_migration;")
            .unwrap();
        assert!(apply_migrations(&mut connection).unwrap());
        let changes = connection.total_changes();
        assert!(!apply_migrations(&mut connection).unwrap());
        assert_eq!(connection.total_changes(), changes);
        validate_input(&connection).unwrap();
    }

    #[test]
    fn unregistered_missing_and_substituted_exclusion_schemas_fail_read_only() {
        let mut connection = schema_twenty_one();
        for unregistered in [
            migration_sql(VERSION),
            "CREATE TABLE document_study_exclusions(document_id TEXT PRIMARY KEY) STRICT;",
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
            "DROP TRIGGER document_study_exclusions_follow_filing;",
            "DROP TRIGGER knowledge_collection_view_study_exclusion_insert;",
            // Same columns without the cascade is not the registered table.
            "DROP TABLE document_study_exclusions;
             CREATE TABLE document_study_exclusions(
                 document_id TEXT PRIMARY KEY CHECK(length(document_id) = 32),
                 created_at INTEGER NOT NULL CHECK(created_at >= 0)
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
