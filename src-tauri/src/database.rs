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
        checksum: "49dd1e24e463c9a08a59e243d90143e2874459dd2a4a8d5f61613d5df7ebcae3",
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
                "The local database lock could not be acquired. Restart ostudy.",
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

    #[test]
    fn every_released_schema_fixture_upgrades_without_losing_seed_data() {
        for fixture in RELEASED_SCHEMA_FIXTURES {
            let mut connection =
                Connection::open_in_memory().expect("fixture database should open");

            // A release fixture is a complete historical SQL dump. Loading it
            // before configuring foreign keys avoids making the test depend on
            // the table ordering chosen by SQLite's dump implementation.
            connection
                .execute_batch(fixture.sql)
                .unwrap_or_else(|error| panic!("{} fixture should load: {error}", fixture.release));
            configure_connection(&connection).expect("fixture connection should configure");

            let starting_version: i64 = connection
                .query_row("PRAGMA user_version", [], |row| row.get(0))
                .expect("fixture user_version should be readable");
            assert_eq!(
                starting_version, fixture.schema_version,
                "fixture {}",
                fixture.release
            );

            apply_migrations(&mut connection).unwrap_or_else(|error| {
                panic!("{} fixture should upgrade: {error}", fixture.release)
            });

            let version: i64 = connection
                .query_row("PRAGMA user_version", [], |row| row.get(0))
                .expect("user_version should be readable");
            assert_eq!(
                version, LATEST_SCHEMA_VERSION,
                "fixture {}",
                fixture.release
            );

            let preserved: (String, String, String) = connection
                .query_row(
                    "SELECT d.name, n.front_text, c.direction
                     FROM decks d
                     JOIN notes n ON n.deck_id = d.id
                     JOIN cards c ON c.note_id = n.id
                     WHERE d.id = '00000000000000000000000000000001'",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .expect("fixture deck and card should survive migration");
            assert_eq!(preserved.0, "Released schema fixture");
            assert_eq!(preserved.1, "Fixture question");
            assert_eq!(preserved.2, "forward");

            let schedule_count: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM card_schedules
                     WHERE card_id = '00000000000000000000000000000003'",
                    [],
                    |row| row.get(0),
                )
                .expect("fixture schedule should remain readable");
            assert_eq!(schedule_count, 1);

            let integrity: String = connection
                .query_row("PRAGMA integrity_check", [], |row| row.get(0))
                .expect("integrity check should run");
            assert_eq!(integrity, "ok", "fixture {}", fixture.release);
        }
    }
}
