//! Practice-session selection for bounded scheduled, explicit extra, and custom study.

use std::{
    collections::{BTreeSet, HashMap, HashSet},
    hash::{Hash, Hasher},
};

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::Deserialize;
use tauri::State;

use crate::{
    authoring::{get_generated_card_for_practice, presentation_variant_for_card},
    database::Database,
    error::CommandError,
    learning_signals::needs_more_practice,
    paths::AppPaths,
    practice::{PracticeCardDto, PracticeSessionDto},
    settings::load_settings,
    util::{bool_to_integer, generate_id, unix_time_millis, validate_entity_id},
    workload::{
        planned_extra_cards, planned_scheduled_cards, workload_overview, PlannedCard,
        WorkloadOverviewDto,
    },
};

const MAX_SESSION_DECKS: usize = 100;
const MAX_SESSION_CARDS: usize = 10_000;
const DAY_MS: i64 = 86_400_000;

type PendingCard = (String, String, String, i64, i64, bool, String);

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct PracticeWorkloadInput {
    pub deck_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct StartPracticeInput {
    pub deck_ids: Vec<String>,
    pub shuffle: bool,
    pub mode: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct StartCardPracticeInput {
    pub card_ids: Vec<String>,
    pub shuffle: bool,
}

#[tauri::command]
pub fn get_practice_workload(
    database: State<'_, Database>,
    input: PracticeWorkloadInput,
) -> Result<WorkloadOverviewDto, CommandError> {
    let deck_ids = validate_deck_ids(input.deck_ids)?;
    database.with_connection(|connection| {
        let settings = load_settings(connection)?;
        workload_overview(connection, &deck_ids, &settings)
    })
}

#[tauri::command]
pub fn start_selected_practice(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: StartPracticeInput,
) -> Result<PracticeSessionDto, CommandError> {
    let deck_ids = validate_deck_ids(input.deck_ids)?;
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
            deck_ids,
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
    let card_ids = validate_card_ids(input.card_ids)?;
    database.with_connection(|connection| {
        start_card_practice_in_connection(connection, &paths, card_ids, input.shuffle)
    })
}

fn start_selected_practice_in_connection(
    connection: &mut Connection,
    paths: &AppPaths,
    deck_ids: Vec<String>,
    shuffle: bool,
    mode: &str,
) -> Result<PracticeSessionDto, CommandError> {
    let deck_names = load_deck_names(connection, &deck_ids)?;
    let planned = if mode == "scheduled" {
        let settings = load_settings(connection)?;
        planned_scheduled_cards(connection, &deck_ids, &settings)?.0
    } else {
        planned_extra_cards(connection, &deck_ids)?
    };
    if planned.is_empty() {
        let message = if mode == "scheduled" {
            "No cards fit the selected decks' remaining scheduled workload. Start an extra session to study anyway."
        } else {
            "The selected decks do not contain any active cards available for extra practice."
        };
        return Err(CommandError::new("no_cards_available", message));
    }
    let pending = hydrate_pending_variants(connection, planned)?;
    create_practice_session(
        connection,
        paths,
        deck_ids,
        deck_names,
        pending,
        shuffle,
        mode,
    )
}

fn start_card_practice_in_connection(
    connection: &mut Connection,
    paths: &AppPaths,
    card_ids: Vec<String>,
    shuffle: bool,
) -> Result<PracticeSessionDto, CommandError> {
    let now = unix_time_millis()?;
    let mut deck_names = HashMap::new();
    let mut selected_decks = BTreeSet::new();
    let mut pending = Vec::with_capacity(card_ids.len());
    for card_id in card_ids {
        let selected: Option<(String, String, String, String, i64, i64)> = connection
            .query_row(
                "SELECT c.id, c.note_id, c.deck_id, d.name, s.due_at, s.state
                 FROM cards c
                 JOIN notes n ON n.id = c.note_id
                 JOIN decks d ON d.id = c.deck_id
                 JOIN card_schedules s ON s.card_id = c.id
                 WHERE c.id = ?1
                   AND c.deleted_at IS NULL
                   AND c.suspended_at IS NULL
                   AND (c.buried_until IS NULL OR c.buried_until <= ?2)
                   AND n.deleted_at IS NULL
                   AND d.deleted_at IS NULL
                   AND d.archived_at IS NULL",
                params![card_id, now],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                    ))
                },
            )
            .optional()
            .map_err(CommandError::database)?;
        let Some((active_card_id, note_id, deck_id, deck_name, due_at, state)) = selected else {
            return Err(CommandError::new(
                "practice_card_not_found",
                "A selected card is suspended, buried, deleted, or otherwise unavailable.",
            ));
        };
        let variant = presentation_variant_for_card(connection, &active_card_id)?;
        let weak = needs_more_practice(connection, &active_card_id)?;
        selected_decks.insert(deck_id.clone());
        deck_names.insert(deck_id.clone(), deck_name);
        pending.push((deck_id, active_card_id, note_id, due_at, state, weak, variant));
    }
    create_practice_session(
        connection,
        paths,
        selected_decks.into_iter().collect(),
        deck_names,
        pending,
        shuffle,
        "custom",
    )
}

fn load_deck_names(
    connection: &Connection,
    deck_ids: &[String],
) -> Result<HashMap<String, String>, CommandError> {
    let mut names = HashMap::new();
    for deck_id in deck_ids {
        let name: String = connection
            .query_row(
                "SELECT name FROM decks
                 WHERE id = ?1 AND deleted_at IS NULL AND archived_at IS NULL",
                [deck_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(CommandError::database)?
            .ok_or_else(|| {
                CommandError::new(
                    "deck_not_found",
                    "One of the selected decks no longer exists.",
                )
            })?;
        names.insert(deck_id.clone(), name);
    }
    Ok(names)
}

fn hydrate_pending_variants(
    connection: &Connection,
    cards: Vec<PlannedCard>,
) -> Result<Vec<PendingCard>, CommandError> {
    cards
        .into_iter()
        .map(|card| {
            let variant = presentation_variant_for_card(connection, &card.card_id)?;
            Ok((
                card.deck_id,
                card.card_id,
                card.note_id,
                card.due_at,
                card.state,
                card.needs_more_practice,
                variant,
            ))
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn create_practice_session(
    connection: &mut Connection,
    paths: &AppPaths,
    selected_deck_ids: Vec<String>,
    deck_names: HashMap<String, String>,
    mut pending_cards: Vec<PendingCard>,
    shuffle: bool,
    mode: &str,
) -> Result<PracticeSessionDto, CommandError> {
    if pending_cards.len() > MAX_SESSION_CARDS {
        return Err(CommandError::new(
            "too_many_practice_cards",
            format!("A practice session can contain at most {MAX_SESSION_CARDS} cards."),
        ));
    }
    let now = unix_time_millis()?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    let session_id = generate_id(&transaction)?;
    if shuffle {
        pending_cards.sort_by_key(|(_, card_id, _, due_at, state, weak, _)| {
            (
                due_at.div_euclid(DAY_MS),
                !*weak,
                *state == 0,
                shuffle_key(&session_id, card_id),
            )
        });
    }
    let selected_deck_ids_json =
        serde_json::to_string(&selected_deck_ids).map_err(CommandError::serialization)?;
    transaction
        .execute(
            "INSERT INTO practice_sessions (
                 id, selected_deck_ids_json, shuffle, card_count, mode, started_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                session_id,
                selected_deck_ids_json,
                bool_to_integer(shuffle),
                i64::try_from(pending_cards.len()).unwrap_or(i64::MAX),
                mode,
                now,
            ],
        )
        .map_err(CommandError::database)?;
    for (position, (_, card_id, _, _, _, _, variant)) in pending_cards.iter().enumerate() {
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

    let mut cards = Vec::with_capacity(pending_cards.len());
    for (deck_id, card_id, _, due_at, state, weak, variant) in pending_cards {
        let card = get_generated_card_for_practice(connection, paths, &card_id, &variant)?;
        let deck_name = deck_names
            .get(&deck_id)
            .cloned()
            .unwrap_or_else(|| "Deck".to_string());
        cards.push(PracticeCardDto {
            card,
            deck_name,
            due_at,
            schedule_state: state,
            needs_more_practice: weak,
        });
    }
    Ok(PracticeSessionDto {
        id: session_id,
        cards,
        shuffle,
        mode: mode.to_string(),
        started_at: now,
    })
}

fn validate_deck_ids(values: Vec<String>) -> Result<Vec<String>, CommandError> {
    let mut unique = BTreeSet::new();
    for value in values {
        validate_entity_id(&value, "deck")?;
        unique.insert(value);
    }
    if unique.is_empty() {
        return Err(CommandError::new(
            "no_decks_selected",
            "Select at least one deck for the practice session.",
        ));
    }
    if unique.len() > MAX_SESSION_DECKS {
        return Err(CommandError::new(
            "too_many_decks",
            "A practice session can include at most 100 decks.",
        ));
    }
    Ok(unique.into_iter().collect())
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
