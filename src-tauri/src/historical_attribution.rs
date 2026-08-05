//! Immutable session and review attribution used by historical Analytics.
//!
//! Review events retain stable deck/tag IDs and the exact prepared presentation.
//! Display labels are resolved from current retained deck/tag rows so renames are
//! reflected without moving history between identities. Deleted deck IDs are
//! filtered immediately and purged when their Undo window is finalized.

use std::collections::{BTreeMap, BTreeSet};

use rusqlite::{params, params_from_iter, types::Value as SqlValue, Connection};
use sha2::{Digest, Sha256};

use crate::{
    authoring::{render_projected_presentation, ProjectedPresentationInput},
    error::CommandError,
    study_calendar::StudyTimeContext,
    util::bool_to_integer,
};

const MAX_QUESTION_EXCERPT_CHARS: usize = 1_000;
const ID_CHUNK_SIZE: usize = 250;

#[derive(Clone, Debug)]
pub struct AttributionTag {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug)]
pub struct PreparedAttribution {
    pub card_id: String,
    pub note_id: String,
    pub note_revision: i64,
    pub deck_id: String,
    pub card_type: String,
    pub card_label: String,
    pub presentation_variant: String,
    pub question_doc_json: String,
    pub answer_doc_json: String,
    pub answer_text: String,
    pub asset_ids: Vec<String>,
    pub question_excerpt: String,
    pub question_text_hash: String,
    pub question_was_truncated: bool,
    pub tags: Vec<AttributionTag>,
}

#[derive(Clone, Debug)]
struct AttributionSourceRow {
    card_id: String,
    note_id: String,
    note_revision: i64,
    deck_id: String,
    note_type: String,
    direction: String,
    template_ordinal: i64,
    front_doc_json: String,
    back_doc_json: String,
    front_text: String,
    back_text: String,
    cloze_source: String,
    cloze_mode: String,
    reps: i64,
}

pub fn prepare_attributions(
    connection: &Connection,
    cards: &[(String, String)],
) -> Result<BTreeMap<String, PreparedAttribution>, CommandError> {
    let variants = cards.iter().cloned().collect::<BTreeMap<String, String>>();
    let mut rows = BTreeMap::<String, AttributionSourceRow>::new();
    let card_ids = variants.keys().cloned().collect::<Vec<_>>();

    for chunk in card_ids.chunks(ID_CHUNK_SIZE) {
        let mut values = Vec::<SqlValue>::new();
        let placeholders = chunk
            .iter()
            .map(|card_id| {
                values.push(card_id.clone().into());
                format!("?{}", values.len())
            })
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT c.id, c.note_id, n.revision, c.deck_id,
                    n.note_type, c.direction, c.template_ordinal,
                    n.front_doc, n.back_doc, n.front_text, n.back_text,
                    n.cloze_source, n.cloze_mode, s.reps
             FROM cards c
             JOIN notes n ON n.id = c.note_id
             JOIN decks d ON d.id = c.deck_id
             JOIN card_schedules s ON s.card_id = c.id
             WHERE c.id IN ({placeholders})
               AND c.deleted_at IS NULL
               AND c.suspended_at IS NULL
               AND n.deleted_at IS NULL
               AND d.deleted_at IS NULL
               AND d.archived_at IS NULL"
        );
        let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
        let loaded = statement
            .query_map(params_from_iter(values.iter()), |row| {
                Ok(AttributionSourceRow {
                    card_id: row.get(0)?,
                    note_id: row.get(1)?,
                    note_revision: row.get(2)?,
                    deck_id: row.get(3)?,
                    note_type: row.get(4)?,
                    direction: row.get(5)?,
                    template_ordinal: row.get(6)?,
                    front_doc_json: row.get(7)?,
                    back_doc_json: row.get(8)?,
                    front_text: row.get(9)?,
                    back_text: row.get(10)?,
                    cloze_source: row.get(11)?,
                    cloze_mode: row.get(12)?,
                    reps: row.get(13)?,
                })
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        for row in loaded {
            rows.insert(row.card_id.clone(), row);
        }
    }

    if rows.len() != card_ids.len() {
        return Err(CommandError::new(
            "practice_card_not_found",
            "At least one selected card is suspended, archived, deleted, or otherwise unavailable.",
        ));
    }

    let mut asset_ids = load_asset_ids_for_notes(
        connection,
        &rows
            .values()
            .map(|row| row.note_id.clone())
            .collect::<Vec<_>>(),
    )?;
    let mut tags = load_tags_for_notes(
        connection,
        &rows
            .values()
            .map(|row| row.note_id.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>(),
    )?;
    let mut prepared = BTreeMap::new();
    for (card_id, row) in rows {
        let variant = variants.get(&card_id).cloned().unwrap_or_default();
        let rendered = render_projected_presentation(ProjectedPresentationInput {
            direction: row.direction,
            template_ordinal: row.template_ordinal,
            note_type: row.note_type,
            front_doc_json: row.front_doc_json,
            back_doc_json: row.back_doc_json,
            front_text: row.front_text,
            back_text: row.back_text,
            cloze_source: row.cloze_source,
            cloze_mode: row.cloze_mode,
            reps: row.reps,
            presentation_variant: (!variant.is_empty()).then_some(variant.as_str()),
        })?;
        let (question_excerpt, question_was_truncated) = excerpt(&rendered.front_text);
        let question_text_hash = hash_text(&rendered.front_text);
        prepared.insert(
            card_id.clone(),
            PreparedAttribution {
                card_id,
                note_id: row.note_id.clone(),
                note_revision: row.note_revision,
                deck_id: row.deck_id,
                card_type: rendered.card_type,
                card_label: rendered.card_label,
                presentation_variant: variant,
                question_doc_json: serde_json::to_string(&rendered.front_doc)
                    .map_err(CommandError::serialization)?,
                answer_doc_json: serde_json::to_string(&rendered.back_doc)
                    .map_err(CommandError::serialization)?,
                answer_text: rendered.back_text,
                asset_ids: asset_ids.remove(&row.note_id).unwrap_or_default(),
                question_excerpt,
                question_text_hash,
                question_was_truncated,
                tags: tags.remove(&row.note_id).unwrap_or_default(),
            },
        );
    }
    Ok(prepared)
}

pub fn insert_session_snapshots(
    connection: &Connection,
    session_id: &str,
    attributions: impl IntoIterator<Item = PreparedAttribution>,
    captured_at: i64,
) -> Result<(), CommandError> {
    for attribution in attributions {
        connection
            .execute(
                "INSERT INTO practice_session_card_snapshots (
                     session_id, card_id, note_id, note_revision, deck_id,
                     card_type, card_label, presentation_variant,
                     question_doc, answer_doc, answer_text,
                     question_excerpt, question_text_hash, question_was_truncated,
                     captured_at
                 ) VALUES (
                     ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15
                 )",
                params![
                    session_id,
                    attribution.card_id,
                    attribution.note_id,
                    attribution.note_revision,
                    attribution.deck_id,
                    attribution.card_type,
                    attribution.card_label,
                    attribution.presentation_variant,
                    attribution.question_doc_json,
                    attribution.answer_doc_json,
                    attribution.answer_text,
                    attribution.question_excerpt,
                    attribution.question_text_hash,
                    bool_to_integer(attribution.question_was_truncated),
                    captured_at,
                ],
            )
            .map_err(CommandError::database)?;
        for asset_id in &attribution.asset_ids {
            connection
                .execute(
                    "INSERT INTO practice_session_card_asset_snapshots
                 (session_id, card_id, asset_id) VALUES (?1, ?2, ?3)",
                    params![session_id, attribution.card_id, asset_id],
                )
                .map_err(CommandError::database)?;
        }
        for tag in attribution.tags {
            connection
                .execute(
                    "INSERT INTO practice_session_card_tag_snapshots (
                         session_id, card_id, tag_id, captured_name
                     ) VALUES (?1, ?2, ?3, ?4)",
                    params![session_id, attribution.card_id, tag.id, tag.name],
                )
                .map_err(CommandError::database)?;
        }
    }
    Ok(())
}

pub fn insert_review_attribution(
    connection: &Connection,
    review_event_id: &str,
    session_id: &str,
    card_id: &str,
    reviewed_at: i64,
) -> Result<(), CommandError> {
    let context = StudyTimeContext::at(reviewed_at)?;
    let inserted = connection
        .execute(
            "INSERT INTO review_event_attributions (
                 review_event_id, card_id, note_id, note_revision, deck_id,
                 card_type, card_label, presentation_variant,
                 question_excerpt, question_text_hash, question_was_truncated,
                 review_local_date, utc_offset_minutes, time_zone_id,
                 attribution_source, captured_at
             )
             SELECT ?1, s.card_id, s.note_id, s.note_revision, s.deck_id,
                    s.card_type, s.card_label, s.presentation_variant,
                    s.question_excerpt, s.question_text_hash,
                    s.question_was_truncated,
                    ?4, ?5, ?6, 'captured', ?3
             FROM practice_session_card_snapshots s
             WHERE s.session_id = ?2 AND s.card_id = ?7",
            params![
                review_event_id,
                session_id,
                reviewed_at,
                context.local_date_string(),
                context.utc_offset_minutes,
                context.time_zone_id,
                card_id,
            ],
        )
        .map_err(CommandError::database)?;
    if inserted != 1 {
        return Err(CommandError::new(
            "review_attribution_missing",
            "The prepared review presentation is missing. The review was not recorded.",
        ));
    }
    connection
        .execute(
            "INSERT INTO review_event_tag_attributions (
                 review_event_id, tag_id, captured_name
             )
             SELECT ?1, tag_id, captured_name
             FROM practice_session_card_tag_snapshots
             WHERE session_id = ?2 AND card_id = ?3",
            params![review_event_id, session_id, card_id],
        )
        .map_err(CommandError::database)?;
    Ok(())
}

pub fn backfill_legacy_review_attributions(connection: &Connection) -> Result<i64, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT r.id, r.card_id, r.reviewed_at,
                    c.note_id, n.revision, c.deck_id,
                    n.note_type, c.direction, c.template_ordinal,
                    n.front_text, n.back_text,
                    COALESCE(psc.presentation_variant, '')
             FROM review_events r
             JOIN cards c ON c.id = r.card_id
             JOIN notes n ON n.id = c.note_id
             LEFT JOIN practice_session_cards psc
               ON psc.session_id = r.session_id AND psc.card_id = r.card_id
             LEFT JOIN review_event_attributions a ON a.review_event_id = r.id
             WHERE a.review_event_id IS NULL
             ORDER BY r.reviewed_at, r.created_at, r.id",
        )
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, i64>(8)?,
                row.get::<_, String>(9)?,
                row.get::<_, String>(10)?,
                row.get::<_, String>(11)?,
            ))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    drop(statement);

    let mut changed = 0_i64;
    for (
        review_id,
        card_id,
        reviewed_at,
        note_id,
        note_revision,
        deck_id,
        note_type,
        direction,
        template_ordinal,
        front_text,
        back_text,
        presentation_variant,
    ) in rows
    {
        let question = if direction == "reverse" {
            back_text
        } else {
            front_text
        };
        let (question_excerpt, question_was_truncated) = excerpt(&question);
        let context = StudyTimeContext::at(reviewed_at)?;
        let card_label = if note_type == "cloze" {
            "Fill in the blank".to_string()
        } else if direction == "reverse" {
            "Reverse".to_string()
        } else {
            "Basic".to_string()
        };
        connection
            .execute(
                "INSERT INTO review_event_attributions (
                     review_event_id, card_id, note_id, note_revision, deck_id,
                     card_type, card_label, presentation_variant,
                     question_excerpt, question_text_hash, question_was_truncated,
                     review_local_date, utc_offset_minutes, time_zone_id,
                     attribution_source, captured_at
                 ) VALUES (
                     ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8,
                     ?9, ?10, ?11, ?12, ?13, ?14,
                     'legacy_current_state_backfill', ?15
                 )",
                params![
                    review_id,
                    card_id,
                    note_id,
                    note_revision,
                    deck_id,
                    if note_type == "cloze" {
                        "cloze"
                    } else {
                        "basic"
                    },
                    card_label,
                    if template_ordinal >= 0 {
                        presentation_variant
                    } else {
                        String::new()
                    },
                    question_excerpt,
                    hash_text(&question),
                    bool_to_integer(question_was_truncated),
                    context.local_date_string(),
                    context.utc_offset_minutes,
                    context.time_zone_id,
                    reviewed_at,
                ],
            )
            .map_err(CommandError::database)?;
        connection
            .execute(
                "INSERT INTO review_event_tag_attributions (
                     review_event_id, tag_id, captured_name
                 )
                 SELECT ?1, t.id, t.name
                 FROM note_tags nt
                 JOIN tags t ON t.id = nt.tag_id
                 WHERE nt.note_id = ?2",
                params![review_id, note_id],
            )
            .map_err(CommandError::database)?;
        changed += 1;
    }
    Ok(changed)
}

pub fn purge_deck_history(connection: &Connection, deck_id: &str) -> Result<(), CommandError> {
    let session_ids = {
        let mut statement = connection
            .prepare(
                "SELECT DISTINCT psc.session_id
             FROM practice_session_cards psc
             JOIN cards c ON c.id=psc.card_id
             WHERE c.deck_id=?1",
            )
            .map_err(CommandError::database)?;
        let rows = statement
            .query_map([deck_id], |row| row.get::<_, String>(0))
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        rows
    };
    connection
        .execute(
            "DELETE FROM review_events
         WHERE card_id IN (SELECT id FROM cards WHERE deck_id=?1)
            OR id IN (SELECT review_event_id FROM review_event_attributions WHERE deck_id=?1)",
            [deck_id],
        )
        .map_err(CommandError::database)?;
    connection
        .execute(
            "DELETE FROM practice_session_card_snapshots WHERE deck_id=?1",
            [deck_id],
        )
        .map_err(CommandError::database)?;
    connection
        .execute(
            "DELETE FROM practice_session_cards
         WHERE card_id IN (SELECT id FROM cards WHERE deck_id=?1)",
            [deck_id],
        )
        .map_err(CommandError::database)?;
    for session_id in session_ids {
        connection.execute(
            "UPDATE practice_sessions SET
                 card_count=(SELECT COUNT(*) FROM practice_session_cards WHERE session_id=?1),
                 viewed_count=(SELECT COUNT(*) FROM practice_session_cards WHERE session_id=?1 AND viewed_at IS NOT NULL),
                 revealed_count=(SELECT COUNT(*) FROM practice_session_cards WHERE session_id=?1 AND revealed_at IS NOT NULL),
                 rated_count=(SELECT COUNT(*) FROM practice_session_cards WHERE session_id=?1 AND rated_at IS NOT NULL),
                 correct_count=(SELECT COUNT(*) FROM practice_session_cards WHERE session_id=?1 AND rating>1)
             WHERE id=?1",
            [session_id],
        ).map_err(CommandError::database)?;
    }
    connection.execute(
        "DELETE FROM practice_sessions
         WHERE NOT EXISTS (SELECT 1 FROM practice_session_cards psc WHERE psc.session_id=practice_sessions.id)
           AND NOT EXISTS (SELECT 1 FROM review_events r WHERE r.session_id=practice_sessions.id)",
        [],
    ).map_err(CommandError::database)?;
    Ok(())
}

fn load_asset_ids_for_notes(
    connection: &Connection,
    note_ids: &[String],
) -> Result<BTreeMap<String, Vec<String>>, CommandError> {
    let mut result = BTreeMap::<String, Vec<String>>::new();
    for chunk in note_ids.chunks(ID_CHUNK_SIZE) {
        let values = chunk
            .iter()
            .cloned()
            .map(SqlValue::Text)
            .collect::<Vec<_>>();
        let placeholders = (1..=values.len())
            .map(|index| format!("?{index}"))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT note_id, asset_id FROM note_assets
             WHERE note_id IN ({placeholders}) ORDER BY note_id, asset_id"
        );
        let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
        let rows = statement
            .query_map(params_from_iter(values.iter()), |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(CommandError::database)?;
        for row in rows {
            let (note, asset) = row.map_err(CommandError::database)?;
            result.entry(note).or_default().push(asset);
        }
    }
    Ok(result)
}

fn load_tags_for_notes(
    connection: &Connection,
    note_ids: &[String],
) -> Result<BTreeMap<String, Vec<AttributionTag>>, CommandError> {
    let mut result = BTreeMap::<String, Vec<AttributionTag>>::new();
    for chunk in note_ids.chunks(ID_CHUNK_SIZE) {
        let mut values = Vec::<SqlValue>::new();
        let placeholders = chunk
            .iter()
            .map(|note_id| {
                values.push(note_id.clone().into());
                format!("?{}", values.len())
            })
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT nt.note_id, t.id, t.name
             FROM note_tags nt
             JOIN tags t ON t.id = nt.tag_id
             WHERE nt.note_id IN ({placeholders})
             ORDER BY nt.note_id, t.name COLLATE NOCASE, t.id"
        );
        let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
        let rows = statement
            .query_map(params_from_iter(values.iter()), |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    AttributionTag {
                        id: row.get(1)?,
                        name: row.get(2)?,
                    },
                ))
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        for (note_id, tag) in rows {
            result.entry(note_id).or_default().push(tag);
        }
    }
    Ok(result)
}

fn excerpt(value: &str) -> (String, bool) {
    let mut chars = value.chars();
    let excerpt = chars
        .by_ref()
        .take(MAX_QUESTION_EXCERPT_CHARS)
        .collect::<String>();
    let truncated = chars.next().is_some();
    (excerpt, truncated)
}

fn hash_text(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn question_evidence_is_bounded_and_hashed_completely() {
        let text = "a".repeat(MAX_QUESTION_EXCERPT_CHARS + 20);
        let (value, truncated) = excerpt(&text);
        assert_eq!(value.chars().count(), MAX_QUESTION_EXCERPT_CHARS);
        assert!(truncated);
        assert_eq!(hash_text(&text).len(), 64);
    }
}
