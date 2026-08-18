use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use rusqlite::{backup::Backup, params, Connection, OptionalExtension, TransactionBehavior};
use serde::Deserialize;
use serde_json::json;

use crate::util::unix_time_millis;

const PUBLIC_SCHEMA_LINEAGE: &str = "ostudy-public-v1";
const PUBLIC_BASELINE_VERSION: i64 = 1;
const PUBLIC_BASELINE_NAME: &str = "public_baseline";
const PUBLIC_BASELINE_CHECKSUM: &str =
    "2d978f8be026ecb820e71ba77a8585e1de51cb08fc2820714526a33284167f85";
const PRE_PUBLIC_CONTRACT_JSON: &str = include_str!("pre_public_v26_ledger.json");

const REBUILD_AI_PROFILES_SQL: &str = r#"
CREATE TABLE ai_provider_profiles_public (
    id                      TEXT PRIMARY KEY CHECK(length(id) = 32),
    display_name            TEXT NOT NULL CHECK(length(trim(display_name)) BETWEEN 1 AND 100),
    normalized_name         TEXT NOT NULL CHECK(length(normalized_name) BETWEEN 1 AND 100),
    provider_kind           TEXT NOT NULL CHECK(length(provider_kind) BETWEEN 1 AND 80),
    base_url                TEXT NOT NULL CHECK(length(base_url) BETWEEN 1 AND 2048),
    model_id                TEXT NOT NULL DEFAULT '' CHECK(length(model_id) <= 300),
    auth_mode               TEXT NOT NULL CHECK(auth_mode IN ('none', 'bearer')),
    enabled                 INTEGER NOT NULL DEFAULT 1 CHECK(enabled IN (0, 1)),
    allow_insecure_http     INTEGER NOT NULL DEFAULT 0 CHECK(allow_insecure_http IN (0, 1)),
    require_zero_data_retention INTEGER NOT NULL DEFAULT 0
                                CHECK(require_zero_data_retention IN (0, 1)),
    request_timeout_ms      INTEGER NOT NULL DEFAULT 60000
                            CHECK(request_timeout_ms BETWEEN 1000 AND 300000),
    credential_storage      TEXT NOT NULL DEFAULT 'none'
                            CHECK(credential_storage IN ('none', 'secure', 'session')),
    credential_epoch        INTEGER NOT NULL DEFAULT 0 CHECK(credential_epoch >= 0),
    revision                INTEGER NOT NULL DEFAULT 1 CHECK(revision >= 1),
    created_at              INTEGER NOT NULL,
    updated_at              INTEGER NOT NULL,
    deleted_at              INTEGER
) STRICT;

INSERT INTO ai_provider_profiles_public (
    id, display_name, normalized_name, provider_kind, base_url, model_id,
    auth_mode, enabled, allow_insecure_http, require_zero_data_retention,
    request_timeout_ms, credential_storage, credential_epoch, revision,
    created_at, updated_at, deleted_at
)
SELECT
    id, display_name, normalized_name, provider_kind, base_url, model_id,
    auth_mode, enabled, allow_insecure_http, 0,
    request_timeout_ms, credential_storage, credential_epoch, revision,
    created_at, updated_at, deleted_at
FROM ai_provider_profiles;

DROP TABLE ai_provider_profiles;
ALTER TABLE ai_provider_profiles_public RENAME TO ai_provider_profiles;

CREATE UNIQUE INDEX ai_provider_profiles_active_name_idx
    ON ai_provider_profiles(normalized_name)
    WHERE deleted_at IS NULL;

CREATE INDEX ai_provider_profiles_active_updated_idx
    ON ai_provider_profiles(updated_at DESC)
    WHERE deleted_at IS NULL;

CREATE TABLE schema_metadata (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
) STRICT;

INSERT INTO schema_metadata(key, value)
VALUES ('schema_lineage', 'ostudy-public-v1');

DELETE FROM schema_migrations;
DELETE FROM data_repairs;
"#;

#[derive(Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
struct PrePublicContract {
    user_version: i64,
    migrations: Vec<LedgerEntry>,
    repairs: Vec<LedgerEntry>,
}

#[derive(Debug, Deserialize, Eq, PartialEq)]
struct LedgerEntry {
    version: i64,
    name: String,
    checksum: String,
}

#[derive(Debug, Eq, PartialEq)]
struct PreservedDataSnapshot {
    table_counts: BTreeMap<String, i64>,
    deck_ids: Vec<String>,
    note_ids: Vec<String>,
    card_ids: Vec<String>,
    review_ids: Vec<String>,
    review_attribution_ids: Vec<String>,
    ai_profile_ids: Vec<String>,
}

impl PreservedDataSnapshot {
    fn count(&self, table: &str) -> i64 {
        self.table_counts.get(table).copied().unwrap_or(0)
    }
}

#[derive(Debug)]
pub(super) struct AdoptionReport {
    pub backup_path: PathBuf,
    pub deck_count: i64,
    pub card_count: i64,
    pub review_count: i64,
}

pub(super) fn adopt_final_pre_public_database(
    database_path: &Path,
) -> Result<Option<AdoptionReport>, String> {
    let contract = pre_public_contract()?;
    let mut connection = Connection::open(database_path)
        .map_err(|error| format!("Could not reopen the pre-public database: {error}"))?;
    if !is_exact_final_pre_public_database(&connection, &contract)? {
        return Ok(None);
    }

    verify_integrity(&connection)?;
    let before = preserved_data_snapshot(&connection)?;
    let backup_path = create_database_backup(&connection, database_path, contract.user_version)?;

    crate::logging::info_target(
        "database.migration",
        "pre_public_database_adoption_started",
        "The final pre-public database is being adopted into the public schema lineage.",
        json!({
            "legacyVersion": contract.user_version,
            "backupFile": backup_path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("pre-public.backup"),
            "deckCount": before.count("decks"),
            "cardCount": before.count("cards"),
            "reviewCount": before.count("review_events"),
        }),
    );

    if let Err(error) = apply_adoption(&mut connection, &before) {
        return Err(format!(
            "The pre-public database could not be adopted safely: {error} A consistent database backup remains at {}.",
            backup_path.display()
        ));
    }

    crate::logging::info_target(
        "database.migration",
        "pre_public_database_adoption_completed",
        "The final pre-public database was adopted without replacing learner data.",
        json!({
            "legacyVersion": contract.user_version,
            "schemaLineage": PUBLIC_SCHEMA_LINEAGE,
            "schemaVersion": PUBLIC_BASELINE_VERSION,
            "backupFile": backup_path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("pre-public.backup"),
            "deckCount": before.count("decks"),
            "cardCount": before.count("cards"),
            "reviewCount": before.count("review_events"),
        }),
    );

    Ok(Some(AdoptionReport {
        backup_path,
        deck_count: before.count("decks"),
        card_count: before.count("cards"),
        review_count: before.count("review_events"),
    }))
}

fn pre_public_contract() -> Result<PrePublicContract, String> {
    let contract: PrePublicContract = serde_json::from_str(PRE_PUBLIC_CONTRACT_JSON)
        .map_err(|error| format!("The embedded pre-public lineage contract is invalid: {error}"))?;
    let expected_migration_count =
        usize::try_from(contract.user_version).unwrap_or(usize::MAX);
    if contract.user_version <= 0
        || contract.migrations.len() != expected_migration_count
        || contract
            .migrations
            .iter()
            .enumerate()
            .any(|(index, entry)| entry.version != i64::try_from(index + 1).unwrap_or(i64::MAX))
        || contract
            .repairs
            .iter()
            .enumerate()
            .any(|(index, entry)| entry.version != i64::try_from(index + 1).unwrap_or(i64::MAX))
    {
        return Err("The embedded pre-public lineage contract is not contiguous.".to_string());
    }
    Ok(contract)
}

fn is_exact_final_pre_public_database(
    connection: &Connection,
    contract: &PrePublicContract,
) -> Result<bool, String> {
    if table_exists(connection, "schema_metadata")? {
        return Ok(false);
    }
    for table in [
        "schema_migrations",
        "data_repairs",
        "deck_folders",
        "decks",
        "notes",
        "cards",
        "review_events",
        "review_event_attributions",
        "ai_provider_profiles",
    ] {
        if !table_exists(connection, table)? {
            return Ok(false);
        }
    }
    if column_exists(
        connection,
        "ai_provider_profiles",
        "require_zero_data_retention",
    )? {
        return Ok(false);
    }

    let user_version: i64 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|error| format!("Could not read the pre-public SQLite user_version: {error}"))?;
    if user_version != contract.user_version {
        return Ok(false);
    }

    let migrations = read_ledger(connection, "schema_migrations")?;
    let repairs = read_ledger(connection, "data_repairs")?;
    Ok(migrations == contract.migrations && repairs == contract.repairs)
}

fn read_ledger(connection: &Connection, table: &str) -> Result<Vec<LedgerEntry>, String> {
    let sql = format!("SELECT version, name, checksum FROM {table} ORDER BY version");
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| format!("Could not inspect {table}: {error}"))?;
    statement
        .query_map([], |row| {
            Ok(LedgerEntry {
                version: row.get(0)?,
                name: row.get(1)?,
                checksum: row.get(2)?,
            })
        })
        .map_err(|error| format!("Could not read {table}: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Could not decode {table}: {error}"))
}

fn table_exists(connection: &Connection, table: &str) -> Result<bool, String> {
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = ?1)",
            [table],
            |row| row.get::<_, i64>(0),
        )
        .map(|value| value != 0)
        .map_err(|error| format!("Could not inspect table {table}: {error}"))
}

fn column_exists(connection: &Connection, table: &str, column: &str) -> Result<bool, String> {
    let sql = format!("PRAGMA table_info({table})");
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| format!("Could not inspect columns for {table}: {error}"))?;
    let mut rows = statement
        .query([])
        .map_err(|error| format!("Could not read columns for {table}: {error}"))?;
    while let Some(row) = rows
        .next()
        .map_err(|error| format!("Could not decode columns for {table}: {error}"))?
    {
        let name: String = row
            .get(1)
            .map_err(|error| format!("Could not decode a column name for {table}: {error}"))?;
        if name == column {
            return Ok(true);
        }
    }
    Ok(false)
}

fn preserved_data_snapshot(connection: &Connection) -> Result<PreservedDataSnapshot, String> {
    Ok(PreservedDataSnapshot {
        table_counts: preserved_table_counts(connection)?,
        deck_ids: text_column_values(connection, "decks", "id")?,
        note_ids: text_column_values(connection, "notes", "id")?,
        card_ids: text_column_values(connection, "cards", "id")?,
        review_ids: text_column_values(connection, "review_events", "id")?,
        review_attribution_ids: text_column_values(
            connection,
            "review_event_attributions",
            "review_event_id",
        )?,
        ai_profile_ids: text_column_values(connection, "ai_provider_profiles", "id")?,
    })
}

fn preserved_table_counts(connection: &Connection) -> Result<BTreeMap<String, i64>, String> {
    let mut statement = connection
        .prepare(
            "SELECT name FROM sqlite_schema
             WHERE type = 'table'
               AND name NOT LIKE 'sqlite_%'
               AND name NOT GLOB 'cards_fts_*'
               AND name NOT IN ('schema_metadata', 'schema_migrations', 'data_repairs')
             ORDER BY name",
        )
        .map_err(|error| format!("Could not enumerate pre-public tables: {error}"))?;
    let tables = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|error| format!("Could not read pre-public table names: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Could not decode a pre-public table name: {error}"))?;

    let mut counts = BTreeMap::new();
    for table in tables {
        let quoted = table.replace('"', "\"\"");
        let sql = format!("SELECT COUNT(*) FROM \"{quoted}\"");
        let count = connection
            .query_row(&sql, [], |row| row.get(0))
            .map_err(|error| format!("Could not count rows in {table}: {error}"))?;
        counts.insert(table, count);
    }
    Ok(counts)
}

fn text_column_values(
    connection: &Connection,
    table: &str,
    column: &str,
) -> Result<Vec<String>, String> {
    let sql = format!("SELECT {column} FROM {table} ORDER BY {column}");
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| format!("Could not inspect {table}.{column}: {error}"))?;
    statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|error| format!("Could not read {table}.{column}: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Could not decode {table}.{column}: {error}"))
}

fn create_database_backup(
    source: &Connection,
    database_path: &Path,
    legacy_version: i64,
) -> Result<PathBuf, String> {
    let parent = database_path
        .parent()
        .ok_or_else(|| "The database path has no parent directory.".to_string())?;
    let file_name = database_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("library.sqlite3");
    let timestamp = unix_time_millis().map_err(|error| error.message)?;
    let mut backup_path = parent.join(format!(
        "{file_name}.pre-public-v{legacy_version}-{timestamp}.backup"
    ));
    for suffix in 1..=100 {
        if !backup_path.exists() {
            break;
        }
        backup_path = parent.join(format!(
            "{file_name}.pre-public-v{legacy_version}-{timestamp}-{suffix}.backup"
        ));
    }
    if backup_path.exists() {
        return Err("Could not allocate a unique pre-public database backup path.".to_string());
    }

    let backup_result = (|| {
        let mut destination = Connection::open(&backup_path)
            .map_err(|error| format!("Could not create the pre-public database backup: {error}"))?;
        {
            let backup = Backup::new(source, &mut destination)
                .map_err(|error| format!("Could not start the SQLite backup: {error}"))?;
            backup
                .run_to_completion(64, Duration::from_millis(10), None)
                .map_err(|error| format!("Could not finish the SQLite backup: {error}"))?;
        }
        verify_integrity(&destination)
    })();

    if let Err(error) = backup_result {
        let _ = fs::remove_file(&backup_path);
        return Err(error);
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&backup_path, fs::Permissions::from_mode(0o600)).map_err(|error| {
            format!("Could not restrict the pre-public database backup permissions: {error}")
        })?;
    }

    Ok(backup_path)
}

fn apply_adoption(
    connection: &mut Connection,
    expected_snapshot: &PreservedDataSnapshot,
) -> Result<(), String> {
    connection
        .execute_batch("PRAGMA foreign_keys = OFF;")
        .map_err(|error| format!("Could not prepare the pre-public adoption: {error}"))?;

    let adoption_result = (|| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| format!("Could not begin the pre-public adoption: {error}"))?;
        transaction
            .execute_batch(REBUILD_AI_PROFILES_SQL)
            .map_err(|error| format!("Could not rebuild the provider-profile schema: {error}"))?;
        let applied_at = unix_time_millis().map_err(|error| error.message)?;
        transaction
            .execute(
                "INSERT INTO schema_migrations (version, name, checksum, applied_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    PUBLIC_BASELINE_VERSION,
                    PUBLIC_BASELINE_NAME,
                    PUBLIC_BASELINE_CHECKSUM,
                    applied_at,
                ],
            )
            .map_err(|error| format!("Could not record the public baseline: {error}"))?;
        transaction
            .execute_batch("PRAGMA user_version = 1;")
            .map_err(|error| format!("Could not set the public SQLite user_version: {error}"))?;

        let after = preserved_data_snapshot(&transaction)?;
        if &after != expected_snapshot {
            return Err(format!(
                "Learner data changed during adoption (before {expected_snapshot:?}, after {after:?})."
            ));
        }
        verify_integrity(&transaction)?;
        transaction
            .commit()
            .map_err(|error| format!("Could not commit the pre-public adoption: {error}"))
    })();

    let enforcement_result = connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .map_err(|error| format!("Could not restore SQLite foreign-key enforcement: {error}"));

    match (adoption_result, enforcement_result) {
        (Err(error), _) => Err(error),
        (Ok(()), Err(error)) => Err(error),
        (Ok(()), Ok(())) => verify_integrity(connection),
    }
}

fn verify_integrity(connection: &Connection) -> Result<(), String> {
    let foreign_key_violation: Option<(String, i64)> = connection
        .query_row(
            "SELECT [table], COALESCE(rowid, -1) FROM pragma_foreign_key_check LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|error| format!("Could not validate pre-public foreign keys: {error}"))?;
    if let Some((table, rowid)) = foreign_key_violation {
        return Err(format!(
            "The pre-public database contains an invalid foreign-key reference in {table} at row {rowid}."
        ));
    }
    let result: String = connection
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(|error| format!("Could not run SQLite integrity_check: {error}"))?;
    if result != "ok" {
        return Err(format!("SQLite integrity_check failed: {result}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
