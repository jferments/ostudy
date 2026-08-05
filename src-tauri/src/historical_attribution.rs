//! Immutable practice-session and accepted-review attribution.
//!
//! Review evidence keeps stable deck/tag IDs and the exact prepared presentation.
//! Analytics resolves retained deck/tag IDs to their current names. Final deck
//! deletion purges every review, snapshot, and session source for that deck.

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
    pub captured_deck_name: String,
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
struct SourceRow {
    card_id: String,
    note_id: String,
    note_revision: i64,
    deck_id: String,
    deck_name: String,
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
    let variants = cards.iter().cloned().collect::<BTreeMap<_, _>>();
    let card_ids = variants.keys().cloned().collect::<Vec<_>>();
    let mut rows = BTreeMap::<String, SourceRow>::new();

    for chunk in card_ids.chunks(ID_CHUNK_SIZE) {
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
            "SELECT c.id, c.note_id, n.revision, c.deck_id, d.name,
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
               AND (c.buried_until IS NULL OR c.buried_until <= unixepoch('subsec') * 1000)
               AND n.deleted_at IS NULL
               AND d.deleted_at IS NULL
               AND d.archived_at IS NULL"
        );
        let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
        let loaded = statement
            .query_map(params_from_iter(values.iter()), source_row)
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
            "At least one selected card is suspended, buried, archived, deleted, or unavailable.",
        ));
    }

    let note_ids = rows
        .values()
        .map(|row| row.note_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut assets = load_assets_for_notes(connection, &note_ids)?;
    let mut tags = load_tags_for_notes(connection, &note_ids)?;
    let mut prepared = BTreeMap::new();

    for (card_id, row) in rows {
        let presentation_variant = variants.get(&card_id).cloned().unwrap_or_default();
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
            presentation_variant: (!presentation_variant.is_empty())
                .then_some(presentation_variant.clone()),
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
                captured_deck_name: row.deck_name,
                card_type: rendered.card_type,
                card_label: rendered.card_label,
                presentation_variant,
                question_doc_json: serde_json::to_string(&rendered.front_doc)
                    .map_err(CommandError::serialization)?,
                answer_doc_json: serde_json::to_string(&rendered.back_doc)
                    .map_err(CommandError::serialization)?,
                answer_text: rendered.back_text,
                asset_ids: assets.remove(&row.note_id).unwrap_or_default(),
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
                     captured_deck_name, card_type, card_label, presentation_variant,
                     question_doc, answer_doc, answer_text,
                     question_excerpt, question_text_hash, question_was_truncated,
                     captured_at
                 ) VALUES (
                     ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12,
                     ?13, ?14, ?15, ?16
                 )",
                params![
                    session_id,
                    attribution.card_id.as_str(),
                    attribution.note_id.as_str(),
                    attribution.note_revision,
                    attribution.deck_id.as_str(),
                    attribution.captured_deck_name.as_str(),
                    attribution.card_type.as_str(),
                    attribution.card_label.as_str(),
                    attribution.presentation_variant.as_str(),
                    attribution.question_doc_json.as_str(),
                    attribution.answer_doc_json.as_str(),
                    attribution.answer_text.as_str(),
                    attribution.question_excerpt.as_str(),
                    attribution.question_text_hash.as_str(),
                    bool_to_integer(attribution.question_was_truncated),
                    captured_at,
                ],
            )
            .map_err(CommandError::database)?;
        for asset_id in attribution.asset_ids {
            connection
                .execute(
                    "INSERT INTO practice_session_card_asset_snapshots
                     (session_id, card_id, asset_id) VALUES (?1, ?2, ?3)",
                    params![session_id, attribution.card_id.as_str(), asset_id],
                )
                .map_err(CommandError::database)?;
        }
        for tag in attribution.tags {
            connection
                .execute(
                    "INSERT INTO practice_session_card_tag_snapshots
                     (session_id, card_id, tag_id, captured_name)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![session_id, attribution.card_id.as_str(), tag.id, tag.name],
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
    let (review_local_date, utc_offset_minutes, time_zone_id) =
        context.event_attribution(reviewed_at)?;
    let inserted = connection
        .execute(
            "INSERT INTO review_event_attributions (
                 review_event_id, reviewed_at, review_local_date,
                 time_zone_id, utc_offset_minutes,
                 card_id, note_id, note_revision, deck_id, captured_deck_name,
                 card_type, card_label, presentation_variant,
                 question_excerpt, question_text_hash, question_was_truncated,
                 attribution_source, captured_at
             )
             SELECT ?1, ?4, ?5, ?6, ?7,
                    s.card_id, s.note_id, s.note_revision, s.deck_id,
                    s.captured_deck_name, s.card_type, s.card_label,
                    s.presentation_variant, s.question_excerpt,
                    s.question_text_hash, s.question_was_truncated,
                    'captured', ?4
             FROM practice_session_card_snapshots s
             WHERE s.session_id = ?2 AND s.card_id = ?3",
            params![
                review_event_id,
                session_id,
                card_id,
                reviewed_at,
                review_local_date,
                time_zone_id,
                utc_offset_minutes,
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

pub fn backfill_historical_attribution(connection: &Connection) -> Result<i64, CommandError> {
    let context = StudyTimeContext::capture()?;
    let mut changed = 0_i64;
    loop {
        let rows = load_unattributed_reviews(connection, ID_CHUNK_SIZE)?;
        if rows.is_empty() {
            break;
        }
        let note_ids = rows
            .iter()
            .map(|row| row.note_id.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let tags = load_tags_for_notes(connection, &note_ids)?;
        for row in rows {
            let rendered = render_projected_presentation(ProjectedPresentationInput {
                direction: row.direction.clone(),
                template_ordinal: row.template_ordinal,
                note_type: row.note_type.clone(),
                front_doc_json: row.front_doc_json.clone(),
                back_doc_json: row.back_doc_json.clone(),
                front_text: row.front_text.clone(),
                back_text: row.back_text.clone(),
                cloze_source: row.cloze_source.clone(),
                cloze_mode: row.cloze_mode.clone(),
                reps: row.reps,
                presentation_variant: (!row.presentation_variant.is_empty())
                    .then_some(row.presentation_variant.clone()),
            })?;
            let (question_excerpt, question_was_truncated) = excerpt(&rendered.front_text);
            let (review_local_date, utc_offset_minutes, time_zone_id) =
                context.event_attribution(row.reviewed_at)?;
            connection
                .execute(
                    "INSERT INTO review_event_attributions (
                         review_event_id, reviewed_at, review_local_date,
                         time_zone_id, utc_offset_minutes,
                         card_id, note_id, note_revision, deck_id, captured_deck_name,
                         card_type, card_label, presentation_variant,
                         question_excerpt, question_text_hash, question_was_truncated,
                         attribution_source, captured_at
                     ) VALUES (
                         ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                         ?11, ?12, ?13, ?14, ?15, ?16,
                         'legacy_current_state_backfill', ?17
                     )",
                    params![
                        row.review_event_id.as_str(),
                        row.reviewed_at,
                        review_local_date.as_str(),
                        time_zone_id.as_str(),
                        utc_offset_minutes,
                        row.card_id.as_str(),
                        row.note_id.as_str(),
                        row.note_revision,
                        row.deck_id.as_str(),
                        row.deck_name.as_str(),
                        rendered.card_type.as_str(),
                        rendered.card_label.as_str(),
                        row.presentation_variant.as_str(),
                        question_excerpt.as_str(),
                        hash_text(&rendered.front_text),
                        bool_to_integer(question_was_truncated),
                        context.now_utc_millis,
                    ],
                )
                .map_err(CommandError::database)?;
            for tag in tags.get(&row.note_id).into_iter().flatten() {
                connection
                    .execute(
                        "INSERT INTO review_event_tag_attributions
                         (review_event_id, tag_id, captured_name)
                         VALUES (?1, ?2, ?3)",
                        params![
                            row.review_event_id.as_str(),
                            tag.id.as_str(),
                            tag.name.as_str(),
                        ],
                    )
                    .map_err(CommandError::database)?;
            }
            changed += 1;
        }
    }
    changed += backfill_open_session_snapshots(connection, &context)?;
    Ok(changed)
}

pub fn purge_deck_history(connection: &Connection, deck_id: &str) -> Result<(), CommandError> {
    let session_ids = {
        let mut statement = connection
            .prepare(
                "SELECT DISTINCT session_id
                 FROM practice_session_card_snapshots
                 WHERE deck_id = ?1",
            )
            .map_err(CommandError::database)?;
        let rows = statement
            .query_map([deck_id], |row| row.get::<_, String>(0))
            .map_err(CommandError::database)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?
    };

    connection
        .execute(
            "DELETE FROM review_events
             WHERE id IN (
                 SELECT review_event_id
                 FROM review_event_attributions
                 WHERE deck_id = ?1
             )
             OR card_id IN (SELECT id FROM cards WHERE deck_id = ?1)",
            [deck_id],
        )
        .map_err(CommandError::database)?;
    connection
        .execute(
            "DELETE FROM practice_session_cards
             WHERE (session_id, card_id) IN (
                 SELECT session_id, card_id
                 FROM practice_session_card_snapshots
                 WHERE deck_id = ?1
             )
             OR card_id IN (SELECT id FROM cards WHERE deck_id = ?1)",
            [deck_id],
        )
        .map_err(CommandError::database)?;
    connection
        .execute(
            "DELETE FROM practice_session_card_snapshots WHERE deck_id = ?1",
            [deck_id],
        )
        .map_err(CommandError::database)?;

    for session_id in session_ids {
        connection
            .execute(
                "UPDATE practice_sessions SET
                     card_count = (
                         SELECT COUNT(*) FROM practice_session_cards
                         WHERE session_id = ?1
                     ),
                     viewed_count = (
                         SELECT COUNT(*) FROM practice_session_cards
                         WHERE session_id = ?1 AND viewed_at IS NOT NULL
                     ),
                     revealed_count = (
                         SELECT COUNT(*) FROM practice_session_cards
                         WHERE session_id = ?1 AND revealed_at IS NOT NULL
                     ),
                     rated_count = (
                         SELECT COUNT(*) FROM practice_session_cards
                         WHERE session_id = ?1 AND rated_at IS NOT NULL
                     ),
                     correct_count = (
                         SELECT COUNT(*) FROM practice_session_cards
                         WHERE session_id = ?1 AND rating BETWEEN 2 AND 4
                     )
                 WHERE id = ?1",
                [session_id],
            )
            .map_err(CommandError::database)?;
    }
    connection
        .execute(
            "DELETE FROM practice_sessions
             WHERE NOT EXISTS (
                 SELECT 1 FROM practice_session_cards psc
                 WHERE psc.session_id = practice_sessions.id
             )
             AND NOT EXISTS (
                 SELECT 1 FROM review_events r
                 WHERE r.session_id = practice_sessions.id
             )",
            [],
        )
        .map_err(CommandError::database)?;
    Ok(())
}

#[derive(Clone, Debug)]
struct LegacyReviewRow {
    review_event_id: String,
    reviewed_at: i64,
    card_id: String,
    note_id: String,
    note_revision: i64,
    deck_id: String,
    deck_name: String,
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
    presentation_variant: String,
}

fn load_unattributed_reviews(
    connection: &Connection,
    limit: usize,
) -> Result<Vec<LegacyReviewRow>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT r.id, r.reviewed_at, r.card_id,
                    c.note_id, n.revision, c.deck_id, d.name,
                    n.note_type, c.direction, c.template_ordinal,
                    n.front_doc, n.back_doc, n.front_text, n.back_text,
                    n.cloze_source, n.cloze_mode, s.reps,
                    COALESCE(psc.presentation_variant, '')
             FROM review_events r
             JOIN cards c ON c.id = r.card_id
             JOIN notes n ON n.id = c.note_id
             JOIN decks d ON d.id = c.deck_id
             JOIN card_schedules s ON s.card_id = c.id
             LEFT JOIN practice_session_cards psc
               ON psc.session_id = r.session_id AND psc.card_id = r.card_id
             LEFT JOIN review_event_attributions a ON a.review_event_id = r.id
             WHERE a.review_event_id IS NULL
             ORDER BY r.reviewed_at, r.created_at, r.id
             LIMIT ?1",
        )
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map([i64::try_from(limit).unwrap_or(i64::MAX)], |row| {
            Ok(LegacyReviewRow {
                review_event_id: row.get(0)?,
                reviewed_at: row.get(1)?,
                card_id: row.get(2)?,
                note_id: row.get(3)?,
                note_revision: row.get(4)?,
                deck_id: row.get(5)?,
                deck_name: row.get(6)?,
                note_type: row.get(7)?,
                direction: row.get(8)?,
                template_ordinal: row.get(9)?,
                front_doc_json: row.get(10)?,
                back_doc_json: row.get(11)?,
                front_text: row.get(12)?,
                back_text: row.get(13)?,
                cloze_source: row.get(14)?,
                cloze_mode: row.get(15)?,
                reps: row.get(16)?,
                presentation_variant: row.get(17)?,
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    Ok(rows)
}

fn backfill_open_session_snapshots(
    connection: &Connection,
    context: &StudyTimeContext,
) -> Result<i64, CommandError> {
    let cards = {
        let mut statement = connection
            .prepare(
                "SELECT psc.session_id, psc.card_id,
                        COALESCE(psc.presentation_variant, ''), ps.started_at
                 FROM practice_session_cards psc
                 JOIN practice_sessions ps ON ps.id = psc.session_id
                 LEFT JOIN practice_session_card_snapshots s
                   ON s.session_id = psc.session_id AND s.card_id = psc.card_id
                 WHERE s.card_id IS NULL
                   AND ps.completed_at IS NULL
                 ORDER BY psc.session_id, psc.position",
            )
            .map_err(CommandError::database)?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        rows
    };
    let mut changed = 0_i64;
    for session in cards.chunks(ID_CHUNK_SIZE) {
        let requested = session
            .iter()
            .map(|(_, card_id, variant, _)| (card_id.clone(), variant.clone()))
            .collect::<Vec<_>>();
        let mut prepared = prepare_attributions(connection, &requested)?;
        for (session_id, card_id, _, started_at) in session {
            if let Some(attribution) = prepared.remove(card_id) {
                insert_session_snapshots(
                    connection,
                    session_id,
                    std::iter::once(attribution),
                    (*started_at).min(context.now_utc_millis),
                )?;
                changed += 1;
            }
        }
    }
    Ok(changed)
}

fn source_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SourceRow> {
    Ok(SourceRow {
        card_id: row.get(0)?,
        note_id: row.get(1)?,
        note_revision: row.get(2)?,
        deck_id: row.get(3)?,
        deck_name: row.get(4)?,
        note_type: row.get(5)?,
        direction: row.get(6)?,
        template_ordinal: row.get(7)?,
        front_doc_json: row.get(8)?,
        back_doc_json: row.get(9)?,
        front_text: row.get(10)?,
        back_text: row.get(11)?,
        cloze_source: row.get(12)?,
        cloze_mode: row.get(13)?,
        reps: row.get(14)?,
    })
}

fn load_assets_for_notes(
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
            "SELECT note_id, asset_id
             FROM note_assets
             WHERE note_id IN ({placeholders})
             ORDER BY note_id, asset_id"
        );
        let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
        let rows = statement
            .query_map(params_from_iter(values.iter()), |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(CommandError::database)?;
        for row in rows {
            let (note_id, asset_id) = row.map_err(CommandError::database)?;
            result.entry(note_id).or_default().push(asset_id);
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
            .map_err(CommandError::database)?;
        for row in rows {
            let (note_id, tag) = row.map_err(CommandError::database)?;
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
