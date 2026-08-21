use std::{
    path::Path,
    sync::Mutex,
    time::{Duration, Instant},
};

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
#[cfg(test)]
use serde::Serialize;
use serde_json::json;

use crate::{
    data_repairs::{apply_required_repairs, verify_repair_ledger, RepairPhase},
    error::CommandError,
    logging::LogLevel,
    util::unix_time_millis,
};

const PUBLIC_SCHEMA_LINEAGE: &str = "ostudy-public-v1";
pub(crate) const PRE_PUBLIC_RESET_PREFIX: &str = "PRE_PUBLIC_DATABASE_REQUIRES_RESET";

struct Migration {
    version: i64,
    name: &'static str,
    checksum: &'static str,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "public_baseline",
        checksum: "2d978f8be026ecb820e71ba77a8585e1de51cb08fc2820714526a33284167f85",
        sql: include_str!("../migrations/0001_public_baseline.sql"),
    },
    Migration {
        version: 2,
        name: "canonical_documents",
        checksum: "dba2a5fd43b9544c1a4da9e060106f157ca5ac839a5c87ca831d7ceef8bb7733",
        sql: include_str!("../migrations/0002_canonical_documents.sql"),
    },
];

pub(crate) const LATEST_SCHEMA_VERSION: i64 = 2;

pub struct Database {
    connection: Mutex<Connection>,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self, String> {
        crate::logging::debug_target(
            "database.connection",
            "database_open_started",
            "The local SQLite database is being opened.",
            json!({ "fileName": path.file_name().and_then(|value| value.to_str()).unwrap_or("library.sqlite3") }),
        );
        let mut connection = Connection::open(path)
            .map_err(|error| format!("Could not open the local SQLite database: {error}"))?;
        initialize_connection(&mut connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    pub(crate) fn with_connection<T>(
        &self,
        operation: impl FnOnce(&mut Connection) -> Result<T, CommandError>,
    ) -> Result<T, CommandError> {
        let mut connection = self.connection.lock().map_err(|_| {
            CommandError::new(
                "database_lock_poisoned",
                "The local database lock could not be acquired. Restart ostudy.",
            )
        })?;
        operation(&mut connection)
    }

    #[cfg(test)]
    pub(crate) fn open_in_memory() -> Result<Self, String> {
        let mut connection = Connection::open_in_memory()
            .map_err(|error| format!("Could not open an in-memory database: {error}"))?;
        initialize_connection(&mut connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum DatabaseLineage {
    Empty,
    Public,
    PrePublic,
    Unknown(String),
}

fn initialize_connection(connection: &mut Connection) -> Result<(), String> {
    // Lineage inspection must happen before journal/durability configuration.
    // `PRAGMA journal_mode = WAL` can write SQLite state, so running it first
    // would violate the promise that an unsupported pre-public database is
    // rejected without modification.
    let lineage = inspect_lineage(connection)?;
    match lineage {
        DatabaseLineage::Empty | DatabaseLineage::Public => {}
        DatabaseLineage::PrePublic => {
            return Err(format!(
                "{PRE_PUBLIC_RESET_PREFIX}: This database predates ostudy's public schema baseline. Stop ostudy, preserve a backup if needed, and run scripts/reset-pre-public-data.sh with the exact application data directory. The database was not modified."
            ));
        }
        DatabaseLineage::Unknown(value) => {
            return Err(format!(
                "This database belongs to an unsupported schema lineage ({value}). The database was not modified."
            ));
        }
    }
    configure_connection(connection)?;

    let started = Instant::now();
    let migrated = apply_migrations(connection, matches!(lineage, DatabaseLineage::Empty))?;
    apply_required_repairs(
        connection,
        RepairPhase::AfterMigration(LATEST_SCHEMA_VERSION),
    )?;
    verify_repair_ledger(connection)?;
    if migrated || crate::logging::enabled(LogLevel::Debug, "ostudy.database.integrity") {
        verify_integrity(connection)?;
    }
    crate::logging::info_target(
        "database.migration",
        "database_schema_ready",
        "The public database schema is ready.",
        json!({
            "schemaLineage": PUBLIC_SCHEMA_LINEAGE,
            "schemaVersion": LATEST_SCHEMA_VERSION,
            "migrationApplied": migrated,
            "durationMs": i64::try_from(started.elapsed().as_millis()).unwrap_or(i64::MAX),
        }),
    );
    Ok(())
}

fn configure_connection(connection: &Connection) -> Result<(), String> {
    connection
        .busy_timeout(Duration::from_secs(5))
        .map_err(|error| format!("Could not configure SQLite busy timeout: {error}"))?;
    connection
        .execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA journal_mode = WAL;
             PRAGMA synchronous = FULL;",
        )
        .map_err(|error| format!("Could not configure SQLite durability settings: {error}"))
}

fn inspect_lineage(connection: &Connection) -> Result<DatabaseLineage, String> {
    let metadata_exists: i64 = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = 'schema_metadata')",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("Could not inspect the schema lineage table: {error}"))?;
    if metadata_exists != 0 {
        let value: Option<String> = connection
            .query_row(
                "SELECT value FROM schema_metadata WHERE key = 'schema_lineage'",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| format!("Could not read the schema lineage: {error}"))?;
        return Ok(match value {
            Some(value) if value == PUBLIC_SCHEMA_LINEAGE => DatabaseLineage::Public,
            Some(value) => DatabaseLineage::Unknown(value),
            None => DatabaseLineage::Unknown("missing".to_string()),
        });
    }

    let object_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%'",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("Could not inspect database contents: {error}"))?;
    if object_count == 0 {
        Ok(DatabaseLineage::Empty)
    } else {
        Ok(DatabaseLineage::PrePublic)
    }
}

fn apply_migrations(connection: &mut Connection, empty_database: bool) -> Result<bool, String> {
    let mut applied = false;
    if !empty_database {
        verify_public_ledger_exists(connection)?;
    }
    let newest_version = if empty_database {
        0
    } else {
        connection
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
                [],
                |row| row.get(0),
            )
            .map_err(|error| format!("Could not read the database schema version: {error}"))?
    };
    if newest_version > LATEST_SCHEMA_VERSION {
        return Err(format!(
            "This database was created by a newer ostudy version (public schema {newest_version})."
        ));
    }

    for migration in MIGRATIONS {
        let existing: Option<(String, String)> = if empty_database && migration.version == 1 {
            None
        } else {
            connection
                .query_row(
                    "SELECT name, checksum FROM schema_migrations WHERE version = ?1",
                    [migration.version],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()
                .map_err(|error| {
                    format!("Could not inspect migration {}: {error}", migration.version)
                })?
        };
        if let Some((name, checksum)) = existing {
            if name != migration.name || checksum != migration.checksum {
                return Err(format!(
                    "Public migration {} does not match this application build. The database was not modified.",
                    migration.version
                ));
            }
            continue;
        }
        apply_transactional_migration(connection, migration)?;
        applied = true;
    }
    Ok(applied)
}

fn verify_public_ledger_exists(connection: &Connection) -> Result<(), String> {
    let exists: i64 = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = 'schema_migrations')",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|error| format!("Could not inspect the public migration ledger: {error}"))?;
    if exists == 0 {
        return Err(
            "The public schema lineage is present but its migration ledger is missing.".to_string(),
        );
    }
    Ok(())
}

fn apply_transactional_migration(
    connection: &mut Connection,
    migration: &Migration,
) -> Result<(), String> {
    let started = Instant::now();
    crate::logging::info_target(
        "database.migration",
        "database_migration_started",
        "A public database migration started.",
        json!({ "version": migration.version, "name": migration.name }),
    );
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| {
            format!(
                "Could not begin database migration {}: {error}",
                migration.version
            )
        })?;
    transaction
        .execute_batch(migration.sql)
        .map_err(|error| format!("Could not apply migration {}: {error}", migration.version))?;
    let applied_at = unix_time_millis().map_err(|error| error.message)?;
    transaction
        .execute(
            "INSERT INTO schema_migrations (version, name, checksum, applied_at) VALUES (?1, ?2, ?3, ?4)",
            params![migration.version, migration.name, migration.checksum, applied_at],
        )
        .map_err(|error| format!("Could not record migration {}: {error}", migration.version))?;
    transaction
        .execute_batch(&format!("PRAGMA user_version = {};", migration.version))
        .map_err(|error| format!("Could not set SQLite user_version: {error}"))?;
    transaction
        .commit()
        .map_err(|error| format!("Could not commit migration {}: {error}", migration.version))?;
    crate::logging::info_target(
        "database.migration",
        "database_migration_completed",
        "A public database migration completed.",
        json!({
            "version": migration.version,
            "name": migration.name,
            "durationMs": i64::try_from(started.elapsed().as_millis()).unwrap_or(i64::MAX),
        }),
    );
    Ok(())
}

fn verify_integrity(connection: &Connection) -> Result<(), String> {
    let foreign_key_violation: Option<(String, i64)> = connection
        .query_row(
            "SELECT [table], rowid FROM pragma_foreign_key_check LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|error| format!("Could not run SQLite foreign-key validation: {error}"))?;
    if let Some((table, rowid)) = foreign_key_violation {
        return Err(format!(
            "The migrated database contains an invalid foreign-key reference in table {table} at row {rowid}."
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
fn public_baseline_connection() -> Result<Connection, String> {
    let mut connection = Connection::open_in_memory()
        .map_err(|error| format!("Could not open a public-baseline fixture database: {error}"))?;
    configure_connection(&connection)?;
    apply_transactional_migration(&mut connection, &MIGRATIONS[0])?;
    Ok(connection)
}

#[cfg(test)]
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct SchemaObject {
    object_type: String,
    name: String,
    sql: String,
}

#[cfg(test)]
fn schema_objects(connection: &Connection) -> Result<Vec<SchemaObject>, rusqlite::Error> {
    let mut statement = connection.prepare(
        "SELECT type, name, RTRIM(sql, ';') FROM sqlite_schema
         WHERE sql IS NOT NULL AND name NOT LIKE 'sqlite_%'
           AND name NOT GLOB 'cards_fts_*'
         ORDER BY CASE type WHEN 'table' THEN 0 WHEN 'view' THEN 1
                  WHEN 'index' THEN 2 WHEN 'trigger' THEN 3 ELSE 4 END,
                  name",
    )?;
    let objects = statement
        .query_map([], |row| {
            Ok(SchemaObject {
                object_type: row.get(0)?,
                name: row.get(1)?,
                sql: row.get::<_, String>(2)?.trim().to_string(),
            })
        })?
        .collect();
    objects
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_sqlite_version_is_the_audited_release() {
        assert_eq!(rusqlite::version(), "3.53.2");
    }

    #[test]
    fn regenerate_public_baseline_schema_manifest() {
        if std::env::var_os("OSTUDY_REGENERATE_PUBLIC_SCHEMA_MANIFEST").is_none() {
            return;
        }
        use sha2::{Digest, Sha256};

        let connection = public_baseline_connection().expect("baseline database should open");
        let objects = serde_json::to_value(
            schema_objects(&connection).expect("public baseline objects should load"),
        )
        .expect("public baseline objects should serialize");
        let manifest = serde_json::json!({
            "formatVersion": 1,
            "schemaLineage": PUBLIC_SCHEMA_LINEAGE,
            "schemaVersion": MIGRATIONS[0].version,
            "baselineSha256": format!(
                "{:x}",
                Sha256::digest(MIGRATIONS[0].sql.as_bytes())
            ),
            "objects": objects,
        });
        let output = serde_json::to_string_pretty(&manifest)
            .expect("public schema manifest should serialize");
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/public-baseline-schema.json");
        std::fs::write(path, format!("{output}\n"))
            .expect("public schema manifest should be written");
    }

    #[test]
    fn latest_schema_is_idempotent_and_public_baseline_fixture_is_immutable() {
        let database = Database::open_in_memory().expect("database should open");
        database
            .with_connection(|connection| {
                let before = connection.total_changes();
                initialize_connection(connection)
                    .map_err(|error| CommandError::new("migration_error", error))?;
                assert_eq!(before, connection.total_changes());
                let version: i64 = connection
                    .query_row("PRAGMA user_version", [], |row| row.get(0))
                    .map_err(CommandError::database)?;
                assert_eq!(version, LATEST_SCHEMA_VERSION);
                let lineage: String = connection
                    .query_row(
                        "SELECT value FROM schema_metadata WHERE key = 'schema_lineage'",
                        [],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(lineage, PUBLIC_SCHEMA_LINEAGE);
                Ok(())
            })
            .expect("latest public schema should be idempotent");

        let connection = public_baseline_connection().expect("baseline database should open");
        let manifest: serde_json::Value = serde_json::from_str(include_str!(
            "../tests/fixtures/public-baseline-schema.json"
        ))
        .expect("public baseline manifest should decode");
        assert_eq!(
            manifest
                .get("schemaVersion")
                .and_then(serde_json::Value::as_i64),
            Some(MIGRATIONS[0].version)
        );
        let expected = manifest
            .get("objects")
            .cloned()
            .expect("baseline manifest should contain schema objects");
        let actual = serde_json::to_value(
            schema_objects(&connection).expect("public baseline objects should load"),
        )
        .expect("public baseline objects should serialize");
        assert_eq!(actual, expected);
    }

    #[test]
    fn pre_public_database_requires_explicit_non_destructive_reset() {
        let mut connection = Connection::open_in_memory().expect("database should open");
        connection
            .execute_batch(
                "CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY, name TEXT NOT NULL, checksum TEXT NOT NULL, applied_at INTEGER NOT NULL) STRICT;
                 INSERT INTO schema_migrations VALUES (26, 'deck_folders', 'legacy', 0);
                 CREATE TABLE decks (id TEXT PRIMARY KEY) STRICT;",
            )
            .expect("legacy fixture should load");
        let error = initialize_connection(&mut connection).expect_err("legacy database must stop");
        assert!(error.starts_with(PRE_PUBLIC_RESET_PREFIX));
        let decks_exist: i64 = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name = 'decks')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(decks_exist, 1, "legacy data must not be silently deleted");
    }

    #[test]
    fn pre_public_disk_database_is_byte_for_byte_unchanged_on_rejection() {
        let unique = unix_time_millis().unwrap_or(0);
        let root = std::env::temp_dir().join(format!(
            "ostudy-pre-public-database-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).expect("temporary database directory should exist");
        let path = root.join("library.sqlite3");
        {
            let connection = Connection::open(&path).expect("legacy database should open");
            connection
                .execute_batch(
                    "CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY, name TEXT NOT NULL, checksum TEXT NOT NULL, applied_at INTEGER NOT NULL) STRICT;
                     INSERT INTO schema_migrations VALUES (26, 'deck_folders', 'legacy', 0);
                     CREATE TABLE decks (id TEXT PRIMARY KEY) STRICT;",
                )
                .expect("legacy fixture should load");
        }
        let before = std::fs::read(&path).expect("legacy database bytes should be readable");
        let error = match Database::open(&path) {
            Ok(_) => panic!("legacy database must not open"),
            Err(error) => error,
        };
        assert!(error.starts_with(PRE_PUBLIC_RESET_PREFIX));
        let after = std::fs::read(&path).expect("legacy database bytes should remain readable");
        assert_eq!(before, after, "legacy database bytes must remain unchanged");
        assert!(
            !root.join("library.sqlite3-wal").exists(),
            "legacy rejection must not create a WAL sidecar"
        );
        assert!(
            !root.join("library.sqlite3-shm").exists(),
            "legacy rejection must not create a shared-memory sidecar"
        );
        std::fs::remove_dir_all(&root).expect("temporary database directory should be removed");
    }

    #[test]
    fn public_migration_checksum_and_future_version_are_enforced() {
        let mut connection = Connection::open_in_memory().expect("database should open");
        initialize_connection(&mut connection).expect("public migrations should apply");
        connection
            .execute(
                "UPDATE schema_migrations SET checksum = 'modified' WHERE version = 1",
                [],
            )
            .unwrap();
        assert!(initialize_connection(&mut connection).is_err());
        connection
            .execute(
                "UPDATE schema_migrations SET checksum = ?1 WHERE version = 1",
                [MIGRATIONS[0].checksum],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO schema_migrations (version, name, checksum, applied_at)
                 VALUES (?1, 'future', 'future', 0)",
                [LATEST_SCHEMA_VERSION + 1],
            )
            .unwrap();
        assert!(initialize_connection(&mut connection).is_err());
    }

    #[test]
    fn openrouter_is_a_valid_extensible_provider_key() {
        let database = Database::open_in_memory().expect("database should open");
        database
            .with_connection(|connection| {
                connection
                    .execute(
                        "INSERT INTO ai_provider_profiles (
                             id, display_name, normalized_name, provider_kind, base_url,
                             model_id, auth_mode, enabled, allow_insecure_http,
                             request_timeout_ms, credential_storage, credential_epoch,
                             revision, created_at, updated_at
                         ) VALUES (?1, 'OpenRouter', 'openrouter', 'openrouter',
                                   'https://openrouter.ai/api/v1/', 'openai/example',
                                   'bearer', 1, 0, 60000, 'session', 0, 1, 0, 0)",
                        ["a".repeat(32)],
                    )
                    .map_err(CommandError::database)?;
                Ok(())
            })
            .expect("OpenRouter profile should satisfy the public baseline");
    }
}
