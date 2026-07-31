use std::{path::Path, sync::Mutex, time::Duration};

use rusqlite::{params, Connection, OptionalExtension};

use crate::{error::CommandError, util::unix_time_millis};

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
        checksum: "0d6e5db5aecd25e7d6e88f104bd5bba59c334164e1a6160acb83d8e0727e41be",
        sql: include_str!("../migrations/0003_fsrs_reviews.sql"),
    },
];

const LATEST_SCHEMA_VERSION: i64 = 3;

pub struct Database {
    connection: Mutex<Connection>,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self, String> {
        let mut connection = Connection::open(path)
            .map_err(|error| format!("Could not open {}: {error}", path.display()))?;

        configure_connection(&connection)?;
        apply_migrations(&mut connection)?;

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
                "The local database lock could not be acquired. Restart the application.",
            )
        })?;

        operation(&mut connection)
    }

    #[cfg(test)]
    pub(crate) fn open_in_memory() -> Result<Self, String> {
        let mut connection = Connection::open_in_memory()
            .map_err(|error| format!("Could not open an in-memory database: {error}"))?;

        configure_connection(&connection)?;
        apply_migrations(&mut connection)?;

        Ok(Self {
            connection: Mutex::new(connection),
        })
    }
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
        .map_err(|error| format!("Could not configure SQLite durability settings: {error}"))?;

    Ok(())
}

fn apply_migrations(connection: &mut Connection) -> Result<(), String> {
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_migrations (\n\
                 version     INTEGER PRIMARY KEY,\n\
                 name        TEXT NOT NULL,\n\
                 checksum    TEXT NOT NULL,\n\
                 applied_at  INTEGER NOT NULL\n\
             ) STRICT;",
        )
        .map_err(|error| format!("Could not initialize the migration ledger: {error}"))?;

    let newest_version: Option<i64> = connection
        .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
            row.get(0)
        })
        .map_err(|error| format!("Could not read the database schema version: {error}"))?;

    if newest_version.unwrap_or(0) > LATEST_SCHEMA_VERSION {
        return Err(format!(
            "This database was created by a newer Study App version (schema {}).",
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
                "INSERT INTO schema_migrations (version, name, checksum, applied_at)\n\
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

    #[test]
    fn bundled_sqlite_version_is_the_audited_release() {
        assert_eq!(rusqlite::version(), "3.53.2");
    }

    #[test]
    fn migrations_are_idempotent_and_reach_latest_schema() {
        let database = Database::open_in_memory().expect("database should open");

        database
            .with_connection(|connection| {
                apply_migrations(connection)
                    .map_err(|error| CommandError::new("migration_error", error))?;
                let version: i64 = connection
                    .query_row(
                        "SELECT MAX(version) FROM schema_migrations",
                        [],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(version, LATEST_SCHEMA_VERSION);

                let table_count: i64 = connection
                    .query_row(
                        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN ('decks', 'notes', 'cards', 'assets', 'app_settings', 'practice_sessions', 'card_schedules', 'review_events')",
                        [],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(table_count, 8);
                Ok(())
            })
            .expect("second migration pass should succeed");
    }
}
