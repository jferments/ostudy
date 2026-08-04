#!/usr/bin/env python3
"""Remove superseded runtime wrappers and satisfy warning-free Clippy."""

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


path = "src-tauri/src/authoring/commands.rs"
text = read(path)
text = replace_once(
    text,
    '''

pub(crate) fn backfill_duplicate_hashes(connection: &mut Connection) -> Result<(), String> {
    let transaction = connection
        .transaction()
        .map_err(|error| format!("Could not start duplicate backfill: {error}"))?;
    backfill_duplicate_hashes_in_connection(&transaction)?;
    transaction
        .commit()
        .map_err(|error| format!("Could not commit duplicate backfill: {error}"))
}''',
    "",
    "obsolete duplicate backfill wrapper",
)
write(path, text)

path = "src-tauri/src/authoring/stage3_v2/normalization.rs"
text = read(path)
text = replace_once(
    text,
    '''

pub(crate) fn normalize_stage3_schema(
    connection: &mut Connection,
) -> Result<(), CommandError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    apply_stage3_data_repairs(&transaction)?;
    transaction.commit().map_err(CommandError::database)
}''',
    "",
    "obsolete global Stage 3 normalizer",
)
write(path, text)

path = "src-tauri/src/database.rs"
text = read(path)
text = replace_once(
    text,
    '''fn apply_migrations(connection: &mut Connection) -> Result<(), String> {
    apply_migration_range(connection, 1, LATEST_SCHEMA_VERSION)
}''',
    '''#[cfg(test)]
fn apply_migrations(connection: &mut Connection) -> Result<(), String> {
    apply_migration_range(connection, 1, LATEST_SCHEMA_VERSION)
}''',
    "test-only migration helper",
)
write(path, text)

path = "src-tauri/src/library_backup.rs"
text = read(path)
text = replace_once(
    text,
    "if version < 3 || version > LATEST_SCHEMA_VERSION {",
    "if !(3..=LATEST_SCHEMA_VERSION).contains(&version) {",
    "backup schema range",
)
write(path, text)

path = "src-tauri/src/studydeck.rs"
text = read(path)
text = replace_once(
    text,
    '''fn normalize_tag_name(value: &str) -> String {
    value
        .trim()
        .split_whitespace()''',
    '''fn normalize_tag_name(value: &str) -> String {
    value
        .split_whitespace()''',
    "tag normalization",
)
write(path, text)

path = "src-tauri/src/studydeck/import.rs"
text = read(path)
text = replace_once(
    text,
    '''use crate::scheduler::insert_new_schedule;
use rusqlite::TransactionBehavior;
''',
    '''use crate::scheduler::insert_new_schedule;
use rusqlite::TransactionBehavior;

struct StoredImportOperation {
    request_fingerprint: String,
    status: String,
    result_deck_id: Option<String>,
    result_card_count: Option<i64>,
    result_asset_count: Option<i64>,
}
''',
    "stored import operation type",
)
text = replace_once(
    text,
    '''    let existing: Option<(String, String, Option<String>, Option<i64>, Option<i64>)> = connection
        .query_row(
            "SELECT request_fingerprint, status, result_deck_id,
                    result_card_count, result_asset_count
             FROM deck_import_operations WHERE id = ?1",
            [operation_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .optional()
        .map_err(CommandError::database)?;
    if let Some((recorded_fingerprint, status, deck_id, card_count, asset_count)) = existing {
        if recorded_fingerprint != fingerprint {''',
    '''    let existing: Option<StoredImportOperation> = connection
        .query_row(
            "SELECT request_fingerprint, status, result_deck_id,
                    result_card_count, result_asset_count
             FROM deck_import_operations WHERE id = ?1",
            [operation_id],
            |row| {
                Ok(StoredImportOperation {
                    request_fingerprint: row.get(0)?,
                    status: row.get(1)?,
                    result_deck_id: row.get(2)?,
                    result_card_count: row.get(3)?,
                    result_asset_count: row.get(4)?,
                })
            },
        )
        .optional()
        .map_err(CommandError::database)?;
    if let Some(existing) = existing {
        if existing.request_fingerprint != fingerprint {''',
    "stored import operation query",
)
text = replace_once(
    text,
    '''        if status == "completed" {
            let deck_id = deck_id.ok_or_else(|| {''',
    '''        if existing.status == "completed" {
            let deck_id = existing.result_deck_id.ok_or_else(|| {''',
    "stored completed import",
)
text = replace_once(
    text,
    '''                card_count: card_count.unwrap_or_default(),
                asset_count: asset_count.unwrap_or_default(),''',
    '''                card_count: existing.result_card_count.unwrap_or_default(),
                asset_count: existing.result_asset_count.unwrap_or_default(),''',
    "stored import counts",
)
write(path, text)

print("Applied Stage 3.4/3.5 lint and stale-wrapper cleanup.")
