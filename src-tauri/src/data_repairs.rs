//! One-time, versioned data canonicalization for released ostudy databases.
//!
//! Structural migrations remain immutable SQL history. Repairs use the same
//! validated Rust transformations as normal authoring. Once every repair row
//! exists, ordinary startup performs reads only.

use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};

use crate::{
    authoring::{apply_stage3_data_repairs, backfill_duplicate_hashes_in_connection},
    historical_attribution::backfill_historical_attribution,
    util::unix_time_millis,
};

struct DataRepair {
    version: i64,
    name: &'static str,
    checksum: &'static str,
    apply: fn(&Transaction<'_>) -> Result<i64, String>,
}

const REPAIRS: &[DataRepair] = &[
    DataRepair {
        version: 1,
        name: "duplicate_fingerprints",
        checksum: "fb2f07be41d479048759ac7d754a59022f2c63c3f60082eaf9a40fd28bab7993",
        apply: repair_duplicate_fingerprints,
    },
    DataRepair {
        version: 2,
        name: "template_and_generated_card_canonicalization",
        checksum: "ec486bad97c8c5495755bae160bf019d61c084444cc09d88ba8e1f0135b3e663",
        apply: repair_stage3_data,
    },
    DataRepair {
        version: 3,
        name: "review_attribution_backfill",
        checksum: "05b5e6894736d84ebe60efc0ff399ae39c9c612c4cc8c9a2316f3622fd80c03d",
        apply: repair_review_attribution,
    },
];

pub(crate) fn apply_required_repairs_through(
    connection: &mut Connection,
    maximum_version: i64,
) -> Result<(), String> {
    apply_repair_range(connection, 1, maximum_version)
}

pub(crate) fn apply_required_repairs_after(
    connection: &mut Connection,
    minimum_exclusive: i64,
) -> Result<(), String> {
    apply_repair_range(connection, minimum_exclusive.saturating_add(1), i64::MAX)
}

pub(crate) fn apply_required_repairs(connection: &mut Connection) -> Result<(), String> {
    apply_repair_range(connection, 1, i64::MAX)
}

fn apply_repair_range(
    connection: &mut Connection,
    minimum_version: i64,
    maximum_version: i64,
) -> Result<(), String> {
    ensure_ledger_exists(connection)?;
    for repair in REPAIRS
        .iter()
        .filter(|repair| repair.version >= minimum_version && repair.version <= maximum_version)
    {
        let existing: Option<(String, String)> = connection
            .query_row(
                "SELECT name, checksum FROM data_repairs WHERE version = ?1",
                [repair.version],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|error| {
                format!("Could not inspect data repair {}: {error}", repair.version)
            })?;
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
                "INSERT INTO data_repairs (
                     version, name, checksum, applied_at, changed_rows
                 ) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    repair.version,
                    repair.name,
                    repair.checksum,
                    applied_at,
                    changed_rows.max(0),
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
    for repair in REPAIRS {
        let recorded: Option<(String, String)> = connection
            .query_row(
                "SELECT name, checksum FROM data_repairs WHERE version = ?1",
                [repair.version],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|error| format!("Could not verify data repair {}: {error}", repair.version))?;
        if recorded
            .as_ref()
            .map(|row| (row.0.as_str(), row.1.as_str()))
            != Some((repair.name, repair.checksum))
        {
            return Err(format!(
                "Required data repair {} is missing or does not match this application build.",
                repair.version
            ));
        }
    }
    Ok(())
}

fn ensure_ledger_exists(connection: &Connection) -> Result<(), String> {
    let exists: i64 = connection
        .query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM sqlite_master
                 WHERE type = 'table' AND name = 'data_repairs'
             )",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("Could not inspect the data repair ledger: {error}"))?;
    if exists == 0 {
        return Err("The data repair ledger migration has not been applied.".to_string());
    }
    Ok(())
}

fn repair_duplicate_fingerprints(transaction: &Transaction<'_>) -> Result<i64, String> {
    backfill_duplicate_hashes_in_connection(transaction)
}

fn repair_stage3_data(transaction: &Transaction<'_>) -> Result<i64, String> {
    apply_stage3_data_repairs(transaction).map_err(|error| error.message)
}

fn repair_review_attribution(transaction: &Transaction<'_>) -> Result<i64, String> {
    backfill_historical_attribution(transaction).map_err(|error| error.message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{database::Database, error::CommandError};

    #[test]
    fn canonical_database_repair_pass_is_a_no_op() {
        let database = Database::open_in_memory().expect("database should open");
        database
            .with_connection(|connection| {
                let before = connection.total_changes();
                apply_required_repairs(connection)
                    .map_err(|error| CommandError::new("repair_failed", error))?;
                let after = connection.total_changes();
                assert_eq!(before, after);
                verify_repair_ledger(connection)
                    .map_err(|error| CommandError::new("repair_failed", error))?;
                Ok(())
            })
            .expect("second repair pass should not write");
    }
}
