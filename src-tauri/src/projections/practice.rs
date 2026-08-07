use std::collections::{BTreeMap, HashMap};

use rusqlite::{params, params_from_iter, types::Value as SqlValue, Connection, OptionalExtension};
use sha2::{Digest, Sha256};

use crate::{
    assets::load_assets_for_notes,
    authoring::{
        projected_presentation_variant, render_projected_presentation, ProjectedPresentationInput,
        RenderedCardPresentation,
    },
    cards::CardPresentationDto,
    error::CommandError,
    paths::AppPaths,
    practice::{PracticeCardDto, PracticeCardWindowDto},
    projections::{
        learning_signals as signal_projection,
        scope::{chunks, push_value},
    },
    study_calendar::StudyTimeContext,
    studyability,
    workload::PlannedCard,
};

const QUESTION_EXCERPT_CHARS: usize = 1_000;
pub const PRACTICE_WINDOW_MAX: i64 = 50;

#[derive(Clone, Debug)]
pub struct TagSnapshot {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug)]
pub struct PreparedPracticeCard {
    pub deck_id: String,
    pub deck_name: String,
    pub card_id: String,
    pub note_id: String,
    pub note_revision: i64,
    pub direction: String,
    pub due_at: i64,
    pub state: i64,
    pub needs_more_practice: bool,
    pub presentation_variant: String,
    pub card_type: String,
    pub card_label: String,
    pub front_doc_json: String,
    pub back_doc_json: String,
    pub front_text: String,
    pub back_text: String,
    pub question_excerpt: String,
    pub question_text_hash: String,
    pub question_was_truncated: bool,
    pub created_at: i64,
    pub updated_at: i64,
    pub tags: Vec<TagSnapshot>,
}

impl PreparedPracticeCard {
    pub fn to_dto(
        &self,
        assets: Vec<crate::assets::AssetDto>,
    ) -> Result<PracticeCardDto, CommandError> {
        Ok(PracticeCardDto {
            presentation: CardPresentationDto {
                id: self.card_id.clone(),
                note_id: self.note_id.clone(),
                deck_id: self.deck_id.clone(),
                direction: self.direction.clone(),
                front_doc: serde_json::from_str(&self.front_doc_json)
                    .map_err(CommandError::serialization)?,
                back_doc: serde_json::from_str(&self.back_doc_json)
                    .map_err(CommandError::serialization)?,
                front_text: self.front_text.clone(),
                back_text: self.back_text.clone(),
                assets,
                created_at: self.created_at,
                updated_at: self.updated_at,
                revision: self.note_revision,
            },
            deck_name: self.deck_name.clone(),
            due_at: self.due_at,
            schedule_state: self.state,
            needs_more_practice: self.needs_more_practice,
        })
    }
}

struct ProjectionRow {
    deck_id: String,
    deck_name: String,
    card_id: String,
    note_id: String,
    direction: String,
    template_ordinal: i64,
    note_type: String,
    front_doc: String,
    back_doc: String,
    front_text: String,
    back_text: String,
    cloze_source: String,
    cloze_mode: String,
    note_revision: i64,
    created_at: i64,
    updated_at: i64,
    due_at: i64,
    state: i64,
    reps: i64,
    needs_more_practice: bool,
}

pub fn prepare_new_session_cards(
    connection: &Connection,
    planned: &[PlannedCard],
    time: &StudyTimeContext,
) -> Result<Vec<PreparedPracticeCard>, CommandError> {
    let card_ids = planned
        .iter()
        .map(|card| card.card_id.clone())
        .collect::<Vec<_>>();
    let rows = load_projection_rows(connection, &card_ids, time)?;
    let note_ids = rows
        .values()
        .map(|row| row.note_id.clone())
        .collect::<Vec<_>>();
    let tags = load_tags_for_notes(connection, &note_ids)?;

    let mut prepared = Vec::with_capacity(planned.len());
    for planned_card in planned {
        let row = rows.get(&planned_card.card_id).ok_or_else(|| {
            CommandError::new(
                "practice_card_not_found",
                "A selected card is suspended, buried, deleted, archived, or otherwise unavailable.",
            )
        })?;
        let presentation_variant = projected_presentation_variant(
            &row.note_type,
            &row.cloze_source,
            &row.cloze_mode,
            row.reps,
        )?;
        let rendered = render_projected_presentation(ProjectedPresentationInput {
            direction: &row.direction,
            template_ordinal: row.template_ordinal,
            note_type: &row.note_type,
            front_doc_json: &row.front_doc,
            back_doc_json: &row.back_doc,
            front_text: &row.front_text,
            back_text: &row.back_text,
            cloze_source: &row.cloze_source,
            cloze_mode: &row.cloze_mode,
            reps: row.reps,
            presentation_variant: (!presentation_variant.is_empty())
                .then_some(presentation_variant.as_str()),
        })?;
        prepared.push(prepared_from_rendered(
            row,
            presentation_variant,
            rendered,
            tags.get(&row.note_id).cloned().unwrap_or_default(),
        )?);
    }
    Ok(prepared)
}

fn load_projection_rows(
    connection: &Connection,
    card_ids: &[String],
    time: &StudyTimeContext,
) -> Result<BTreeMap<String, ProjectionRow>, CommandError> {
    let mut result = BTreeMap::new();
    for chunk in chunks(card_ids) {
        let mut values = vec![SqlValue::Integer(time.now_utc_millis)];
        let placeholders = chunk
            .iter()
            .map(|card_id| format!("?{}", push_value(&mut values, card_id.clone())))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "WITH {signals}
             SELECT
                 c.deck_id, d.name, c.id, c.note_id, c.direction, c.template_ordinal,
                 n.note_type, n.front_doc, n.back_doc, n.front_text, n.back_text,
                 n.cloze_source, n.cloze_mode, n.revision, n.created_at, n.updated_at,
                 s.due_at, s.state, s.reps, {signal_value}
             FROM cards c
             JOIN notes n ON n.id = c.note_id
             JOIN decks d ON d.id = c.deck_id
             JOIN card_schedules s ON s.card_id = c.id
             {signal_join}
             WHERE c.id IN ({placeholders})
               AND {studyable}",
            signals = signal_projection::LEARNING_SIGNAL_CTES,
            signal_value = signal_projection::value_sql(),
            signal_join = signal_projection::join_sql(),
            studyable = studyability::studyable_card_sql("?1"),
        );
        let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
        let rows = statement
            .query_map(params_from_iter(values.iter()), |row| {
                Ok(ProjectionRow {
                    deck_id: row.get(0)?,
                    deck_name: row.get(1)?,
                    card_id: row.get(2)?,
                    note_id: row.get(3)?,
                    direction: row.get(4)?,
                    template_ordinal: row.get(5)?,
                    note_type: row.get(6)?,
                    front_doc: row.get(7)?,
                    back_doc: row.get(8)?,
                    front_text: row.get(9)?,
                    back_text: row.get(10)?,
                    cloze_source: row.get(11)?,
                    cloze_mode: row.get(12)?,
                    note_revision: row.get(13)?,
                    created_at: row.get(14)?,
                    updated_at: row.get(15)?,
                    due_at: row.get(16)?,
                    state: row.get(17)?,
                    reps: row.get(18)?,
                    needs_more_practice: row.get::<_, i64>(19)? != 0,
                })
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        for row in rows {
            result.insert(row.card_id.clone(), row);
        }
    }
    if result.len() != card_ids.len() {
        return Err(CommandError::new(
            "practice_card_not_found",
            "A selected card is suspended, buried, deleted, archived, or otherwise unavailable.",
        ));
    }
    Ok(result)
}

fn load_tags_for_notes(
    connection: &Connection,
    note_ids: &[String],
) -> Result<HashMap<String, Vec<TagSnapshot>>, CommandError> {
    let mut result = HashMap::<String, Vec<TagSnapshot>>::new();
    for chunk in chunks(note_ids) {
        let mut values = Vec::<SqlValue>::new();
        let placeholders = chunk
            .iter()
            .map(|note_id| format!("?{}", push_value(&mut values, note_id.clone())))
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
                    TagSnapshot {
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

fn prepared_from_rendered(
    row: &ProjectionRow,
    presentation_variant: String,
    rendered: RenderedCardPresentation,
    tags: Vec<TagSnapshot>,
) -> Result<PreparedPracticeCard, CommandError> {
    let front_doc_json =
        serde_json::to_string(&rendered.front_doc).map_err(CommandError::serialization)?;
    let back_doc_json =
        serde_json::to_string(&rendered.back_doc).map_err(CommandError::serialization)?;
    let (question_excerpt, question_was_truncated) = bounded_excerpt(&rendered.front_text);
    let question_text_hash = format!("{:x}", Sha256::digest(rendered.front_text.as_bytes()));
    Ok(PreparedPracticeCard {
        deck_id: row.deck_id.clone(),
        deck_name: row.deck_name.clone(),
        card_id: row.card_id.clone(),
        note_id: row.note_id.clone(),
        note_revision: row.note_revision,
        direction: row.direction.clone(),
        due_at: row.due_at,
        state: row.state,
        needs_more_practice: row.needs_more_practice,
        presentation_variant,
        card_type: rendered.card_type,
        card_label: rendered.card_label,
        front_doc_json,
        back_doc_json,
        front_text: rendered.front_text,
        back_text: rendered.back_text,
        question_excerpt,
        question_text_hash,
        question_was_truncated,
        created_at: row.created_at,
        updated_at: row.updated_at,
        tags,
    })
}

fn bounded_excerpt(value: &str) -> (String, bool) {
    (
        value.chars().take(QUESTION_EXCERPT_CHARS).collect(),
        value.chars().count() > QUESTION_EXCERPT_CHARS,
    )
}

pub fn insert_session_snapshot(
    connection: &Connection,
    session_id: &str,
    card: &PreparedPracticeCard,
    captured_at: i64,
) -> Result<(), CommandError> {
    connection
        .execute(
            "INSERT INTO practice_session_card_snapshots (
                 session_id, card_id, note_id, note_revision,
                 owner_deck_id_at_capture, captured_deck_name,
                 direction, card_type, card_label, presentation_variant,
                 front_doc_json, back_doc_json, front_text, back_text,
                 question_excerpt, question_text_hash, question_was_truncated, captured_at
             ) VALUES (
                 ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9,
                 ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18
             )",
            params![
                session_id,
                card.card_id,
                card.note_id,
                card.note_revision,
                card.deck_id,
                card.deck_name,
                card.direction,
                card.card_type,
                card.card_label,
                card.presentation_variant,
                card.front_doc_json,
                card.back_doc_json,
                card.front_text,
                card.back_text,
                card.question_excerpt,
                card.question_text_hash,
                i64::from(card.question_was_truncated),
                captured_at,
            ],
        )
        .map_err(CommandError::database)?;
    for tag in &card.tags {
        connection
            .execute(
                "INSERT INTO practice_session_card_tag_snapshots (
                     session_id, card_id, tag_id, tag_name
                 ) VALUES (?1, ?2, ?3, ?4)",
                params![session_id, card.card_id, tag.id, tag.name],
            )
            .map_err(CommandError::database)?;
    }
    Ok(())
}

pub fn load_practice_window(
    connection: &Connection,
    paths: &AppPaths,
    session_id: &str,
    start_position: i64,
    limit: i64,
) -> Result<PracticeCardWindowDto, CommandError> {
    if start_position < 0 || !(1..=PRACTICE_WINDOW_MAX).contains(&limit) {
        return Err(CommandError::new(
            "invalid_practice_window",
            format!("Request 1 to {PRACTICE_WINDOW_MAX} practice cards at a valid position."),
        ));
    }
    let card_count: i64 = connection
        .query_row(
            "SELECT card_count FROM practice_sessions WHERE id = ?1",
            [session_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(CommandError::database)?
        .ok_or_else(|| {
            CommandError::new(
                "practice_session_not_found",
                "That practice session no longer exists.",
            )
        })?;

    let sql = format!(
        "WITH {signals}
         SELECT
             psc.position,
             snap.card_id, snap.note_id, c.deck_id, d.name, snap.direction,
             snap.front_doc_json, snap.back_doc_json, snap.front_text, snap.back_text,
             snap.note_revision, snap.captured_at,
             s.due_at, s.state, {signal_value}
         FROM practice_session_cards psc
         JOIN practice_session_card_snapshots snap
           ON snap.session_id = psc.session_id AND snap.card_id = psc.card_id
         JOIN cards c ON c.id = psc.card_id
         JOIN decks d ON d.id = c.deck_id
         JOIN card_schedules s ON s.card_id = c.id
         {signal_join}
         WHERE psc.session_id = ?1 AND psc.position >= ?2
         ORDER BY psc.position ASC
         LIMIT ?3",
        signals = signal_projection::LEARNING_SIGNAL_CTES,
        signal_value = signal_projection::value_sql(),
        signal_join = signal_projection::join_sql(),
    );
    let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
    let rows = statement
        .query_map(params![session_id, start_position, limit], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, String>(9)?,
                row.get::<_, i64>(10)?,
                row.get::<_, i64>(11)?,
                row.get::<_, i64>(12)?,
                row.get::<_, i64>(13)?,
                row.get::<_, i64>(14)? != 0,
            ))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    let note_ids = rows.iter().map(|row| row.2.clone()).collect::<Vec<_>>();
    let mut assets = load_assets_for_notes(connection, paths, &note_ids)?;
    let mut cards = Vec::with_capacity(rows.len());
    let mut last_position = start_position.saturating_sub(1);
    for (
        position,
        card_id,
        note_id,
        deck_id,
        deck_name,
        direction,
        front_doc_json,
        back_doc_json,
        front_text,
        back_text,
        note_revision,
        captured_at,
        due_at,
        state,
        needs_more_practice,
    ) in rows
    {
        last_position = position;
        cards.push(PracticeCardDto {
            presentation: CardPresentationDto {
                id: card_id,
                note_id: note_id.clone(),
                deck_id,
                direction,
                front_doc: serde_json::from_str(&front_doc_json)
                    .map_err(CommandError::serialization)?,
                back_doc: serde_json::from_str(&back_doc_json)
                    .map_err(CommandError::serialization)?,
                front_text,
                back_text,
                assets: assets.remove(&note_id).unwrap_or_default(),
                created_at: captured_at,
                updated_at: captured_at,
                revision: note_revision,
            },
            deck_name,
            due_at,
            schedule_state: state,
            needs_more_practice,
        });
    }
    if start_position < card_count && cards.is_empty() {
        return Err(CommandError::new(
            "practice_window_unavailable",
            "The requested practice cards are no longer available.",
        ));
    }
    Ok(PracticeCardWindowDto {
        start_position,
        cards,
        has_more: last_position.saturating_add(1) < card_count,
    })
}

pub fn insert_review_attribution(
    connection: &Connection,
    review_event_id: &str,
    session_id: &str,
    card_id: &str,
    time: &StudyTimeContext,
) -> Result<(), CommandError> {
    let local_date = time.local_date_string_for_timestamp(time.now_utc_millis)?;
    let inserted = connection
        .execute(
            "INSERT INTO review_event_attributions (
                 review_event_id, reviewed_at, review_local_date,
                 time_zone_id, utc_offset_minutes,
                 card_id, note_id, note_revision,
                 owner_deck_id_at_review, captured_deck_name,
                 direction, card_type, card_label, presentation_variant,
                 question_excerpt, question_text_hash, question_was_truncated,
                 attribution_source
             )
             SELECT
                 ?1, ?2, ?3, ?4, ?5,
                 snap.card_id, snap.note_id, snap.note_revision,
                 c.deck_id, snap.captured_deck_name,
                 snap.direction, snap.card_type, snap.card_label,
                 snap.presentation_variant, snap.question_excerpt,
                 snap.question_text_hash, snap.question_was_truncated,
                 'captured'
             FROM practice_session_card_snapshots snap
             JOIN cards c ON c.id = snap.card_id
             WHERE snap.session_id = ?6 AND snap.card_id = ?7",
            params![
                review_event_id,
                time.now_utc_millis,
                local_date,
                time.time_zone_id,
                time.utc_offset_minutes,
                session_id,
                card_id,
            ],
        )
        .map_err(CommandError::database)?;
    if inserted != 1 {
        return insert_current_fallback_attribution(connection, review_event_id, card_id, time);
    }
    connection
        .execute(
            "INSERT INTO review_event_tag_attributions (
                 review_event_id, tag_id, tag_name
             )
             SELECT ?1, tag_id, tag_name
             FROM practice_session_card_tag_snapshots
             WHERE session_id = ?2 AND card_id = ?3",
            params![review_event_id, session_id, card_id],
        )
        .map_err(CommandError::database)?;
    Ok(())
}

fn insert_current_fallback_attribution(
    connection: &Connection,
    review_event_id: &str,
    card_id: &str,
    time: &StudyTimeContext,
) -> Result<(), CommandError> {
    let current: (String, i64, String, String, String, String, String) = connection
        .query_row(
            "SELECT c.note_id, n.revision, c.deck_id, d.name, c.direction,
                    n.note_type,
                    CASE WHEN c.direction = 'reverse' THEN n.back_text ELSE n.front_text END
             FROM cards c
             JOIN notes n ON n.id = c.note_id
             JOIN decks d ON d.id = c.deck_id
             WHERE c.id = ?1",
            [card_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                ))
            },
        )
        .map_err(CommandError::database)?;
    let (excerpt, truncated) = bounded_excerpt(&current.6);
    let hash = format!("{:x}", Sha256::digest(current.6.as_bytes()));
    connection
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
                 ?10, ?11, ?12, ?13, '', ?14, ?15, ?16, 'legacy_current_state_backfill'
             )",
            params![
                review_event_id,
                time.now_utc_millis,
                time.local_date_string_for_timestamp(time.now_utc_millis)?,
                time.time_zone_id,
                time.utc_offset_minutes,
                card_id,
                current.0,
                current.1,
                current.2,
                current.3,
                current.4,
                current.5,
                if current.4 == "reverse" {
                    "Reverse"
                } else {
                    "Basic"
                },
                excerpt,
                hash,
                i64::from(truncated),
            ],
        )
        .map_err(CommandError::database)?;
    connection
        .execute(
            "INSERT INTO review_event_tag_attributions (
                 review_event_id, tag_id, tag_name
             )
             SELECT ?1, t.id, t.name
             FROM note_tags nt
             JOIN tags t ON t.id = nt.tag_id
             WHERE nt.note_id = ?2",
            params![review_event_id, current.0],
        )
        .map_err(CommandError::database)?;
    Ok(())
}
