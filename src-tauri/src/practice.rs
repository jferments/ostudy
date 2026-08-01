//! Practice-session records and atomic FSRS review submission.
//!
//! Session selection is owned by `practice_selection`; this module owns the
//! durable session events and review transaction. An answer must be revealed
//! before rating. Each accepted rating appends the immutable review event and
//! updates the mutable schedule atomically. A unique `submission_id` makes
//! retries and accidental double-clicks idempotent.

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    cards::CardDto,
    database::Database,
    error::CommandError,
    scheduler::{
        apply_review, load_schedule, update_schedule, ReviewOutcome, ScheduleTransition,
        SCHEDULER_NAME, SCHEDULER_VERSION,
    },
    settings::load_settings,
    util::{bool_to_integer, generate_id, unix_time_millis, validate_entity_id},
};

const MAX_RESPONSE_TIME_MS: i64 = 86_400_000;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PracticeCardDto {
    pub card: CardDto,
    pub deck_name: String,
    pub due_at: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PracticeSessionDto {
    pub id: String,
    pub cards: Vec<PracticeCardDto>,
    pub shuffle: bool,
    pub started_at: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PracticeCardEventInput {
    pub session_id: String,
    pub card_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitReviewInput {
    pub session_id: String,
    pub card_id: String,
    pub submission_id: String,
    pub outcome: String,
    pub response_time_ms: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewResultDto {
    pub card_id: String,
    pub rating: i64,
    pub outcome: String,
    pub due_at: i64,
    pub completed: bool,
    pub rated_count: i64,
    pub correct_count: i64,
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
pub fn submit_practice_review(
    database: State<'_, Database>,
    input: SubmitReviewInput,
) -> Result<ReviewResultDto, CommandError> {
    validate_review_input(&input)?;
    database.with_connection(|connection| submit_review_in_connection(connection, input))
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

fn submit_review_in_connection(
    connection: &mut Connection,
    input: SubmitReviewInput,
) -> Result<ReviewResultDto, CommandError> {
    let outcome = ReviewOutcome::parse(&input.outcome)?;
    let reviewed_at = unix_time_millis()?;
    let desired_retention = load_settings(connection)?.desired_retention;

    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;

    if let Some(existing) = review_result_for_submission(&transaction, &input.submission_id)? {
        transaction.commit().map_err(CommandError::database)?;
        return Ok(existing);
    }

    let session_state: Option<(Option<i64>, Option<i64>, Option<i64>)> = transaction
        .query_row(
            "SELECT psc.revealed_at, psc.rated_at, ps.completed_at\n\
             FROM practice_session_cards psc\n\
             JOIN practice_sessions ps ON ps.id = psc.session_id\n\
             WHERE psc.session_id = ?1 AND psc.card_id = ?2",
            params![input.session_id, input.card_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(CommandError::database)?;
    let Some((revealed_at, rated_at, completed_at)) = session_state else {
        return Err(CommandError::new(
            "practice_card_not_found",
            "That card is not part of this practice session.",
        ));
    };
    if completed_at.is_some() {
        return Err(CommandError::new(
            "practice_session_complete",
            "This practice session has already ended.",
        ));
    }
    if revealed_at.is_none() {
        return Err(CommandError::new(
            "answer_not_revealed",
            "Reveal the answer before rating the card.",
        ));
    }
    if rated_at.is_some() {
        return Err(CommandError::new(
            "practice_card_already_rated",
            "This card has already been rated in the current session.",
        ));
    }

    let current = load_schedule(&transaction, &input.card_id)?;
    let transition = apply_review(current, outcome, reviewed_at, desired_retention)?;
    insert_review_event(&transaction, &input, &transition, reviewed_at)?;
    update_schedule(&transaction, &transition.after)?;

    let updated_session_card = transaction
        .execute(
            "UPDATE practice_session_cards\n\
             SET rated_at = ?1, rating = ?2, response_time_ms = ?3\n\
             WHERE session_id = ?4 AND card_id = ?5 AND rated_at IS NULL",
            params![
                reviewed_at,
                outcome.rating_number(),
                input.response_time_ms,
                input.session_id,
                input.card_id,
            ],
        )
        .map_err(CommandError::database)?;
    if updated_session_card != 1 {
        return Err(CommandError::new(
            "practice_card_already_rated",
            "This card has already been rated in the current session.",
        ));
    }

    refresh_session_counters(&transaction, &input.session_id, reviewed_at)?;
    let (rated_count, correct_count, completed_at): (i64, i64, Option<i64>) = transaction
        .query_row(
            "SELECT rated_count, correct_count, completed_at\n\
             FROM practice_sessions WHERE id = ?1",
            [input.session_id.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(CommandError::database)?;

    let result = ReviewResultDto {
        card_id: input.card_id,
        rating: outcome.rating_number(),
        outcome: outcome.as_str().to_string(),
        due_at: transition.after.due_at,
        completed: completed_at.is_some(),
        rated_count,
        correct_count,
    };
    transaction.commit().map_err(CommandError::database)?;
    Ok(result)
}

fn insert_review_event(
    connection: &Connection,
    input: &SubmitReviewInput,
    transition: &ScheduleTransition,
    created_at: i64,
) -> Result<(), CommandError> {
    let event_id = generate_id(connection)?;
    connection
        .execute(
            "INSERT INTO review_events (\n\
                 id, submission_id, session_id, card_id, reviewed_at, rating,\n\
                 answer_correct, confidence, response_time_ms,\n\
                 state_before, state_after, due_before, due_after,\n\
                 stability_before, stability_after, difficulty_before, difficulty_after,\n\
                 scheduled_days_before, scheduled_days_after,\n\
                 scheduler_name, scheduler_version, created_at\n\
             ) VALUES (\n\
                 ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11,\n\
                 ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22\n\
             )",
            params![
                event_id,
                input.submission_id,
                input.session_id,
                input.card_id,
                transition.reviewed_at,
                transition.outcome.rating_number(),
                bool_to_integer(transition.outcome.answer_correct()),
                transition.outcome.as_str(),
                input.response_time_ms,
                transition.before.state,
                transition.after.state,
                transition.before.due_at,
                transition.after.due_at,
                transition.before.stability,
                transition.after.stability,
                transition.before.difficulty,
                transition.after.difficulty,
                transition.before.scheduled_days,
                transition.after.scheduled_days,
                SCHEDULER_NAME,
                SCHEDULER_VERSION,
                created_at,
            ],
        )
        .map_err(CommandError::database)?;
    Ok(())
}

fn review_result_for_submission(
    connection: &Connection,
    submission_id: &str,
) -> Result<Option<ReviewResultDto>, CommandError> {
    connection
        .query_row(
            "SELECT r.card_id, r.rating, r.confidence, r.due_after,\n\
                    ps.completed_at, ps.rated_count, ps.correct_count\n\
             FROM review_events r\n\
             LEFT JOIN practice_sessions ps ON ps.id = r.session_id\n\
             WHERE r.submission_id = ?1",
            [submission_id],
            |row| {
                Ok(ReviewResultDto {
                    card_id: row.get(0)?,
                    rating: row.get(1)?,
                    outcome: row.get(2)?,
                    due_at: row.get(3)?,
                    completed: row.get::<_, Option<i64>>(4)?.is_some(),
                    rated_count: row.get::<_, Option<i64>>(5)?.unwrap_or(0),
                    correct_count: row.get::<_, Option<i64>>(6)?.unwrap_or(0),
                })
            },
        )
        .optional()
        .map_err(CommandError::database)
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

fn validate_review_input(input: &SubmitReviewInput) -> Result<(), CommandError> {
    validate_entity_id(&input.session_id, "practice session")?;
    validate_entity_id(&input.card_id, "card")?;
    validate_entity_id(&input.submission_id, "review submission")?;
    ReviewOutcome::parse(&input.outcome)?;
    if !(0..=MAX_RESPONSE_TIME_MS).contains(&input.response_time_ms) {
        return Err(CommandError::new(
            "invalid_response_time",
            "The recorded response time is outside the supported range.",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{scheduler::insert_new_schedule, util::unix_time_millis};

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
    fn review_submission_is_idempotent_and_updates_the_schedule() {
        let database = Database::open_in_memory().expect("database should open");
        let now = unix_time_millis().expect("clock should work");

        let (session_id, card_id, submission_id) = database
            .with_connection(|connection| {
                let deck_id = generate_id(connection)?;
                let note_id = generate_id(connection)?;
                let card_id = generate_id(connection)?;
                let session_id = generate_id(connection)?;
                let submission_id = generate_id(connection)?;
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
                        "INSERT INTO practice_sessions (id, selected_deck_ids_json, shuffle, card_count, viewed_count, revealed_count, started_at)\n\
                         VALUES (?1, '[]', 0, 1, 1, 1, ?2)",
                        params![session_id, now],
                    )
                    .map_err(CommandError::database)?;
                connection
                    .execute(
                        "INSERT INTO practice_session_cards (session_id, card_id, position, viewed_at, revealed_at)\n\
                         VALUES (?1, ?2, 0, ?3, ?3)",
                        params![session_id, card_id, now],
                    )
                    .map_err(CommandError::database)?;
                Ok((session_id, card_id, submission_id))
            })
            .expect("fixtures should be created");

        let make_input = || SubmitReviewInput {
            session_id: session_id.clone(),
            card_id: card_id.clone(),
            submission_id: submission_id.clone(),
            outcome: "somewhat_confident".to_string(),
            response_time_ms: 1_500,
        };
        let first = database
            .with_connection(|connection| submit_review_in_connection(connection, make_input()))
            .expect("first review should succeed");
        let retry = database
            .with_connection(|connection| submit_review_in_connection(connection, make_input()))
            .expect("retry should be idempotent");
        assert_eq!(first.card_id, retry.card_id);
        assert_eq!(first.due_at, retry.due_at);

        database
            .with_connection(|connection| {
                let review_count: i64 = connection
                    .query_row("SELECT COUNT(*) FROM review_events", [], |row| row.get(0))
                    .map_err(CommandError::database)?;
                let reps: i64 = connection
                    .query_row(
                        "SELECT reps FROM card_schedules WHERE card_id = ?1",
                        [card_id.as_str()],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(review_count, 1);
                assert_eq!(reps, 1);
                Ok(())
            })
            .expect("review state should load");
    }
}
