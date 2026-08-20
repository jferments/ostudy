//! Versioned public data repairs with explicit migration prerequisites.
//!
//! The pre-release repair implementations were folded into the public baseline.
//! Future repairs remain independently versioned, checksum-protected, and
//! transactional rather than being hidden inside schema SQL.

use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};

use crate::util::unix_time_millis;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RepairPhase {
    AfterMigration(i64),
}

struct DataRepair {
    version: i64,
    name: &'static str,
    checksum: &'static str,
    phase: RepairPhase,
    apply: fn(&Transaction<'_>) -> Result<i64, String>,
}

const REPAIRS: &[DataRepair] = &[DataRepair {
    version: 1,
    name: "remove_deck_view_density_setting",
    checksum: "0aa10944af1956a70bd0c4dc4abb524948d6501fe556a90870fc770a7bd0e030",
    phase: RepairPhase::AfterMigration(1),
    apply: remove_deck_view_density_setting,
}];

fn remove_deck_view_density_setting(transaction: &Transaction<'_>) -> Result<i64, String> {
    let changed = transaction
        .execute("DELETE FROM app_settings WHERE key = 'deckViewDensity'", [])
        .map_err(|error| format!("Could not remove the obsolete deck density setting: {error}"))?;
    i64::try_from(changed)
        .map_err(|_| "The obsolete deck density repair changed too many rows.".to_string())
}

pub(crate) fn apply_required_repairs(
    connection: &mut Connection,
    phase: RepairPhase,
) -> Result<(), String> {
    ensure_ledger_exists(connection)?;
    for repair in REPAIRS.iter().filter(|repair| repair.phase == phase) {
        let existing = recorded_repair(connection, repair.version)?;
        if let Some((name, checksum)) = existing {
            if name != repair.name || checksum != repair.checksum {
                return Err(format!(
                    "Data repair {} does not match this application build. The database was not modified.",
                    repair.version
                ));
            }
            continue;
        }
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| format!("Could not start data repair {}: {error}", repair.version))?;
        let changed_rows = (repair.apply)(&transaction)?;
        if changed_rows <= 0 {
            transaction.rollback().map_err(|error| {
                format!(
                    "Could not close unnecessary data repair {}: {error}",
                    repair.version
                )
            })?;
            continue;
        }
        let applied_at = unix_time_millis().map_err(|error| error.message)?;
        transaction
            .execute(
                "INSERT INTO data_repairs (version, name, checksum, applied_at, changed_rows)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    repair.version,
                    repair.name,
                    repair.checksum,
                    applied_at,
                    changed_rows.max(0)
                ],
            )
            .map_err(|error| format!("Could not record data repair {}: {error}", repair.version))?;
        transaction
            .commit()
            .map_err(|error| format!("Could not commit data repair {}: {error}", repair.version))?;
    }
    Ok(())
}

pub(crate) fn verify_repair_ledger(connection: &Connection) -> Result<(), String> {
    ensure_ledger_exists(connection)?;
    let mut statement = connection
        .prepare("SELECT version, name, checksum FROM data_repairs ORDER BY version")
        .map_err(|error| format!("Could not inspect the data repair ledger: {error}"))?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|error| format!("Could not read the data repair ledger: {error}"))?;
    for row in rows {
        let (version, name, checksum) =
            row.map_err(|error| format!("Could not decode a data repair row: {error}"))?;
        let expected = REPAIRS.iter().find(|repair| repair.version == version);
        if expected.map(|repair| (repair.name, repair.checksum))
            != Some((name.as_str(), checksum.as_str()))
        {
            return Err(format!(
                "Data repair {version} is not recognized by this application build."
            ));
        }
    }
    Ok(())
}

fn recorded_repair(
    connection: &Connection,
    version: i64,
) -> Result<Option<(String, String)>, String> {
    connection
        .query_row(
            "SELECT name, checksum FROM data_repairs WHERE version = ?1",
            [version],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|error| format!("Could not inspect data repair {version}: {error}"))
}

fn ensure_ledger_exists(connection: &Connection) -> Result<(), String> {
    let exists: i64 = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = 'data_repairs')",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("Could not inspect the data repair ledger: {error}"))?;
    if exists == 0 {
        return Err(
            "The data repair ledger is missing from the public schema baseline.".to_string(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repair_database() -> Connection {
        let connection = Connection::open_in_memory().expect("repair database should open");
        connection
            .execute_batch(
                "CREATE TABLE app_settings (
                    key TEXT PRIMARY KEY,
                    value_json TEXT NOT NULL,
                    updated_at INTEGER NOT NULL
                 );
                 CREATE TABLE data_repairs (
                    version INTEGER PRIMARY KEY,
                    name TEXT NOT NULL,
                    checksum TEXT NOT NULL,
                    applied_at INTEGER NOT NULL,
                    changed_rows INTEGER NOT NULL
                 );
                 INSERT INTO app_settings (key, value_json, updated_at)
                 VALUES ('deckViewDensity', '\"comfortable\"', 1);",
            )
            .expect("repair fixture should initialize");
        connection
    }

    #[test]
    fn versioned_repair_removes_the_obsolete_density_key() {
        let mut connection = repair_database();
        apply_required_repairs(&mut connection, RepairPhase::AfterMigration(1))
            .expect("repair should apply");
        let remaining: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM app_settings WHERE key = 'deckViewDensity'",
                [],
                |row| row.get(0),
            )
            .expect("obsolete setting count should load");
        let changed_rows: i64 = connection
            .query_row(
                "SELECT changed_rows FROM data_repairs WHERE version = 1",
                [],
                |row| row.get(0),
            )
            .expect("repair ledger row should load");
        assert_eq!(remaining, 0);
        assert_eq!(changed_rows, 1);
        verify_repair_ledger(&connection).expect("repair ledger should verify");
    }

    #[test]
    fn unnecessary_repair_does_not_mutate_or_record_the_public_baseline() {
        let mut connection = repair_database();
        connection
            .execute("DELETE FROM app_settings WHERE key = 'deckViewDensity'", [])
            .expect("obsolete key should be removable from the fixture");
        let before = connection.total_changes();

        apply_required_repairs(&mut connection, RepairPhase::AfterMigration(1))
            .expect("unnecessary repair should be harmless");

        assert_eq!(connection.total_changes(), before);
        let ledger_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM data_repairs", [], |row| row.get(0))
            .expect("repair ledger count should load");
        assert_eq!(ledger_count, 0);
    }

    #[test]
    fn versioned_repair_is_idempotent() {
        let mut connection = repair_database();
        apply_required_repairs(&mut connection, RepairPhase::AfterMigration(1))
            .expect("repair should apply");
        let before = connection.total_changes();
        apply_required_repairs(&mut connection, RepairPhase::AfterMigration(1))
            .expect("recorded repair should be skipped");
        assert_eq!(connection.total_changes(), before);
    }
}
