use std::{path::Path, sync::Mutex, time::Duration};

use rusqlite::{params, Connection, OptionalExtension};

use crate::{
    data_repairs::{apply_required_repairs, verify_repair_ledger, RepairPhase},
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
    Migration { version: 1, name: "create_decks", checksum: "771e157d6f7eb5e1c024862966718e46ef744298f49287636c75167065904fd6", sql: include_str!("../migrations/0001_decks.sql") },
    Migration { version: 2, name: "cards_assets_settings_practice", checksum: "af64ceea1bfb3f1c04fed33f4558fae051174b50b15873d9ddf5ab4b36e55c8b", sql: include_str!("../migrations/0002_cards_assets_settings_practice.sql") },
    Migration { version: 3, name: "fsrs_reviews", checksum: "49dd1e24e463c9a08a59e243d90143e2874459dd2a4a8d5f61613d5df7ebcae3", sql: include_str!("../migrations/0003_fsrs_reviews.sql") },
    Migration { version: 4, name: "card_organization", checksum: "0e0e25b3de24c04614aa949fce883fb6cc5bf12d1aab6e08dcaab12c548a768d", sql: include_str!("../migrations/0004_card_organization.sql") },
    Migration { version: 5, name: "remove_legacy_duplicate_trigger", checksum: "e275254085414039a7cf5f3a5e08ae35a61ea8f7d3d1b8e016a8d1f087c03029", sql: include_str!("../migrations/0005_remove_legacy_duplicate_trigger.sql") },
    Migration { version: 6, name: "card_types_and_templates", checksum: "c07b900b2119f6eae6a4de58aeb3af9ab5882ee21d5c29ac7a5c7fcee6b86be8", sql: include_str!("../migrations/0006_card_types_and_templates.sql") },
    Migration { version: 7, name: "rich_templates_single_cloze", checksum: "91d120cb0b72c72c40d499a2c8be9b090be96ca760b2bb5a71072e2b71ed45e1", sql: include_str!("../migrations/0007_rich_templates_single_cloze.sql") },
    Migration { version: 8, name: "scheduling_controls", checksum: "862b5277d92154d5586b7924023fb64f7cfee08d566b52efbd625fcbab410b2e", sql: include_str!("../migrations/0008_scheduling_controls.sql") },
    Migration { version: 9, name: "operation_boundaries", checksum: "1482b734fadc2e71720e53c397131916179a3e7966c8461bf9d4fe3d5e51dc15", sql: include_str!("../migrations/0009_operation_boundaries.sql") },
    Migration { version: 10, name: "transactional_schedule_actions", checksum: "bff6ab81045823654725e1f022de7c2c9b1715c63205273a24fd7db03eb0a82a", sql: include_str!("../migrations/0010_transactional_schedule_actions.sql") },
    Migration { version: 11, name: "managed_asset_lifecycle", checksum: "fc9d0b9601cbb45a99bb93b8ed448e150013186b88cb87430c84193176ad7280", sql: include_str!("../migrations/0011_managed_asset_lifecycle.sql") },
    Migration { version: 12, name: "versioned_data_repairs", checksum: "6776fd4d70ed8bac79dd01774d27d7dca97ece65742dabd69a55077dc6a721a3", sql: include_str!("../migrations/0012_versioned_data_repairs.sql") },
    Migration { version: 13, name: "canonical_data_model", checksum: "2e56fa0b68049f8a4d2e937b1dbdb18ec45d9c9041b06f5994b3c847f1d2f049", sql: include_str!("../migrations/0013_canonical_data_model.sql") },
    Migration { version: 14, name: "review_attribution_and_projection_indexes", checksum: "ed5b4f510d014b34f7c6b83c3fcf7914baaa3942e51345a74fb8a7179cc1e11a", sql: include_str!("../migrations/0014_review_attribution_and_projection_indexes.sql") },
    Migration { version: 15, name: "ai_provider_profiles", checksum: "0aa10f24550381da1ffd8c5939d56540186b913faea280dcd8dc492bee3aae2e", sql: include_str!("../migrations/0015_ai_provider_profiles.sql") },
    Migration { version: 16, name: "ai_provider_probe_capabilities", checksum: "12b4ec2518478a9798a19e87577f440a89943d06abfe7cb725f56727a854b75a", sql: include_str!("../migrations/0016_ai_provider_probe_capabilities.sql") },
];

pub(crate) const LATEST_SCHEMA_VERSION: i64 = 16;

pub struct Database {
    connection: Mutex<Connection>,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self, String> {
        let mut connection = Connection::open(path)
            .map_err(|error| format!("Could not open {}: {error}", path.display()))?;
        initialize_connection(&mut connection)?;
        Ok(Self { connection: Mutex::new(connection) })
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
        Ok(Self { connection: Mutex::new(connection) })
    }
}

fn initialize_connection(connection: &mut Connection) -> Result<(), String> {
    configure_connection(connection)?;
    apply_migrations_through(connection, 12)?;
    apply_required_repairs(connection, RepairPhase::BeforeMigration(13))?;
    apply_migrations_after(connection, 12)?;
    apply_required_repairs(connection, RepairPhase::AfterMigration(14))?;
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

fn apply_migrations_through(connection: &mut Connection, maximum_version: i64) -> Result<(), String> {
    apply_migration_range(connection, 1, maximum_version)
}

fn apply_migrations_after(connection: &mut Connection, minimum_exclusive: i64) -> Result<(), String> {
    apply_migration_range(connection, minimum_exclusive.saturating_add(1), LATEST_SCHEMA_VERSION)
}

fn apply_migration_range(
    connection: &mut Connection,
    minimum_version: i64,
    maximum_version: i64,
) -> Result<(), String> {
    initialize_migration_ledger(connection)?;
    let newest_version: Option<i64> = connection
        .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| row.get(0))
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
            .map_err(|error| format!("Could not inspect migration {}: {error}", migration.version))?;
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
        if migration.version == 13 {
            apply_foreign_key_disabled_migration(connection, migration, applied_at)?;
        } else {
            apply_transactional_migration(connection, migration, applied_at)?;
        }
    }
    Ok(())
}

fn apply_transactional_migration(
    connection: &mut Connection,
    migration: &Migration,
    applied_at: i64,
) -> Result<(), String> {
    let transaction = connection.transaction().map_err(|error| {
        format!("Could not begin database migration {}: {error}", migration.version)
    })?;
    transaction
        .execute_batch(migration.sql)
        .map_err(|error| format!("Could not apply migration {}: {error}", migration.version))?;
    record_migration(&transaction, migration, applied_at)?;
    transaction
        .commit()
        .map_err(|error| format!("Could not commit migration {}: {error}", migration.version))
}

fn record_migration(
    connection: &Connection,
    migration: &Migration,
    applied_at: i64,
) -> Result<(), String> {
    connection
        .execute(
            "INSERT INTO schema_migrations (version, name, checksum, applied_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![migration.version, migration.name, migration.checksum, applied_at],
        )
        .map_err(|error| format!("Could not record migration {}: {error}", migration.version))?;
    connection
        .execute_batch(&format!("PRAGMA user_version = {};", migration.version))
        .map_err(|error| {
            format!(
                "Could not set SQLite user_version for migration {}: {error}",
                migration.version
            )
        })
}

fn apply_foreign_key_disabled_migration(
    connection: &mut Connection,
    migration: &Migration,
    applied_at: i64,
) -> Result<(), String> {
    connection
        .execute_batch("PRAGMA foreign_keys = OFF;")
        .map_err(|error| format!("Could not disable foreign keys for migration {}: {error}", migration.version))?;

    let migration_result = (|| {
        let transaction = connection.transaction().map_err(|error| {
            format!("Could not begin database migration {}: {error}", migration.version)
        })?;
        transaction
            .execute_batch(migration.sql)
            .map_err(|error| format!("Could not apply migration {}: {error}", migration.version))?;

        let has_violation = {
            let mut statement = transaction
                .prepare("PRAGMA foreign_key_check")
                .map_err(|error| format!("Could not prepare migration {} foreign-key validation: {error}", migration.version))?;
            let mut rows = statement.query([]).map_err(|error| {
                format!("Could not run migration {} foreign-key validation: {error}", migration.version)
            })?;
            rows.next()
                .map_err(|error| format!("Could not inspect migration {} foreign-key validation: {error}", migration.version))?
                .is_some()
        };
        if has_violation {
            return Err(format!(
                "Migration {} would leave invalid foreign-key references. The database was not modified.",
                migration.version
            ));
        }
        record_migration(&transaction, migration, applied_at)?;
        transaction
            .commit()
            .map_err(|error| format!("Could not commit migration {}: {error}", migration.version))
    })();

    let enforcement_result = connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .map_err(|error| format!("Could not restore foreign-key enforcement after migration {}: {error}", migration.version));

    match (migration_result, enforcement_result) {
        (Err(error), _) => Err(error),
        (Ok(()), Err(error)) => Err(error),
        (Ok(()), Ok(())) => Ok(()),
    }
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
    fn migrations_and_repairs_are_idempotent_and_reach_latest_schema() {
        let database = Database::open_in_memory().expect("database should open");
        database
            .with_connection(|connection| {
                let before = connection.total_changes();
                apply_migrations(connection)
                    .map_err(|error| CommandError::new("migration_error", error))?;
                apply_required_repairs(connection, RepairPhase::BeforeMigration(13))
                    .map_err(|error| CommandError::new("repair_error", error))?;
                apply_required_repairs(connection, RepairPhase::AfterMigration(14))
                    .map_err(|error| CommandError::new("repair_error", error))?;
                assert_eq!(before, connection.total_changes());
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
                    "practice_session_card_snapshots",
                    "practice_session_card_tag_snapshots",
                    "review_event_attributions",
                    "review_event_tag_attributions",
                    "data_repairs",
                    "ai_provider_profiles",
                    "ai_task_defaults",
                    "ai_provider_probe_results",
                    "ai_provider_capabilities",
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
                Ok(())
            })
            .expect("canonical reopen should not write");
    }

    #[test]
    fn released_schema_upgrades_forward_without_losing_integrity() {
        let fixture = &RELEASED_SCHEMA_FIXTURES[0];
        assert_eq!(fixture.release, "0.1.0");
        assert_eq!(fixture.schema_version, 3);
        let mut connection = Connection::open_in_memory().expect("database should open");
        connection.execute_batch(fixture.sql).expect("release fixture should load");
        initialize_connection(&mut connection).expect("fixture should upgrade");
        let version: i64 = connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .expect("version should load");
        assert_eq!(version, LATEST_SCHEMA_VERSION);
        let foreign_key_errors: i64 = connection
            .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| row.get(0))
            .expect("foreign key check should run");
        assert_eq!(foreign_key_errors, 0);
    }
}
