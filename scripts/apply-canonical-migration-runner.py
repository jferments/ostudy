#!/usr/bin/env python3
"""Restore the production migration-13 runner and exact checksum once."""

from __future__ import annotations

import hashlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DATABASE = ROOT / "src-tauri/src/database.rs"
MIGRATION = ROOT / "src-tauri/migrations/0013_canonical_data_model.sql"

text = DATABASE.read_text(encoding="utf-8")
checksum = hashlib.sha256(MIGRATION.read_bytes()).hexdigest()

old_checksum = 'checksum: "5e1359dea514e0c4d50928def26edf90c3cab9fc9432328deb22b98ba2ae1261",'
new_checksum = f'checksum: "{checksum}",'
if text.count(old_checksum) != 1:
    raise SystemExit("migration 13 registration checksum did not match")
text = text.replace(old_checksum, new_checksum, 1)

old_assertion = '''        assert_eq!(
            MIGRATIONS[12].checksum,
            "5e1359dea514e0c4d50928def26edf90c3cab9fc9432328deb22b98ba2ae1261"
        );'''
new_assertion = f'''        assert_eq!(MIGRATIONS[12].checksum, "{checksum}");'''
if text.count(old_assertion) != 1:
    raise SystemExit("migration 13 checksum assertion did not match")
text = text.replace(old_assertion, new_assertion, 1)

old_dispatch = '''        let applied_at = unix_time_millis().map_err(|error| error.message)?;
        let transaction = connection.transaction().map_err(|error| {'''
new_dispatch = '''        let applied_at = unix_time_millis().map_err(|error| error.message)?;
        if migration.version == 13 {
            apply_foreign_key_disabled_migration(connection, migration, applied_at)?;
            continue;
        }
        let transaction = connection.transaction().map_err(|error| {'''
if text.count(old_dispatch) != 1:
    raise SystemExit("migration dispatch insertion point did not match")
text = text.replace(old_dispatch, new_dispatch, 1)

marker = '''    Ok(())
}

#[cfg(test)]
mod tests {'''
function = '''    Ok(())
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
mod tests {'''
if text.count(marker) != 1:
    raise SystemExit("migration helper insertion point did not match")
text = text.replace(marker, function, 1)

DATABASE.write_text(text, encoding="utf-8")
print(f"Registered migration 13 checksum {checksum} and restored its physical rebuild runner.")
