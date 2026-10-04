//! Additive release-v3 admission for deck-merge receipts.

use rusqlite::Connection;

pub(super) fn validate_schema(connection: &Connection, version: i64) -> Result<(), String> {
    super::validate_additive_table(
        connection,
        version,
        3,
        "deck_merge_operations",
        "deck-merge receipt",
    )?;
    if version < 3 {
        return Ok(());
    }
    crate::deck_merge::validate_stored_receipts(connection)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::release::{
        apply_migrations, preservation_receipt, validate_backup_input, validate_input,
        validate_supported_input, write_release_ledger, BASELINE_SQL, RELEASE_MIGRATIONS,
        SCHEMA_VERSION,
    };
    use rusqlite::params;

    fn schema_two() -> Connection {
        let mut connection = Connection::open_in_memory().unwrap();
        let transaction = connection.transaction().unwrap();
        transaction.execute_batch(BASELINE_SQL).unwrap();
        write_release_ledger(&transaction).unwrap();
        let second = &RELEASE_MIGRATIONS[0];
        transaction.execute_batch(second.sql).unwrap();
        transaction
            .execute(
                "INSERT INTO schema_migrations(version,name,checksum,applied_at)
                 VALUES (2,?1,?2,1)",
                params![second.name, second.checksum],
            )
            .unwrap();
        transaction.pragma_update(None, "user_version", 2).unwrap();
        transaction
            .execute_batch(
                "INSERT INTO decks(id,name,created_at,updated_at)
                 VALUES ('11111111111111111111111111111111','Schema two sentinel',10,20);",
            )
            .unwrap();
        transaction.commit().unwrap();
        assert_eq!(validate_supported_input(&connection).unwrap(), 2);
        connection
    }

    #[test]
    fn schema_two_backup_upgrades_additively_to_current_schema() {
        let mut connection = schema_two();
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
        let rows: i64 = connection
            .query_row("SELECT COUNT(*) FROM deck_merge_operations", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(rows, 0);

        let comparison = connection.transaction().unwrap();
        comparison
            .execute_batch(
                "DROP TABLE study_target_generation_instructions;
                 DROP TABLE deck_merge_operations;",
            )
            .unwrap();
        assert_eq!(preservation_receipt(&comparison).unwrap(), before);
        comparison.rollback().unwrap();
    }

    #[test]
    fn interrupted_third_migration_rolls_back_and_retries_exactly_once() {
        let mut connection = schema_two();
        let before = preservation_receipt(&connection).unwrap();
        connection
            .execute_batch(
                "CREATE TEMP TRIGGER reject_merge_migration
                 BEFORE INSERT ON schema_migrations
                 WHEN NEW.version=3
                 BEGIN SELECT RAISE(ABORT,'injected failure'); END;",
            )
            .unwrap();
        assert!(apply_migrations(&mut connection).is_err());
        assert_eq!(validate_supported_input(&connection).unwrap(), 2);
        assert_eq!(preservation_receipt(&connection).unwrap(), before);
        validate_schema(&connection, 2).unwrap();

        connection
            .execute_batch("DROP TRIGGER reject_merge_migration;")
            .unwrap();
        assert!(apply_migrations(&mut connection).unwrap());
        let changes = connection.total_changes();
        assert!(!apply_migrations(&mut connection).unwrap());
        assert_eq!(connection.total_changes(), changes);
        validate_input(&connection).unwrap();
    }

    #[test]
    fn unregistered_missing_substituted_and_corrupt_receipt_tables_fail_read_only() {
        let mut connection = schema_two();
        {
            let transaction = connection.transaction().unwrap();
            transaction
                .execute_batch(super::super::migration_sql(3))
                .unwrap();
            let changes = transaction.total_changes();
            assert!(validate_supported_input(&transaction).is_err());
            assert_eq!(transaction.total_changes(), changes);
            transaction.rollback().unwrap();
        }

        apply_migrations(&mut connection).unwrap();
        for damage in [
            "DROP TABLE deck_merge_operations;",
            "DROP TABLE deck_merge_operations;
             CREATE TABLE deck_merge_operations(
                 operation_id TEXT,request_sha256 TEXT,result_json TEXT,created_at INTEGER
             );",
            "INSERT INTO deck_merge_operations VALUES(
                 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
                 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
                 '{}',1
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
