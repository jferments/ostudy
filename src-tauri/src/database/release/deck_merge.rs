//! Additive release-v3 admission for deck-merge replay receipts.

use rusqlite::{Connection, OptionalExtension};

pub(super) fn validate_schema(connection: &Connection, version: i64) -> Result<(), String> {
    let actual: Option<String> = connection
        .query_row(
            "SELECT sql FROM sqlite_schema
             WHERE type='table' AND name='deck_merge_operations'",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    if version < 3 {
        if actual.is_some() {
            return Err("An unregistered deck-merge migration is present.".to_string());
        }
        return Ok(());
    }

    let actual = actual.ok_or_else(|| "The deck-merge receipt table is missing.".to_string())?;
    let expected = Connection::open_in_memory().map_err(|error| error.to_string())?;
    expected
        .execute_batch(super::RELEASE_MIGRATIONS[1].sql)
        .map_err(|error| error.to_string())?;
    let expected_sql: String = expected
        .query_row(
            "SELECT sql FROM sqlite_schema WHERE name='deck_merge_operations'",
            [],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    if super::normalize_schema_sql(&actual) != super::normalize_schema_sql(&expected_sql) {
        return Err("The deck-merge receipt schema does not match this build.".to_string());
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
    fn schema_two_backup_upgrades_additively_to_schema_three() {
        let mut connection = schema_two();
        let before = preservation_receipt(&connection).unwrap();
        let changes = connection.total_changes();
        validate_backup_input(&connection).unwrap();
        assert_eq!(connection.total_changes(), changes);

        assert!(apply_migrations(&mut connection).unwrap());
        assert_eq!(validate_supported_input(&connection).unwrap(), SCHEMA_VERSION);
        validate_input(&connection).unwrap();
        let rows: i64 = connection
            .query_row("SELECT COUNT(*) FROM deck_merge_operations", [], |row| row.get(0))
            .unwrap();
        assert_eq!(rows, 0);

        let comparison = connection.transaction().unwrap();
        comparison.execute_batch("DROP TABLE deck_merge_operations;").unwrap();
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
            transaction.execute_batch(RELEASE_MIGRATIONS[1].sql).unwrap();
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
