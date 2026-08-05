//! Bounded practice-session selection for scheduled, extra, tag-scoped, and
//! exact custom study.

use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    hash::{Hash, Hasher},
};

use rusqlite::{
    params, params_from_iter, types::Value as SqlValue, Connection, OptionalExtension,
    TransactionBehavior,
};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    assets::load_assets_for_notes,
    database::Database,
    error::CommandError,
    paths::AppPaths,
    practice::{PracticeCardWindowDto, PracticeSessionDto},
    projections::{
        learning_signals as signal_projection,
        practice::{
            insert_session_snapshot, load_practice_window, prepare_new_session_cards,
            PreparedPracticeCard, PRACTICE_WINDOW_MAX,
        },
        scope::{chunks, normalize_entity_ids, push_value},
    },
    settings::load_settings,
    study_calendar::StudyTimeContext,
    studyability,
    util::{bool_to_integer, generate_id, validate_entity_id},
    workload::{
        planned_extra_cards_for_scope as planned_extra_cards,
        planned_scheduled_cards_for_scope as planned_scheduled_cards,
        workload_overview_for_scope as workload_overview, PlannedCard, StudyScope,
        WorkloadOverviewDto,
    },
};

const MAX_SESSION_DECKS: usize = 100;
const MAX_SESSION_TAGS: usize = 32;
const MAX_SESSION_CARDS: usize = 10_000;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct PracticeWorkloadInput {
    #[serde(default)]
    pub deck_ids: Vec<String>,
    #[serde(default)]
    pub tag_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
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
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct StartCardPracticeInput {
    pub operation_id: String,
    pub card_ids: Vec<String>,
    pub shuffle: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct PracticeCardWindowInput {
    pub session_id: String,
    pub start_position: i64,
    pub limit: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SelectedPracticeRequestIdentity<'a> {
    deck_ids: &'a [String],
    tag_ids: &'a [String],
    shuffle: bool,
    mode: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CardPracticeRequestIdentity<'a> {
    card_ids: &'a [String],
    shuffle: bool,
}

#[tauri::command]
pub fn get_practice_workload(
    database: State<'_, Database>,
    input: PracticeWorkloadInput,
) -> Result<WorkloadOverviewDto, CommandError> {
    let (deck_ids, tag_ids) = validate_scope(input.deck_ids, input.tag_ids)?;
    database.with_connection(|connection| {
        validate_current_scope(connection, &deck_ids, &tag_ids)?;
        let settings = load_settings(connection)?;
        let time = StudyTimeContext::capture()?;
        workload_overview(
            connection,
            StudyScope {
                deck_ids: &deck_ids,
                tag_ids: &tag_ids,
            },
            &settings,
            &time,
        )
    })
}

#[tauri::command]
pub fn start_selected_practice(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: StartPracticeInput,
) -> Result<PracticeSessionDto, CommandError> {
    validate_entity_id(&input.operation_id, "practice start operation")?;
    let (deck_ids, tag_ids) = validate_scope(input.deck_ids, input.tag_ids)?;
    if !matches!(input.mode.as_str(), "scheduled" | "extra") {
        return Err(CommandError::new(
            "invalid_practice_mode",
            "Choose scheduled practice or an explicit extra session.",
        ));
    }
    database.with_connection(|connection| {
        start_selected_practice_in_connection(
            connection,
            &paths,
            input.operation_id,
            deck_ids,
            tag_ids,
            input.shuffle,
            &input.mode,
        )
    })
}

#[tauri::command]
pub fn start_card_practice(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: StartCardPracticeInput,
) -> Result<PracticeSessionDto, CommandError> {
    validate_entity_id(&input.operation_id, "practice start operation")?;
    let card_ids = validate_card_ids(input.card_ids)?;
    database.with_connection(|connection| {
        start_card_practice_in_connection(
            connection,
            &paths,
            input.operation_id,
            card_ids,
            input.shuffle,
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
    database.with_connection(|connection| {
        load_practice_window(
            connection,
            &paths,
            &input.session_id,
            input.start_position,
            input.limit,
        )
    })
}

pub(crate) fn start_selected_practice_in_connection(
    connection: &mut Connection,
    paths: &AppPaths,
    operation_id: String,
    deck_ids: Vec<String>,
    tag_ids: Vec<String>,
    shuffle: bool,
    mode: &str,
) -> Result<PracticeSessionDto, CommandError> {
    let request_fingerprint =
        selected_practice_request_fingerprint(&deck_ids, &tag_ids, shuffle, mode)?;
    if let Some(existing) =
        practice_session_for_operation(connection, paths, &operation_id, &request_fingerprint)?
    {
        return Ok(existing);
    }
    validate_current_scope(connection, &deck_ids, &tag_ids)?;
    let time = StudyTimeContext::capture()?;
    let scope = StudyScope {
        deck_ids: &deck_ids,
        tag_ids: &tag_ids,
    };
    let planned = if mode == "scheduled" {
        let settings = load_settings(connection)?;
        planned_scheduled_cards(connection, scope, &settings, &time)?.0
    } else {
        planned_extra_cards(connection, scope, &time)?
    };
    if planned.is_empty() {
        let message = if mode == "scheduled" {
            "No cards fit the selected scope's remaining scheduled workload. Start an extra session to study anyway."
        } else {
            "The selected scope does not contain any studyable cards available for extra practice."
        };
        return Err(CommandError::new("no_cards_available", message));
    }
    create_practice_session(
        connection,
        paths,
        operation_id,
        request_fingerprint,
        deck_ids,
        tag_ids,
        planned,
        shuffle,
        mode,
        &time,
    )
}

pub(crate) fn start_card_practice_in_connection(
    connection: &mut Connection,
    paths: &AppPaths,
    operation_id: String,
    card_ids: Vec<String>,
    shuffle: bool,
) -> Result<PracticeSessionDto, CommandError> {
    let request_fingerprint = card_practice_request_fingerprint(&card_ids, shuffle)?;
    if let Some(existing) =
        practice_session_for_operation(connection, paths, &operation_id, &request_fingerprint)?
    {
        return Ok(existing);
    }
    let time = StudyTimeContext::capture()?;
    let planned = load_exact_planned_cards(connection, &card_ids, &time)?;
    let deck_ids = planned
        .iter()
        .map(|card| card.deck_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    create_practice_session(
        connection,
        paths,
        operation_id,
        request_fingerprint,
        deck_ids,
        Vec::new(),
        planned,
        shuffle,
        "custom",
        &time,
    )
}

fn selected_practice_request_fingerprint(
    deck_ids: &[String],
    tag_ids: &[String],
    shuffle: bool,
    mode: &str,
) -> Result<String, CommandError> {
    let identity = SelectedPracticeRequestIdentity {
        deck_ids,
        tag_ids,
        shuffle,
        mode,
    };
    let json = serde_json::to_string(&identity).map_err(CommandError::serialization)?;
    Ok(format!("practice-selected:v2:{json}"))
}

fn card_practice_request_fingerprint(
    card_ids: &[String],
    shuffle: bool,
) -> Result<String, CommandError> {
    let identity = CardPracticeRequestIdentity { card_ids, shuffle };
    let json = serde_json::to_string(&identity).map_err(CommandError::serialization)?;
    Ok(format!("practice-cards:v2:{json}"))
}

fn practice_session_for_operation(
    connection: &Connection,
    paths: &AppPaths,
    operation_id: &str,
    request_fingerprint: &str,
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
    let Some((session_id, stored_fingerprint)) = existing else {
        return Ok(None);
    };
    if stored_fingerprint.as_deref() != Some(request_fingerprint) {
        return Err(CommandError::new(
            "idempotency_conflict",
            "That practice retry identifier was already used for a different session request.",
        ));
    }
    load_practice_session_start(connection, paths, &session_id)
        .map(Some)
        .map_err(|error| {
            crate::logging::error(
                "committed_practice_session_reload_failed",
                format!("session {session_id}: {} ({})", error.message, error.code),
            );
            CommandError::new(
                "committed_practice_session_unavailable",
                "The practice session was created, but its first card window could not be reloaded. No duplicate session was created. Return to Practice and retry the same start action.",
            )
        })
}

fn load_practice_session_start(
    connection: &Connection,
    paths: &AppPaths,
    session_id: &str,
) -> Result<PracticeSessionDto, CommandError> {
    let metadata: (i64, i64, String, i64) = connection
        .query_row(
            "SELECT card_count, shuffle, mode, started_at
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
    let first_window = load_practice_window(
        connection,
        paths,
        session_id,
        0,
        PRACTICE_WINDOW_MAX,
    )?;
    Ok(PracticeSessionDto {
        id: session_id.to_string(),
        card_count: metadata.0,
        first_window,
        shuffle: metadata.1 != 0,
        mode: metadata.2,
        started_at: metadata.3,
    })
}

#[allow(clippy::too_many_arguments)]
fn create_practice_session(
    connection: &mut Connection,
    paths: &AppPaths,
    operation_id: String,
    request_fingerprint: String,
    selected_deck_ids: Vec<String>,
    selected_tag_ids: Vec<String>,
    planned: Vec<PlannedCard>,
    shuffle: bool,
    mode: &str,
    time: &StudyTimeContext,
) -> Result<PracticeSessionDto, CommandError> {
    if planned.len() > MAX_SESSION_CARDS {
        return Err(CommandError::new(
            "too_many_practice_cards",
            format!("A practice session can contain at most {MAX_SESSION_CARDS} cards."),
        ));
    }
    let session_id = generate_id(connection)?;
    let mut prepared = prepare_new_session_cards(connection, &planned, time)?;
    if shuffle {
        prepared.sort_by_key(|card| {
            let due_date = time
                .local_date_for_timestamp(card.due_at)
                .unwrap_or(time.local_date);
            (
                due_date,
                !card.needs_more_practice,
                card.state == 0,
                shuffle_key(&session_id, &card.card_id),
            )
        });
    }

    // Build the entire fallible first response window before durable success.
    let first_window = first_window_from_prepared(connection, paths, &prepared)?;
    let selected_deck_ids_json =
        serde_json::to_string(&selected_deck_ids).map_err(CommandError::serialization)?;
    let selected_tag_ids_json =
        serde_json::to_string(&selected_tag_ids).map_err(CommandError::serialization)?;
    let card_count = i64::try_from(prepared.len()).unwrap_or(i64::MAX);
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    transaction
        .execute(
            "INSERT INTO practice_sessions (
                 id, selected_deck_ids_json, selected_tag_ids_json,
                 shuffle, card_count, mode, started_at,
                 start_operation_id, start_request_fingerprint
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                session_id,
                selected_deck_ids_json,
                selected_tag_ids_json,
                bool_to_integer(shuffle),
                card_count,
                mode,
                time.now_utc_millis,
                operation_id,
                request_fingerprint,
            ],
        )
        .map_err(CommandError::database)?;
    for (position, card) in prepared.iter().enumerate() {
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
        insert_session_snapshot(&transaction, &session_id, card, time.now_utc_millis)?;
    }
    transaction.commit().map_err(CommandError::database)?;

    Ok(PracticeSessionDto {
        id: session_id,
        card_count,
        first_window,
        shuffle,
        mode: mode.to_string(),
        started_at: time.now_utc_millis,
    })
}

fn first_window_from_prepared(
    connection: &Connection,
    paths: &AppPaths,
    prepared: &[PreparedPracticeCard],
) -> Result<PracticeCardWindowDto, CommandError> {
    let window_size = usize::try_from(PRACTICE_WINDOW_MAX).unwrap_or(50);
    let first = prepared.iter().take(window_size).collect::<Vec<_>>();
    let note_ids = first
        .iter()
        .map(|card| card.note_id.clone())
        .collect::<Vec<_>>();
    let mut assets = load_assets_for_notes(connection, paths, &note_ids)?;
    let cards = first
        .into_iter()
        .map(|card| card.to_dto(assets.remove(&card.note_id).unwrap_or_default()))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(PracticeCardWindowDto {
        start_position: 0,
        cards,
        has_more: prepared.len() > window_size,
    })
}

fn load_exact_planned_cards(
    connection: &Connection,
    card_ids: &[String],
    time: &StudyTimeContext,
) -> Result<Vec<PlannedCard>, CommandError> {
    let mut rows = BTreeMap::<String, PlannedCard>::new();
    for chunk in chunks(card_ids) {
        let mut values = vec![SqlValue::Integer(time.now_utc_millis)];
        let placeholders = chunk
            .iter()
            .map(|card_id| format!("?{}", push_value(&mut values, card_id.clone())))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "WITH {signals}
             SELECT c.deck_id, c.id, c.note_id, s.due_at, s.state, {signal_value}
             FROM cards c
             JOIN notes n ON n.id = c.note_id
             JOIN decks d ON d.id = c.deck_id
             JOIN card_schedules s ON s.card_id = c.id
             {signal_join}
             WHERE c.id IN ({placeholders}) AND {studyable}",
            signals = signal_projection::LEARNING_SIGNAL_CTES,
            signal_value = signal_projection::value_sql(),
            signal_join = signal_projection::join_sql(),
            studyable = studyability::studyable_card_sql("?1"),
        );
        let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
        let chunk_rows = statement
            .query_map(params_from_iter(values.iter()), |row| {
                Ok(PlannedCard {
                    deck_id: row.get(0)?,
                    card_id: row.get(1)?,
                    note_id: row.get(2)?,
                    due_at: row.get(3)?,
                    state: row.get(4)?,
                    needs_more_practice: row.get::<_, i64>(5)? != 0,
                })
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        for row in chunk_rows {
            rows.insert(row.card_id.clone(), row);
        }
    }
    if rows.len() != card_ids.len() {
        return Err(CommandError::new(
            "practice_card_not_found",
            "A selected card is suspended, buried, deleted, archived, or otherwise unavailable.",
        ));
    }
    card_ids
        .iter()
        .map(|card_id| {
            rows.remove(card_id).ok_or_else(|| {
                CommandError::new(
                    "practice_card_not_found",
                    "A selected card is no longer available.",
                )
            })
        })
        .collect()
}

fn validate_scope(
    deck_ids: Vec<String>,
    tag_ids: Vec<String>,
) -> Result<(Vec<String>, Vec<String>), CommandError> {
    let deck_ids = normalize_entity_ids(deck_ids, "deck", MAX_SESSION_DECKS)?;
    let tag_ids = normalize_entity_ids(tag_ids, "tag", MAX_SESSION_TAGS)?;
    if deck_ids.is_empty() && tag_ids.is_empty() {
        return Err(CommandError::new(
            "empty_practice_scope",
            "Select at least one deck or tag for the practice session.",
        ));
    }
    Ok((deck_ids, tag_ids))
}

fn validate_current_scope(
    connection: &Connection,
    deck_ids: &[String],
    tag_ids: &[String],
) -> Result<(), CommandError> {
    validate_id_set_exists(
        connection,
        "decks",
        "id",
        "deleted_at IS NULL AND archived_at IS NULL",
        deck_ids,
        "One of the selected decks no longer exists or is archived.",
    )?;
    validate_id_set_exists(
        connection,
        "tags",
        "id",
        "1 = 1",
        tag_ids,
        "One of the selected tags no longer exists.",
    )
}

fn validate_id_set_exists(
    connection: &Connection,
    table: &str,
    column: &str,
    condition: &str,
    ids: &[String],
    message: &str,
) -> Result<(), CommandError> {
    if ids.is_empty() {
        return Ok(());
    }
    let allowed = [("decks", "id"), ("tags", "id")];
    if !allowed.contains(&(table, column)) {
        return Err(CommandError::new(
            "invalid_scope_table",
            "The practice scope requested an unsupported entity type.",
        ));
    }
    let mut found = 0_usize;
    for chunk in chunks(ids) {
        let mut values = Vec::<SqlValue>::new();
        let placeholders = chunk
            .iter()
            .map(|id| format!("?{}", push_value(&mut values, id.clone())))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT COUNT(*) FROM {table}
             WHERE {column} IN ({placeholders}) AND {condition}"
        );
        let count: i64 = connection
            .query_row(&sql, params_from_iter(values.iter()), |row| row.get(0))
            .map_err(CommandError::database)?;
        found = found.saturating_add(usize::try_from(count).unwrap_or(usize::MAX));
    }
    if found != ids.len() {
        return Err(CommandError::new("practice_scope_not_found", message));
    }
    Ok(())
}

fn validate_card_ids(values: Vec<String>) -> Result<Vec<String>, CommandError> {
    let mut seen = HashSet::new();
    let mut unique = Vec::new();
    for value in values {
        validate_entity_id(&value, "card")?;
        if seen.insert(value.clone()) {
            unique.push(value);
        }
    }
    if unique.is_empty() {
        return Err(CommandError::new(
            "no_cards_selected",
            "Select at least one card for the practice session.",
        ));
    }
    if unique.len() > MAX_SESSION_CARDS {
        return Err(CommandError::new(
            "too_many_practice_cards",
            format!("A temporary practice session can contain at most {MAX_SESSION_CARDS} cards."),
        ));
    }
    Ok(unique)
}

fn shuffle_key(session_id: &str, card_id: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    session_id.hash(&mut hasher);
    card_id.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod operation_tests;
