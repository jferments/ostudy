//! One-time, versioned data repairs with explicit migration prerequisites.
//!
//! Once every required repair row exists, normal startup only verifies the
//! ledgers and performs no application-data writes.

use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use sha2::{Digest, Sha256};

use crate::{
    authoring::{apply_stage3_data_repairs, backfill_duplicate_hashes_in_connection},
    study_calendar::StudyTimeContext,
    util::unix_time_millis,
};

const QUESTION_EXCERPT_CHARS: usize = 1_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RepairPhase {
    BeforeMigration(i64),
    AfterMigration(i64),
}

struct DataRepair {
    version: i64,
    name: &'static str,
    checksum: &'static str,
    phase: RepairPhase,
    apply: fn(&Transaction<'_>) -> Result<i64, String>,
}

const REPAIRS: &[DataRepair] = &[
    DataRepair {
        version: 1,
        name: "duplicate_fingerprints",
        checksum: "fb2f07be41d479048759ac7d754a59022f2c63c3f60082eaf9a40fd28bab7993",
        phase: RepairPhase::BeforeMigration(13),
        apply: repair_duplicate_fingerprints,
    },
    DataRepair {
        version: 2,
        name: "template_and_generated_card_canonicalization",
        checksum: "ec486bad97c8c5495755bae160bf019d61c084444cc09d88ba8e1f0135b3e663",
        phase: RepairPhase::BeforeMigration(13),
        apply: repair_stage3_data,
    },
    DataRepair {
        version: 3,
        name: "review_event_historical_attribution",
        checksum: "97014e9b56c2608f9d83d3ba449d6f4308a1fcb3292cfef5c2640ed2cc5e7700",
        phase: RepairPhase::AfterMigration(14),
        apply: repair_review_attribution,
    },
];

pub(crate) fn apply_required_repairs(
    connection: &mut Connection,
    phase: RepairPhase,
) -> Result<(), String> {
    ensure_ledger_exists(connection)?;
    for repair in REPAIRS.iter().filter(|repair| repair.phase == phase) {
        let existing = recorded_repair(connection, repair.version)?;
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
    Ok(())
}

pub(crate) fn verify_repair_ledger(connection: &Connection) -> Result<(), String> {
    ensure_ledger_exists(connection)?;
    for repair in REPAIRS {
        let recorded = recorded_repair(connection, repair.version)?;
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

fn recorded_repair(
    connection: &Connection,
    version: i64,
) -> Result<Option<(String, String)>, String> {
    connection
        .query_row(
            "SELECT name, checksum FROM data_repairs WHERE version = ?1",
            [version],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|error| format!("Could not inspect data repair {version}: {error}"))
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

#[derive(Clone)]
struct LegacyPresentationRow {
    review_event_id: Option<String>,
    session_id: Option<String>,
    card_id: String,
    note_id: String,
    note_revision: i64,
    deck_id: String,
    deck_name: String,
    direction: String,
    note_type: String,
    front_doc: String,
    back_doc: String,
    front_text: String,
    back_text: String,
    cloze_source: String,
    presentation_variant: String,
    occurred_at: i64,
}

fn repair_review_attribution(transaction: &Transaction<'_>) -> Result<i64, String> {
    let reviews = {
        let mut statement = transaction
            .prepare(
                "SELECT
                     r.id, r.session_id, r.card_id, c.note_id, n.revision,
                     c.deck_id, d.name, c.direction, n.note_type,
                     n.front_doc, n.back_doc, n.front_text, n.back_text,
                     n.cloze_source, COALESCE(psc.presentation_variant, ''), r.reviewed_at
                 FROM review_events r
                 JOIN cards c ON c.id = r.card_id
                 JOIN notes n ON n.id = c.note_id
                 JOIN decks d ON d.id = c.deck_id
                 LEFT JOIN practice_session_cards psc
                   ON psc.session_id = r.session_id AND psc.card_id = r.card_id
                 LEFT JOIN review_event_attributions a ON a.review_event_id = r.id
                 WHERE a.review_event_id IS NULL
                 ORDER BY r.reviewed_at, r.created_at, r.id",
            )
            .map_err(|error| format!("Could not prepare review attribution repair: {error}"))?;
        let rows = statement
            .query_map([], |row| {
                Ok(LegacyPresentationRow {
                    review_event_id: Some(row.get(0)?),
                    session_id: row.get(1)?,
                    card_id: row.get(2)?,
                    note_id: row.get(3)?,
                    note_revision: row.get(4)?,
                    deck_id: row.get(5)?,
                    deck_name: row.get(6)?,
                    direction: row.get(7)?,
                    note_type: row.get(8)?,
                    front_doc: row.get(9)?,
                    back_doc: row.get(10)?,
                    front_text: row.get(11)?,
                    back_text: row.get(12)?,
                    cloze_source: row.get(13)?,
                    presentation_variant: row.get(14)?,
                    occurred_at: row.get(15)?,
                })
            })
            .map_err(|error| format!("Could not read reviews for attribution repair: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("Could not collect attribution repair rows: {error}"))?;
        rows
    };

    let sessions = {
        let mut statement = transaction
            .prepare(
                "SELECT
                     NULL, psc.session_id, psc.card_id, c.note_id, n.revision,
                     c.deck_id, d.name, c.direction, n.note_type,
                     n.front_doc, n.back_doc, n.front_text, n.back_text,
                     n.cloze_source, COALESCE(psc.presentation_variant, ''), ps.started_at
                 FROM practice_session_cards psc
                 JOIN practice_sessions ps ON ps.id = psc.session_id
                 JOIN cards c ON c.id = psc.card_id
                 JOIN notes n ON n.id = c.note_id
                 JOIN decks d ON d.id = c.deck_id
                 LEFT JOIN practice_session_card_snapshots snap
                   ON snap.session_id = psc.session_id AND snap.card_id = psc.card_id
                 WHERE snap.card_id IS NULL
                 ORDER BY psc.session_id, psc.position",
            )
            .map_err(|error| format!("Could not prepare session attribution repair: {error}"))?;
        let rows = statement
            .query_map([], |row| {
                Ok(LegacyPresentationRow {
                    review_event_id: row.get(0)?,
                    session_id: row.get(1)?,
                    card_id: row.get(2)?,
                    note_id: row.get(3)?,
                    note_revision: row.get(4)?,
                    deck_id: row.get(5)?,
                    deck_name: row.get(6)?,
                    direction: row.get(7)?,
                    note_type: row.get(8)?,
                    front_doc: row.get(9)?,
                    back_doc: row.get(10)?,
                    front_text: row.get(11)?,
                    back_text: row.get(12)?,
                    cloze_source: row.get(13)?,
                    presentation_variant: row.get(14)?,
                    occurred_at: row.get(15)?,
                })
            })
            .map_err(|error| format!("Could not read sessions for attribution repair: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("Could not collect session repair rows: {error}"))?;
        rows
    };

    let mut changed = 0_i64;
    for row in sessions {
        insert_legacy_session_snapshot(transaction, &row)?;
        changed = changed.saturating_add(1);
    }
    for row in reviews {
        insert_legacy_review_attribution(transaction, &row)?;
        changed = changed.saturating_add(1);
    }
    Ok(changed)
}

fn insert_legacy_session_snapshot(
    transaction: &Transaction<'_>,
    row: &LegacyPresentationRow,
) -> Result<(), String> {
    let session_id = row
        .session_id
        .as_deref()
        .ok_or_else(|| "A practice-session repair row has no session ID.".to_string())?;
    let presentation = legacy_presentation(row);
    transaction
        .execute(
            "INSERT INTO practice_session_card_snapshots (
                 session_id, card_id, note_id, note_revision,
                 owner_deck_id_at_capture, captured_deck_name,
                 direction, card_type, card_label, presentation_variant,
                 front_doc_json, back_doc_json, front_text, back_text,
                 question_excerpt, question_text_hash, question_was_truncated, captured_at
             ) VALUES (
                 ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                 ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18
             )",
            params![
                session_id,
                row.card_id,
                row.note_id,
                row.note_revision,
                row.deck_id,
                row.deck_name,
                row.direction,
                presentation.card_type,
                presentation.card_label,
                row.presentation_variant,
                presentation.front_doc,
                presentation.back_doc,
                presentation.front_text,
                presentation.back_text,
                presentation.excerpt,
                presentation.hash,
                i64::from(presentation.truncated),
                row.occurred_at,
            ],
        )
        .map_err(|error| format!("Could not insert a repaired session snapshot: {error}"))?;
    copy_current_tags_to_session(transaction, session_id, row)?;
    Ok(())
}

fn insert_legacy_review_attribution(
    transaction: &Transaction<'_>,
    row: &LegacyPresentationRow,
) -> Result<(), String> {
    let review_event_id = row
        .review_event_id
        .as_deref()
        .ok_or_else(|| "A review attribution repair row has no review ID.".to_string())?;
    let presentation = legacy_presentation(row);
    let time = StudyTimeContext::capture_at(row.occurred_at).map_err(|error| error.message)?;
    let local_date = time
        .local_date_string_for_timestamp(row.occurred_at)
        .map_err(|error| error.message)?;
    transaction
        .execute(
            "INSERT INTO review_event_attributions (
                 review_event_id, reviewed_at, review_local_date,
                 time_zone_id, utc_offset_minutes,
                 card_id, note_id, note_revision,
                 owner_deck_id_at_review, captured_deck_name,
                 direction, card_type, card_label, presentation_variant,
                 question_excerpt, question_text_hash, question_was_truncated,
                 attribution_source
             ) VALUES (
                 ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9,
                 ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17,
                 'legacy_current_state_backfill'
             )",
            params![
                review_event_id,
                row.occurred_at,
                local_date,
                time.time_zone_id,
                time.utc_offset_minutes,
                row.card_id,
                row.note_id,
                row.note_revision,
                row.deck_id,
                row.deck_name,
                row.direction,
                presentation.card_type,
                presentation.card_label,
                row.presentation_variant,
                presentation.excerpt,
                presentation.hash,
                i64::from(presentation.truncated),
            ],
        )
        .map_err(|error| format!("Could not insert repaired review attribution: {error}"))?;
    copy_current_tags_to_review(transaction, review_event_id, row)
}

struct LegacyPresentation {
    card_type: String,
    card_label: String,
    front_doc: String,
    back_doc: String,
    front_text: String,
    back_text: String,
    excerpt: String,
    hash: String,
    truncated: bool,
}

fn legacy_presentation(row: &LegacyPresentationRow) -> LegacyPresentation {
    let (front_doc, back_doc, front_text, back_text, card_label) =
        if row.note_type == "basic" && row.direction == "reverse" {
            (
                row.back_doc.clone(),
                row.front_doc.clone(),
                row.back_text.clone(),
                row.front_text.clone(),
                "Reverse".to_string(),
            )
        } else if row.note_type == "cloze" {
            let question = if row.cloze_source.is_empty() {
                row.front_text.clone()
            } else {
                row.cloze_source.clone()
            };
            (
                row.front_doc.clone(),
                row.back_doc.clone(),
                question,
                row.back_text.clone(),
                "Fill in the blank · reconstructed".to_string(),
            )
        } else {
            (
                row.front_doc.clone(),
                row.back_doc.clone(),
                row.front_text.clone(),
                row.back_text.clone(),
                "Basic".to_string(),
            )
        };
    let (excerpt, truncated) = bounded_excerpt(&front_text);
    let hash = format!("{:x}", Sha256::digest(front_text.as_bytes()));
    LegacyPresentation {
        card_type: if row.note_type == "cloze" {
            "cloze".to_string()
        } else {
            "basic".to_string()
        },
        card_label,
        front_doc,
        back_doc,
        front_text,
        back_text,
        excerpt,
        hash,
        truncated,
    }
}

fn bounded_excerpt(value: &str) -> (String, bool) {
    let excerpt = value
        .chars()
        .take(QUESTION_EXCERPT_CHARS)
        .collect::<String>();
    let truncated = value.chars().count() > QUESTION_EXCERPT_CHARS;
    (excerpt, truncated)
}

fn copy_current_tags_to_session(
    transaction: &Transaction<'_>,
    session_id: &str,
    row: &LegacyPresentationRow,
) -> Result<(), String> {
    transaction
        .execute(
            "INSERT OR IGNORE INTO practice_session_card_tag_snapshots (
                 session_id, card_id, tag_id, tag_name
             )
             SELECT ?1, ?2, t.id, t.name
             FROM note_tags nt
             JOIN tags t ON t.id = nt.tag_id
             WHERE nt.note_id = ?3",
            params![session_id, row.card_id, row.note_id],
        )
        .map_err(|error| format!("Could not repair session tag attribution: {error}"))?;
    Ok(())
}

fn copy_current_tags_to_review(
    transaction: &Transaction<'_>,
    review_event_id: &str,
    row: &LegacyPresentationRow,
) -> Result<(), String> {
    transaction
        .execute(
            "INSERT OR IGNORE INTO review_event_tag_attributions (
                 review_event_id, tag_id, tag_name
             )
             SELECT ?1, t.id, t.name
             FROM note_tags nt
             JOIN tags t ON t.id = nt.tag_id
             WHERE nt.note_id = ?2",
            params![review_event_id, row.note_id],
        )
        .map_err(|error| format!("Could not repair review tag attribution: {error}"))?;
    Ok(())
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
                apply_required_repairs(connection, RepairPhase::BeforeMigration(13))
                    .map_err(|error| CommandError::new("repair_failed", error))?;
                apply_required_repairs(connection, RepairPhase::AfterMigration(14))
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
