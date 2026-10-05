//! Additive release-v5 admission for per-review FSRS scheduler context.
//!
//! Rows are written only by new reviews. Upgrading never backfills a row for a
//! review recorded before this table existed, so the table starts empty.

use rusqlite::Connection;

pub(super) fn validate_schema(connection: &Connection, version: i64) -> Result<(), String> {
    super::validate_additive_table(
        connection,
        version,
        5,
        "review_event_scheduler_context",
        "review scheduler-context",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::release::{
        apply_migrations, migration_sql, preservation_receipt, validate_backup_input,
        validate_input, validate_supported_input, write_release_ledger, BASELINE_SQL,
        RELEASE_MIGRATIONS, SCHEMA_VERSION,
    };
    use rusqlite::params;

    const DECK: &str = "11111111111111111111111111111111";
    const NOTE: &str = "22222222222222222222222222222222";
    const CARD: &str = "33333333333333333333333333333333";
    const REVIEW: &str = "44444444444444444444444444444444";

    /// A schema-4 library exactly as the previous release left it, with one
    /// recorded review and its analytics attribution.
    fn schema_four() -> Connection {
        let mut connection = Connection::open_in_memory().unwrap();
        let transaction = connection.transaction().unwrap();
        transaction.execute_batch(BASELINE_SQL).unwrap();
        write_release_ledger(&transaction).unwrap();
        for migration in RELEASE_MIGRATIONS.iter().filter(|m| m.version <= 4) {
            transaction.execute_batch(migration.sql).unwrap();
            transaction
                .execute(
                    "INSERT INTO schema_migrations(version,name,checksum,applied_at)
                     VALUES (?1,?2,?3,1)",
                    params![migration.version, migration.name, migration.checksum],
                )
                .unwrap();
        }
        transaction.pragma_update(None, "user_version", 4).unwrap();
        let document = r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Schema four"}]}]}"#;
        transaction
            .execute(
                "INSERT INTO decks(id,name,created_at,updated_at)
                 VALUES (?1,'Schema four sentinel',10,20)",
                [DECK],
            )
            .unwrap();
        transaction
            .execute(
                "INSERT INTO notes(id,deck_id,note_type,content_format,content_schema_version,
                                   front_doc,back_doc,front_text,back_text,
                                   created_at,updated_at,revision)
                 VALUES (?1,?2,'basic','tiptap-json',1,?3,?3,'Schema four','Schema four',10,10,1)",
                params![NOTE, DECK, document],
            )
            .unwrap();
        transaction
            .execute(
                "INSERT INTO cards(id,note_id,deck_id,direction,template_ordinal,created_at,updated_at)
                 VALUES (?1,?2,?3,'forward',0,10,10)",
                params![CARD, NOTE, DECK],
            )
            .unwrap();
        transaction
            .execute(
                "INSERT INTO card_schedules(card_id,state,due_at,stability,difficulty,elapsed_days,
                                            scheduled_days,reps,lapses,last_review_at,
                                            scheduler_name,scheduler_version,updated_at,revision)
                 VALUES (?1,2,5000,3.0,4.5,0,5,1,0,23,'fsrs','rs-fsrs-1.2.1',23,2)",
                [CARD],
            )
            .unwrap();
        transaction
            .execute(
                "INSERT INTO review_events(id,submission_id,session_id,card_id,reviewed_at,rating,
                                           answer_correct,confidence,response_time_ms,
                                           state_before,state_after,due_before,due_after,
                                           stability_before,stability_after,
                                           difficulty_before,difficulty_after,
                                           scheduled_days_before,scheduled_days_after,
                                           scheduler_name,scheduler_version,created_at)
                 VALUES (?1,?2,NULL,?3,23,3,1,'somewhat_confident',1200,0,2,10,5000,
                         0.0,3.0,0.0,4.5,0,5,'fsrs','rs-fsrs-1.2.1',23)",
                params![REVIEW, "5".repeat(32), CARD],
            )
            .unwrap();
        transaction.commit().unwrap();
        assert_eq!(validate_supported_input(&connection).unwrap(), 4);
        connection
    }

    fn count(connection: &Connection, sql: &str) -> i64 {
        connection.query_row(sql, [], |row| row.get(0)).unwrap()
    }

    #[test]
    fn schema_four_upgrades_additively_and_keeps_old_reviews_without_context() {
        let mut connection = schema_four();
        let before = preservation_receipt(&connection).unwrap();
        let changes = connection.total_changes();
        validate_backup_input(&connection).unwrap();
        assert_eq!(connection.total_changes(), changes);

        assert!(apply_migrations(&mut connection).unwrap());
        assert_eq!(
            validate_supported_input(&connection).unwrap(),
            SCHEMA_VERSION
        );
        validate_input(&connection).unwrap();
        assert_eq!(count(&connection, "SELECT COUNT(*) FROM review_events"), 1);
        assert_eq!(
            count(
                &connection,
                "SELECT COUNT(*) FROM review_event_scheduler_context"
            ),
            0,
            "an upgrade must not invent scheduler context for an old review"
        );

        let comparison = connection.transaction().unwrap();
        comparison
            .execute_batch("DROP TABLE review_event_scheduler_context;")
            .unwrap();
        assert_eq!(preservation_receipt(&comparison).unwrap(), before);
        comparison.rollback().unwrap();
    }

    #[test]
    fn interrupted_fifth_migration_rolls_back_to_schema_four_and_retries() {
        let mut connection = schema_four();
        let before = preservation_receipt(&connection).unwrap();
        connection
            .execute_batch(
                "CREATE TEMP TRIGGER reject_context_migration
                 BEFORE INSERT ON schema_migrations
                 WHEN NEW.version=5
                 BEGIN SELECT RAISE(ABORT,'injected failure'); END;",
            )
            .unwrap();
        assert!(apply_migrations(&mut connection).is_err());
        assert_eq!(validate_supported_input(&connection).unwrap(), 4);
        assert_eq!(preservation_receipt(&connection).unwrap(), before);
        validate_schema(&connection, 4).unwrap();

        connection
            .execute_batch("DROP TRIGGER reject_context_migration;")
            .unwrap();
        assert!(apply_migrations(&mut connection).unwrap());
        let changes = connection.total_changes();
        assert!(!apply_migrations(&mut connection).unwrap());
        assert_eq!(connection.total_changes(), changes);
        validate_input(&connection).unwrap();
    }

    #[test]
    fn unregistered_missing_and_substituted_context_tables_fail_read_only() {
        let mut connection = schema_four();
        {
            let transaction = connection.transaction().unwrap();
            transaction.execute_batch(migration_sql(5)).unwrap();
            let changes = transaction.total_changes();
            assert!(validate_supported_input(&transaction).is_err());
            assert_eq!(transaction.total_changes(), changes);
            transaction.rollback().unwrap();
        }

        apply_migrations(&mut connection).unwrap();
        for damage in [
            "DROP TABLE review_event_scheduler_context;",
            // Same columns without the range checks is not the registered table.
            "DROP TABLE review_event_scheduler_context;
             CREATE TABLE review_event_scheduler_context(
                 review_event_id TEXT PRIMARY KEY REFERENCES review_events(id) ON DELETE CASCADE,
                 previous_review_at INTEGER, elapsed_days INTEGER NOT NULL,
                 retrievability REAL, reps_before INTEGER NOT NULL,
                 lapses_before INTEGER NOT NULL, desired_retention REAL NOT NULL,
                 scheduler_parameters_sha256 TEXT NOT NULL, active_time_ms INTEGER
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
