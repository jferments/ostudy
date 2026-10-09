//! Release-v7 admission for the study activity ledger and review rollups.
//!
//! Migration 7 adds three tables and the triggers that keep the rollup cache
//! honest. It writes no rows: the retained ledger fills as deletions purge
//! review history, and every deck's rollups are built on first use.

use std::sync::OnceLock;

use rusqlite::Connection;

use super::object_delta::{self, Delta};

const VERSION: i64 = 7;

pub(super) fn validate_schema(connection: &Connection, version: i64) -> Result<(), String> {
    static DELTA: OnceLock<Result<Delta, String>> = OnceLock::new();
    object_delta::validate(connection, version, VERSION, "study activity", &DELTA)
}

#[cfg(test)]
mod tests {
    use crate::database::release::{
        apply_migrations_through, migration_sql, subjects_fixture::release_library, validate_input,
        validate_supported_input, SCHEMA_VERSION,
    };

    #[test]
    fn a_partial_or_altered_study_activity_schema_is_refused_read_only() {
        let mut connection = release_library(6);
        {
            // Half of migration 7 at schema 6 is not schema 6.
            let transaction = connection.transaction().unwrap();
            transaction
                .execute_batch(
                    "CREATE TABLE review_day_rollup_built_decks (
                         deck_id TEXT PRIMARY KEY REFERENCES decks(id) ON DELETE CASCADE
                     ) STRICT, WITHOUT ROWID;",
                )
                .unwrap();
            assert!(validate_supported_input(&transaction).is_err());
            transaction.rollback().unwrap();
        }
        {
            // The whole migration without its ledger entry is not schema 6 either.
            let transaction = connection.transaction().unwrap();
            transaction.execute_batch(migration_sql(7)).unwrap();
            assert!(validate_supported_input(&transaction).is_err());
            transaction.rollback().unwrap();
        }

        assert!(apply_migrations_through(&mut connection, SCHEMA_VERSION).unwrap());
        validate_input(&connection).unwrap();
        for damage in [
            "DROP TRIGGER review_day_rollups_after_review;",
            "DROP TRIGGER review_day_rollups_card_change;",
            "DROP TABLE study_activity_retained;",
            "DROP TABLE review_day_rollups;
             CREATE TABLE review_day_rollups(deck_id TEXT, wall_date TEXT, wall_hour INTEGER);",
        ] {
            let transaction = connection.transaction().unwrap();
            transaction.execute_batch(damage).unwrap();
            let changes = transaction.total_changes();
            assert!(validate_supported_input(&transaction).is_err(), "{damage}");
            assert_eq!(transaction.total_changes(), changes);
            transaction.rollback().unwrap();
        }
        validate_input(&connection).unwrap();
    }
}
