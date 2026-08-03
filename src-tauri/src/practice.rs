//! Practice-session events and atomic FSRS review submission.

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    cards::CardDto,
    database::Database,
    error::CommandError,
    scheduler::{
        apply_review, load_schedule, preview_intervals, update_schedule, ReviewOutcome,
        ReviewPreviewSetDto, ScheduleTransition, SCHEDULER_NAME, SCHEDULER_VERSION,
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
    pub schedule_state: i64,
    pub needs_more_practice: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PracticeSessionDto {
    pub id: String,
    pub cards: Vec<PracticeCardDto>,
    pub shuffle: bool,
    pub mode: String,
    pub started_at: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PracticeCardEventInput {
    pub session_id: String,
    pub card_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SubmitReviewInput {
    pub session_id: String,
    pub card_id: String,
    pub submission_id: String,
    pub outcome: String,
    pub response_time_ms: i64,
    pub schedule_revision: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewResultDto {
    pub card_id: String,
    pub rating: i64,
    pub outcome: String,
    pub due_at: i64,
    pub interval_ms: i64,
    pub completed: bool,
    pub rated_count: i64,
    pub correct_count: i64,
}

#[tauri::command]
pub fn mark_practice_card_viewed(
    database: State<'_, Database>,
    input: PracticeCardEventInput,
) -> Result<(), CommandError> {
    validate_card_event(&input)?;
    database.with_connection(|connection| {
        record_practice_event(connection, &input.session_id, &input.card_id, false)
    })
}

#[tauri::command]
pub fn mark_practice_card_revealed(
    database: State<'_, Database>,
    input: PracticeCardEventInput,
) -> Result<ReviewPreviewSetDto, CommandError> {
    validate_card_event(&input)?;
    database.with_connection(|connection| {
        record_practice_event(connection, &input.session_id, &input.card_id, true)?;
        let now = unix_time_millis()?;
        let desired_retention = load_settings(connection)?.desired_retention;
        let schedule = load_schedule(connection, &input.card_id)?;
        preview_intervals(&schedule, now, desired_retention)
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
                "UPDATE practice_sessions
                 SET completed_at = COALESCE(completed_at, ?1)
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

fn validate_card_event(input: &PracticeCardEventInput) -> Result<(), CommandError> {
    validate_entity_id(&input.session_id, "practice session")?;
    validate_entity_id(&input.card_id, "card")
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
    let sql = if revealed {
        "UPDATE practice_session_cards
         SET viewed_at = COALESCE(viewed_at, ?1), revealed_at = COALESCE(revealed_at, ?1)
         WHERE session_id = ?2 AND card_id = ?3
           AND rated_at IS NULL AND dismissed_at IS NULL
           AND EXISTS (
               SELECT 1 FROM practice_sessions ps
               WHERE ps.id = ?2 AND ps.completed_at IS NULL
           )"
    } else {
        "UPDATE practice_session_cards
         SET viewed_at = COALESCE(viewed_at, ?1)
         WHERE session_id = ?2 AND card_id = ?3
           AND rated_at IS NULL AND dismissed_at IS NULL
           AND EXISTS (
               SELECT 1 FROM practice_sessions ps
               WHERE ps.id = ?2 AND ps.completed_at IS NULL
           )"
    };
    let changed = transaction
        .execute(sql, params![now, session_id, card_id])
        .map_err(CommandError::database)?;
    if changed == 0 {
        return Err(practice_card_state_error(&transaction, session_id)?);
    }
    refresh_session_counters(&transaction, session_id, now)?;
    transaction.commit().map_err(CommandError::database)
}

fn practice_card_state_error(
    connection: &Connection,
    session_id: &str,
) -> Result<CommandError, CommandError> {
    let completed_at: Option<Option<i64>> = connection
        .query_row(
            "SELECT completed_at FROM practice_sessions WHERE id = ?1",
            [session_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(CommandError::database)?;
    Ok(match completed_at {
        Some(Some(_)) => CommandError::new(
            "practice_session_complete",
            "This practice session has already ended.",
        ),
        Some(None) => CommandError::new(
            "practice_card_not_found",
            "That card is not an unrated part of this practice session.",
        ),
        None => CommandError::new(
            "practice_session_not_found",
            "That practice session no longer exists.",
        ),
    })
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

    let session_state: Option<(Option<i64>, Option<i64>, Option<i64>, Option<i64>)> = transaction
        .query_row(
            "SELECT psc.revealed_at, psc.rated_at, psc.dismissed_at, ps.completed_at
             FROM practice_session_cards psc
             JOIN practice_sessions ps ON ps.id = psc.session_id
             WHERE psc.session_id = ?1 AND psc.card_id = ?2",
            params![input.session_id, input.card_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()
        .map_err(CommandError::database)?;
    let Some((revealed_at, rated_at, dismissed_at, completed_at)) = session_state else {
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
    if dismissed_at.is_some() {
        return Err(CommandError::new(
            "practice_card_dismissed",
            "This card was removed from the current session.",
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
    if current.revision != input.schedule_revision {
        return Err(CommandError::new(
            "schedule_changed",
            "This card's schedule changed after the interval preview. Reveal it again and choose a rating.",
        ));
    }
    let transition = apply_review(current, outcome, reviewed_at, desired_retention)?;
    insert_review_event(&transaction, &input, &transition, reviewed_at)?;
    update_schedule(&transaction, &transition.after)?;

    let updated = transaction
        .execute(
            "UPDATE practice_session_cards
             SET rated_at = ?1, rating = ?2, response_time_ms = ?3
             WHERE session_id = ?4 AND card_id = ?5
               AND rated_at IS NULL AND dismissed_at IS NULL",
            params![
                reviewed_at,
                outcome.rating_number(),
                input.response_time_ms,
                input.session_id,
                input.card_id,
            ],
        )
        .map_err(CommandError::database)?;
    if updated != 1 {
        return Err(CommandError::new(
            "practice_card_already_rated",
            "This card has already been rated in the current session.",
        ));
    }

    refresh_session_counters(&transaction, &input.session_id, reviewed_at)?;
    let (rated_count, correct_count, completed_at): (i64, i64, Option<i64>) = transaction
        .query_row(
            "SELECT rated_count, correct_count, completed_at
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
        interval_ms: transition.after.due_at.saturating_sub(reviewed_at),
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
            "INSERT INTO review_events (
                 id, submission_id, session_id, card_id, reviewed_at, rating,
                 answer_correct, confidence, response_time_ms,
                 state_before, state_after, due_before, due_after,
                 stability_before, stability_after, difficulty_before, difficulty_after,
                 scheduled_days_before, scheduled_days_after,
                 scheduler_name, scheduler_version, created_at
             ) VALUES (
                 ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11,
                 ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22
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
            "SELECT r.card_id, r.rating, r.confidence, r.due_after, r.reviewed_at,
                    ps.completed_at, ps.rated_count, ps.correct_count
             FROM review_events r
             LEFT JOIN practice_sessions ps ON ps.id = r.session_id
             WHERE r.submission_id = ?1",
            [submission_id],
            |row| {
                let due_at: i64 = row.get(3)?;
                let reviewed_at: i64 = row.get(4)?;
                Ok(ReviewResultDto {
                    card_id: row.get(0)?,
                    rating: row.get(1)?,
                    outcome: row.get(2)?,
                    due_at,
                    interval_ms: due_at.saturating_sub(reviewed_at),
                    completed: row.get::<_, Option<i64>>(5)?.is_some(),
                    rated_count: row.get::<_, Option<i64>>(6)?.unwrap_or(0),
                    correct_count: row.get::<_, Option<i64>>(7)?.unwrap_or(0),
                })
            },
        )
        .optional()
        .map_err(CommandError::database)
}

pub(crate) fn refresh_session_counters(
    connection: &Connection,
    session_id: &str,
    now: i64,
) -> Result<(), CommandError> {
    let changed = connection
        .execute(
            "UPDATE practice_sessions
             SET viewed_count = (
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
                 ),
                 completed_at = CASE
                     WHEN (SELECT COUNT(*) FROM practice_session_cards
                           WHERE session_id = ?1
                             AND (rated_at IS NOT NULL OR dismissed_at IS NOT NULL)) >= card_count
                     THEN COALESCE(completed_at, ?2)
                     ELSE completed_at
                 END
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
    if input.schedule_revision < 1 {
        return Err(CommandError::new(
            "invalid_schedule_revision",
            "The interval preview is invalid. Reveal the answer again.",
        ));
    }
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

    #[test]
    fn review_input_requires_a_preview_revision() {
        let input = SubmitReviewInput {
            session_id: "a".repeat(32),
            card_id: "b".repeat(32),
            submission_id: "c".repeat(32),
            outcome: "wrong".to_string(),
            response_time_ms: 100,
            schedule_revision: 0,
        };
        assert_eq!(
            validate_review_input(&input)
                .expect_err("revision should fail")
                .code,
            "invalid_schedule_revision"
        );
    }
}
