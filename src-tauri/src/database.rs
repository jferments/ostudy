use std::{path::Path, sync::Mutex, time::Duration};

use rusqlite::{params, Connection, OptionalExtension};

use crate::{
    data_repairs::{apply_required_repairs, verify_repair_ledger},
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
    Migration {
        version: 8,
        name: "scheduling_controls",
        checksum: "862b5277d92154d5586b7924023fb64f7cfee08d566b52efbd625fcbab410b2e",
        sql: include_str!("../migrations/0008_scheduling_controls.sql"),
    },
    Migration {
        version: 9,
        name: "operation_boundaries",
        checksum: "1482b734fadc2e71720e53c397131916179a3e7966c8461bf9d4fe3d5e51dc15",
        sql: include_str!("../migrations/0009_operation_boundaries.sql"),
    },
    Migration {
        version: 10,
        name: "transactional_schedule_actions",
        checksum: "bff6ab81045823654725e1f022de7c2c9b1715c63205273a24fd7db03eb0a82a",
        sql: include_str!("../migrations/0010_transactional_schedule_actions.sql"),
    },
    Migration {
        version: 11,
        name: "managed_asset_lifecycle",
        checksum: "fc9d0b9601cbb45a99bb93b8ed448e150013186b88cb87430c84193176ad7280",
        sql: include_str!("../migrations/0011_managed_asset_lifecycle.sql"),
    },
    Migration {
        version: 12,
        name: "versioned_data_repairs",
        checksum: "6776fd4d70ed8bac79dd01774d27d7dca97ece65742dabd69a55077dc6a721a3",
        sql: include_str!("../migrations/0012_versioned_data_repairs.sql"),
    },
    Migration {
        version: 13,
        name: "canonical_data_model",
        checksum: "5e1359dea514e0c4d50928def26edf90c3cab9fc9432328deb22b98ba2ae1261",
        sql: include_str!("../migrations/0013_canonical_data_model.sql"),
    },
];

pub(crate) const LATEST_SCHEMA_VERSION: i64 = 13;

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
    apply_migrations_through(connection, 12)?;
    apply_required_repairs(connection)?;
    apply_migrations_after(connection, 12)?;
    verify_repair_ledger(connection)?;
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

#[cfg(test)]
fn apply_migrations(connection: &mut Connection) -> Result<(), String> {
    apply_migration_range(connection, 1, LATEST_SCHEMA_VERSION)
}

fn apply_migrations_through(
    connection: &mut Connection,
    maximum_version: i64,
) -> Result<(), String> {
    apply_migration_range(connection, 1, maximum_version)
}

fn apply_migrations_after(
    connection: &mut Connection,
    minimum_exclusive: i64,
) -> Result<(), String> {
    apply_migration_range(
        connection,
        minimum_exclusive.saturating_add(1),
        LATEST_SCHEMA_VERSION,
    )
}

fn apply_migration_range(
    connection: &mut Connection,
    minimum_version: i64,
    maximum_version: i64,
) -> Result<(), String> {
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

    for migration in MIGRATIONS.iter().filter(|migration| {
        migration.version >= minimum_version && migration.version <= maximum_version
    }) {
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
    fn migrations_and_repairs_are_idempotent_and_reach_schema_thirteen() {
        let database = Database::open_in_memory().expect("database should open");
        database
            .with_connection(|connection| {
                apply_migrations(connection)
                    .map_err(|error| CommandError::new("migration_error", error))?;
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
                    "schedule_action_operations",
                    "schedule_action_events",
                    "asset_drafts",
                    "asset_reconciliation_events",
                    "deletion_operations",
                    "data_repairs",
                    "deck_import_operations",
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
                for (table, column) in [
                    ("cards", "buried_until"),
                    ("card_schedules", "revision"),
                    ("practice_sessions", "mode"),
                    ("practice_session_cards", "dismissed_at"),
                    ("practice_sessions", "start_operation_id"),
                    ("practice_sessions", "start_request_fingerprint"),
                    ("review_events", "request_fingerprint"),
                    ("review_events", "result_completed"),
                    ("review_events", "result_rated_count"),
                    ("review_events", "result_correct_count"),
                    ("schedule_action_operations", "request_fingerprint"),
                    ("schedule_action_operations", "changed_count"),
                    ("schedule_action_events", "operation_id"),
                    ("assets", "lifecycle_state"),
                    ("assets", "draft_id"),
                    ("assets", "staged_relative_path"),
                    ("assets", "quarantine_relative_path"),
                    ("deletion_operations", "expires_at"),
                    ("data_repairs", "changed_rows"),
                    ("deck_import_operations", "request_fingerprint"),
                ] {
                    let exists: i64 = connection
                        .query_row(
                            "SELECT EXISTS(
                                 SELECT 1 FROM pragma_table_info(?1) WHERE name = ?2
                             )",
                            params![table, column],
                            |row| row.get(0),
                        )
                        .map_err(CommandError::database)?;
                    assert_eq!(exists, 1, "missing {table}.{column}");
                }
                Ok(())
            })
            .expect("second initialization should succeed");
    }

    #[test]
    fn schema_four_upgrades_forward_without_rewriting_history_or_losing_cards() {
        let mut connection = Connection::open_in_memory().expect("database should open");
        configure_connection(&connection).expect("connection should configure");
        initialize_migration_ledger(&connection).expect("ledger should initialize");
        for migration in MIGRATIONS.iter().filter(|migration| migration.version <= 4) {
            let transaction = connection.transaction().expect("migration should start");
            transaction
                .execute_batch(migration.sql)
                .expect("historical migration should apply");
            transaction
                .execute(
                    "INSERT INTO schema_migrations (version, name, checksum, applied_at)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![
                        migration.version,
                        migration.name,
                        migration.checksum,
                        migration.version
                    ],
                )
                .expect("migration should be recorded");
            transaction
                .execute_batch(&format!("PRAGMA user_version = {};", migration.version))
                .expect("user version should advance");
            transaction.commit().expect("migration should commit");
        }
        connection
            .execute(
                "INSERT INTO decks (
                     id, name, description, sort_order, created_at, updated_at, revision
                 ) VALUES (?1, 'Preserved deck', '', 0, 10, 10, 1)",
                ["11111111111111111111111111111111"],
            )
            .expect("deck should insert");
        connection
            .execute(
                "INSERT INTO notes (
                     id, deck_id, note_type, content_format, content_schema_version,
                     front_doc, back_doc, front_text, back_text, duplicate_hash,
                     created_at, updated_at, revision
                 ) VALUES (
                     ?1, ?2, 'basic', 'tiptap-json', 1,
                     '{\"type\":\"doc\",\"content\":[{\"type\":\"paragraph\",\"content\":[{\"type\":\"text\",\"text\":\"Question\"}]}]}',
                     '{\"type\":\"doc\",\"content\":[{\"type\":\"paragraph\",\"content\":[{\"type\":\"text\",\"text\":\"Answer\"}]}]}',
                     'Question', 'Answer', 'legacy-hash', 10, 10, 1
                 )",
                params![
                    "22222222222222222222222222222222",
                    "11111111111111111111111111111111"
                ],
            )
            .expect("note should insert");
        connection
            .execute(
                "INSERT INTO cards (
                     id, note_id, deck_id, direction, template_ordinal, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, 'forward', 0, 10, 10)",
                params![
                    "33333333333333333333333333333333",
                    "22222222222222222222222222222222",
                    "11111111111111111111111111111111"
                ],
            )
            .expect("card should insert");

        initialize_connection(&mut connection).expect("schema 4 should upgrade");

        let preserved: (String, String, String) = connection
            .query_row(
                "SELECT n.front_text, n.back_text, c.id
                 FROM notes n JOIN cards c ON c.note_id = n.id
                 WHERE n.id = ?1",
                ["22222222222222222222222222222222"],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("card should remain");
        assert_eq!(preserved.0, "Question");
        assert_eq!(preserved.1, "Answer");
        assert_eq!(preserved.2, "33333333333333333333333333333333");
        let trigger_exists: i64 = connection
            .query_row(
                "SELECT EXISTS(
                     SELECT 1 FROM sqlite_master
                     WHERE type = 'trigger'
                       AND name = 'notes_duplicate_hash_after_legacy_update'
                 )",
                [],
                |row| row.get(0),
            )
            .expect("trigger query should work");
        assert_eq!(trigger_exists, 0);

        let migrated_version: i64 = connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .expect("version should be readable");
        assert_eq!(migrated_version, LATEST_SCHEMA_VERSION);
    }

    #[test]
    fn every_released_schema_fixture_upgrades_to_latest() {
        for fixture in RELEASED_SCHEMA_FIXTURES {
            let mut connection = Connection::open_in_memory().expect("fixture should open");
            connection
                .execute_batch(fixture.sql)
                .unwrap_or_else(|error| panic!("{} fixture should load: {error}", fixture.release));
            initialize_connection(&mut connection).unwrap_or_else(|error| {
                panic!("{} fixture should upgrade: {error}", fixture.release)
            });
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
        assert_eq!(
            MIGRATIONS[7].checksum,
            "862b5277d92154d5586b7924023fb64f7cfee08d566b52efbd625fcbab410b2e"
        );
        assert_eq!(
            MIGRATIONS[8].checksum,
            "1482b734fadc2e71720e53c397131916179a3e7966c8461bf9d4fe3d5e51dc15"
        );
        assert_eq!(
            MIGRATIONS[9].checksum,
            "bff6ab81045823654725e1f022de7c2c9b1715c63205273a24fd7db03eb0a82a"
        );
        assert_eq!(
            MIGRATIONS[10].checksum,
            "fc9d0b9601cbb45a99bb93b8ed448e150013186b88cb87430c84193176ad7280"
        );
        assert_eq!(
            MIGRATIONS[11].checksum,
            "6776fd4d70ed8bac79dd01774d27d7dca97ece65742dabd69a55077dc6a721a3"
        );
        assert_eq!(
            MIGRATIONS[12].checksum,
            "5e1359dea514e0c4d50928def26edf90c3cab9fc9432328deb22b98ba2ae1261"
        );
    }
}
