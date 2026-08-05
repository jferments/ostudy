//! Bounded practice planning, immutable session snapshots, and window hydration.

use std::{
    collections::{BTreeMap, BTreeSet},
    hash::{Hash, Hasher},
};

use rusqlite::{
    params, params_from_iter, types::Value as SqlValue, Connection, OptionalExtension,
    TransactionBehavior,
};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    assets::AssetDto,
    database::Database,
    error::CommandError,
    historical_attribution::{insert_session_snapshots, prepare_attributions},
    paths::AppPaths,
    practice::{PracticeCardDto, PracticeSessionDto},
    projections::{
        learning_signals::LEARNING_SIGNAL_CTE,
        scope::{validated_ids, MAX_DECK_SCOPE, MAX_EXACT_CARD_SCOPE, MAX_TAG_SCOPE},
    },
    settings::load_settings,
    study_calendar::StudyTimeContext,
    util::{bool_to_integer, generate_id, validate_entity_id},
    workload::{
        planned_extra_cards_scoped, planned_scheduled_cards_scoped, workload_overview_scoped,
        PlannedCard, WorkloadOverviewDto,
    },
};

const FIRST_PRACTICE_WINDOW: i64 = 20;
const MAX_PRACTICE_WINDOW: i64 = 50;
const MAX_SESSION_CARDS: usize = 10_000;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PracticeWorkloadInput {
    #[serde(default)]
    pub deck_ids: Vec<String>,
    #[serde(default)]
    pub tag_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StartPracticeInput {
    pub operation_id: String,
    #[serde(default)]
    pub deck_ids: Vec<String>,
    #[serde(default)]
    pub tag_ids: Vec<String>,
    pub shuffle: bool,
    pub mode: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StartCardPracticeInput {
    pub operation_id: String,
    pub card_ids: Vec<String>,
    pub shuffle: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PracticeCardWindowInput {
    pub session_id: String,
    pub start_position: i64,
    pub limit: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PracticeCardWindowDto {
    pub start_position: i64,
    pub cards: Vec<PracticeCardDto>,
    pub has_more: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SelectedIdentity<'a> {
    deck_ids: &'a [String],
    tag_ids: &'a [String],
    shuffle: bool,
    mode: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CardIdentity<'a> {
    card_ids: &'a [String],
    shuffle: bool,
}

struct SessionPlan {
    operation_id: String,
    fingerprint: String,
    cards: Vec<PlannedCard>,
    shuffle: bool,
    mode: String,
    context: StudyTimeContext,
}

#[tauri::command]
pub fn get_practice_workload(
    database: State<'_, Database>,
    input: PracticeWorkloadInput,
) -> Result<WorkloadOverviewDto, CommandError> {
    let deck_ids = validated_ids(input.deck_ids, "deck", MAX_DECK_SCOPE)?;
    let tag_ids = validated_ids(input.tag_ids, "tag", MAX_TAG_SCOPE)?;
    database.with_connection(|connection| {
        let context = StudyTimeContext::capture()?;
        let settings = load_settings(connection)?;
        workload_overview_scoped(connection, &deck_ids, &tag_ids, &settings, &context)
    })
}

#[tauri::command]
pub fn start_selected_practice(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: StartPracticeInput,
) -> Result<PracticeSessionDto, CommandError> {
    validate_entity_id(&input.operation_id, "practice operation")?;
    let deck_ids = validated_ids(input.deck_ids, "deck", MAX_DECK_SCOPE)?;
    let tag_ids = validated_ids(input.tag_ids, "tag", MAX_TAG_SCOPE)?;
    if !matches!(input.mode.as_str(), "scheduled" | "extra") {
        return Err(CommandError::new(
            "invalid_practice_mode",
            "Choose scheduled or extra practice.",
        ));
    }
    let fingerprint = selected_fingerprint(&deck_ids, &tag_ids, input.shuffle, &input.mode)?;
    database.with_connection(|connection| {
        if let Some(existing) =
            session_for_operation(connection, &paths, &input.operation_id, &fingerprint)?
        {
            return Ok(existing);
        }
        let context = StudyTimeContext::capture()?;
        let settings = load_settings(connection)?;
        let cards = if input.mode == "scheduled" {
            planned_scheduled_cards_scoped(
                connection,
                &deck_ids,
                &tag_ids,
                &settings,
                &context,
            )?
            .0
        } else {
            planned_extra_cards_scoped(connection, &deck_ids, &tag_ids, &context)?
        };
        if cards.is_empty() {
            return Err(CommandError::new(
                "no_cards_available",
                if input.mode == "scheduled" {
                    "No cards fit this scope's remaining scheduled workload."
                } else {
                    "This scope has no studyable cards available for extra practice."
                },
            ));
        }
        create_session(
            connection,
            &paths,
            SessionPlan {
                operation_id: input.operation_id,
                fingerprint,
                cards,
                shuffle: input.shuffle,
                mode: input.mode,
                context,
            },
        )
    })
}

#[tauri::command]
pub fn start_card_practice(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: StartCardPracticeInput,
) -> Result<PracticeSessionDto, CommandError> {
    validate_entity_id(&input.operation_id, "practice operation")?;
    let card_ids = validated_ids(input.card_ids, "card", MAX_EXACT_CARD_SCOPE)?;
    if card_ids.is_empty() {
        return Err(CommandError::new(
            "no_cards_selected",
            "Select at least one card to practice.",
        ));
    }
    let fingerprint = card_fingerprint(&card_ids, input.shuffle)?;
    database.with_connection(|connection| {
        if let Some(existing) =
            session_for_operation(connection, &paths, &input.operation_id, &fingerprint)?
        {
            return Ok(existing);
        }
        let context = StudyTimeContext::capture()?;
        let cards = load_custom_cards(connection, &card_ids, &context)?;
        create_session(
            connection,
            &paths,
            SessionPlan {
                operation_id: input.operation_id,
                fingerprint,
                cards,
                shuffle: input.shuffle,
                mode: "custom".to_string(),
                context,
            },
        )
    })
}

#[tauri::command]
pub fn get_practice_card_window(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: PracticeCardWindowInput,
) -> Result<PracticeCardWindowDto, CommandError> {
    validate_entity_id(&input.session_id, "practice session")?;
    if input.start_position < 0 || !(1..=MAX_PRACTICE_WINDOW).contains(&input.limit) {
        return Err(CommandError::new(
            "invalid_practice_window",
            format!("Practice windows must contain 1 to {MAX_PRACTICE_WINDOW} cards."),
        ));
    }
    database.with_connection(|connection| {
        load_practice_card_window(
            connection,
            &paths,
            &input.session_id,
            input.start_position,
            input.limit,
        )
    })
}

fn create_session(
    connection: &mut Connection,
    paths: &AppPaths,
    mut plan: SessionPlan,
) -> Result<PracticeSessionDto, CommandError> {
    if plan.cards.is_empty() || plan.cards.len() > MAX_SESSION_CARDS {
        return Err(CommandError::new(
            "too_many_practice_cards",
            format!("A practice session must contain 1 to {MAX_SESSION_CARDS} cards."),
        ));
    }
    let session_id = generate_id(connection)?;
    if plan.shuffle {
        plan.cards.sort_by_key(|card| {
            (
                plan.context
                    .local_day_sort_key(card.due_at)
                    .unwrap_or(i32::MAX),
                !card.needs_more_practice,
                card.state == 0,
                shuffle_key(&session_id, &card.card_id),
            )
        });
    }
    let attribution_input = plan
        .cards
        .iter()
        .map(|card| (card.card_id.clone(), card.presentation_variant.clone()))
        .collect::<Vec<_>>();
    let attributions = prepare_attributions(connection, &attribution_input)?;
    let selected_decks = plan
        .cards
        .iter()
        .map(|card| card.deck_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let selected_json =
        serde_json::to_string(&selected_decks).map_err(CommandError::serialization)?;
    let card_count = i64::try_from(plan.cards.len()).unwrap_or(i64::MAX);
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    transaction
        .execute(
            "INSERT INTO practice_sessions (
                 id, selected_deck_ids_json, shuffle, card_count, mode,
                 started_at, start_operation_id, start_request_fingerprint
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                session_id,
                selected_json,
                bool_to_integer(plan.shuffle),
                card_count,
                plan.mode,
                plan.context.now_utc_millis,
                plan.operation_id,
                plan.fingerprint,
            ],
        )
        .map_err(CommandError::database)?;
    for (position, card) in plan.cards.iter().enumerate() {
        transaction
            .execute(
                "INSERT INTO practice_session_cards (
                     session_id, card_id, position, presentation_variant
                 ) VALUES (?1, ?2, ?3, ?4)",
                params![
                    session_id,
                    card.card_id,
                    i64::try_from(position).unwrap_or(i64::MAX),
                    card.presentation_variant,
                ],
            )
            .map_err(CommandError::database)?;
    }
    insert_session_snapshots(
        &transaction,
        &session_id,
        attributions.into_values(),
        plan.context.now_utc_millis,
    )?;
    transaction.commit().map_err(CommandError::database)?;
    load_session(connection, paths, &session_id)
}

fn load_session(
    connection: &Connection,
    paths: &AppPaths,
    session_id: &str,
) -> Result<PracticeSessionDto, CommandError> {
    let metadata: (i64, String, i64, i64) = connection
        .query_row(
            "SELECT shuffle, mode, started_at, card_count
             FROM practice_sessions WHERE id = ?1",
            [session_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()
        .map_err(CommandError::database)?
        .ok_or_else(|| {
            CommandError::new(
                "practice_session_not_found",
                "That practice session no longer exists.",
            )
        })?;
    let window = load_practice_card_window(
        connection,
        paths,
        session_id,
        0,
        FIRST_PRACTICE_WINDOW,
    )?;
    Ok(PracticeSessionDto {
        id: session_id.to_string(),
        cards: window.cards,
        card_count: metadata.3,
        has_more: window.has_more,
        shuffle: metadata.0 != 0,
        mode: metadata.1,
        started_at: metadata.2,
    })
}

fn load_practice_card_window(
    connection: &Connection,
    paths: &AppPaths,
    session_id: &str,
    start: i64,
    limit: i64,
) -> Result<PracticeCardWindowDto, CommandError> {
    let total: i64 = connection
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
         SELECT psc.position, s.card_id, s.note_id, s.note_revision,
                s.deck_id, COALESCE(d.name, s.captured_deck_name),
                s.question_doc, s.answer_doc, s.question_excerpt, s.answer_text,
                COALESCE(cs.due_at, s.captured_at),
                COALESCE(cs.state, 0),
                COALESCE(ls.needs_more_practice, 0),
                s.captured_at
         FROM practice_session_cards psc
         JOIN practice_session_card_snapshots s
           ON s.session_id = psc.session_id AND s.card_id = psc.card_id
         LEFT JOIN decks d ON d.id = s.deck_id AND d.deleted_at IS NULL
         LEFT JOIN card_schedules cs ON cs.card_id = s.card_id
         LEFT JOIN learning_signals ls ON ls.card_id = s.card_id
         WHERE psc.session_id = ?1
           AND psc.position >= ?2
           AND psc.position < ?2 + ?3
         ORDER BY psc.position",
        signals = LEARNING_SIGNAL_CTE,
    );
    let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
    let rows = statement
        .query_map(params![session_id, start, limit], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
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
            ))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    let card_ids = rows.iter().map(|row| row.1.clone()).collect::<Vec<_>>();
    let mut assets = load_window_assets(connection, paths, session_id, &card_ids)?;
    let cards = rows
        .into_iter()
        .map(
            |(
                _position,
                card_id,
                note_id,
                revision,
                deck_id,
                deck_name,
                question_doc,
                answer_doc,
                question_text,
                answer_text,
                due_at,
                schedule_state,
                weak,
                captured_at,
            )| {
                let front_doc = serde_json::from_str(&question_doc)
                    .map_err(CommandError::serialization)?;
                let back_doc =
                    serde_json::from_str(&answer_doc).map_err(CommandError::serialization)?;
                Ok(PracticeCardDto {
                    card: crate::cards::CardDto {
                        id: card_id.clone(),
                        note_id,
                        deck_id,
                        direction: "forward".to_string(),
                        front_doc,
                        back_doc,
                        front_text: question_text,
                        back_text: answer_text,
                        assets: assets.remove(&card_id).unwrap_or_default(),
                        created_at: captured_at,
                        updated_at: captured_at,
                        revision,
                    },
                    deck_name,
                    due_at,
                    schedule_state,
                    needs_more_practice: weak != 0,
                })
            },
        )
        .collect::<Result<Vec<_>, CommandError>>()?;
    Ok(PracticeCardWindowDto {
        start_position: start,
        has_more: start.saturating_add(i64::try_from(cards.len()).unwrap_or(i64::MAX)) < total,
        cards,
    })
}

fn load_window_assets(
    connection: &Connection,
    paths: &AppPaths,
    session_id: &str,
    card_ids: &[String],
) -> Result<BTreeMap<String, Vec<AssetDto>>, CommandError> {
    let mut map = BTreeMap::<String, Vec<AssetDto>>::new();
    if card_ids.is_empty() {
        return Ok(map);
    }
    let mut values = vec![SqlValue::Text(session_id.to_string())];
    let placeholders = card_ids
        .iter()
        .map(|card_id| {
            values.push(SqlValue::Text(card_id.clone()));
            format!("?{}", values.len())
        })
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT ss.card_id, a.id, a.original_filename, a.mime_type,
                a.byte_size, a.relative_path, a.created_at
         FROM practice_session_card_asset_snapshots ss
         JOIN assets a ON a.id = ss.asset_id
         WHERE ss.session_id = ?1 AND ss.card_id IN ({placeholders})
           AND a.deleted_at IS NULL
         ORDER BY ss.card_id, a.created_at, a.id"
    );
    let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
    let rows = statement
        .query_map(params_from_iter(values.iter()), |row| {
            let relative_path: String = row.get(5)?;
            Ok((
                row.get::<_, String>(0)?,
                AssetDto {
                    id: row.get(1)?,
                    original_filename: row.get(2)?,
                    mime_type: row.get(3)?,
                    byte_size: row.get(4)?,
                    file_path: paths
                        .resolve_relative(&relative_path)
                        .to_string_lossy()
                        .into_owned(),
                    created_at: row.get(6)?,
                },
            ))
        })
        .map_err(CommandError::database)?;
    for row in rows {
        let (card_id, asset) = row.map_err(CommandError::database)?;
        map.entry(card_id).or_default().push(asset);
    }
    Ok(map)
}

fn load_custom_cards(
    connection: &Connection,
    ids: &[String],
    context: &StudyTimeContext,
) -> Result<Vec<PlannedCard>, CommandError> {
    let mut result = BTreeMap::new();
    for chunk in ids.chunks(250) {
        let mut values = vec![SqlValue::Integer(context.now_utc_millis)];
        let placeholders = chunk
            .iter()
            .map(|id| {
                values.push(SqlValue::Text(id.clone()));
                format!("?{}", values.len())
            })
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "WITH {signals}
             SELECT c.deck_id, c.id, c.note_id, s.due_at, s.state,
                    COALESCE(ls.needs_more_practice, 0),
                    n.note_type, n.cloze_source, n.cloze_mode, s.reps
             FROM cards c
             JOIN notes n ON n.id = c.note_id
             JOIN decks d ON d.id = c.deck_id
             JOIN card_schedules s ON s.card_id = c.id
             LEFT JOIN learning_signals ls ON ls.card_id = c.id
             WHERE c.id IN ({placeholders})
               AND c.deleted_at IS NULL AND n.deleted_at IS NULL
               AND d.deleted_at IS NULL AND d.archived_at IS NULL
               AND c.suspended_at IS NULL
               AND (c.buried_until IS NULL OR c.buried_until <= ?1)",
            signals = LEARNING_SIGNAL_CTE,
        );
        let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
        let rows = statement
            .query_map(params_from_iter(values.iter()), |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, i64>(9)?,
                ))
            })
            .map_err(CommandError::database)?;
        for row in rows {
            let (deck_id, card_id, note_id, due_at, state, weak, kind, source, mode, reps) =
                row.map_err(CommandError::database)?;
            let variant =
                crate::authoring::presentation_variant_for_projected(&kind, &source, &mode, reps)?;
            result.insert(
                card_id.clone(),
                PlannedCard {
                    deck_id,
                    card_id,
                    note_id,
                    due_at,
                    state,
                    needs_more_practice: weak != 0,
                    presentation_variant: variant,
                },
            );
        }
    }
    if result.len() != ids.len() {
        return Err(CommandError::new(
            "practice_card_not_found",
            "A selected card is suspended, buried, archived, deleted, or unavailable.",
        ));
    }
    Ok(ids.iter().filter_map(|id| result.remove(id)).collect())
}

fn session_for_operation(
    connection: &Connection,
    paths: &AppPaths,
    operation_id: &str,
    fingerprint: &str,
) -> Result<Option<PracticeSessionDto>, CommandError> {
    let existing: Option<(String, Option<String>)> = connection
        .query_row(
            "SELECT id, start_request_fingerprint
             FROM practice_sessions WHERE start_operation_id = ?1",
            [operation_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(CommandError::database)?;
    let Some((session_id, stored)) = existing else {
        return Ok(None);
    };
    if stored.as_deref() != Some(fingerprint) {
        return Err(CommandError::new(
            "idempotency_conflict",
            "That practice retry identifier was already used for a different request.",
        ));
    }
    load_session(connection, paths, &session_id).map(Some)
}

fn selected_fingerprint(
    decks: &[String],
    tags: &[String],
    shuffle: bool,
    mode: &str,
) -> Result<String, CommandError> {
    let json = serde_json::to_string(&SelectedIdentity {
        deck_ids: decks,
        tag_ids: tags,
        shuffle,
        mode,
    })
    .map_err(CommandError::serialization)?;
    Ok(format!("practice-selected:v3:{json}"))
}

fn card_fingerprint(cards: &[String], shuffle: bool) -> Result<String, CommandError> {
    let json = serde_json::to_string(&CardIdentity {
        card_ids: cards,
        shuffle,
    })
    .map_err(CommandError::serialization)?;
    Ok(format!("practice-cards:v3:{json}"))
}

fn shuffle_key(session_id: &str, card_id: &str) -> u64 {
    let mut state = std::collections::hash_map::DefaultHasher::new();
    session_id.hash(&mut state);
    card_id.hash(&mut state);
    state.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn practice_windows_are_bounded() {
        assert_eq!(FIRST_PRACTICE_WINDOW, 20);
        assert_eq!(MAX_PRACTICE_WINDOW, 50);
    }
}
