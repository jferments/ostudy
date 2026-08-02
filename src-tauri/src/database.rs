use std::{path::Path, sync::Mutex, time::Duration};

use rusqlite::{params, Connection, OptionalExtension};

use crate::{
    authoring::{backfill_duplicate_hashes, normalize_stage3_schema},
    error::CommandError,
    util::unix_time_millis,
};

struct Migration {
    version: i64,
    name: &'static str,
    checksum: &'static str,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "create_decks",
        checksum: "771e157d6f7eb5e1c024862966718e46ef744298f49287636c75167065904fd6",
        sql: include_str!("../migrations/0001_decks.sql"),
    },
    Migration {
        version: 2,
        name: "cards_assets_settings_practice",
        checksum: "af64ceea1bfb3f1c04fed33f4558fae051174b50b15873d9ddf5ab4b36e55c8b",
        sql: include_str!("../migrations/0002_cards_assets_settings_practice.sql"),
    },
    Migration {
        version: 3,
        name: "fsrs_reviews",
        checksum: "49dd1e24e463c9a08a59e243d90143e2874459dd2a4a8d5f61613d5df7ebcae3",
        sql: include_str!("../migrations/0003_fsrs_reviews.sql"),
    },
    Migration {
        version: 4,
        name: "card_organization",
        checksum: "0e0e25b3de24c04614aa949fce883fb6cc5bf12d1aab6e08dcaab12c548a768d",
        sql: include_str!("../migrations/0004_card_organization.sql"),
    },
    Migration {
        version: 5,
        name: "remove_legacy_duplicate_trigger",
        checksum: "e275254085414039a7cf5f3a5e08ae35a61ea8f7d3d1b8e016a8d1f087c03029",
        sql: include_str!("../migrations/0005_remove_legacy_duplicate_trigger.sql"),
    },
    Migration {
        version: 6,
        name: "card_types_and_templates",
        checksum: "c07b900b2119f6eae6a4de58aeb3af9ab5882ee21d5c29ac7a5c7fcee6b86be8",
        sql: include_str!("../migrations/0006_card_types_and_templates.sql"),
    },
    Migration {
        version: 7,
        name: "rich_templates_single_cloze",
        checksum: "91d120cb0b72c72c40d499a2c8be9b090be96ca760b2bb5a71072e2b71ed45e1",
        sql: include_str!("../migrations/0007_rich_templates_single_cloze.sql"),
    },
];

const LATEST_SCHEMA_VERSION: i64 = 7;

pub struct Database {
    connection: Mutex<Connection>,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self, String> {
        let mut connection = Connection::open(path)
            .map_err(|error| format!("Could not open {}: {error}", path.display()))?;
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

fn initialize_connection(connection: &mut Connection) -> Result<(), String> {
    configure_connection(connection)?;
    apply_migrations(connection)?;
    backfill_duplicate_hashes(connection).map_err(|error| error.message)?;
    normalize_stage3_schema(connection).map_err(|error| error.message)?;
    Ok(())
}

fn configure_connection(connection: &Connection) -> Result<(), String> {
    connection
        .busy_timeout(Duration::from_secs(5))
        .map_err(|error| format!("Could not configure SQLite busy timeout: {error}"))?;
    connection
        .execute_batch(
            "PRAGMA foreign_keys = ON;\n\
             PRAGMA journal_mode = WAL;\n\
             PRAGMA synchronous = FULL;",
        )
        .map_err(|error| format!("Could not configure SQLite durability settings: {error}"))
}

fn initialize_migration_ledger(connection: &Connection) -> Result<(), String> {
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_migrations (\n\
                 version     INTEGER PRIMARY KEY,\n\
                 name        TEXT NOT NULL,\n\
                 checksum    TEXT NOT NULL,\n\
                 applied_at  INTEGER NOT NULL\n\
             ) STRICT;",
        )
        .map_err(|error| format!("Could not initialize the migration ledger: {error}"))
}

fn apply_migrations(connection: &mut Connection) -> Result<(), String> {
    initialize_migration_ledger(connection)?;
    let newest_version: Option<i64> = connection
        .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
            row.get(0)
        })
        .map_err(|error| format!("Could not read the database schema version: {error}"))?;
    if newest_version.unwrap_or(0) > LATEST_SCHEMA_VERSION {
        return Err(format!(
            "This database was created by a newer ostudy version (schema {}).",
            newest_version.unwrap_or_default()
        ));
    }

    for migration in MIGRATIONS {
        let existing: Option<(String, String)> = connection
            .query_row(
                "SELECT name, checksum FROM schema_migrations WHERE version = ?1",
                [migration.version],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|error| {
                format!("Could not inspect migration {}: {error}", migration.version)
            })?;
        if let Some((name, checksum)) = existing {
            if name != migration.name || checksum != migration.checksum {
                return Err(format!(
                    "Migration {} does not match this application build. The database was not modified.",
                    migration.version
                ));
            }
            continue;
        }

        let applied_at = unix_time_millis().map_err(|error| error.message)?;
        let transaction = connection.transaction().map_err(|error| {
            format!(
                "Could not begin database migration {}: {error}",
                migration.version
            )
        })?;
        transaction
            .execute_batch(migration.sql)
            .map_err(|error| format!("Could not apply migration {}: {error}", migration.version))?;
        transaction
            .execute(
                "INSERT INTO schema_migrations (version, name, checksum, applied_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    migration.version,
                    migration.name,
                    migration.checksum,
                    applied_at
                ],
            )
            .map_err(|error| {
                format!("Could not record migration {}: {error}", migration.version)
            })?;
        transaction
            .execute_batch(&format!("PRAGMA user_version = {};", migration.version))
            .map_err(|error| {
                format!(
                    "Could not set SQLite user_version for migration {}: {error}",
                    migration.version
                )
            })?;
        transaction.commit().map_err(|error| {
            format!("Could not commit migration {}: {error}", migration.version)
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct ReleasedSchemaFixture {
        release: &'static str,
        schema_version: i64,
        sql: &'static str,
    }

    const RELEASED_SCHEMA_FIXTURES: &[ReleasedSchemaFixture] = &[ReleasedSchemaFixture {
        release: "0.1.0",
        schema_version: 3,
        sql: include_str!("../tests/fixtures/v0.1.0.sql"),
    }];

    #[test]
    fn bundled_sqlite_version_is_the_audited_release() {
        assert_eq!(rusqlite::version(), "3.53.2");
    }

    #[test]
    fn migrations_are_idempotent_and_reach_schema_seven() {
        let database = Database::open_in_memory().expect("database should open");
        database
            .with_connection(|connection| {
                apply_migrations(connection)
                    .map_err(|error| CommandError::new("migration_error", error))?;
                normalize_stage3_schema(connection)?;
                let version: i64 = connection
                    .query_row("PRAGMA user_version", [], |row| row.get(0))
                    .map_err(CommandError::database)?;
                assert_eq!(version, LATEST_SCHEMA_VERSION);
                for table in [
                    "decks",
                    "notes",
                    "cards",
                    "card_schedules",
                    "review_events",
                    "tags",
                    "note_tags",
                    "card_search_fts",
                    "card_templates",
                    "card_template_fields",
                    "card_template_rules",
                    "card_template_assets",
                ] {
                    let exists: i64 = connection
                        .query_row(
                            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name = ?1)",
                            [table],
                            |row| row.get(0),
                        )
                        .map_err(CommandError::database)?;
                    assert_eq!(exists, 1, "missing table {table}");
                }
                for column in ["front_doc", "back_doc"] {
                    let exists: i64 = connection
                        .query_row(
                            "SELECT EXISTS(
                                 SELECT 1 FROM pragma_table_info('card_template_rules')
                                 WHERE name = ?1
                             )",
                            [column],
                            |row| row.get(0),
                        )
                        .map_err(CommandError::database)?;
                    assert_eq!(exists, 1, "missing rule column {column}");
                }
                let variant_exists: i64 = connection
                    .query_row(
                        "SELECT EXISTS(
                             SELECT 1 FROM pragma_table_info('practice_session_cards')
                             WHERE name = 'presentation_variant'
                         )",
                        [],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(variant_exists, 1);
                Ok(())
            })
            .expect("second initialization should succeed");
    }

    #[test]
    fn every_released_schema_fixture_upgrades_to_latest() {
        for fixture in RELEASED_SCHEMA_FIXTURES {
            let mut connection = Connection::open_in_memory().expect("fixture should open");
            connection
                .execute_batch(fixture.sql)
                .unwrap_or_else(|error| panic!("{} fixture should load: {error}", fixture.release));
            initialize_connection(&mut connection)
                .unwrap_or_else(|error| panic!("{} fixture should upgrade: {error}", fixture.release));
            let version: i64 = connection
                .query_row("PRAGMA user_version", [], |row| row.get(0))
                .expect("version should be readable");
            assert_eq!(version, LATEST_SCHEMA_VERSION);
            assert!(fixture.schema_version < version);
            let integrity: String = connection
                .query_row("PRAGMA integrity_check", [], |row| row.get(0))
                .expect("integrity check should run");
            assert_eq!(integrity, "ok");
        }
    }

    #[test]
    fn historical_migration_checksums_remain_exact() {
        assert_eq!(
            MIGRATIONS[3].checksum,
            "0e0e25b3de24c04614aa949fce883fb6cc5bf12d1aab6e08dcaab12c548a768d"
        );
        assert_eq!(
            MIGRATIONS[5].checksum,
            "c07b900b2119f6eae6a4de58aeb3af9ab5882ee21d5c29ac7a5c7fcee6b86be8"
        );
        assert_eq!(
            MIGRATIONS[6].checksum,
            "91d120cb0b72c72c40d499a2c8be9b090be96ca760b2bb5a71072e2b71ed45e1"
        );
    }
}
