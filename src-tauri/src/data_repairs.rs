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

const REPAIRS: &[DataRepair] = &[];

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
