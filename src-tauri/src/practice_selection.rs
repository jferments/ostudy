//! Practice-session selection for both scheduled and voluntary study.
//!
//! Each selected deck contributes its due cards when it has any. When a deck
//! has no cards due, all of its active cards are included instead. This keeps
//! the normal scheduled workflow intact while allowing a learner to practice a
//! deck intentionally before its next due date.

use std::{
    collections::{BTreeSet, HashMap},
    hash::{Hash, Hasher},
};

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use tauri::State;

use crate::{
    cards::get_card_from_connection,
    database::Database,
    error::CommandError,
    paths::AppPaths,
    practice::{PracticeCardDto, PracticeSessionDto, StartPracticeInput},
    util::{bool_to_integer, generate_id, unix_time_millis, validate_entity_id},
};

const MAX_SESSION_DECKS: usize = 100;

type PendingCard = (String, String, String, i64);
type DeckCard = (String, String, i64);

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

fn start_selected_practice_in_connection(
    connection: &mut Connection,
    paths: &AppPaths,
    deck_ids: Vec<String>,
    shuffle: bool,
) -> Result<PracticeSessionDto, CommandError> {
    let now = unix_time_millis()?;
    let mut deck_names = HashMap::new();
    let mut pending_cards: Vec<PendingCard> = Vec::new();

    for deck_id in &deck_ids {
        let deck_name: String = connection
            .query_row(
                "SELECT name FROM decks\n\
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
            pending_cards.push((deck_id.clone(), card_id, note_id, due_at));
        }
    }

    if pending_cards.is_empty() {
        return Err(CommandError::new(
            "no_cards_available",
            "The selected decks do not contain any active flashcards to practice.",
        ));
    }

    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    let session_id = generate_id(&transaction)?;
    if shuffle {
        pending_cards.sort_by_key(|(_, card_id, _, _)| shuffle_key(&session_id, card_id));
    }

    let selected_deck_ids_json =
        serde_json::to_string(&deck_ids).map_err(CommandError::serialization)?;
    transaction
        .execute(
            "INSERT INTO practice_sessions (\n\
                 id, selected_deck_ids_json, shuffle, card_count, started_at\n\
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

    for (position, (_, card_id, _, _)) in pending_cards.iter().enumerate() {
        transaction
            .execute(
                "INSERT INTO practice_session_cards (session_id, card_id, position)\n\
                 VALUES (?1, ?2, ?3)",
                params![
                    session_id,
                    card_id,
                    i64::try_from(position).unwrap_or(i64::MAX),
                ],
            )
            .map_err(CommandError::database)?;
    }
    transaction.commit().map_err(CommandError::database)?;

    let mut cards = Vec::with_capacity(pending_cards.len());
    for (deck_id, _, note_id, due_at) in pending_cards {
        let card = get_card_from_connection(connection, paths, &note_id)?;
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
        "SELECT c.id, c.note_id, s.due_at\n\
         FROM cards c\n\
         JOIN notes n ON n.id = c.note_id\n\
         JOIN card_schedules s ON s.card_id = c.id\n\
         WHERE c.deck_id = ?1\n\
           AND c.deleted_at IS NULL\n\
           AND c.suspended_at IS NULL\n\
           AND n.deleted_at IS NULL\n\
           {due_clause}\n\
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
                "INSERT INTO decks (id, name, description, sort_order, created_at, updated_at, revision)\n\
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
                "INSERT INTO notes (\n\
                     id, deck_id, note_type, content_format, content_schema_version,\n\
                     front_doc, back_doc, front_text, back_text, created_at, updated_at, revision\n\
                 ) VALUES (\n\
                     ?1, ?2, 'basic', 'tiptap-json', 1,\n\
                     '{\"type\":\"doc\",\"content\":[{\"type\":\"paragraph\"}]}',\n\
                     '{\"type\":\"doc\",\"content\":[{\"type\":\"paragraph\"}]}',\n\
                     'Q', 'A', ?3, ?3, 1\n\
                 )",
                params![note_id, deck_id, now],
            )
            .map_err(CommandError::database)?;
        connection
            .execute(
                "INSERT INTO cards (\n\
                     id, note_id, deck_id, direction, template_ordinal, created_at, updated_at\n\
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
