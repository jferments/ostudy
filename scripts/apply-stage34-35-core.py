#!/usr/bin/env python3
"""Apply the Stage 3.5 repair-ledger and canonical-schema wiring once."""

from __future__ import annotations

import hashlib
import re
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


def regex_once(text: str, pattern: str, replacement: str, label: str) -> str:
    updated, count = re.subn(pattern, replacement, text, count=1, flags=re.S)
    if count != 1:
        raise SystemExit(f"{label}: expected one regex match, found {count}")
    return updated


# Make duplicate-fingerprint repair callable inside the repair transaction.
commands_path = "src-tauri/src/authoring/commands.rs"
commands = read(commands_path)
commands = regex_once(
    commands,
    r"pub\(crate\) fn backfill_duplicate_hashes\(connection: &mut Connection\) -> Result<\(\), String> \{.*?\n\}\n?$",
    '''pub(crate) fn backfill_duplicate_hashes_in_connection(
    connection: &Connection,
) -> Result<i64, String> {
    let rows = {
        let mut statement = connection
            .prepare(
                "SELECT id, front_text, back_text
                 FROM notes
                 WHERE duplicate_hash = '' AND deleted_at IS NULL",
            )
            .map_err(|error| format!("Could not prepare duplicate backfill: {error}"))?;
        statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(|error| format!("Could not read duplicate backfill rows: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("Could not collect duplicate backfill rows: {error}"))?
    };

    for (note_id, front_text, back_text) in &rows {
        let normalized = normalized_pair(front_text, back_text);
        let hash = duplicate_hash(&normalized.0, &normalized.1);
        connection
            .execute(
                "UPDATE notes SET duplicate_hash = ?1 WHERE id = ?2",
                params![hash, note_id],
            )
            .map_err(|error| format!("Could not update duplicate fingerprint: {error}"))?;
    }
    Ok(i64::try_from(rows.len()).unwrap_or(i64::MAX))
}

pub(crate) fn backfill_duplicate_hashes(connection: &mut Connection) -> Result<(), String> {
    let transaction = connection
        .transaction()
        .map_err(|error| format!("Could not start duplicate backfill: {error}"))?;
    backfill_duplicate_hashes_in_connection(&transaction)?;
    transaction
        .commit()
        .map_err(|error| format!("Could not commit duplicate backfill: {error}"))
}
''',
    "duplicate repair helper",
)
write(commands_path, commands)

# Convert the old global startup normalizer into a versioned repair function.
normalization_path = "src-tauri/src/authoring/stage3_v2/normalization.rs"
normalization = read(normalization_path)
old_header = '''pub(crate) fn normalize_stage3_schema(
    connection: &mut Connection,
) -> Result<(), CommandError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    normalize_legacy_template_documents(&transaction)?;
    normalize_explicit_template_placeholders(&transaction)?;
    normalize_cloze_card_rows(&transaction)?;
    transaction.commit().map_err(CommandError::database)
}
'''
new_header = '''pub(crate) fn apply_stage3_data_repairs(
    connection: &Connection,
) -> Result<i64, CommandError> {
    let before = connection.total_changes();
    normalize_abandoned_image_occlusion(connection)?;
    normalize_legacy_template_documents(connection)?;
    normalize_explicit_template_placeholders(connection)?;
    normalize_cloze_card_rows(connection)?;
    let changed = connection.total_changes().saturating_sub(before);
    Ok(i64::try_from(changed).unwrap_or(i64::MAX))
}

pub(crate) fn normalize_stage3_schema(
    connection: &mut Connection,
) -> Result<(), CommandError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    apply_stage3_data_repairs(&transaction)?;
    transaction.commit().map_err(CommandError::database)
}

fn normalize_abandoned_image_occlusion(
    connection: &Connection,
) -> Result<(), CommandError> {
    let note_ids = {
        let mut statement = connection
            .prepare("SELECT id FROM notes WHERE note_type = 'image_occlusion'")
            .map_err(CommandError::database)?;
        statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?
    };
    let now = unix_time_millis()?;
    for note_id in note_ids {
        connection
            .execute(
                "UPDATE notes
                 SET note_type = 'basic', reverse_enabled = 0,
                     cloze_source = '', cloze_mode = 'one_at_a_time',
                     updated_at = MAX(updated_at, ?1), revision = revision + 1
                 WHERE id = ?2 AND note_type = 'image_occlusion'",
                params![now, note_id],
            )
            .map_err(CommandError::database)?;
        let cards = {
            let mut statement = connection
                .prepare(
                    "SELECT rowid, id, deleted_at
                     FROM cards WHERE note_id = ?1
                     ORDER BY CASE WHEN deleted_at IS NULL THEN 0 ELSE 1 END,
                              created_at, id",
                )
                .map_err(CommandError::database)?;
            statement
                .query_map([note_id.as_str()], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<i64>>(2)?,
                    ))
                })
                .map_err(CommandError::database)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(CommandError::database)?
        };
        if let Some(canonical) = cards.first() {
            connection
                .execute(
                    "UPDATE cards
                     SET direction = 'forward', template_ordinal = 0,
                         deleted_at = NULL, updated_at = MAX(updated_at, ?1)
                     WHERE id = ?2",
                    params![now, canonical.1],
                )
                .map_err(CommandError::database)?;
            for (rowid, card_id, _) in cards.iter().skip(1) {
                connection
                    .execute(
                        "UPDATE cards
                         SET direction = 'forward', template_ordinal = ?1,
                             deleted_at = COALESCE(deleted_at, ?2),
                             updated_at = MAX(updated_at, ?2)
                         WHERE id = ?3",
                        params![1_500_000_i64.saturating_add(*rowid), now, card_id],
                    )
                    .map_err(CommandError::database)?;
            }
        }
    }
    Ok(())
}
'''
normalization = replace_once(normalization, old_header, new_header, "stage3 repair entry")
normalization = replace_once(
    normalization,
    "SET direction = 'cloze', template_ordinal = ?1,",
    "SET direction = 'forward', template_ordinal = ?1,",
    "retired cloze direction",
)
write(normalization_path, normalization)

# Register migration 12, run repairs before the schema-narrowing migration 13,
# and make canonical startup read-only after the ledgers are complete.
database_path = "src-tauri/src/database.rs"
database = read(database_path)
database = replace_once(
    database,
    '''use crate::{
    authoring::{backfill_duplicate_hashes, normalize_stage3_schema},
    error::CommandError,
    util::unix_time_millis,
};''',
    '''use crate::{
    data_repairs::{apply_required_repairs, verify_repair_ledger},
    error::CommandError,
    util::unix_time_millis,
};''',
    "database imports",
)
checksum12 = hashlib.sha256(read("src-tauri/migrations/0012_versioned_data_repairs.sql").encode()).hexdigest()
checksum13 = hashlib.sha256(read("src-tauri/migrations/0013_canonical_data_model.sql").encode()).hexdigest()
needle = '''    Migration {
        version: 11,
        name: "managed_asset_lifecycle",
        checksum: "fc9d0b9601cbb45a99bb93b8ed448e150013186b88cb87430c84193176ad7280",
        sql: include_str!("../migrations/0011_managed_asset_lifecycle.sql"),
    },
];

const LATEST_SCHEMA_VERSION: i64 = 11;'''
replacement = f'''    Migration {{
        version: 11,
        name: "managed_asset_lifecycle",
        checksum: "fc9d0b9601cbb45a99bb93b8ed448e150013186b88cb87430c84193176ad7280",
        sql: include_str!("../migrations/0011_managed_asset_lifecycle.sql"),
    }},
    Migration {{
        version: 12,
        name: "versioned_data_repairs",
        checksum: "{checksum12}",
        sql: include_str!("../migrations/0012_versioned_data_repairs.sql"),
    }},
    Migration {{
        version: 13,
        name: "canonical_data_model",
        checksum: "{checksum13}",
        sql: include_str!("../migrations/0013_canonical_data_model.sql"),
    }},
];

pub(crate) const LATEST_SCHEMA_VERSION: i64 = 13;'''
database = replace_once(database, needle, replacement, "migration registration")
database = replace_once(
    database,
    '''fn initialize_connection(connection: &mut Connection) -> Result<(), String> {
    configure_connection(connection)?;
    apply_migrations(connection)?;
    backfill_duplicate_hashes(connection)?;
    normalize_stage3_schema(connection).map_err(|error| error.message)?;
    Ok(())
}''',
    '''fn initialize_connection(connection: &mut Connection) -> Result<(), String> {
    configure_connection(connection)?;
    apply_migrations_through(connection, 12)?;
    apply_required_repairs(connection)?;
    apply_migrations_after(connection, 12)?;
    verify_repair_ledger(connection)?;
    Ok(())
}''',
    "startup ordering",
)
database = replace_once(
    database,
    "fn apply_migrations(connection: &mut Connection) -> Result<(), String> {\n",
    '''fn apply_migrations(connection: &mut Connection) -> Result<(), String> {
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
''',
    "migration range function",
)
database = replace_once(
    database,
    "    for migration in MIGRATIONS {\n",
    '''    for migration in MIGRATIONS
        .iter()
        .filter(|migration| {
            migration.version >= minimum_version && migration.version <= maximum_version
        })
    {
''',
    "migration range loop",
)
database = database.replace(
    "migrations_are_idempotent_and_reach_schema_eleven",
    "migrations_and_repairs_are_idempotent_and_reach_schema_thirteen",
)
database = replace_once(
    database,
    '''                    "asset_reconciliation_events",
                    "deletion_operations",''',
    '''                    "asset_reconciliation_events",
                    "deletion_operations",
                    "data_repairs",
                    "deck_import_operations",''',
    "new schema tables",
)
database = replace_once(
    database,
    '''                    ("deletion_operations", "expires_at"),''',
    '''                    ("deletion_operations", "expires_at"),
                    ("data_repairs", "changed_rows"),
                    ("deck_import_operations", "request_fingerprint"),''',
    "new schema columns",
)
database = replace_once(
    database,
    '''        assert_eq!(
            MIGRATIONS[10].checksum,
            "fc9d0b9601cbb45a99bb93b8ed448e150013186b88cb87430c84193176ad7280"
        );''',
    f'''        assert_eq!(
            MIGRATIONS[10].checksum,
            "fc9d0b9601cbb45a99bb93b8ed448e150013186b88cb87430c84193176ad7280"
        );
        assert_eq!(MIGRATIONS[11].checksum, "{checksum12}");
        assert_eq!(MIGRATIONS[12].checksum, "{checksum13}");''',
    "checksum assertions",
)
write(database_path, database)

# Register the repair module at the native composition root.
lib_path = "src-tauri/src/lib.rs"
lib = read(lib_path)
lib = replace_once(lib, "mod database;\n", "mod database;\nmod data_repairs;\n", "repair module")
write(lib_path, lib)

print("Applied Stage 3.5 repair and canonical-schema wiring.")
