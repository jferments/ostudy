//! oStudy 0.1 release-lineage ownership and the one-time public-v1 converter.
//!
//! The release baseline is derived byte-for-byte from the qualified legacy
//! schema-30 result. Existing learner rows therefore do not need a table-by-table
//! rewrite. Conversion first proves the operation on an isolated SQLite backup,
//! including a typed digest of every non-ledger row, then commits only lineage
//! metadata and ledger replacement in one SQLite transaction.

use rusqlite::{
    params, types::ValueRef, Connection, OptionalExtension, Transaction, TransactionBehavior,
};
use sha2::{Digest, Sha256};

use crate::util::unix_time_millis;

use super::{entry, verify_integrity, LEGACY_PUBLIC_SCHEMA_VERSION};

#[path = "release/deck_merge.rs"]
mod deck_merge;

#[path = "release/staging.rs"]
mod staging;

pub(crate) const SCHEMA_LINEAGE: &str = "ostudy-release-v1";
pub(crate) const BASELINE_VERSION: i64 = 1;
pub(crate) const SCHEMA_VERSION: i64 = 3;
pub(crate) const BASELINE_NAME: &str = "release_baseline";
pub(crate) const BASELINE_CHECKSUM: &str =
    "64553c98070ad58a0aa73563cf7e9972e9dcb6a27666895a78d8a53882fc26e8";
const BASELINE_SQL: &str = include_str!("../../schema/0001_release_baseline.sql");

struct ReleaseMigration {
    version: i64,
    name: &'static str,
    checksum: &'static str,
    sql: &'static str,
}

const RELEASE_MIGRATIONS: &[ReleaseMigration] = &[
    ReleaseMigration {
        version: 2,
        name: "selection_deck_operations",
        checksum: "c854f5d50338beec395399d723038fdfb6d4436af3305718d5a1c6a70ca65238",
        sql: include_str!("../../schema/0002_selection_deck_operations.sql"),
    },
    ReleaseMigration {
        version: 3,
        name: "deck_merge_operations",
        checksum: "f6b820ff03ba890d4c4b0f0fd434aaa6725d4c6ddf4f43e1a334755bccca2edc",
        sql: include_str!("../../schema/0003_deck_merge_operations.sql"),
    },
];

pub(super) fn migration_contract_rows() -> impl Iterator<Item = (i64, &'static str, &'static str)> {
    RELEASE_MIGRATIONS
        .iter()
        .map(|migration| (migration.version, migration.name, migration.checksum))
}

pub(super) fn initialize_fresh(connection: &mut Connection) -> Result<(), String> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| format!("Could not begin release-baseline initialization: {error}"))?;
    transaction
        .execute_batch(BASELINE_SQL)
        .map_err(|error| format!("Could not create the oStudy 0.1 release schema: {error}"))?;
    write_release_ledger(&transaction)?;
    validate_baseline_input(&transaction)?;
    transaction
        .commit()
        .map_err(|error| format!("Could not commit the oStudy 0.1 release schema: {error}"))?;
    apply_migrations(connection)?;
    validate_input(connection)
}

/// Read-only admission for an already-current release-lineage database.
pub(super) fn validate_input(connection: &Connection) -> Result<(), String> {
    let version = validate_supported_input(connection)?;
    if version != SCHEMA_VERSION {
        return Err(format!(
            "This release database is schema {version}; schema {SCHEMA_VERSION} is required after migration."
        ));
    }
    Ok(())
}

/// Backup admission accepts every immutable release-v1 version this build knows
/// how to migrate. It never applies a migration to the read-only input.
pub(super) fn validate_backup_input(connection: &Connection) -> Result<(), String> {
    validate_supported_input(connection).map(|_| ())
}

pub(super) fn validate_supported_input(connection: &Connection) -> Result<i64, String> {
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
    let user_version: i64 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|error| format!("Could not read the release SQLite user_version: {error}"))?;
    if !(BASELINE_VERSION..=SCHEMA_VERSION).contains(&user_version) {
        return Err(format!(
            "Release schema {user_version} is not supported by this oStudy build."
        ));
    }
    let rows: i64 = connection
        .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
            row.get(0)
        })
        .map_err(|error| format!("Could not inspect the release migration ledger: {error}"))?;
    if rows != user_version {
        return Err(
            "The release migration ledger does not match SQLite user_version. The database was not modified."
                .to_string(),
        );
    }
    let baseline: (i64, String, String) = connection
        .query_row(
            "SELECT version, name, checksum FROM schema_migrations WHERE version = ?1",
            [BASELINE_VERSION],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|error| format!("Could not read the release baseline ledger: {error}"))?;
    if baseline
        != (
            BASELINE_VERSION,
            BASELINE_NAME.to_string(),
            BASELINE_CHECKSUM.to_string(),
        )
    {
        return Err(
            "The release baseline ledger does not match this oStudy build. The database was not modified."
                .to_string(),
        );
    }
    for migration in RELEASE_MIGRATIONS {
        let recorded: Option<(String, String)> = connection
            .query_row(
                "SELECT name, checksum FROM schema_migrations WHERE version = ?1",
                [migration.version],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|error| {
                format!(
                    "Could not inspect release migration {}: {error}",
                    migration.version
                )
            })?;
        if migration.version <= user_version {
            let Some((name, checksum)) = recorded else {
                return Err(format!(
                    "Release migration {} is missing. The database was not modified.",
                    migration.version
                ));
            };
            if name != migration.name || checksum != migration.checksum {
                return Err(format!(
                    "Release migration {} does not match this oStudy build. The database was not modified.",
                    migration.version
                ));
            }
        } else if recorded.is_some() {
            return Err(
                "The release migration ledger contains a migration beyond SQLite user_version."
                    .to_string(),
            );
        }
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
    validate_selection_receipt_schema(connection, user_version)?;
    deck_merge::validate_schema(connection, user_version)?;
    entry::validate_current_payload(connection)?;
    Ok(user_version)
}

fn validate_selection_receipt_schema(connection: &Connection, version: i64) -> Result<(), String> {
    let actual: Option<String> = connection
        .query_row(
            "SELECT sql FROM sqlite_schema WHERE type='table' AND name='selection_deck_operations'",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    if version == BASELINE_VERSION {
        if actual.is_some() {
            return Err("An unregistered selection-deck migration is present.".to_string());
        }
        return Ok(());
    }
    let actual =
        actual.ok_or_else(|| "The selection-deck receipt table is missing.".to_string())?;
    let expected = Connection::open_in_memory().map_err(|error| error.to_string())?;
    expected
        .execute_batch(RELEASE_MIGRATIONS[0].sql)
        .map_err(|error| error.to_string())?;
    let sql: String = expected
        .query_row(
            "SELECT sql FROM sqlite_schema WHERE name='selection_deck_operations'",
            [],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    if normalize_schema_sql(&actual) != normalize_schema_sql(&sql) {
        return Err("The selection-deck receipt schema does not match this build.".to_string());
    }
    crate::authoring::selection_deck::validate_stored_receipts(connection)
}

fn validate_baseline_input(connection: &Connection) -> Result<(), String> {
    let version = validate_supported_input(connection)?;
    if version != BASELINE_VERSION {
        return Err("The staged release baseline has an unexpected schema version.".to_string());
    }
    Ok(())
}

pub(super) fn apply_migrations(connection: &mut Connection) -> Result<bool, String> {
    let current = validate_supported_input(connection)?;
    let mut applied = false;
    for migration in RELEASE_MIGRATIONS {
        if migration.version <= current {
            continue;
        }
        if format!("{:x}", Sha256::digest(migration.sql.as_bytes())) != migration.checksum {
            return Err(format!(
                "Bundled release migration {} checksum mismatch.",
                migration.version
            ));
        }
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| {
                format!(
                    "Could not begin release migration {}: {error}",
                    migration.version
                )
            })?;
        transaction.execute_batch(migration.sql).map_err(|error| {
            format!(
                "Could not apply release migration {}: {error}",
                migration.version
            )
        })?;
        let applied_at = unix_time_millis().map_err(|error| error.message)?;
        transaction
            .execute(
                "INSERT INTO schema_migrations(version, name, checksum, applied_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    migration.version,
                    migration.name,
                    migration.checksum,
                    applied_at
                ],
            )
            .map_err(|error| {
                format!(
                    "Could not record release migration {}: {error}",
                    migration.version
                )
            })?;
        transaction
            .pragma_update(None, "user_version", migration.version)
            .map_err(|error| {
                format!(
                    "Could not set release SQLite user_version to {}: {error}",
                    migration.version
                )
            })?;
        validate_supported_input(&transaction)?;
        verify_integrity(&transaction)?;
        transaction.commit().map_err(|error| {
            format!(
                "Could not commit release migration {}: {error}",
                migration.version
            )
        })?;
        applied = true;
    }
    validate_input(connection)?;
    Ok(applied)
}

/// Convert a fully normalized legacy schema-30 connection without rewriting
/// learner-owned rows. The isolated candidate must prove both schema equivalence
/// and a typed value digest before the live metadata transaction can begin.
pub(super) fn convert_legacy_current(connection: &mut Connection) -> Result<(), String> {
    convert_legacy_current_with(connection, staging::Candidate::copy_from)
}

fn convert_legacy_current_with<F>(connection: &mut Connection, stage: F) -> Result<(), String>
where
    F: FnOnce(&Connection) -> Result<staging::Candidate, String>,
{
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

    let mut candidate = stage(connection)?;
    crate::logging::info_target(
        "database.migration",
        "release_conversion_staged",
        "A private disk-backed release conversion candidate is ready for validation.",
        serde_json::json!({
            "estimatedSourceBytes": candidate.estimated_bytes(),
            "diskBacked": true,
        }),
    );
    {
        let staged = candidate.connection_mut();
        let transaction = staged
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| format!("Could not begin staged release conversion: {error}"))?;
        write_release_ledger(&transaction)?;
        validate_baseline_input(&transaction)?;
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
    verify_integrity(candidate.connection())?;
    verify_schema_matches_release_baseline(candidate.connection())?;
    apply_migrations(candidate.connection_mut())?;
    validate_input(candidate.connection())?;

    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| format!("Could not begin release lineage conversion: {error}"))?;
    write_release_ledger(&transaction)?;
    validate_baseline_input(&transaction)?;
    if preservation_receipt(&transaction)? != before {
        return Err(
            "Learner data changed during release lineage conversion. The transaction was rolled back."
                .to_string(),
        );
    }
    transaction
        .commit()
        .map_err(|error| format!("Could not commit release lineage conversion: {error}"))?;
    apply_migrations(connection)?;
    verify_integrity(connection)?;
    Ok(())
}

fn write_release_ledger(transaction: &Transaction<'_>) -> Result<(), String> {
    transaction
        .execute(
            "INSERT INTO schema_metadata(key, value)
             VALUES ('schema_lineage', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [SCHEMA_LINEAGE],
        )
        .map_err(|error| format!("Could not set the release schema lineage: {error}"))?;
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
            params![
                BASELINE_VERSION,
                BASELINE_NAME,
                BASELINE_CHECKSUM,
                applied_at
            ],
        )
        .map_err(|error| format!("Could not record the release baseline: {error}"))?;
    transaction
        .pragma_update(None, "user_version", BASELINE_VERSION)
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
    let objects = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                normalize_schema_sql(&row.get::<_, String>(2)?),
            ))
        })
        .map_err(|error| format!("Could not read release schema objects: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Could not decode release schema objects: {error}"))?;
    Ok(objects)
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

fn normalize_schema_sql(sql: &str) -> String {
    let mut normalized = String::with_capacity(sql.len());
    let mut characters = sql.chars().peekable();
    let mut quote: Option<char> = None;
    let mut in_brackets = false;
    let mut pending_space = false;

    while let Some(character) = characters.next() {
        if let Some(delimiter) = quote {
            normalized.push(character);
            if character == delimiter {
                if characters.peek().copied() == Some(delimiter) {
                    normalized.push(characters.next().expect("peeked quote should exist"));
                } else {
                    quote = None;
                }
            }
            continue;
        }

        if in_brackets {
            normalized.push(character);
            if character == ']' {
                in_brackets = false;
            }
            continue;
        }

        match character {
            '\'' | '"' | '`' => {
                if pending_space && !normalized.is_empty() {
                    normalized.push(' ');
                }
                pending_space = false;
                normalized.push(character);
                quote = Some(character);
            }
            '[' => {
                if pending_space && !normalized.is_empty() {
                    normalized.push(' ');
                }
                pending_space = false;
                normalized.push(character);
                in_brackets = true;
            }
            character if character.is_whitespace() => pending_space = true,
            _ => {
                if pending_space && !normalized.is_empty() {
                    normalized.push(' ');
                }
                pending_space = false;
                normalized.push(character);
            }
        }
    }

    normalized.trim().to_string()
}

fn quote_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_migration_checksums_match_committed_sql() {
        for migration in RELEASE_MIGRATIONS {
            assert_eq!(
                format!("{:x}", Sha256::digest(migration.sql.as_bytes())),
                migration.checksum,
                "release migration {} checksum changed",
                migration.version
            );
        }
    }

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
        assert_eq!(migrations, SCHEMA_VERSION);
        assert_eq!(repairs, 0);
        assert_eq!(
            super::super::entry::user_version(&connection).unwrap(),
            SCHEMA_VERSION
        );
        let receipt_table: i64 = connection
            .query_row(
                "SELECT EXISTS(
                     SELECT 1 FROM sqlite_schema
                     WHERE type = 'table' AND name = 'selection_deck_operations'
                 )",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(receipt_table, 1);
        let merge_receipt_table: i64 = connection
            .query_row(
                "SELECT EXISTS(
                     SELECT 1 FROM sqlite_schema
                     WHERE type = 'table' AND name = 'deck_merge_operations'
                 )",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(merge_receipt_table, 1);
    }

    #[test]
    fn schema_one_is_readable_and_migrates_forward() {
        let mut connection = Connection::open_in_memory().unwrap();
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .unwrap();
        transaction.execute_batch(BASELINE_SQL).unwrap();
        write_release_ledger(&transaction).unwrap();
        transaction.commit().unwrap();

        assert_eq!(
            validate_supported_input(&connection).unwrap(),
            BASELINE_VERSION
        );
        validate_backup_input(&connection).unwrap();
        assert!(validate_input(&connection).is_err());
        assert!(apply_migrations(&mut connection).unwrap());
        validate_input(&connection).unwrap();
        assert_eq!(
            super::super::entry::user_version(&connection).unwrap(),
            SCHEMA_VERSION
        );
    }

    #[test]
    fn malformed_release_ledgers_and_receipts_are_rejected_without_writes() {
        let mut connection = Connection::open_in_memory().unwrap();
        initialize_fresh(&mut connection).unwrap();
        for corruption in [
            "UPDATE schema_migrations SET checksum='bad' WHERE version=2;",
            "PRAGMA user_version=3;",
            "DELETE FROM schema_migrations WHERE version=1;",
            "DROP TABLE selection_deck_operations;",
            "INSERT INTO selection_deck_operations VALUES('aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb', '{}', 1);",
        ] {
            let transaction = connection.transaction().unwrap();
            transaction.execute_batch(corruption).unwrap();
            let changes = transaction.total_changes();
            assert!(validate_backup_input(&transaction).is_err(), "{corruption}");
            assert_eq!(transaction.total_changes(), changes);
            transaction.rollback().unwrap();
        }
        validate_input(&connection).unwrap();
    }

    #[test]
    fn migration_failure_rolls_back_ddl_ledger_and_version_then_retries_cleanly() {
        let mut connection = Connection::open_in_memory().unwrap();
        let transaction = connection.transaction().unwrap();
        transaction.execute_batch(BASELINE_SQL).unwrap();
        write_release_ledger(&transaction).unwrap();
        transaction.commit().unwrap();
        connection
            .execute_batch(
                "CREATE TEMP TRIGGER fail_release_migration BEFORE INSERT ON schema_migrations
             WHEN NEW.version=2 BEGIN SELECT RAISE(ABORT,'injected migration failure'); END;",
            )
            .unwrap();
        assert!(apply_migrations(&mut connection).is_err());
        assert_eq!(
            validate_supported_input(&connection).unwrap(),
            BASELINE_VERSION
        );
        connection
            .execute_batch("DROP TRIGGER fail_release_migration;")
            .unwrap();
        assert!(apply_migrations(&mut connection).unwrap());
        let changes = connection.total_changes();
        assert!(!apply_migrations(&mut connection).unwrap());
        assert_eq!(connection.total_changes(), changes);
        validate_backup_input(&connection).unwrap();
    }

    #[test]
    fn schema_equivalence_ignores_formatting_only_differences() {
        let canonical =
            "CREATE TABLE \"example\" ( value TEXT NOT NULL CHECK(value IN ('a  b', 'c')) ) STRICT";
        let reformatted = "CREATE   TABLE \"example\" (
            value TEXT NOT NULL
            CHECK(value IN ('a  b', 'c'))
        ) STRICT";
        let changed_literal =
            "CREATE TABLE \"example\" ( value TEXT NOT NULL CHECK(value IN ('a b', 'c')) ) STRICT";
        assert_eq!(
            normalize_schema_sql(canonical),
            normalize_schema_sql(reformatted)
        );
        assert_ne!(
            normalize_schema_sql(canonical),
            normalize_schema_sql(changed_literal)
        );
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
        validate_input(&connection).unwrap();
        for table in [
            "selection_deck_operations",
            "deck_merge_operations",
            "study_target_generation_instructions",
        ] {
            assert_eq!(
                connection
                    .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| row
                        .get::<_, i64>(0))
                    .unwrap(),
                0
            );
        }
        // Release-only operation-intent/receipt tables are additive and empty.
        // Compare every pre-existing typed value after excluding only them.
        let comparison = connection.transaction().unwrap();
        comparison
            .execute_batch(
                "DROP TABLE study_target_generation_instructions;
                 DROP TABLE deck_merge_operations;
                 DROP TABLE selection_deck_operations;",
            )
            .unwrap();
        assert_eq!(preservation_receipt(&comparison).unwrap(), before);
        comparison.rollback().unwrap();
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

#[cfg(test)]
#[path = "release/h09_tests.rs"]
mod h09_tests;
