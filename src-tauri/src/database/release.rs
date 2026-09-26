//! oStudy 0.1 release-lineage ownership and the one-time public-v1 converter.
//!
//! The release baseline is derived byte-for-byte from the qualified legacy
//! schema-30 result. Existing learner rows therefore do not need a table-by-table
//! rewrite. Conversion first proves the operation on an isolated SQLite backup,
//! including a typed digest of every non-ledger row, then commits only lineage
//! metadata and ledger replacement in one SQLite transaction.

use std::time::Duration;

use rusqlite::{
    backup::Backup, params, types::ValueRef, Connection, OptionalExtension, Transaction,
    TransactionBehavior,
};
use sha2::{Digest, Sha256};

use crate::util::unix_time_millis;

use super::{entry, verify_integrity, LEGACY_PUBLIC_SCHEMA_VERSION};

pub(crate) const SCHEMA_LINEAGE: &str = "ostudy-release-v1";
pub(crate) const SCHEMA_VERSION: i64 = 1;
pub(crate) const BASELINE_NAME: &str = "release_baseline";
pub(crate) const BASELINE_CHECKSUM: &str =
    "64553c98070ad58a0aa73563cf7e9972e9dcb6a27666895a78d8a53882fc26e8";
const BASELINE_SQL: &str = include_str!("../../schema/0001_release_baseline.sql");

pub(super) fn initialize_fresh(connection: &mut Connection) -> Result<(), String> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| format!("Could not begin release-baseline initialization: {error}"))?;
    transaction
        .execute_batch(BASELINE_SQL)
        .map_err(|error| format!("Could not create the oStudy 0.1 release schema: {error}"))?;
    write_release_ledger(&transaction)?;
    validate_input(&transaction)?;
    transaction
        .commit()
        .map_err(|error| format!("Could not commit the oStudy 0.1 release schema: {error}"))?;
    Ok(())
}

/// Read-only admission for an already-current release-lineage database.
pub(super) fn validate_input(connection: &Connection) -> Result<(), String> {
    let lineage: Option<String> = connection
        .query_row(
            "SELECT value FROM schema_metadata WHERE key = 'schema_lineage'",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| format!("Could not read the release schema lineage: {error}"))?;
    if lineage.as_deref() != Some(SCHEMA_LINEAGE) {
        return Err("The release schema lineage does not match this oStudy build.".to_string());
    }
    let rows: i64 = connection
        .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
            row.get(0)
        })
        .map_err(|error| format!("Could not inspect the release migration ledger: {error}"))?;
    if rows != 1 {
        return Err(
            "The release migration ledger must contain exactly one baseline row.".to_string(),
        );
    }
    let recorded: (i64, String, String) = connection
        .query_row(
            "SELECT version, name, checksum FROM schema_migrations",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|error| format!("Could not read the release migration ledger: {error}"))?;
    if recorded
        != (
            SCHEMA_VERSION,
            BASELINE_NAME.to_string(),
            BASELINE_CHECKSUM.to_string(),
        )
    {
        return Err(
            "The release baseline ledger does not match this oStudy build. The database was not modified."
                .to_string(),
        );
    }
    let user_version: i64 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|error| format!("Could not read the release SQLite user_version: {error}"))?;
    if user_version != SCHEMA_VERSION {
        return Err(
            "SQLite user_version and the release baseline ledger disagree. The database was not modified."
                .to_string(),
        );
    }
    let repair_rows: i64 = connection
        .query_row("SELECT COUNT(*) FROM data_repairs", [], |row| row.get(0))
        .map_err(|error| format!("Could not inspect the release repair ledger: {error}"))?;
    if repair_rows != 0 {
        return Err(
            "The release repair ledger contains an unsupported repair. The database was not modified."
                .to_string(),
        );
    }
    entry::validate_current_payload(connection)?;
    Ok(())
}

/// Convert a fully normalized legacy schema-30 connection without rewriting
/// learner-owned rows. The isolated candidate must prove both schema equivalence
/// and a typed value digest before the live metadata transaction can begin.
pub(super) fn convert_legacy_current(connection: &mut Connection) -> Result<(), String> {
    let legacy_version = entry::validate_public_input(connection)?;
    if legacy_version != LEGACY_PUBLIC_SCHEMA_VERSION {
        return Err(format!(
            "Legacy conversion requires public-v1 schema {LEGACY_PUBLIC_SCHEMA_VERSION}, found {legacy_version}."
        ));
    }
    super::verify_repair_ledger(connection)?;
    verify_integrity(connection)?;
    verify_schema_matches_release_baseline(connection)?;
    let before = preservation_receipt(connection)?;

    let mut staged = Connection::open_in_memory()
        .map_err(|error| format!("Could not open the staged release converter: {error}"))?;
    {
        let backup = Backup::new(connection, &mut staged).map_err(|error| {
            format!("Could not stage the legacy library for conversion: {error}")
        })?;
        backup
            .run_to_completion(128, Duration::from_millis(1), None)
            .map_err(|error| format!("Could not copy the legacy library into staging: {error}"))?;
    }
    {
        let transaction = staged
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| format!("Could not begin staged release conversion: {error}"))?;
        write_release_ledger(&transaction)?;
        validate_input(&transaction)?;
        if preservation_receipt(&transaction)? != before {
            return Err(
                "Learner data changed in the staged release conversion. The live database was not modified."
                    .to_string(),
            );
        }
        transaction
            .commit()
            .map_err(|error| format!("Could not commit staged release conversion: {error}"))?;
    }
    verify_integrity(&staged)?;
    verify_schema_matches_release_baseline(&staged)?;

    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| format!("Could not begin release lineage conversion: {error}"))?;
    write_release_ledger(&transaction)?;
    validate_input(&transaction)?;
    if preservation_receipt(&transaction)? != before {
        return Err(
            "Learner data changed during release lineage conversion. The transaction was rolled back."
                .to_string(),
        );
    }
    transaction
        .commit()
        .map_err(|error| format!("Could not commit release lineage conversion: {error}"))?;
    verify_integrity(connection)?;
    Ok(())
}

fn write_release_ledger(transaction: &Transaction<'_>) -> Result<(), String> {
    let changed = transaction
        .execute(
            "UPDATE schema_metadata
             SET value = ?1
             WHERE key = 'schema_lineage'",
            [SCHEMA_LINEAGE],
        )
        .map_err(|error| format!("Could not set the release schema lineage: {error}"))?;
    if changed != 1 {
        return Err("The release schema lineage row is missing or duplicated.".to_string());
    }
    transaction
        .execute("DELETE FROM schema_migrations", [])
        .map_err(|error| format!("Could not replace the legacy migration ledger: {error}"))?;
    transaction
        .execute("DELETE FROM data_repairs", [])
        .map_err(|error| format!("Could not replace the legacy repair ledger: {error}"))?;
    let applied_at = unix_time_millis().map_err(|error| error.message)?;
    transaction
        .execute(
            "INSERT INTO schema_migrations(version, name, checksum, applied_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![SCHEMA_VERSION, BASELINE_NAME, BASELINE_CHECKSUM, applied_at],
        )
        .map_err(|error| format!("Could not record the release baseline: {error}"))?;
    transaction
        .pragma_update(None, "user_version", SCHEMA_VERSION)
        .map_err(|error| format!("Could not stamp the release schema version: {error}"))?;
    Ok(())
}

fn verify_schema_matches_release_baseline(connection: &Connection) -> Result<(), String> {
    let baseline = Connection::open_in_memory()
        .map_err(|error| format!("Could not open the release schema verifier: {error}"))?;
    baseline
        .execute_batch(BASELINE_SQL)
        .map_err(|error| format!("Could not load the release schema verifier: {error}"))?;
    let actual = schema_objects(connection)?;
    let expected = schema_objects(&baseline)?;
    if actual != expected {
        return Err(
            "The legacy schema-30 objects do not match the canonical 0.1 release baseline. The database was not relabelled."
                .to_string(),
        );
    }
    Ok(())
}

fn schema_objects(connection: &Connection) -> Result<Vec<(String, String, String)>, String> {
    let mut statement = connection
        .prepare(
            "SELECT type, name, RTRIM(sql, ';') FROM sqlite_schema
             WHERE sql IS NOT NULL
               AND name NOT LIKE 'sqlite_%'
               AND name NOT GLOB 'card_search_fts_*'
             ORDER BY CASE type
                        WHEN 'table' THEN 0
                        WHEN 'view' THEN 1
                        WHEN 'index' THEN 2
                        WHEN 'trigger' THEN 3
                        ELSE 4
                      END,
                      name",
        )
        .map_err(|error| format!("Could not inspect release schema objects: {error}"))?;
    statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?.trim().to_string(),
            ))
        })
        .map_err(|error| format!("Could not read release schema objects: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Could not decode release schema objects: {error}"))
}

/// Hash every non-ledger SQLite value with explicit type/length framing. Row
/// digests are sorted per table, so physical row order and page layout are not
/// preservation semantics.
fn preservation_receipt(connection: &Connection) -> Result<String, String> {
    let mut table_statement = connection
        .prepare(
            "SELECT name FROM sqlite_schema
             WHERE type = 'table'
               AND name NOT LIKE 'sqlite_%'
               AND name NOT GLOB 'card_search_fts_*'
               AND name NOT IN ('schema_migrations', 'data_repairs')
             ORDER BY name",
        )
        .map_err(|error| format!("Could not enumerate release preservation tables: {error}"))?;
    let mut tables = table_statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|error| format!("Could not read release preservation tables: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Could not decode release preservation tables: {error}"))?;

    let mut aggregate = Sha256::new();
    for table in tables.drain(..) {
        let identifier = quote_identifier(&table);
        let mut statement = connection
            .prepare(&format!("SELECT * FROM {identifier}"))
            .map_err(|error| format!("Could not inspect preservation table {table}: {error}"))?;
        let column_count = statement.column_count();
        let mut rows = statement
            .query([])
            .map_err(|error| format!("Could not read preservation table {table}: {error}"))?;
        let mut row_hashes = Vec::new();
        while let Some(row) = rows
            .next()
            .map_err(|error| format!("Could not advance preservation table {table}: {error}"))?
        {
            if table == "schema_metadata" {
                let key = row
                    .get_ref(0)
                    .map_err(|error| format!("Could not read schema metadata key: {error}"))?;
                if matches!(key, ValueRef::Text(value) if value == b"schema_lineage") {
                    continue;
                }
            }
            let mut row_digest = Sha256::new();
            for index in 0..column_count {
                hash_value(
                    &mut row_digest,
                    row.get_ref(index)
                        .map_err(|error| format!("Could not read preservation value: {error}"))?,
                );
            }
            row_hashes.push(row_digest.finalize().to_vec());
        }
        row_hashes.sort();
        aggregate.update((table.len() as u64).to_be_bytes());
        aggregate.update(table.as_bytes());
        aggregate.update((column_count as u64).to_be_bytes());
        aggregate.update((row_hashes.len() as u64).to_be_bytes());
        for hash in row_hashes {
            aggregate.update(hash);
        }
    }
    Ok(format!("{:x}", aggregate.finalize()))
}

fn hash_value(digest: &mut Sha256, value: ValueRef<'_>) {
    match value {
        ValueRef::Null => digest.update([0]),
        ValueRef::Integer(value) => {
            digest.update([1]);
            digest.update(value.to_be_bytes());
        }
        ValueRef::Real(value) => {
            digest.update([2]);
            digest.update(value.to_bits().to_be_bytes());
        }
        ValueRef::Text(value) => {
            digest.update([3]);
            digest.update((value.len() as u64).to_be_bytes());
            digest.update(value);
        }
        ValueRef::Blob(value) => {
            digest.update([4]);
            digest.update((value.len() as u64).to_be_bytes());
            digest.update(value);
        }
    }
}

fn quote_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_release_uses_one_baseline_and_no_historical_repairs() {
        let mut connection = Connection::open_in_memory().unwrap();
        initialize_fresh(&mut connection).unwrap();
        validate_input(&connection).unwrap();
        let migrations: i64 = connection
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .unwrap();
        let repairs: i64 = connection
            .query_row("SELECT COUNT(*) FROM data_repairs", [], |row| row.get(0))
            .unwrap();
        assert_eq!(migrations, 1);
        assert_eq!(repairs, 0);
        assert_eq!(super::super::entry::user_version(&connection).unwrap(), 1);
    }

    #[test]
    fn schema30_conversion_preserves_all_nonledger_values() {
        let mut connection = super::super::public_baseline_connection().unwrap();
        for migration in &super::super::MIGRATIONS[1..] {
            super::super::apply_transactional_migration(&mut connection, migration).unwrap();
        }
        super::super::apply_required_repairs(
            &mut connection,
            super::super::RepairPhase::AfterMigration(LEGACY_PUBLIC_SCHEMA_VERSION),
        )
        .unwrap();
        connection
            .execute_batch(
                "INSERT INTO decks(id, name, created_at, updated_at)
                 VALUES ('11111111111111111111111111111111', 'Preserved', 10, 20);
                 INSERT INTO app_settings(key, value_json, updated_at)
                 VALUES ('release-converter-sentinel', '{\"answer\":42}', 30);",
            )
            .unwrap();
        let before = preservation_receipt(&connection).unwrap();
        convert_legacy_current(&mut connection).unwrap();
        assert_eq!(preservation_receipt(&connection).unwrap(), before);
        validate_input(&connection).unwrap();
        let deck: (String, i64, i64) = connection
            .query_row(
                "SELECT name, created_at, updated_at FROM decks
                 WHERE id = '11111111111111111111111111111111'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(deck, ("Preserved".to_string(), 10, 20));
    }
}
