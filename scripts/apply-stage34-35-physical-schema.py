#!/usr/bin/env python3
"""Wire migration 13's physical parent-table rebuild and strengthen tests."""

from __future__ import annotations

import hashlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def write(path: str, content: str) -> None:
    (ROOT / path).write_text(content, encoding="utf-8")


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one match, found {count}")
    return text.replace(old, new, 1)


checksum = hashlib.sha256(read("src-tauri/migrations/0013_canonical_data_model.sql").encode()).hexdigest()

path = "src-tauri/src/database.rs"
text = read(path)
text = replace_once(
    text,
    'checksum: "5e1359dea514e0c4d50928def26edf90c3cab9fc9432328deb22b98ba2ae1261",',
    f'checksum: "{checksum}",',
    "migration 13 checksum",
)
text = replace_once(
    text,
    '''        let applied_at = unix_time_millis().map_err(|error| error.message)?;
        let transaction = connection.transaction().map_err(|error| {''',
    '''        let applied_at = unix_time_millis().map_err(|error| error.message)?;
        if migration.version == 13 {
            apply_foreign_key_disabled_migration(connection, migration, applied_at)?;
            continue;
        }
        let transaction = connection.transaction().map_err(|error| {''',
    "migration 13 special handling",
)
text = replace_once(
    text,
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

    let result = (|| {
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
                        "Could not prepare the migration {} foreign-key check: {error}",
                        migration.version
                    )
                })?;
            let mut rows = statement.query([]).map_err(|error| {
                format!(
                    "Could not run the migration {} foreign-key check: {error}",
                    migration.version
                )
            })?;
            rows.next()
                .map_err(|error| {
                    format!(
                        "Could not inspect the migration {} foreign-key check: {error}",
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

    let restore_result = connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .map_err(|error| {
            format!(
                "Could not restore foreign-key enforcement after migration {}: {error}",
                migration.version
            )
        });
    match (result, restore_result) {
        (Err(error), _) => Err(error),
        (Ok(()), Err(error)) => Err(error),
        (Ok(()), Ok(())) => Ok(()),
    }
}

#[cfg(test)]
mod tests {''',
    "foreign-key-disabled migration function",
)
text = replace_once(
    text,
    '''        assert_eq!(
            MIGRATIONS[12].checksum,
            "5e1359dea514e0c4d50928def26edf90c3cab9fc9432328deb22b98ba2ae1261"
        );''',
    f'''        assert_eq!(MIGRATIONS[12].checksum, "{checksum}");''',
    "migration 13 checksum test",
)
write(path, text)

path = "src-tauri/src/stage34_35_regression_tests.rs"
text = read(path)
text = replace_once(
    text,
    '''    for column in ["front_pattern", "back_pattern"] {''',
    '''    for (table, forbidden) in [
        ("notes", "image_occlusion"),
        ("cards", "image_occlusion"),
        ("cards", "'cloze'"),
    ] {
        let table_sql: String = connection
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = ?1",
                [table],
                |row| row.get(0),
            )
            .expect("canonical table SQL should exist");
        assert!(
            !table_sql.contains(forbidden),
            "canonical {table} schema still contains {forbidden}: {table_sql}"
        );
    }

    for column in ["front_pattern", "back_pattern"] {''',
    "physical schema assertions",
)
write(path, text)

path = "scripts/verify-stage34-35.mjs"
text = read(path)
text = replace_once(
    text,
    '''  "notes_canonical_type_before_insert",
  "cards_canonical_direction_before_insert",''',
    '''  "CREATE TABLE notes_canonical",
  "CHECK(note_type IN ('basic', 'cloze'))",
  "CREATE TABLE cards_canonical",
  "CHECK(direction IN ('forward', 'reverse'))",
  "PRAGMA foreign_key_check",''',
    "physical schema verifier",
)
write(path, text)

path = "docs/decisions/0009-coherent-transfer-backup-and-repairs.md"
text = read(path)
text = text.replace(
    "Migration 13 then removes the legacy text template columns and installs canonical guards that reject abandoned note types, card directions, and nullable rich template layouts.",
    "Migration 13 physically rebuilds the note and card tables with only `basic`/`cloze` note types and `forward`/`reverse` card directions, rebuilds the FTS projection, removes the legacy text template columns, and rejects nullable rich template layouts. The migration runner disables foreign-key enforcement only around this transaction, runs `foreign_key_check` before commit, and restores enforcement immediately afterward.",
)
write(path, text)

print(f"Applied physical schema cutover with migration checksum {checksum}.")
