//! Lightweight practice-session event tracking.
//!
//! Phase 2 session construction, interval previews, direct 0-3 review scoring,
//! scheduling, struggling-card detection, and undo live in `study`. This module
//! retains only the generic viewed/revealed/completed session events shared by
//! the frontend.

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::Deserialize;
use tauri::State;

use crate::{
    database::Database,
    error::CommandError,
    util::{unix_time_millis, validate_entity_id},
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PracticeCardEventInput {
    pub session_id: String,
    pub card_id: String,
}

#[tauri::command]
pub fn mark_practice_card_viewed(
    database: State<'_, Database>,
    input: PracticeCardEventInput,
) -> Result<(), CommandError> {
    validate_entity_id(&input.session_id, "practice session")?;
    validate_entity_id(&input.card_id, "card")?;
    database.with_connection(|connection| {
        record_practice_event(connection, &input.session_id, &input.card_id, false)
    })
}

#[tauri::command]
pub fn mark_practice_card_revealed(
    database: State<'_, Database>,
    input: PracticeCardEventInput,
) -> Result<(), CommandError> {
    validate_entity_id(&input.session_id, "practice session")?;
    validate_entity_id(&input.card_id, "card")?;
    database.with_connection(|connection| {
        record_practice_event(connection, &input.session_id, &input.card_id, true)
    })
}

#[tauri::command]
pub fn complete_practice_session(
    database: State<'_, Database>,
    session_id: String,
) -> Result<(), CommandError> {
    validate_entity_id(&session_id, "practice session")?;
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        let changed = connection
            .execute(
                "UPDATE practice_sessions\n\
                 SET completed_at = COALESCE(completed_at, ?1)\n\
                 WHERE id = ?2",
                params![now, session_id],
            )
            .map_err(CommandError::database)?;
        if changed == 0 {
            return Err(CommandError::new(
                "practice_session_not_found",
                "That practice session no longer exists.",
            ));
        }
        Ok(())
    })
}

fn record_practice_event(
    connection: &mut Connection,
    session_id: &str,
    card_id: &str,
    revealed: bool,
) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;

    let changed = if revealed {
        transaction.execute(
            "UPDATE practice_session_cards\n\
             SET viewed_at = COALESCE(viewed_at, ?1),\n\
                 revealed_at = COALESCE(revealed_at, ?1)\n\
             WHERE session_id = ?2\n\
               AND card_id = ?3\n\
               AND rated_at IS NULL\n\
               AND EXISTS (\n\
                   SELECT 1 FROM practice_sessions ps\n\
                   WHERE ps.id = ?2 AND ps.completed_at IS NULL\n\
               )",
            params![now, session_id, card_id],
        )
    } else {
        transaction.execute(
            "UPDATE practice_session_cards\n\
             SET viewed_at = COALESCE(viewed_at, ?1)\n\
             WHERE session_id = ?2\n\
               AND card_id = ?3\n\
               AND rated_at IS NULL\n\
               AND EXISTS (\n\
                   SELECT 1 FROM practice_sessions ps\n\
                   WHERE ps.id = ?2 AND ps.completed_at IS NULL\n\
               )",
            params![now, session_id, card_id],
        )
    }
    .map_err(CommandError::database)?;

    if changed == 0 {
        let session_completed: Option<Option<i64>> = transaction
            .query_row(
                "SELECT completed_at FROM practice_sessions WHERE id = ?1",
                [session_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(CommandError::database)?;
        return match session_completed {
            Some(Some(_)) => Err(CommandError::new(
                "practice_session_complete",
                "This practice session has already ended.",
            )),
            Some(None) => Err(CommandError::new(
                "practice_card_not_found",
                "That card is not an unrated part of this practice session.",
            )),
            None => Err(CommandError::new(
                "practice_session_not_found",
                "That practice session no longer exists.",
            )),
        };
    }

    refresh_session_counters(&transaction, session_id, now)?;
    transaction.commit().map_err(CommandError::database)
}

fn refresh_session_counters(
    connection: &Connection,
    session_id: &str,
    now: i64,
) -> Result<(), CommandError> {
    let changed = connection
        .execute(
            "UPDATE practice_sessions\n\
             SET viewed_count = (\n\
                     SELECT COUNT(*) FROM practice_session_cards\n\
                     WHERE session_id = ?1 AND viewed_at IS NOT NULL\n\
                 ),\n\
                 revealed_count = (\n\
                     SELECT COUNT(*) FROM practice_session_cards\n\
                     WHERE session_id = ?1 AND revealed_at IS NOT NULL\n\
                 ),\n\
                 rated_count = (\n\
                     SELECT COUNT(*) FROM practice_session_cards\n\
                     WHERE session_id = ?1 AND rated_at IS NOT NULL\n\
                 ),\n\
                 correct_count = (\n\
                     SELECT COUNT(*) FROM practice_session_cards\n\
                     WHERE session_id = ?1 AND rating BETWEEN 2 AND 4\n\
                 ),\n\
                 completed_at = CASE\n\
                     WHEN (SELECT COUNT(*) FROM practice_session_cards\n\
                           WHERE session_id = ?1 AND rated_at IS NOT NULL) >= card_count\n\
                     THEN COALESCE(completed_at, ?2)\n\
                     ELSE completed_at\n\
                 END\n\
             WHERE id = ?1",
            params![session_id, now],
        )
        .map_err(CommandError::database)?;
    if changed == 0 {
        return Err(CommandError::new(
            "practice_session_not_found",
            "That practice session no longer exists.",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        scheduler::insert_new_schedule,
        util::{generate_id, unix_time_millis},
    };

    #[test]
    fn completed_sessions_reject_card_events() {
        let database = Database::open_in_memory().expect("database should open");
        let now = unix_time_millis().expect("clock should work");

        let (session_id, card_id) = database
            .with_connection(|connection| {
                let deck_id = generate_id(connection)?;
                let note_id = generate_id(connection)?;
                let card_id = generate_id(connection)?;
                let session_id = generate_id(connection)?;
                connection
                    .execute(
                        "INSERT INTO decks (id, name, description, sort_order, created_at, updated_at, revision)\n\
                         VALUES (?1, 'Test', '', 0, ?2, ?2, 1)",
                        params![deck_id, now],
                    )
                    .map_err(CommandError::database)?;
                connection
                    .execute(
                        "INSERT INTO notes (id, deck_id, note_type, content_format, content_schema_version, front_doc, back_doc, front_text, back_text, created_at, updated_at, revision)\n\
                         VALUES (?1, ?2, 'basic', 'tiptap-json', 1, '{\"type\":\"doc\",\"content\":[{\"type\":\"paragraph\"}]}', '{\"type\":\"doc\",\"content\":[{\"type\":\"paragraph\"}]}', 'Q', 'A', ?3, ?3, 1)",
                        params![note_id, deck_id, now],
                    )
                    .map_err(CommandError::database)?;
                connection
                    .execute(
                        "INSERT INTO cards (id, note_id, deck_id, direction, template_ordinal, created_at, updated_at)\n\
                         VALUES (?1, ?2, ?3, 'forward', 0, ?4, ?4)",
                        params![card_id, note_id, deck_id, now],
                    )
                    .map_err(CommandError::database)?;
                insert_new_schedule(connection, &card_id, now)?;
                connection
                    .execute(
                        "INSERT INTO practice_sessions (id, selected_deck_ids_json, shuffle, card_count, started_at, completed_at)\n\
                         VALUES (?1, '[]', 0, 1, ?2, ?2)",
                        params![session_id, now],
                    )
                    .map_err(CommandError::database)?;
                connection
                    .execute(
                        "INSERT INTO practice_session_cards (session_id, card_id, position)\n\
                         VALUES (?1, ?2, 0)",
                        params![session_id, card_id],
                    )
                    .map_err(CommandError::database)?;
                Ok((session_id, card_id))
            })
            .expect("fixtures should be created");

        let error = database
            .with_connection(|connection| {
                record_practice_event(connection, &session_id, &card_id, false)
            })
            .expect_err("completed sessions must reject card events");
        assert_eq!(error.code, "practice_session_complete");
    }

    #[test]
    fn reveal_also_records_view_and_is_idempotent() {
        let database = Database::open_in_memory().expect("database should open");
        let now = unix_time_millis().expect("clock should work");

        let (session_id, card_id) = database
            .with_connection(|connection| {
                let deck_id = generate_id(connection)?;
                let note_id = generate_id(connection)?;
                let card_id = generate_id(connection)?;
                let session_id = generate_id(connection)?;
                connection
                    .execute(
                        "INSERT INTO decks (id, name, description, sort_order, created_at, updated_at, revision) VALUES (?1, 'Test', '', 0, ?2, ?2, 1)",
                        params![deck_id, now],
                    )
                    .map_err(CommandError::database)?;
                connection
                    .execute(
                        "INSERT INTO notes (id, deck_id, note_type, content_format, content_schema_version, front_doc, back_doc, front_text, back_text, created_at, updated_at, revision) VALUES (?1, ?2, 'basic', 'tiptap-json', 1, '{\"type\":\"doc\",\"content\":[{\"type\":\"paragraph\"}]}', '{\"type\":\"doc\",\"content\":[{\"type\":\"paragraph\"}]}', 'Q', 'A', ?3, ?3, 1)",
                        params![note_id, deck_id, now],
                    )
                    .map_err(CommandError::database)?;
                connection
                    .execute(
                        "INSERT INTO cards (id, note_id, deck_id, direction, template_ordinal, created_at, updated_at) VALUES (?1, ?2, ?3, 'forward', 0, ?4, ?4)",
                        params![card_id, note_id, deck_id, now],
                    )
                    .map_err(CommandError::database)?;
                insert_new_schedule(connection, &card_id, now)?;
                connection
                    .execute(
                        "INSERT INTO practice_sessions (id, selected_deck_ids_json, shuffle, card_count, started_at) VALUES (?1, '[]', 0, 1, ?2)",
                        params![session_id, now],
                    )
                    .map_err(CommandError::database)?;
                connection
                    .execute(
                        "INSERT INTO practice_session_cards (session_id, card_id, position) VALUES (?1, ?2, 0)",
                        params![session_id, card_id],
                    )
                    .map_err(CommandError::database)?;
                Ok((session_id, card_id))
            })
            .expect("fixtures should be created");

        database
            .with_connection(|connection| {
                record_practice_event(connection, &session_id, &card_id, true)?;
                record_practice_event(connection, &session_id, &card_id, true)?;
                let counters: (i64, i64) = connection
                    .query_row(
                        "SELECT viewed_count, revealed_count FROM practice_sessions WHERE id = ?1",
                        [&session_id],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(counters, (1, 1));
                Ok(())
            })
            .expect("reveal should be recorded");
    }
}
