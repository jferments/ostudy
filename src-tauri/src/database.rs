use std::{path::Path, sync::Mutex, time::Duration};

use rusqlite::{params, Connection, OptionalExtension};

use crate::{authoring::backfill_duplicate_hashes, error::CommandError, util::unix_time_millis};

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
        checksum: "f5003f8967aabc5f7295d87051898d7ea0fb02d5d5c40982b7a4745b34e20e06",
        sql: include_str!("../migrations/0006_card_types_and_templates.sql"),
    },
];

const LATEST_SCHEMA_VERSION: i64 = 6;

pub struct Database {
    connection: Mutex<Connection>,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self, String> {
        let mut connection = Connection::open(path)
            .map_err(|error| format!("Could not open {}: {error}", path.display()))?;

        configure_connection(&connection)?;
        apply_migrations(&mut connection)?;
        backfill_duplicate_hashes(&mut connection)?;

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
        backfill_duplicate_hashes(&mut connection)?;

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

                for table in [
                    "decks",
                    "notes",
                    "cards",
                    "assets",
                    "app_settings",
                    "practice_sessions",
                    "card_schedules",
                    "review_events",
                    "tags",
                    "note_tags",
                    "card_search_fts",
                    "card_templates",
                    "card_template_fields",
                    "card_template_rules",
                ] {
                    let exists: i64 = connection
                        .query_row(
                            "SELECT EXISTS(\n                                 SELECT 1 FROM sqlite_master\n                                 WHERE name = ?1\n                             )",
                            [table],
                            |row| row.get(0),
                        )
                        .map_err(CommandError::database)?;
                    assert_eq!(exists, 1, "missing table {table}");
                }

                for column in ["reverse_enabled", "cloze_source", "cloze_mode"] {
                    let exists: i64 = connection
                        .query_row(
                            "SELECT EXISTS(
                                 SELECT 1 FROM pragma_table_info('notes') WHERE name = ?1
                             )",
                            [column],
                            |row| row.get(0),
                        )
                        .map_err(CommandError::database)?;
                    assert_eq!(exists, 1, "missing notes column {column}");
                }

                let obsolete_trigger_exists: i64 = connection
                    .query_row(
                        "SELECT EXISTS(
                             SELECT 1 FROM sqlite_master
                             WHERE type = 'trigger'
                               AND name = 'notes_duplicate_hash_after_legacy_update'
                         )",
                        [],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(obsolete_trigger_exists, 0);
                Ok(())
            })
            .expect("second migration pass should succeed");
    }

    #[test]
    fn schema_four_upgrades_forward_without_rewriting_history_or_losing_cards() {
        let fixture = &RELEASED_SCHEMA_FIXTURES[0];
        let mut connection = Connection::open_in_memory().expect("fixture database should open");
        connection
            .execute_batch(fixture.sql)
            .expect("released fixture should load");
        configure_connection(&connection).expect("fixture connection should configure");

        let migration = &MIGRATIONS[3];
        let transaction = connection
            .transaction()
            .expect("schema 4 transaction should start");
        transaction
            .execute_batch(migration.sql)
            .expect("schema 4 should apply");
        transaction
            .execute(
                "INSERT INTO schema_migrations (version, name, checksum, applied_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![migration.version, migration.name, migration.checksum, 0_i64],
            )
            .expect("schema 4 should be recorded");
        transaction
            .execute_batch("PRAGMA user_version = 4;")
            .expect("schema 4 user_version should be recorded");
        transaction
            .commit()
            .expect("schema 4 transaction should commit");
        backfill_duplicate_hashes(&mut connection)
            .expect("schema 4 duplicate fingerprints should backfill");

        let trigger_before_upgrade: i64 = connection
            .query_row(
                "SELECT EXISTS(
                     SELECT 1 FROM sqlite_master
                     WHERE type = 'trigger'
                       AND name = 'notes_duplicate_hash_after_legacy_update'
                 )",
                [],
                |row| row.get(0),
            )
            .expect("schema 4 trigger state should be readable");
        assert_eq!(trigger_before_upgrade, 1);

        apply_migrations(&mut connection).expect("schema 4 should upgrade to the latest schema");
        backfill_duplicate_hashes(&mut connection)
            .expect("duplicate fingerprints should remain valid after the upgrade");

        let version: i64 = connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .expect("schema version should be readable");
        assert_eq!(version, LATEST_SCHEMA_VERSION);

        let recorded_schema_four: (String, String) = connection
            .query_row(
                "SELECT name, checksum FROM schema_migrations WHERE version = 4",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("schema 4 ledger entry should be readable");
        assert_eq!(recorded_schema_four.0, MIGRATIONS[3].name);
        assert_eq!(recorded_schema_four.1, MIGRATIONS[3].checksum);

        let trigger_after_upgrade: i64 = connection
            .query_row(
                "SELECT EXISTS(
                     SELECT 1 FROM sqlite_master
                     WHERE type = 'trigger'
                       AND name = 'notes_duplicate_hash_after_legacy_update'
                 )",
                [],
                |row| row.get(0),
            )
            .expect("latest trigger state should be readable");
        assert_eq!(trigger_after_upgrade, 0);

        let preserved: (String, String, String, String, i64, String, String) = connection
            .query_row(
                "SELECT d.name, n.front_text, c.direction, n.duplicate_hash,
                        n.reverse_enabled, n.cloze_source, n.cloze_mode
                 FROM decks d
                 JOIN notes n ON n.deck_id = d.id
                 JOIN cards c ON c.note_id = n.id
                 WHERE d.id = '00000000000000000000000000000001'",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                    ))
                },
            )
            .expect("the existing deck and card should remain");
        assert_eq!(preserved.0, "Released schema fixture");
        assert_eq!(preserved.1, "Fixture question");
        assert_eq!(preserved.2, "forward");
        assert!(!preserved.3.is_empty());
        assert_eq!(preserved.4, 0);
        assert_eq!(preserved.5, "");
        assert_eq!(preserved.6, "one_at_a_time");

        let integrity: String = connection
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))
            .expect("integrity check should run");
        assert_eq!(integrity, "ok");
    }

    #[test]
    fn every_released_schema_fixture_upgrades_without_losing_seed_data() {
        for fixture in RELEASED_SCHEMA_FIXTURES {
            let mut connection =
                Connection::open_in_memory().expect("fixture database should open");

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
            backfill_duplicate_hashes(&mut connection)
                .expect("fixture duplicate fingerprints should backfill");

            let version: i64 = connection
                .query_row("PRAGMA user_version", [], |row| row.get(0))
                .expect("user_version should be readable");
            assert_eq!(
                version, LATEST_SCHEMA_VERSION,
                "fixture {}",
                fixture.release
            );

            let preserved: (String, String, String, String) = connection
                .query_row(
                    "SELECT d.name, n.front_text, c.direction, n.duplicate_hash\n                     FROM decks d\n                     JOIN notes n ON n.deck_id = d.id\n                     JOIN cards c ON c.note_id = n.id\n                     WHERE d.id = '00000000000000000000000000000001'",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .expect("fixture deck and card should survive migration");
            assert_eq!(preserved.0, "Released schema fixture");
            assert_eq!(preserved.1, "Fixture question");
            assert_eq!(preserved.2, "forward");
            assert!(!preserved.3.is_empty());

            let search_count: i64 = connection
                .query_row(
                    "SELECT COUNT(*)\n                     FROM card_search_fts\n                     WHERE card_search_fts MATCH 'fixture'",
                    [],
                    |row| row.get(0),
                )
                .expect("fixture should be indexed");
            assert_eq!(search_count, 1);

            let schedule_count: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM card_schedules\n                     WHERE card_id = '00000000000000000000000000000003'",
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
