#!/usr/bin/env python3
"""Apply the final canonical migration runner and regression corrections once."""

from __future__ import annotations

import hashlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DATABASE = ROOT / "src-tauri/src/database.rs"
MIGRATION = ROOT / "src-tauri/migrations/0013_canonical_data_model.sql"
REGRESSION = ROOT / "src-tauri/src/stage34_35_regression_tests.rs"


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one match, found {count}")
    return text.replace(old, new, 1)


checksum = hashlib.sha256(MIGRATION.read_bytes()).hexdigest()

database = DATABASE.read_text(encoding="utf-8")
database = replace_once(
    database,
    'checksum: "5e1359dea514e0c4d50928def26edf90c3cab9fc9432328deb22b98ba2ae1261",',
    f'checksum: "{checksum}",',
    "migration 13 checksum registration",
)
database = replace_once(
    database,
    '''        assert_eq!(
            MIGRATIONS[12].checksum,
            "5e1359dea514e0c4d50928def26edf90c3cab9fc9432328deb22b98ba2ae1261"
        );''',
    f'''        assert_eq!(MIGRATIONS[12].checksum, "{checksum}");''',
    "migration 13 checksum assertion",
)
database = replace_once(
    database,
    '''        let applied_at = unix_time_millis().map_err(|error| error.message)?;
        let transaction = connection.transaction().map_err(|error| {''',
    '''        let applied_at = unix_time_millis().map_err(|error| error.message)?;
        if migration.version == 13 {
            apply_foreign_key_disabled_migration(connection, migration, applied_at)?;
            continue;
        }
        let transaction = connection.transaction().map_err(|error| {''',
    "migration 13 dispatch",
)
database = replace_once(
    database,
    '''    Ok(())
}

#[cfg(test)]
mod tests {''',
    '''    Ok(())
}

fn apply_foreign_key_disabled_migration(
    connection: &mut Connection,
    migration: &Migration,
    applied_at: i64,
) -> Result<(), String> {
    connection
        .execute_batch("PRAGMA foreign_keys = OFF;")
        .map_err(|error| {
            format!(
                "Could not disable foreign keys for migration {}: {error}",
                migration.version
            )
        })?;

    let migration_result = (|| {
        let transaction = connection.transaction().map_err(|error| {
            format!(
                "Could not begin database migration {}: {error}",
                migration.version
            )
        })?;
        transaction
            .execute_batch(migration.sql)
            .map_err(|error| format!("Could not apply migration {}: {error}", migration.version))?;

        let has_violation = {
            let mut statement = transaction
                .prepare("PRAGMA foreign_key_check")
                .map_err(|error| {
                    format!(
                        "Could not prepare migration {} foreign-key validation: {error}",
                        migration.version
                    )
                })?;
            let mut rows = statement.query([]).map_err(|error| {
                format!(
                    "Could not run migration {} foreign-key validation: {error}",
                    migration.version
                )
            })?;
            rows.next()
                .map_err(|error| {
                    format!(
                        "Could not inspect migration {} foreign-key validation: {error}",
                        migration.version
                    )
                })?
                .is_some()
        };
        if has_violation {
            return Err(format!(
                "Migration {} would leave invalid foreign-key references. The database was not modified.",
                migration.version
            ));
        }

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
        })
    })();

    let enforcement_result = connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .map_err(|error| {
            format!(
                "Could not restore foreign-key enforcement after migration {}: {error}",
                migration.version
            )
        });

    match (migration_result, enforcement_result) {
        (Err(error), _) => Err(error),
        (Ok(()), Err(error)) => Err(error),
        (Ok(()), Ok(())) => Ok(()),
    }
}

#[cfg(test)]
mod tests {''',
    "physical migration runner",
)
DATABASE.write_text(database, encoding="utf-8")

regression = REGRESSION.read_text(encoding="utf-8")
regression = replace_once(
    regression,
    '''    connection
        .execute_batch(CANONICAL_MIGRATION)
        .expect("canonical migration should apply");''',
    '''    connection
        .execute_batch("PRAGMA foreign_keys = OFF;")
        .expect("foreign keys should disable for the physical rebuild");
    connection
        .execute_batch(CANONICAL_MIGRATION)
        .expect("canonical migration should apply");
    let foreign_key_violations: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM pragma_foreign_key_check",
            [],
            |row| row.get(0),
        )
        .expect("foreign-key validation should run");
    assert_eq!(foreign_key_violations, 0);
    connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .expect("foreign keys should be restored after the physical rebuild");''',
    "physical migration regression boundary",
)
REGRESSION.write_text(regression, encoding="utf-8")

print(f"Applied final Stage 3.4/3.5 fixes with migration checksum {checksum}.")
