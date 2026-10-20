//! Additive release-v11 admission for lesson narration.
//!
//! Migration 11 adds the speech setting and the kept narration segments.
//! Narration is made in the app; an upgrade writes none, so both tables start
//! empty. It also rebuilds `prepared_exports` with every row kept, so a lesson
//! can be prepared as a lecture deck (`lecture_pptx`).

use std::sync::OnceLock;

use rusqlite::Connection;

use super::object_delta::{self, Delta};

const VERSION: i64 = 11;

pub(super) fn validate_schema(connection: &Connection, version: i64) -> Result<(), String> {
    static DELTA: OnceLock<Result<Delta, String>> = OnceLock::new();
    object_delta::validate(connection, version, VERSION, "lesson narration", &DELTA)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::release::{
        apply_migrations, apply_migrations_through, migration_sql, preservation_receipt,
        subjects_fixture::release_library, validate_backup_input, validate_input,
        validate_supported_input,
    };

    const NARRATION_TABLES: [&str; 2] = ["speech_settings", "speech_segments"];

    /// A schema-10 library as the previous release left it, with one subject.
    fn schema_ten() -> Connection {
        let connection = release_library(10);
        connection
            .execute(
                "INSERT INTO document_collections(id, name, sort_order, created_at, updated_at, revision)
                 VALUES (?1, 'Renal physiology', 1000, 1, 1, 1)",
                ["5".repeat(32)],
            )
            .unwrap();
        assert_eq!(validate_supported_input(&connection).unwrap(), 10);
        connection
    }

    fn count(connection: &Connection, sql: &str) -> i64 {
        connection.query_row(sql, [], |row| row.get(0)).unwrap()
    }

    #[test]
    fn schema_ten_upgrades_additively_with_no_narration() {
        let mut connection = schema_ten();
        let before = preservation_receipt(&connection).unwrap();
        let changes = connection.total_changes();
        validate_backup_input(&connection).unwrap();
        assert_eq!(connection.total_changes(), changes);

        assert!(apply_migrations_through(&mut connection, VERSION).unwrap());
        assert_eq!(validate_supported_input(&connection).unwrap(), VERSION);
        for table in NARRATION_TABLES {
            assert_eq!(
                count(&connection, &format!("SELECT COUNT(*) FROM {table}")),
                0
            );
        }

        let comparison = connection.transaction().unwrap();
        comparison
            .execute_batch(
                "DROP TABLE speech_segments;
                 DROP TABLE speech_settings;",
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
                "CREATE TEMP TRIGGER reject_narration_migration
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
            .execute_batch("DROP TRIGGER reject_narration_migration;")
            .unwrap();
        assert!(apply_migrations(&mut connection).unwrap());
        let changes = connection.total_changes();
        assert!(!apply_migrations(&mut connection).unwrap());
        assert_eq!(connection.total_changes(), changes);
        validate_input(&connection).unwrap();
    }

    #[test]
    fn unregistered_missing_and_substituted_narration_schemas_fail_read_only() {
        let mut connection = schema_ten();
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
            "DROP TABLE speech_settings;",
            // Same columns without the checks is not the registered table.
            "DROP TABLE speech_segments;
             CREATE TABLE speech_segments(
                 id TEXT PRIMARY KEY, text_sha256 TEXT NOT NULL, engine TEXT NOT NULL,
                 model_id TEXT NOT NULL, voice TEXT NOT NULL, format TEXT NOT NULL,
                 asset_id TEXT NOT NULL UNIQUE REFERENCES assets(id),
                 characters INTEGER NOT NULL, created_at INTEGER NOT NULL,
                 UNIQUE(text_sha256, engine, model_id, voice, format)
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

    #[test]
    fn prepared_exports_keep_their_rows_and_accept_lecture_decks() {
        let mut connection = schema_ten();
        let insert = |connection: &Connection, id: char, purpose: &str| {
            connection.execute(
                "INSERT INTO prepared_exports(
                     id, operation_id, purpose, suggested_filename, media_type, byte_size,
                     sha256, relative_path, state, summary_json, created_at, updated_at,
                     expires_at, published_at, released_at
                 ) VALUES (?1, ?2, ?3, 'out', 'application/octet-stream', 10, ?4, ?5,
                           'published', '{}', 1, 2, 3, 2, NULL)",
                rusqlite::params![
                    id.to_string().repeat(32),
                    id.to_string().repeat(32),
                    purpose,
                    "e".repeat(64),
                    format!("exports/{id}")
                ],
            )
        };
        insert(&connection, 'a', "study_guide_pdf").unwrap();
        assert!(insert(&connection, 'b', "lecture_pptx").is_err());
        let rows = |connection: &Connection| {
            connection
                .query_row(
                    "SELECT group_concat(id || purpose || state || published_at, ',')
                     FROM prepared_exports",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .unwrap()
        };
        let before = rows(&connection);

        apply_migrations_through(&mut connection, VERSION).unwrap();
        assert_eq!(rows(&connection), before);
        insert(&connection, 'b', "lecture_pptx").unwrap();
        assert!(insert(&connection, 'c', "slides").is_err());
        assert_eq!(
            count(
                &connection,
                "SELECT COUNT(*) FROM sqlite_schema
                 WHERE name IN ('prepared_exports_v11')
                    OR (type = 'index' AND name = 'prepared_exports_state_expiry_idx')"
            ),
            1
        );
        validate_input(&connection).unwrap();
    }
}
