#!/usr/bin/env python3
"""Apply the first coordinated Stage 3.6–3.8 native cutover.

This temporary, reviewable applicator exists only because the authenticated
GitHub contents API replaces whole files. It computes the migration checksum from
the checked-in SQL, writes the versioned repair coordinator, and makes small
asserted transformations to the existing scheduling/review code. The file is
removed before the PR is marked ready.
"""

from __future__ import annotations

import hashlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def write(path: str, content: str) -> None:
    target = ROOT / path
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(content, encoding="utf-8")


def replace_once(path: str, old: str, new: str) -> None:
    text = read(path)
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected exactly one match, found {count}")
    write(path, text.replace(old, new, 1))


migration = read("src-tauri/migrations/0014_study_calendar_history_projections.sql")
checksum = hashlib.sha256(migration.encode()).hexdigest()

database = read("src-tauri/src/database.rs")
needle = '''    Migration {
        version: 13,
        name: "canonical_data_model",
        checksum: "2e56fa0b68049f8a4d2e937b1dbdb18ec45d9c9041b06f5994b3c847f1d2f049",
        sql: include_str!("../migrations/0013_canonical_data_model.sql"),
    },
];

pub(crate) const LATEST_SCHEMA_VERSION: i64 = 13;
'''
replacement = f'''    Migration {{
        version: 13,
        name: "canonical_data_model",
        checksum: "2e56fa0b68049f8a4d2e937b1dbdb18ec45d9c9041b06f5994b3c847f1d2f049",
        sql: include_str!("../migrations/0013_canonical_data_model.sql"),
    }},
    Migration {{
        version: 14,
        name: "study_calendar_history_projections",
        checksum: "{checksum}",
        sql: include_str!("../migrations/0014_study_calendar_history_projections.sql"),
    }},
];

pub(crate) const LATEST_SCHEMA_VERSION: i64 = 14;
'''
if needle not in database and 'version: 14,' not in database:
    raise SystemExit("database.rs migration anchor changed")
database = database.replace(needle, replacement, 1)
write("src-tauri/src/database.rs", database)

write(
    "src-tauri/src/data_repairs.rs",
    r'''//! One-time, versioned data canonicalization for released ostudy databases.
//!
//! Structural migrations remain immutable SQL history. Repairs are tracked in a
//! separate ledger because validated rich-data transformations cannot always be
//! expressed safely as static SQL. A repair runs only after its required schema
//! exists. Once every applicable row is recorded, ordinary startup performs
//! reads only and cannot churn user timestamps, revisions, FTS rows, or the WAL.

use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};

use crate::{
    authoring::{apply_stage3_data_repairs, backfill_duplicate_hashes_in_connection},
    historical_attribution::backfill_legacy_review_attributions,
    util::unix_time_millis,
};

struct DataRepair {
    version: i64,
    name: &'static str,
    checksum: &'static str,
    required_schema: i64,
    apply: fn(&Transaction<'_>) -> Result<i64, String>,
}

const REPAIRS: &[DataRepair] = &[
    DataRepair {
        version: 1,
        name: "duplicate_fingerprints",
        checksum: "fb2f07be41d479048759ac7d754a59022f2c63c3f60082eaf9a40fd28bab7993",
        required_schema: 12,
        apply: repair_duplicate_fingerprints,
    },
    DataRepair {
        version: 2,
        name: "template_and_generated_card_canonicalization",
        checksum: "ec486bad97c8c5495755bae160bf019d61c084444cc09d88ba8e1f0135b3e663",
        required_schema: 12,
        apply: repair_stage3_data,
    },
    DataRepair {
        version: 3,
        name: "review_event_historical_attribution",
        checksum: "8e2c34df65a62e31e3ec0d2476dcac18c59f10e54240b2d42a37d22441f2f147",
        required_schema: 14,
        apply: repair_review_attribution,
    },
];

pub(crate) fn apply_required_repairs(connection: &mut Connection) -> Result<(), String> {
    ensure_ledger_exists(connection)?;
    let schema_version: i64 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|error| format!("Could not inspect schema before data repair: {error}"))?;
    for repair in REPAIRS
        .iter()
        .filter(|repair| repair.required_schema <= schema_version)
    {
        let existing: Option<(String, String)> = connection
            .query_row(
                "SELECT name, checksum FROM data_repairs WHERE version = ?1",
                [repair.version],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|error| {
                format!("Could not inspect data repair {}: {error}", repair.version)
            })?;
        if let Some((name, checksum)) = existing {
            if name != repair.name || checksum != repair.checksum {
                return Err(format!(
                    "Data repair {} does not match this application build. The database was not modified.",
                    repair.version
                ));
            }
            continue;
        }

        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| format!("Could not start data repair {}: {error}", repair.version))?;
        let changed_rows = (repair.apply)(&transaction)?;
        let applied_at = unix_time_millis().map_err(|error| error.message)?;
        transaction
            .execute(
                "INSERT INTO data_repairs (
                     version, name, checksum, applied_at, changed_rows
                 ) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    repair.version,
                    repair.name,
                    repair.checksum,
                    applied_at,
                    changed_rows.max(0),
                ],
            )
            .map_err(|error| format!("Could not record data repair {}: {error}", repair.version))?;
        transaction
            .commit()
            .map_err(|error| format!("Could not commit data repair {}: {error}", repair.version))?;
    }
    verify_repair_ledger(connection)
}

pub(crate) fn verify_repair_ledger(connection: &Connection) -> Result<(), String> {
    ensure_ledger_exists(connection)?;
    let schema_version: i64 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|error| format!("Could not inspect schema while verifying repairs: {error}"))?;
    for repair in REPAIRS
        .iter()
        .filter(|repair| repair.required_schema <= schema_version)
    {
        let recorded: Option<(String, String)> = connection
            .query_row(
                "SELECT name, checksum FROM data_repairs WHERE version = ?1",
                [repair.version],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|error| format!("Could not verify data repair {}: {error}", repair.version))?;
        if recorded
            .as_ref()
            .map(|row| (row.0.as_str(), row.1.as_str()))
            != Some((repair.name, repair.checksum))
        {
            return Err(format!(
                "Required data repair {} is missing or does not match this application build.",
                repair.version
            ));
        }
    }
    Ok(())
}

fn ensure_ledger_exists(connection: &Connection) -> Result<(), String> {
    let exists: i64 = connection
        .query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM sqlite_master
                 WHERE type = 'table' AND name = 'data_repairs'
             )",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("Could not inspect the data repair ledger: {error}"))?;
    if exists == 0 {
        return Err("The data repair ledger migration has not been applied.".to_string());
    }
    Ok(())
}

fn repair_duplicate_fingerprints(transaction: &Transaction<'_>) -> Result<i64, String> {
    backfill_duplicate_hashes_in_connection(transaction)
}

fn repair_stage3_data(transaction: &Transaction<'_>) -> Result<i64, String> {
    apply_stage3_data_repairs(transaction).map_err(|error| error.message)
}

fn repair_review_attribution(transaction: &Transaction<'_>) -> Result<i64, String> {
    backfill_legacy_review_attributions(transaction).map_err(|error| error.message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{database::Database, error::CommandError};

    #[test]
    fn canonical_database_repair_pass_is_a_no_op() {
        let database = Database::open_in_memory().expect("database should open");
        database
            .with_connection(|connection| {
                let before = connection.total_changes();
                apply_required_repairs(connection)
                    .map_err(|error| CommandError::new("repair_failed", error))?;
                let after = connection.total_changes();
                assert_eq!(before, after);
                verify_repair_ledger(connection)
                    .map_err(|error| CommandError::new("repair_failed", error))?;
                Ok(())
            })
            .expect("second repair pass should not write");
    }
}
''',
)

# Expose a pure presentation renderer that consumes one already-projected row.
types_path = "src-tauri/src/authoring/stage3_v2/types_commands.rs"
types = read(types_path)
anchor = '''pub(crate) struct RenderedCardPresentation {
    pub(crate) card_type: String,
    pub(crate) card_label: String,
    pub(crate) front_doc: Value,
    pub(crate) back_doc: Value,
    pub(crate) front_text: String,
    pub(crate) back_text: String,
}
'''
insert = anchor + r'''

pub(crate) struct ProjectedPresentationInput<'a> {
    pub(crate) direction: String,
    pub(crate) template_ordinal: i64,
    pub(crate) note_type: String,
    pub(crate) front_doc_json: String,
    pub(crate) back_doc_json: String,
    pub(crate) front_text: String,
    pub(crate) back_text: String,
    pub(crate) cloze_source: String,
    pub(crate) cloze_mode: String,
    pub(crate) reps: i64,
    pub(crate) presentation_variant: Option<&'a str>,
}
'''
if "ProjectedPresentationInput" not in types:
    if anchor not in types:
        raise SystemExit("types_commands presentation anchor changed")
    types = types.replace(anchor, insert, 1)
write(types_path, types)

generated_path = "src-tauri/src/authoring/stage3_v2/generated_cards.rs"
generated = read(generated_path)
anchor = "pub(crate) fn render_stored_card_presentation(\n"
function = r'''pub(crate) fn render_projected_presentation(
    input: ProjectedPresentationInput<'_>,
) -> Result<RenderedCardPresentation, CommandError> {
    let ProjectedPresentationInput {
        direction,
        template_ordinal,
        note_type,
        front_doc_json,
        back_doc_json,
        front_text,
        back_text,
        cloze_source,
        cloze_mode,
        reps,
        presentation_variant,
    } = input;
    if note_type == "basic" {
        let front_doc =
            serde_json::from_str(&front_doc_json).map_err(CommandError::serialization)?;
        let back_doc =
            serde_json::from_str(&back_doc_json).map_err(CommandError::serialization)?;
        return match direction.as_str() {
            "forward" => Ok(RenderedCardPresentation {
                card_type: "basic".to_string(),
                card_label: "Basic".to_string(),
                front_doc,
                back_doc,
                front_text,
                back_text,
            }),
            "reverse" => Ok(RenderedCardPresentation {
                card_type: "basic".to_string(),
                card_label: "Reverse".to_string(),
                front_doc: back_doc,
                back_doc: front_doc,
                front_text: back_text,
                back_text: front_text,
            }),
            _ => Err(CommandError::new(
                "invalid_generated_card",
                "A Basic note contains an unsupported generated-card direction.",
            )),
        };
    }
    if note_type != "cloze" || direction != "forward" || template_ordinal != 0 {
        return Err(CommandError::new(
            "invalid_generated_card",
            "The Fill in the blank note contains an unsupported active card identity.",
        ));
    }
    validate_cloze_mode(&cloze_mode)?;
    let parsed = parse_cloze(&cloze_source)?;
    let target = cloze_target_from_variant(&parsed, &cloze_mode, presentation_variant, reps)?;
    let question = render_cloze_question(&parsed, target);
    let answer = render_cloze_answer(&parsed, target);
    let label = if cloze_mode == "all_together" {
        "Fill in the blank · all blanks".to_string()
    } else {
        format!(
            "Fill in the blank · {} alternating {}",
            parsed.groups.len(),
            if parsed.groups.len() == 1 { "blank" } else { "blanks" }
        )
    };
    Ok(RenderedCardPresentation {
        card_type: "cloze".to_string(),
        card_label: label,
        front_doc: plain_text_document(&question),
        back_doc: plain_text_document(&answer),
        front_text: question,
        back_text: answer,
    })
}

'''
if "fn render_projected_presentation" not in generated:
    if anchor not in generated:
        raise SystemExit("generated_cards renderer anchor changed")
    generated = generated.replace(anchor, function + anchor, 1)
write(generated_path, generated)

# Prepare immutable session-card snapshots in the same session transaction.
practice_selection_path = "src-tauri/src/practice_selection.rs"
practice_selection = read(practice_selection_path)
practice_selection = practice_selection.replace(
    "    error::CommandError,\n    learning_signals::needs_more_practice,",
    "    error::CommandError,\n    historical_attribution::{insert_session_snapshots, prepare_attributions},\n    learning_signals::needs_more_practice,",
    1,
)
old = '''    let selected_deck_ids_json =
        serde_json::to_string(&selected_deck_ids).map_err(CommandError::serialization)?;
    let transaction = connection
'''
new = '''    let attribution_cards = pending_cards
        .iter()
        .map(|(_, card_id, _, _, _, _, variant)| (card_id.clone(), variant.clone()))
        .collect::<Vec<_>>();
    let attributions = prepare_attributions(connection, &attribution_cards)?;
    let selected_deck_ids_json =
        serde_json::to_string(&selected_deck_ids).map_err(CommandError::serialization)?;
    let transaction = connection
'''
if "let attribution_cards = pending_cards" not in practice_selection:
    if old not in practice_selection:
        raise SystemExit("practice selection pretransaction anchor changed")
    practice_selection = practice_selection.replace(old, new, 1)
old = '''    for (position, (_, card_id, _, _, _, _, variant)) in pending_cards.iter().enumerate() {
        transaction
            .execute(
                "INSERT INTO practice_session_cards (
                     session_id, card_id, position, presentation_variant
                 ) VALUES (?1, ?2, ?3, ?4)",
                params![
                    session_id,
                    card_id,
                    i64::try_from(position).unwrap_or(i64::MAX),
                    variant,
                ],
            )
            .map_err(CommandError::database)?;
    }
    transaction.commit().map_err(CommandError::database)?;
'''
new = old.replace(
    "    transaction.commit().map_err(CommandError::database)?;",
    "    insert_session_snapshots(\n        &transaction,\n        &session_id,\n        attributions.into_values(),\n        now,\n    )?;\n    transaction.commit().map_err(CommandError::database)?;",
)
if "insert_session_snapshots(" not in practice_selection:
    if old not in practice_selection:
        raise SystemExit("practice selection insert anchor changed")
    practice_selection = practice_selection.replace(old, new, 1)
write(practice_selection_path, practice_selection)

# A review event and its attribution are one atomic append.
practice_path = "src-tauri/src/practice.rs"
practice = read(practice_path)
practice = practice.replace(
    "    error::CommandError,\n    scheduler::{",
    "    error::CommandError,\n    historical_attribution::insert_review_attribution,\n    scheduler::{",
    1,
)
old = '''    insert_review_event(
        &transaction,
        &input,
        &request_fingerprint,
        &transition,
        &result,
        reviewed_at,
    )?;
    transaction.commit().map_err(CommandError::database)?;
'''
new = '''    let review_event_id = insert_review_event(
        &transaction,
        &input,
        &request_fingerprint,
        &transition,
        &result,
        reviewed_at,
    )?;
    insert_review_attribution(
        &transaction,
        &review_event_id,
        &input.session_id,
        &input.card_id,
        reviewed_at,
    )?;
    transaction.commit().map_err(CommandError::database)?;
'''
if old not in practice and "let review_event_id = insert_review_event" not in practice:
    raise SystemExit("practice review append anchor changed")
practice = practice.replace(old, new, 1)
practice = practice.replace(
    ") -> Result<(), CommandError> {\n    let event_id = generate_id(connection)?;\n    connection\n        .execute(\n            \"INSERT INTO review_events (",
    ") -> Result<String, CommandError> {\n    let event_id = generate_id(connection)?;\n    connection\n        .execute(\n            \"INSERT INTO review_events (",
    1,
)
old = '''        .map_err(CommandError::database)?;
    Ok(())
}

fn review_result_for_submission(
'''
new = '''        .map_err(CommandError::database)?;
    Ok(event_id)
}

fn review_result_for_submission(
'''
if old not in practice and "Ok(event_id)" not in practice:
    raise SystemExit("practice event return anchor changed")
practice = practice.replace(old, new, 1)
write(practice_path, practice)

# Deletion finalization purges deleted-deck history only after Undo expires.
deletion_path = "src-tauri/src/deletion.rs"
deletion = read(deletion_path)
deletion = deletion.replace(
    "    error::CommandError,\n    util::{generate_id, unix_time_millis, validate_entity_id},",
    "    error::CommandError,\n    historical_attribution::purge_deck_history,\n    util::{generate_id, unix_time_millis, validate_entity_id},",
    1,
)
old = '''    for (operation_id, operation) in operations {
        for note_id in &operation.note_ids {
'''
new = '''    for (operation_id, operation) in operations {
        if operation.entity_kind == "deck" {
            if let Some(deck_id) = operation.deck_id.as_deref() {
                purge_deck_history(&transaction, deck_id)?;
            }
        }
        for note_id in &operation.note_ids {
'''
if "purge_deck_history(&transaction" not in deletion:
    if old not in deletion:
        raise SystemExit("deletion finalization anchor changed")
    deletion = deletion.replace(old, new, 1)
write(deletion_path, deletion)

print("Applied Stage 3.6–3.8 core migration, repair, presentation, attribution, and deletion boundaries.")
