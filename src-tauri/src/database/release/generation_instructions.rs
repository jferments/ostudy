//! Additive release-v4 admission for captured StudyTarget generation guidance.

use rusqlite::Connection;

pub(super) fn validate_schema(connection: &Connection, version: i64) -> Result<(), String> {
    super::validate_additive_table(
        connection,
        version,
        4,
        "study_target_generation_instructions",
        "StudyTarget generation-instruction",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::release::{
        apply_migrations, apply_migrations_through, migration_sql, preservation_receipt,
        validate_backup_input, validate_input, validate_supported_input, write_release_ledger,
        BASELINE_SQL, RELEASE_MIGRATIONS,
    };
    use rusqlite::params;

    fn schema_three() -> Connection {
        let mut connection = Connection::open_in_memory().unwrap();
        let transaction = connection.transaction().unwrap();
        transaction.execute_batch(BASELINE_SQL).unwrap();
        write_release_ledger(&transaction).unwrap();
        for migration in RELEASE_MIGRATIONS.iter().filter(|m| m.version <= 3) {
            transaction.execute_batch(migration.sql).unwrap();
            transaction
                .execute(
                    "INSERT INTO schema_migrations(version,name,checksum,applied_at)
                     VALUES (?1,?2,?3,1)",
                    params![migration.version, migration.name, migration.checksum],
                )
                .unwrap();
        }
        transaction.pragma_update(None, "user_version", 3).unwrap();
        transaction
            .execute_batch(
                "INSERT INTO decks(id,name,created_at,updated_at)
                 VALUES ('11111111111111111111111111111111','Schema three sentinel',10,20);",
            )
            .unwrap();
        transaction.commit().unwrap();
        assert_eq!(validate_supported_input(&connection).unwrap(), 3);
        connection
    }

    /// Schema 6 reshapes filing and has its own tests, so the additive releases
    /// are compared through schema 5.
    #[test]
    fn schema_three_upgrades_additively_through_schema_five() {
        let mut connection = schema_three();
        let before = preservation_receipt(&connection).unwrap();
        let changes = connection.total_changes();
        validate_backup_input(&connection).unwrap();
        assert_eq!(connection.total_changes(), changes);

        assert!(apply_migrations_through(&mut connection, 5).unwrap());
        assert_eq!(validate_supported_input(&connection).unwrap(), 5);
        let rows: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM study_target_generation_instructions",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(rows, 0);

        let comparison = connection.transaction().unwrap();
        comparison
            .execute_batch(
                "DROP TABLE review_event_scheduler_context;
                 DROP TABLE study_target_generation_instructions;",
            )
            .unwrap();
        assert_eq!(preservation_receipt(&comparison).unwrap(), before);
        comparison.rollback().unwrap();
    }

    #[test]
    fn interrupted_fourth_migration_rolls_back_to_schema_three_and_retries() {
        let mut connection = schema_three();
        let before = preservation_receipt(&connection).unwrap();
        connection
            .execute_batch(
                "CREATE TEMP TRIGGER reject_guidance_migration
                 BEFORE INSERT ON schema_migrations
                 WHEN NEW.version=4
                 BEGIN SELECT RAISE(ABORT,'injected failure'); END;",
            )
            .unwrap();
        assert!(apply_migrations(&mut connection).is_err());
        assert_eq!(validate_supported_input(&connection).unwrap(), 3);
        assert_eq!(preservation_receipt(&connection).unwrap(), before);
        validate_schema(&connection, 3).unwrap();

        connection
            .execute_batch("DROP TRIGGER reject_guidance_migration;")
            .unwrap();
        assert!(apply_migrations(&mut connection).unwrap());
        let changes = connection.total_changes();
        assert!(!apply_migrations(&mut connection).unwrap());
        assert_eq!(connection.total_changes(), changes);
        validate_input(&connection).unwrap();
    }

    #[test]
    fn unregistered_missing_and_substituted_guidance_tables_fail_read_only() {
        let mut connection = schema_three();
        {
            let transaction = connection.transaction().unwrap();
            transaction.execute_batch(migration_sql(4)).unwrap();
            let changes = transaction.total_changes();
            assert!(validate_supported_input(&transaction).is_err());
            assert_eq!(transaction.total_changes(), changes);
            transaction.rollback().unwrap();
        }

        apply_migrations(&mut connection).unwrap();
        for damage in [
            "DROP TABLE study_target_generation_instructions;",
            "DROP TABLE study_target_generation_instructions;
             CREATE TABLE study_target_generation_instructions(
                 operation_id TEXT,generation_instructions TEXT,created_at INTEGER
             );",
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
