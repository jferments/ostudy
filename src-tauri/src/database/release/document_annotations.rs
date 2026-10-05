//! Additive release-v7 admission for document annotations.
//!
//! Learners create annotations in the annotator; an upgrade writes none, so
//! the table starts empty. Its page index is part of the registered schema.

use rusqlite::{Connection, OptionalExtension};

const VERSION: i64 = 7;
const INDEX: &str = "document_annotations_by_page";

pub(super) fn validate_schema(connection: &Connection, version: i64) -> Result<(), String> {
    super::validate_additive_table(
        connection,
        version,
        VERSION,
        "document_annotations",
        "document annotation",
    )?;
    let actual = index_sql(connection)?;
    if version < VERSION {
        if actual.is_some() {
            return Err(
                "An unregistered release-v7 document annotation index is present.".to_string(),
            );
        }
        return Ok(());
    }
    let expected = Connection::open_in_memory().map_err(|error| error.to_string())?;
    expected
        .execute_batch(super::migration_sql(VERSION))
        .map_err(|error| error.to_string())?;
    let expected = index_sql(&expected)?;
    match (actual, expected) {
        (Some(actual), Some(expected))
            if super::normalize_schema_sql(&actual) == super::normalize_schema_sql(&expected) =>
        {
            Ok(())
        }
        _ => Err("The document annotation index does not match this build.".to_string()),
    }
}

fn index_sql(connection: &Connection) -> Result<Option<String>, String> {
    connection
        .query_row(
            "SELECT sql FROM sqlite_schema WHERE type = 'index' AND name = ?1",
            [INDEX],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())
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

    /// A schema-6 library as the previous release left it, with one document.
    fn schema_six() -> Connection {
        let connection = release_library(6);
        crate::collections::test_support::seed_document(&connection, DOCUMENT, 2, 'a').unwrap();
        assert_eq!(validate_supported_input(&connection).unwrap(), 6);
        connection
    }

    fn count(connection: &Connection, sql: &str) -> i64 {
        connection.query_row(sql, [], |row| row.get(0)).unwrap()
    }

    #[test]
    fn schema_six_upgrades_additively_with_no_annotations() {
        let mut connection = schema_six();
        let before = preservation_receipt(&connection).unwrap();
        let changes = connection.total_changes();
        validate_backup_input(&connection).unwrap();
        assert_eq!(connection.total_changes(), changes);

        assert!(apply_migrations_through(&mut connection, VERSION).unwrap());
        assert_eq!(validate_supported_input(&connection).unwrap(), VERSION);
        assert_eq!(
            count(&connection, "SELECT COUNT(*) FROM document_annotations"),
            0
        );
        assert_eq!(count(&connection, "SELECT COUNT(*) FROM documents"), 1);

        let comparison = connection.transaction().unwrap();
        comparison
            .execute_batch("DROP TABLE document_annotations;")
            .unwrap();
        assert_eq!(preservation_receipt(&comparison).unwrap(), before);
        comparison.rollback().unwrap();
    }

    #[test]
    fn interrupted_seventh_migration_rolls_back_to_schema_six_and_retries() {
        let mut connection = schema_six();
        let before = preservation_receipt(&connection).unwrap();
        connection
            .execute_batch(
                "CREATE TEMP TRIGGER reject_annotation_migration
                 BEFORE INSERT ON schema_migrations
                 WHEN NEW.version=7
                 BEGIN SELECT RAISE(ABORT,'injected failure'); END;",
            )
            .unwrap();
        assert!(apply_migrations(&mut connection).is_err());
        assert_eq!(validate_supported_input(&connection).unwrap(), 6);
        assert_eq!(preservation_receipt(&connection).unwrap(), before);
        validate_schema(&connection, 6).unwrap();

        connection
            .execute_batch("DROP TRIGGER reject_annotation_migration;")
            .unwrap();
        assert!(apply_migrations(&mut connection).unwrap());
        let changes = connection.total_changes();
        assert!(!apply_migrations(&mut connection).unwrap());
        assert_eq!(connection.total_changes(), changes);
        validate_input(&connection).unwrap();
    }

    #[test]
    fn unregistered_missing_and_substituted_annotation_schemas_fail_read_only() {
        let mut connection = schema_six();
        for unregistered in [
            migration_sql(VERSION),
            "CREATE INDEX document_annotations_by_page ON documents(id);",
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
            "DROP TABLE document_annotations;",
            "DROP INDEX document_annotations_by_page;",
            "DROP INDEX document_annotations_by_page;
             CREATE INDEX document_annotations_by_page
                 ON document_annotations(document_id);",
            // Same columns without the checks is not the registered table.
            "DROP TABLE document_annotations;
             CREATE TABLE document_annotations(
                 id TEXT PRIMARY KEY,
                 document_id TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
                 page_number INTEGER NOT NULL, kind TEXT NOT NULL,
                 payload_json TEXT NOT NULL, revision INTEGER NOT NULL,
                 created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL
             ) STRICT;
             CREATE INDEX document_annotations_by_page
                 ON document_annotations(document_id, page_number, created_at, id);",
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
