//! Practice-session selection for scheduled, voluntary, and filtered study.
//!
//! Deck-based sessions use due cards first, falling back to every active card
//! when a deck has nothing due. Search/Filter can also provide an explicit set
//! of card identifiers. A Fill in the blank note owns one scheduled card; the
//! selected blank is snapshotted into the session so it stays stable until the
//! learner rates or abandons that session.

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
    paths::AppPaths,
    practice::{PracticeCardDto, PracticeSessionDto},
    util::{bool_to_integer, generate_id, unix_time_millis, validate_entity_id},
};

const MAX_SESSION_DECKS: usize = 100;
const MAX_SESSION_CARDS: usize = 10_000;

type PendingCard = (String, String, String, i64, String);
type DeckCard = (String, String, i64);

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct StartPracticeInput {
    pub deck_ids: Vec<String>,
    pub shuffle: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct StartCardPracticeInput {
    pub card_ids: Vec<String>,
    pub shuffle: bool,
}

#[tauri::command]
pub fn start_selected_practice(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: StartPracticeInput,
) -> Result<PracticeSessionDto, CommandError> {
    let deck_ids = validate_deck_ids(input.deck_ids)?;
    database.with_connection(|connection| {
        start_selected_practice_in_connection(connection, &paths, deck_ids, input.shuffle)
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
) -> Result<PracticeSessionDto, CommandError> {
    let now = unix_time_millis()?;
    let mut deck_names = HashMap::new();
    let mut pending_cards = Vec::new();

    for deck_id in &deck_ids {
        let deck_name: String = connection
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
        deck_names.insert(deck_id.clone(), deck_name);

        for (card_id, note_id, due_at) in cards_for_selected_deck(connection, deck_id, now)? {
            let variant = presentation_variant_for_card(connection, &card_id)?;
            pending_cards.push((deck_id.clone(), card_id, note_id, due_at, variant));
        }
    }

    create_practice_session(
        connection,
        paths,
        deck_ids,
        deck_names,
        pending_cards,
        shuffle,
        "The selected decks do not contain any active flashcards to practice.",
    )
}

fn start_card_practice_in_connection(
    connection: &mut Connection,
    paths: &AppPaths,
    card_ids: Vec<String>,
    shuffle: bool,
) -> Result<PracticeSessionDto, CommandError> {
    let mut deck_names = HashMap::new();
    let mut selected_decks = BTreeSet::new();
    let mut pending_cards = Vec::with_capacity(card_ids.len());

    for card_id in card_ids {
        let selected: Option<(String, String, String, String, i64)> = connection
            .query_row(
                "SELECT c.id, c.note_id, c.deck_id, d.name, s.due_at
                 FROM cards c
                 JOIN notes n ON n.id = c.note_id
                 JOIN decks d ON d.id = c.deck_id
                 JOIN card_schedules s ON s.card_id = c.id
                 WHERE c.id = ?1
                   AND c.deleted_at IS NULL
                   AND c.suspended_at IS NULL
                   AND n.deleted_at IS NULL
                   AND d.deleted_at IS NULL
                   AND d.archived_at IS NULL",
                [card_id.as_str()],
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
        let Some((active_card_id, note_id, deck_id, deck_name, due_at)) = selected else {
            return Err(CommandError::new(
                "practice_card_not_found",
                "A selected card is no longer available to practice.",
            ));
        };
        let variant = presentation_variant_for_card(connection, &active_card_id)?;
        selected_decks.insert(deck_id.clone());
        deck_names.insert(deck_id.clone(), deck_name);
        pending_cards.push((deck_id, active_card_id, note_id, due_at, variant));
    }

    create_practice_session(
        connection,
        paths,
        selected_decks.into_iter().collect(),
        deck_names,
        pending_cards,
        shuffle,
        "The filtered result does not contain any active cards to practice.",
    )
}

fn create_practice_session(
    connection: &mut Connection,
    paths: &AppPaths,
    selected_deck_ids: Vec<String>,
    deck_names: HashMap<String, String>,
    mut pending_cards: Vec<PendingCard>,
    shuffle: bool,
    empty_message: &str,
) -> Result<PracticeSessionDto, CommandError> {
    if pending_cards.is_empty() {
        return Err(CommandError::new("no_cards_available", empty_message));
    }
    if pending_cards.len() > MAX_SESSION_CARDS {
        return Err(CommandError::new(
            "too_many_practice_cards",
            format!("A temporary practice session can contain at most {MAX_SESSION_CARDS} cards."),
        ));
    }

    let now = unix_time_millis()?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    let session_id = generate_id(&transaction)?;
    if shuffle {
        pending_cards.sort_by_key(|(_, card_id, _, _, _)| shuffle_key(&session_id, card_id));
    }

    let selected_deck_ids_json =
        serde_json::to_string(&selected_deck_ids).map_err(CommandError::serialization)?;
    transaction
        .execute(
            "INSERT INTO practice_sessions (
                 id, selected_deck_ids_json, shuffle, card_count, started_at
             ) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                session_id,
                selected_deck_ids_json,
                bool_to_integer(shuffle),
                i64::try_from(pending_cards.len()).unwrap_or(i64::MAX),
                now,
            ],
        )
        .map_err(CommandError::database)?;

    for (position, (_, card_id, _, _, variant)) in pending_cards.iter().enumerate() {
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
    for (deck_id, card_id, _, due_at, variant) in pending_cards {
        let card = get_generated_card_for_practice(connection, paths, &card_id, &variant)?;
        let deck_name = deck_names
            .get(&deck_id)
            .cloned()
            .unwrap_or_else(|| "Deck".to_string());
        cards.push(PracticeCardDto {
            card,
            deck_name,
            due_at,
        });
    }

    Ok(PracticeSessionDto {
        id: session_id,
        cards,
        shuffle,
        started_at: now,
    })
}

fn cards_for_selected_deck(
    connection: &Connection,
    deck_id: &str,
    now: i64,
) -> Result<Vec<DeckCard>, CommandError> {
    let due_cards = query_deck_cards(connection, deck_id, Some(now))?;
    if due_cards.is_empty() {
        query_deck_cards(connection, deck_id, None)
    } else {
        Ok(due_cards)
    }
}

fn query_deck_cards(
    connection: &Connection,
    deck_id: &str,
    due_at_or_before: Option<i64>,
) -> Result<Vec<DeckCard>, CommandError> {
    let due_clause = if due_at_or_before.is_some() {
        "AND s.due_at <= ?2"
    } else {
        ""
    };
    let sql = format!(
        "SELECT c.id, c.note_id, s.due_at
         FROM cards c
         JOIN notes n ON n.id = c.note_id
         JOIN card_schedules s ON s.card_id = c.id
         WHERE c.deck_id = ?1
           AND c.deleted_at IS NULL
           AND c.suspended_at IS NULL
           AND n.deleted_at IS NULL
           {due_clause}
         ORDER BY s.due_at ASC, c.created_at ASC"
    );
    let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
    let map_row = |row: &rusqlite::Row<'_>| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
        ))
    };
    let rows = match due_at_or_before {
        Some(cutoff) => statement
            .query_map(params![deck_id, cutoff], map_row)
            .map_err(CommandError::database)?,
        None => statement
            .query_map([deck_id], map_row)
            .map_err(CommandError::database)?,
    };
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scheduler::insert_new_schedule;

    #[test]
    fn includes_all_cards_when_a_deck_has_none_due() {
        let database = Database::open_in_memory().expect("database should open");
        let now = unix_time_millis().expect("clock should work");
        database
            .with_connection(|connection| {
                let deck_id = insert_deck(connection, now)?;
                insert_card(connection, &deck_id, now, now + 86_400_000)?;
                insert_card(connection, &deck_id, now, now + 172_800_000)?;
                let selected = cards_for_selected_deck(connection, &deck_id, now)?;
                assert_eq!(selected.len(), 2);
                assert!(selected.iter().all(|(_, _, due_at)| *due_at > now));
                Ok(())
            })
            .expect("fallback selection should succeed");
    }

    #[test]
    fn keeps_due_first_behavior_when_a_deck_has_due_cards() {
        let database = Database::open_in_memory().expect("database should open");
        let now = unix_time_millis().expect("clock should work");
        database
            .with_connection(|connection| {
                let deck_id = insert_deck(connection, now)?;
                let due_card = insert_card(connection, &deck_id, now, now)?;
                insert_card(connection, &deck_id, now, now + 86_400_000)?;
                let selected = cards_for_selected_deck(connection, &deck_id, now)?;
                assert_eq!(selected.len(), 1);
                assert_eq!(selected[0].0, due_card);
                Ok(())
            })
            .expect("scheduled selection should succeed");
    }

    fn insert_deck(connection: &Connection, now: i64) -> Result<String, CommandError> {
        let deck_id = generate_id(connection)?;
        connection
            .execute(
                "INSERT INTO decks (id, name, description, sort_order, created_at, updated_at, revision)
                 VALUES (?1, 'Test', '', 0, ?2, ?2, 1)",
                params![deck_id, now],
            )
            .map_err(CommandError::database)?;
        Ok(deck_id)
    }

    fn insert_card(
        connection: &Connection,
        deck_id: &str,
        now: i64,
        due_at: i64,
    ) -> Result<String, CommandError> {
        let note_id = generate_id(connection)?;
        let card_id = generate_id(connection)?;
        connection
            .execute(
                "INSERT INTO notes (
                     id, deck_id, note_type, content_format, content_schema_version,
                     front_doc, back_doc, front_text, back_text, created_at, updated_at, revision
                 ) VALUES (
                     ?1, ?2, 'basic', 'tiptap-json', 1,
                     '{\"type\":\"doc\",\"content\":[{\"type\":\"paragraph\"}]}',
                     '{\"type\":\"doc\",\"content\":[{\"type\":\"paragraph\"}]}',
                     'Q', 'A', ?3, ?3, 1
                 )",
                params![note_id, deck_id, now],
            )
            .map_err(CommandError::database)?;
        connection
            .execute(
                "INSERT INTO cards (
                     id, note_id, deck_id, direction, template_ordinal, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, 'forward', 0, ?4, ?4)",
                params![card_id, note_id, deck_id, now],
            )
            .map_err(CommandError::database)?;
        insert_new_schedule(connection, &card_id, now)?;
        connection
            .execute(
                "UPDATE card_schedules SET due_at = ?1 WHERE card_id = ?2",
                params![due_at, card_id],
            )
            .map_err(CommandError::database)?;
        Ok(card_id)
    }
}
