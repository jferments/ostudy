//! Phase 2 practice workflow: direct 0-3 ratings, daily limits, interval
//! previews, related-card burying, struggling-card detection, and undo.
//!
//! Durable review events continue to use the Phase 1 FSRS-compatible 1-4
//! rating column. The public score is intentionally the simpler 0-3 scale:
//! 0 Wrong, 1 Hard, 2 OK, 3 Easy. Every accepted review stores a complete
//! before/after schedule snapshot so undo can restore state exactly while the
//! immutable review ledger is reversed by an append-only marker.

use std::{
    collections::BTreeSet,
    hash::{Hash, Hasher},
};

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    database::Database,
    error::CommandError,
    paths::AppPaths,
    phase2::render_card_from_connection,
    scheduler::{
        apply_review, load_schedule, update_schedule, ReviewOutcome, ScheduleRecord,
        ScheduleTransition, SCHEDULER_NAME, SCHEDULER_VERSION,
    },
    settings::load_settings,
    util::{bool_to_integer, generate_id, unix_time_millis, validate_entity_id},
};

const MAX_SESSION_DECKS: usize = 100;
const MAX_RESPONSE_TIME_MS: i64 = 86_400_000;
const MAX_DAY_SPAN_MS: i64 = 172_800_000;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartStudyInput {
    pub deck_ids: Vec<String>,
    pub shuffle: bool,
    pub day_start_ms: i64,
    pub next_day_start_ms: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudyCardDto {
    pub card: crate::cards::CardDto,
    pub deck_name: String,
    pub due_at: i64,
    pub is_new: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudySessionDto {
    pub id: String,
    pub cards: Vec<StudyCardDto>,
    pub shuffle: bool,
    pub started_at: i64,
    pub new_count: usize,
    pub review_count: usize,
    pub new_remaining_today: i64,
    pub reviews_remaining_today: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntervalPreviewInput {
    pub card_id: String,
    pub reviewed_at: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntervalPreviewDto {
    pub score: i64,
    pub label: String,
    pub due_at: i64,
    pub interval_ms: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitStudyReviewInput {
    pub session_id: String,
    pub card_id: String,
    pub submission_id: String,
    pub score: i64,
    pub response_time_ms: i64,
    pub next_day_start_ms: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudyReviewResultDto {
    pub card_id: String,
    pub score: i64,
    pub label: String,
    pub due_at: i64,
    pub interval_ms: i64,
    pub completed: bool,
    pub rated_count: i64,
    pub correct_count: i64,
    pub struggling: bool,
    pub auto_suspended: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UndoStudyReviewInput {
    pub session_id: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UndoStudyReviewResultDto {
    pub card_id: String,
    pub position: i64,
    pub rated_count: i64,
    pub correct_count: i64,
}

#[derive(Clone)]
struct PendingCard {
    deck_name: String,
    note_id: String,
    card_id: String,
    due_at: i64,
    state: i64,
}

#[tauri::command]
pub fn start_study_session(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: StartStudyInput,
) -> Result<StudySessionDto, CommandError> {
    let deck_ids = validate_deck_ids(input.deck_ids)?;
    validate_day_window(input.day_start_ms, input.next_day_start_ms)?;
    database.with_connection(|connection| {
        start_study_in_connection(
            connection,
            &paths,
            deck_ids,
            input.shuffle,
            input.day_start_ms,
        )
    })
}

#[tauri::command]
pub fn preview_review_intervals(
    database: State<'_, Database>,
    input: IntervalPreviewInput,
) -> Result<Vec<IntervalPreviewDto>, CommandError> {
    validate_entity_id(&input.card_id, "card")?;
    validate_timestamp(input.reviewed_at, "review time")?;
    database.with_connection(|connection| {
        preview_intervals_in_connection(connection, &input.card_id, input.reviewed_at)
    })
}

#[tauri::command]
pub fn submit_study_review(
    database: State<'_, Database>,
    input: SubmitStudyReviewInput,
) -> Result<StudyReviewResultDto, CommandError> {
    validate_submit_input(&input)?;
    database.with_connection(|connection| submit_study_in_connection(connection, input))
}

#[tauri::command]
pub fn undo_last_study_review(
    database: State<'_, Database>,
    input: UndoStudyReviewInput,
) -> Result<UndoStudyReviewResultDto, CommandError> {
    validate_entity_id(&input.session_id, "practice session")?;
    database.with_connection(|connection| undo_last_review_in_connection(connection, &input.session_id))
}

fn start_study_in_connection(
    connection: &mut Connection,
    paths: &AppPaths,
    deck_ids: Vec<String>,
    shuffle: bool,
    day_start_ms: i64,
) -> Result<StudySessionDto, CommandError> {
    let now = unix_time_millis()?;
    let settings = load_settings(connection)?;
    let (new_done, reviews_done): (i64, i64) = connection
        .query_row(
            "SELECT\n\
                 COALESCE(SUM(CASE WHEN r.state_before = 0 THEN 1 ELSE 0 END), 0),\n\
                 COALESCE(SUM(CASE WHEN r.state_before <> 0 THEN 1 ELSE 0 END), 0)\n\
             FROM review_events r\n\
             LEFT JOIN review_event_reversals rr ON rr.review_event_id = r.id\n\
             WHERE rr.review_event_id IS NULL AND r.reviewed_at >= ?1",
            [day_start_ms],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(CommandError::database)?;
    let new_remaining = (settings.new_cards_per_day - new_done).max(0);
    let review_remaining = (settings.reviews_per_day - reviews_done).max(0);

    let mut pending_new = Vec::new();
    let mut pending_review = Vec::new();
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
        let mut statement = connection
            .prepare(
                "SELECT c.id, c.note_id, s.due_at, s.state\n\
                 FROM cards c\n\
                 JOIN notes n ON n.id = c.note_id\n\
                 JOIN card_schedules s ON s.card_id = c.id\n\
                 WHERE c.deck_id = ?1\n\
                   AND c.deleted_at IS NULL\n\
                   AND c.suspended_at IS NULL\n\
                   AND (c.buried_until IS NULL OR c.buried_until <= ?2)\n\
                   AND n.deleted_at IS NULL\n\
                   AND s.due_at <= ?2\n\
                 ORDER BY s.due_at ASC, c.created_at ASC",
            )
            .map_err(CommandError::database)?;
        let rows = statement
            .query_map(params![deck_id, now], |row| {
                Ok(PendingCard {
                    deck_name: deck_name.clone(),
                    card_id: row.get(0)?,
                    note_id: row.get(1)?,
                    due_at: row.get(2)?,
                    state: row.get(3)?,
                })
            })
            .map_err(CommandError::database)?;
        for row in rows {
            let card = row.map_err(CommandError::database)?;
            if card.state == 0 {
                pending_new.push(card);
            } else {
                pending_review.push(card);
            }
        }
    }

    pending_review.sort_by(|left, right| {
        left.due_at
            .cmp(&right.due_at)
            .then_with(|| left.card_id.cmp(&right.card_id))
    });
    pending_new.sort_by(|left, right| {
        left.due_at
            .cmp(&right.due_at)
            .then_with(|| left.card_id.cmp(&right.card_id))
    });
    if settings.bury_related_cards {
        retain_one_card_per_note(&mut pending_review, &mut pending_new);
    }

    pending_new.truncate(usize::try_from(new_remaining).unwrap_or(usize::MAX));
    pending_review.truncate(usize::try_from(review_remaining).unwrap_or(usize::MAX));
    let new_count = pending_new.len();
    let review_count = pending_review.len();
    let mut pending = pending_review;
    pending.extend(pending_new);
    if pending.is_empty() {
        let any_due: i64 = connection
            .query_row(
                "SELECT EXISTS(\n\
                     SELECT 1 FROM cards c\n\
                     JOIN notes n ON n.id = c.note_id\n\
                     JOIN card_schedules s ON s.card_id = c.id\n\
                     WHERE c.deck_id IN (SELECT value FROM json_each(?1))\n\
                       AND c.deleted_at IS NULL AND c.suspended_at IS NULL\n\
                       AND (c.buried_until IS NULL OR c.buried_until <= ?2)\n\
                       AND n.deleted_at IS NULL AND s.due_at <= ?2\n\
                 )",
                params![
                    serde_json::to_string(&deck_ids).map_err(CommandError::serialization)?,
                    now,
                ],
                |row| row.get(0),
            )
            .map_err(CommandError::database)?;
        return Err(if any_due != 0 {
            CommandError::new(
                "daily_limit_reached",
                "You reached today’s new-card and review limits. Adjust them in Settings or continue tomorrow.",
            )
        } else {
            CommandError::new(
                "no_cards_due",
                "No cards are due in the selected decks right now.",
            )
        });
    }

    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    let session_id = generate_id(&transaction)?;
    if shuffle {
        pending.sort_by_key(|card| shuffle_key(&session_id, &card.card_id));
    }
    let deck_ids_json = serde_json::to_string(&deck_ids).map_err(CommandError::serialization)?;
    transaction
        .execute(
            "INSERT INTO practice_sessions (\n\
                 id, selected_deck_ids_json, shuffle, card_count, started_at\n\
             ) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                session_id,
                deck_ids_json,
                bool_to_integer(shuffle),
                i64::try_from(pending.len()).unwrap_or(i64::MAX),
                now,
            ],
        )
        .map_err(CommandError::database)?;
    for (position, card) in pending.iter().enumerate() {
        transaction
            .execute(
                "INSERT INTO practice_session_cards(session_id, card_id, position)\n\
                 VALUES (?1, ?2, ?3)",
                params![
                    session_id,
                    card.card_id,
                    i64::try_from(position).unwrap_or(i64::MAX),
                ],
            )
            .map_err(CommandError::database)?;
    }
    transaction.commit().map_err(CommandError::database)?;

    let mut cards = Vec::with_capacity(pending.len());
    for item in pending {
        let rendered = render_card_from_connection(connection, paths, &item.card_id)?;
        cards.push(StudyCardDto {
            card: rendered,
            deck_name: item.deck_name,
            due_at: item.due_at,
            is_new: item.state == 0,
        });
    }
    Ok(StudySessionDto {
        id: session_id,
        cards,
        shuffle,
        started_at: now,
        new_count,
        review_count,
        new_remaining_today: (new_remaining - i64::try_from(new_count).unwrap_or(i64::MAX)).max(0),
        reviews_remaining_today: (review_remaining
            - i64::try_from(review_count).unwrap_or(i64::MAX))
        .max(0),
    })
}

/// Prevent a sibling that was copied into the immutable session snapshot from
/// appearing after another card generated by the same note is reviewed. Reviews
/// take priority over new siblings, and the caller sorts each queue by due time.
fn retain_one_card_per_note(
    pending_review: &mut Vec<PendingCard>,
    pending_new: &mut Vec<PendingCard>,
) {
    let mut queued_notes = BTreeSet::new();
    pending_review.retain(|card| queued_notes.insert(card.note_id.clone()));
    pending_new.retain(|card| queued_notes.insert(card.note_id.clone()));
}

fn preview_intervals_in_connection(
    connection: &mut Connection,
    card_id: &str,
    reviewed_at: i64,
) -> Result<Vec<IntervalPreviewDto>, CommandError> {
    let current = load_schedule(connection, card_id)?;
    let desired_retention = load_settings(connection)?.desired_retention;
    (0..=3)
        .map(|score| {
            let outcome = outcome_for_score(score)?;
            let transition = apply_review(current.clone(), outcome, reviewed_at, desired_retention)?;
            Ok(IntervalPreviewDto {
                score,
                label: score_label(score).to_string(),
                due_at: transition.after.due_at,
                interval_ms: transition.after.due_at.saturating_sub(reviewed_at),
            })
        })
        .collect()
}

fn submit_study_in_connection(
    connection: &mut Connection,
    input: SubmitStudyReviewInput,
) -> Result<StudyReviewResultDto, CommandError> {
    let outcome = outcome_for_score(input.score)?;
    let reviewed_at = unix_time_millis()?;
    if input.next_day_start_ms <= reviewed_at
        || input.next_day_start_ms.saturating_sub(reviewed_at) > MAX_DAY_SPAN_MS
    {
        return Err(CommandError::new(
            "invalid_day_boundary",
            "The next local-day boundary is invalid.",
        ));
    }
    let settings = load_settings(connection)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;

    if let Some(existing) = result_for_submission(&transaction, &input.submission_id)? {
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

    let before = load_schedule(&transaction, &input.card_id)?;
    let transition = apply_review(
        before,
        outcome,
        reviewed_at,
        settings.desired_retention,
    )?;
    let event_id = insert_review_event(&transaction, &input, &transition, reviewed_at)?;
    insert_schedule_snapshot(&transaction, &event_id, &transition)?;
    snapshot_note_card_states(&transaction, &event_id, &input.card_id)?;
    update_schedule(&transaction, &transition.after)?;

    let changed = transaction
        .execute(
            "UPDATE practice_session_cards\n\
             SET rated_at = ?1, rating = ?2, response_time_ms = ?3\n\
             WHERE session_id = ?4 AND card_id = ?5 AND rated_at IS NULL",
            params![
                reviewed_at,
                input.score + 1,
                input.response_time_ms,
                input.session_id,
                input.card_id,
            ],
        )
        .map_err(CommandError::database)?;
    if changed != 1 {
        return Err(CommandError::new(
            "practice_card_already_rated",
            "This card has already been rated in the current session.",
        ));
    }

    if settings.bury_related_cards {
        bury_sibling_cards(
            &transaction,
            &event_id,
            &input.card_id,
            input.next_day_start_ms,
            reviewed_at,
        )?;
    }
    let (struggling, auto_suspended) = update_struggling_state(
        &transaction,
        &input.card_id,
        settings.struggling_card_threshold,
        settings.auto_suspend_struggling_cards,
        reviewed_at,
    )?;
    transaction
        .execute(
            "UPDATE review_card_state_snapshots\n\
             SET struggling_after = ?1, auto_suspended = ?2\n\
             WHERE review_event_id = ?3 AND card_id = ?4",
            params![
                bool_to_integer(struggling),
                bool_to_integer(auto_suspended),
                event_id,
                input.card_id,
            ],
        )
        .map_err(CommandError::database)?;
    refresh_session_counters(&transaction, &input.session_id, reviewed_at)?;
    let (rated_count, correct_count, completed_at): (i64, i64, Option<i64>) = transaction
        .query_row(
            "SELECT rated_count, correct_count, completed_at\n\
             FROM practice_sessions WHERE id = ?1",
            [input.session_id.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(CommandError::database)?;
    let result = StudyReviewResultDto {
        card_id: input.card_id,
        score: input.score,
        label: score_label(input.score).to_string(),
        due_at: transition.after.due_at,
        interval_ms: transition.after.due_at.saturating_sub(reviewed_at),
        completed: completed_at.is_some(),
        rated_count,
        correct_count,
        struggling,
        auto_suspended,
    };
    transaction.commit().map_err(CommandError::database)?;
    Ok(result)
}

fn insert_review_event(
    connection: &Connection,
    input: &SubmitStudyReviewInput,
    transition: &ScheduleTransition,
    created_at: i64,
) -> Result<String, CommandError> {
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
                input.score + 1,
                bool_to_integer(input.score > 0),
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
    Ok(event_id)
}

fn insert_schedule_snapshot(
    connection: &Connection,
    event_id: &str,
    transition: &ScheduleTransition,
) -> Result<(), CommandError> {
    let before = &transition.before;
    let after = &transition.after;
    connection
        .execute(
            "INSERT INTO review_schedule_snapshots (\n\
                 review_event_id, state_before, due_before, stability_before, difficulty_before,\n\
                 elapsed_days_before, scheduled_days_before, reps_before, lapses_before,\n\
                 last_review_at_before, scheduler_name_before, scheduler_version_before,\n\
                 state_after, due_after, stability_after, difficulty_after,\n\
                 elapsed_days_after, scheduled_days_after, reps_after, lapses_after,\n\
                 last_review_at_after, scheduler_name_after, scheduler_version_after\n\
             ) VALUES (\n\
                 ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12,\n\
                 ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23\n\
             )",
            params![
                event_id,
                before.state,
                before.due_at,
                before.stability,
                before.difficulty,
                before.elapsed_days,
                before.scheduled_days,
                before.reps,
                before.lapses,
                before.last_review_at,
                before.scheduler_name,
                before.scheduler_version,
                after.state,
                after.due_at,
                after.stability,
                after.difficulty,
                after.elapsed_days,
                after.scheduled_days,
                after.reps,
                after.lapses,
                after.last_review_at,
                after.scheduler_name,
                after.scheduler_version,
            ],
        )
        .map_err(CommandError::database)?;
    Ok(())
}

fn snapshot_note_card_states(
    connection: &Connection,
    event_id: &str,
    reviewed_card_id: &str,
) -> Result<(), CommandError> {
    let note_id: String = connection
        .query_row(
            "SELECT note_id FROM cards WHERE id = ?1 AND deleted_at IS NULL",
            [reviewed_card_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(CommandError::database)?
        .ok_or_else(|| CommandError::new("card_not_found", "That flashcard no longer exists."))?;
    connection
        .execute(
            "INSERT INTO review_card_state_snapshots (\n\
                 review_event_id, card_id, suspended_at_before,\n\
                 struggling_since_before, struggling_reason_before\n\
             )\n\
             SELECT ?1, id, suspended_at, struggling_since, struggling_reason\n\
             FROM cards\n\
             WHERE note_id = ?2 AND deleted_at IS NULL",
            params![event_id, note_id],
        )
        .map_err(CommandError::database)?;
    Ok(())
}

fn result_for_submission(
    connection: &Connection,
    submission_id: &str,
) -> Result<Option<StudyReviewResultDto>, CommandError> {
    connection
        .query_row(
            "SELECT r.card_id, r.rating - 1, r.due_after, r.reviewed_at,\n\
                    ps.completed_at, ps.rated_count, ps.correct_count,\n\
                    COALESCE(cs.struggling_after, 0), COALESCE(cs.auto_suspended, 0)\n\
             FROM review_events r\n\
             LEFT JOIN review_event_reversals rr ON rr.review_event_id = r.id\n\
             LEFT JOIN practice_sessions ps ON ps.id = r.session_id\n\
             LEFT JOIN review_card_state_snapshots cs\n\
               ON cs.review_event_id = r.id AND cs.card_id = r.card_id\n\
             WHERE r.submission_id = ?1 AND rr.review_event_id IS NULL",
            [submission_id],
            |row| {
                let score: i64 = row.get(1)?;
                let reviewed_at: i64 = row.get(3)?;
                let due_at: i64 = row.get(2)?;
                Ok(StudyReviewResultDto {
                    card_id: row.get(0)?,
                    score,
                    label: score_label(score).to_string(),
                    due_at,
                    interval_ms: due_at.saturating_sub(reviewed_at),
                    completed: row.get::<_, Option<i64>>(4)?.is_some(),
                    rated_count: row.get::<_, Option<i64>>(5)?.unwrap_or(0),
                    correct_count: row.get::<_, Option<i64>>(6)?.unwrap_or(0),
                    struggling: row.get::<_, i64>(7)? != 0,
                    auto_suspended: row.get::<_, i64>(8)? != 0,
                })
            },
        )
        .optional()
        .map_err(CommandError::database)
}

fn bury_sibling_cards(
    connection: &Connection,
    event_id: &str,
    card_id: &str,
    until: i64,
    now: i64,
) -> Result<(), CommandError> {
    let note_id: String = connection
        .query_row("SELECT note_id FROM cards WHERE id = ?1", [card_id], |row| row.get(0))
        .map_err(CommandError::database)?;
    let mut statement = connection
        .prepare(
            "SELECT id, buried_until FROM cards\n\
             WHERE note_id = ?1 AND id <> ?2 AND deleted_at IS NULL",
        )
        .map_err(CommandError::database)?;
    let siblings = statement
        .query_map(params![note_id, card_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<i64>>(1)?))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    drop(statement);
    for (sibling_id, previous) in siblings {
        connection
            .execute(
                "INSERT INTO review_sibling_buries(review_event_id, card_id, previous_buried_until)\n\
                 VALUES (?1, ?2, ?3)",
                params![event_id, sibling_id, previous],
            )
            .map_err(CommandError::database)?;
        let next = previous.map_or(until, |value| value.max(until));
        connection
            .execute(
                "UPDATE cards SET buried_until = ?1, updated_at = ?2 WHERE id = ?3",
                params![next, now, sibling_id],
            )
            .map_err(CommandError::database)?;
    }
    Ok(())
}

fn update_struggling_state(
    connection: &Connection,
    card_id: &str,
    threshold: i64,
    auto_suspend: bool,
    now: i64,
) -> Result<(bool, bool), CommandError> {
    let wrong_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM review_events r\n\
             LEFT JOIN review_event_reversals rr ON rr.review_event_id = r.id\n\
             WHERE r.card_id = ?1 AND r.rating = 1 AND rr.review_event_id IS NULL",
            [card_id],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    let note_id: String = connection
        .query_row("SELECT note_id FROM cards WHERE id = ?1", [card_id], |row| row.get(0))
        .map_err(CommandError::database)?;
    let struggling = wrong_count >= threshold;
    let unsuspended_before: i64 = if struggling && auto_suspend {
        connection
            .query_row(
                "SELECT COUNT(*) FROM cards\n\
                 WHERE note_id = ?1 AND deleted_at IS NULL AND suspended_at IS NULL",
                [note_id.as_str()],
                |row| row.get(0),
            )
            .map_err(CommandError::database)?
    } else {
        0
    };
    if struggling {
        let reason = format!("Missed {wrong_count} times");
        connection
            .execute(
                "UPDATE cards SET struggling_since = COALESCE(struggling_since, ?1),\n\
                     struggling_reason = ?2, suspended_at = CASE WHEN ?3 = 1 THEN COALESCE(suspended_at, ?1) ELSE suspended_at END,\n\
                     updated_at = ?1\n\
                 WHERE note_id = ?4 AND deleted_at IS NULL",
                params![now, reason, bool_to_integer(auto_suspend), note_id],
            )
            .map_err(CommandError::database)?;
    } else {
        connection
            .execute(
                "UPDATE cards SET struggling_since = NULL, struggling_reason = NULL, updated_at = ?1\n\
                 WHERE note_id = ?2 AND deleted_at IS NULL",
                params![now, note_id],
            )
            .map_err(CommandError::database)?;
    }
    Ok((
        struggling,
        struggling && auto_suspend && unsuspended_before > 0,
    ))
}

fn restore_note_card_states(
    connection: &Connection,
    event_id: &str,
    now: i64,
) -> Result<(), CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT card_id, suspended_at_before, struggling_since_before,\n\
                    struggling_reason_before\n\
             FROM review_card_state_snapshots\n\
             WHERE review_event_id = ?1",
        )
        .map_err(CommandError::database)?;
    let snapshots = statement
        .query_map([event_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<i64>>(1)?,
                row.get::<_, Option<i64>>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    drop(statement);
    if snapshots.is_empty() {
        return Err(CommandError::new(
            "review_cannot_be_undone",
            "This review does not contain the exact card-state snapshot required for undo.",
        ));
    }
    for (card_id, suspended_at, struggling_since, struggling_reason) in snapshots {
        connection
            .execute(
                "UPDATE cards\n\
                 SET suspended_at = ?1, struggling_since = ?2, struggling_reason = ?3,\n\
                     updated_at = ?4\n\
                 WHERE id = ?5",
                params![suspended_at, struggling_since, struggling_reason, now, card_id],
            )
            .map_err(CommandError::database)?;
    }
    Ok(())
}

fn undo_last_review_in_connection(
    connection: &mut Connection,
    session_id: &str,
) -> Result<UndoStudyReviewResultDto, CommandError> {
    let now = unix_time_millis()?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    let latest: Option<(String, String, i64, i64)> = transaction
        .query_row(
            "SELECT r.id, r.card_id, psc.position, r.reviewed_at\n\
             FROM review_events r\n\
             JOIN practice_session_cards psc\n\
               ON psc.session_id = r.session_id AND psc.card_id = r.card_id\n\
             LEFT JOIN review_event_reversals rr ON rr.review_event_id = r.id\n\
             WHERE r.session_id = ?1 AND rr.review_event_id IS NULL\n\
             ORDER BY r.reviewed_at DESC, r.created_at DESC, r.id DESC\n\
             LIMIT 1",
            [session_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()
        .map_err(CommandError::database)?;
    let Some((event_id, card_id, position, reviewed_at)) = latest else {
        return Err(CommandError::new(
            "nothing_to_undo",
            "There is no completed review to undo in this session.",
        ));
    };
    let later_review: i64 = transaction
        .query_row(
            "SELECT EXISTS(\n\
                 SELECT 1 FROM review_events r\n\
                 LEFT JOIN review_event_reversals rr ON rr.review_event_id = r.id\n\
                 WHERE r.card_id = ?1 AND r.reviewed_at > ?2 AND rr.review_event_id IS NULL\n\
             )",
            params![card_id, reviewed_at],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    if later_review != 0 {
        return Err(CommandError::new(
            "review_cannot_be_undone",
            "This review cannot be undone because the card has been reviewed again.",
        ));
    }

    let before = load_snapshot_before(&transaction, &event_id, &card_id, now)?;
    update_schedule(&transaction, &before)?;
    let reversal_id = generate_id(&transaction)?;
    transaction
        .execute(
            "INSERT INTO review_event_reversals(id, review_event_id, reversed_at, reason)\n\
             VALUES (?1, ?2, ?3, 'user_undo')",
            params![reversal_id, event_id, now],
        )
        .map_err(CommandError::database)?;
    transaction
        .execute(
            "UPDATE practice_session_cards\n\
             SET rated_at = NULL, rating = NULL, response_time_ms = NULL\n\
             WHERE session_id = ?1 AND card_id = ?2",
            params![session_id, card_id],
        )
        .map_err(CommandError::database)?;

    let mut bury_statement = transaction
        .prepare(
            "SELECT card_id, previous_buried_until FROM review_sibling_buries\n\
             WHERE review_event_id = ?1",
        )
        .map_err(CommandError::database)?;
    let buries = bury_statement
        .query_map([event_id.as_str()], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<i64>>(1)?))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    drop(bury_statement);
    for (sibling_id, previous) in buries {
        transaction
            .execute(
                "UPDATE cards SET buried_until = ?1, updated_at = ?2 WHERE id = ?3",
                params![previous, now, sibling_id],
            )
            .map_err(CommandError::database)?;
    }
    restore_note_card_states(&transaction, &event_id, now)?;
    refresh_session_counters(&transaction, session_id, now)?;
    let (rated_count, correct_count): (i64, i64) = transaction
        .query_row(
            "SELECT rated_count, correct_count FROM practice_sessions WHERE id = ?1",
            [session_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(CommandError::database)?;
    transaction.commit().map_err(CommandError::database)?;
    Ok(UndoStudyReviewResultDto {
        card_id,
        position,
        rated_count,
        correct_count,
    })
}

fn load_snapshot_before(
    connection: &Connection,
    event_id: &str,
    card_id: &str,
    updated_at: i64,
) -> Result<ScheduleRecord, CommandError> {
    connection
        .query_row(
            "SELECT state_before, due_before, stability_before, difficulty_before,\n\
                    elapsed_days_before, scheduled_days_before, reps_before, lapses_before,\n\
                    last_review_at_before, scheduler_name_before, scheduler_version_before\n\
             FROM review_schedule_snapshots WHERE review_event_id = ?1",
            [event_id],
            |row| {
                Ok(ScheduleRecord {
                    card_id: card_id.to_string(),
                    state: row.get(0)?,
                    due_at: row.get(1)?,
                    stability: row.get(2)?,
                    difficulty: row.get(3)?,
                    elapsed_days: row.get(4)?,
                    scheduled_days: row.get(5)?,
                    reps: row.get(6)?,
                    lapses: row.get(7)?,
                    last_review_at: row.get(8)?,
                    scheduler_name: row.get(9)?,
                    scheduler_version: row.get(10)?,
                    updated_at,
                })
            },
        )
        .optional()
        .map_err(CommandError::database)?
        .ok_or_else(|| {
            CommandError::new(
                "review_cannot_be_undone",
                "This older review does not contain the exact schedule snapshot required for undo.",
            )
        })
}

fn refresh_session_counters(
    connection: &Connection,
    session_id: &str,
    now: i64,
) -> Result<(), CommandError> {
    let changed = connection
        .execute(
            "UPDATE practice_sessions SET\n\
                 viewed_count = (SELECT COUNT(*) FROM practice_session_cards\n\
                                 WHERE session_id = ?1 AND viewed_at IS NOT NULL),\n\
                 revealed_count = (SELECT COUNT(*) FROM practice_session_cards\n\
                                   WHERE session_id = ?1 AND revealed_at IS NOT NULL),\n\
                 rated_count = (SELECT COUNT(*) FROM practice_session_cards\n\
                                WHERE session_id = ?1 AND rated_at IS NOT NULL),\n\
                 correct_count = (SELECT COUNT(*) FROM practice_session_cards\n\
                                  WHERE session_id = ?1 AND rating BETWEEN 2 AND 4),\n\
                 completed_at = CASE\n\
                     WHEN (SELECT COUNT(*) FROM practice_session_cards\n\
                           WHERE session_id = ?1 AND rated_at IS NOT NULL) >= card_count\n\
                     THEN COALESCE(completed_at, ?2)\n\
                     ELSE NULL\n\
                 END\n\
             WHERE id = ?1",
            params![session_id, now],
        )
        .map_err(CommandError::database)?;
    if changed != 1 {
        return Err(CommandError::new(
            "practice_session_not_found",
            "That practice session no longer exists.",
        ));
    }
    Ok(())
}

fn outcome_for_score(score: i64) -> Result<ReviewOutcome, CommandError> {
    match score {
        0 => Ok(ReviewOutcome::Wrong),
        1 => Ok(ReviewOutcome::NotConfident),
        2 => Ok(ReviewOutcome::SomewhatConfident),
        3 => Ok(ReviewOutcome::VeryConfident),
        _ => Err(CommandError::new(
            "invalid_review_score",
            "Rate the answer from 0 (Wrong) through 3 (Easy).",
        )),
    }
}

const fn score_label(score: i64) -> &'static str {
    match score {
        0 => "Wrong",
        1 => "Hard",
        2 => "OK",
        3 => "Easy",
        _ => "Unknown",
    }
}

fn validate_submit_input(input: &SubmitStudyReviewInput) -> Result<(), CommandError> {
    validate_entity_id(&input.session_id, "practice session")?;
    validate_entity_id(&input.card_id, "card")?;
    validate_entity_id(&input.submission_id, "review submission")?;
    outcome_for_score(input.score)?;
    if !(0..=MAX_RESPONSE_TIME_MS).contains(&input.response_time_ms) {
        return Err(CommandError::new(
            "invalid_response_time",
            "The recorded response time is outside the supported range.",
        ));
    }
    validate_timestamp(input.next_day_start_ms, "next day boundary")
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

fn validate_day_window(day_start: i64, next_day_start: i64) -> Result<(), CommandError> {
    validate_timestamp(day_start, "day start")?;
    validate_timestamp(next_day_start, "next day start")?;
    if next_day_start <= day_start || next_day_start.saturating_sub(day_start) > MAX_DAY_SPAN_MS {
        return Err(CommandError::new(
            "invalid_day_boundary",
            "The local-day boundaries are invalid.",
        ));
    }
    Ok(())
}

fn validate_timestamp(value: i64, label: &str) -> Result<(), CommandError> {
    const MAX_TIMESTAMP: i64 = 253_402_300_799_999;
    if !(0..=MAX_TIMESTAMP).contains(&value) {
        return Err(CommandError::new(
            "invalid_timestamp",
            format!("The selected {label} is outside the supported date range."),
        ));
    }
    Ok(())
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

    #[test]
    fn public_scores_map_to_fsrs_without_two_stage_questions() {
        assert_eq!(outcome_for_score(0).unwrap(), ReviewOutcome::Wrong);
        assert_eq!(outcome_for_score(1).unwrap(), ReviewOutcome::NotConfident);
        assert_eq!(outcome_for_score(2).unwrap(), ReviewOutcome::SomewhatConfident);
        assert_eq!(outcome_for_score(3).unwrap(), ReviewOutcome::VeryConfident);
        assert!(outcome_for_score(4).is_err());
    }

    #[test]
    fn labels_match_the_everyday_language() {
        assert_eq!(score_label(0), "Wrong");
        assert_eq!(score_label(1), "Hard");
        assert_eq!(score_label(2), "OK");
        assert_eq!(score_label(3), "Easy");
    }

    #[test]
    fn related_cards_are_not_queued_twice_in_one_session() {
        let card = |card_id: &str, note_id: &str, state: i64| PendingCard {
            deck_name: "Deck".to_string(),
            card_id: card_id.to_string(),
            note_id: note_id.to_string(),
            due_at: 0,
            state,
        };
        let mut reviews = vec![card("r1", "note-a", 2), card("r2", "note-a", 2)];
        let mut new = vec![card("n1", "note-a", 0), card("n2", "note-b", 0)];

        retain_one_card_per_note(&mut reviews, &mut new);

        assert_eq!(reviews.len(), 1);
        assert_eq!(new.len(), 1);
        assert_eq!(new[0].note_id, "note-b");
    }

    #[test]
    fn local_day_window_allows_dst_length_days() {
        let start = 1_700_000_000_000;
        assert!(validate_day_window(start, start + 23 * 3_600_000).is_ok());
        assert!(validate_day_window(start, start + 25 * 3_600_000).is_ok());
        assert!(validate_day_window(start, start).is_err());
    }

    #[test]
    fn direct_review_and_undo_restore_the_exact_schedule_and_ledger_view() {
        let database = Database::open_in_memory().expect("database should open");
        let root = temporary_test_directory("review-undo");
        let paths = AppPaths::new(root.clone()).expect("test paths should initialize");

        database
            .with_connection(|connection| {
                let deck = crate::decks::create_deck_in_connection(
                    connection,
                    crate::decks::CreateDeckInput {
                        name: "Review test".to_string(),
                        description: None,
                    },
                )?;
                let card = crate::cards::create_card_in_connection(
                    connection,
                    &paths,
                    &deck.id,
                    crate::cards::validate_content(
                        plain_document("Question"),
                        plain_document("Answer"),
                        "Question".to_string(),
                        "Answer".to_string(),
                    )?,
                )?;
                let before = load_schedule(connection, &card.id)?;
                let session = start_study_in_connection(
                    connection,
                    &paths,
                    vec![deck.id.clone()],
                    false,
                    0,
                )?;
                assert_eq!(session.cards.len(), 1);
                assert_eq!(session.new_count, 1);
                let revealed_at = unix_time_millis()?;
                connection
                    .execute(
                        "UPDATE practice_session_cards SET viewed_at = ?1, revealed_at = ?1
                         WHERE session_id = ?2 AND card_id = ?3",
                        params![revealed_at, session.id.as_str(), card.id.as_str()],
                    )
                    .map_err(CommandError::database)?;

                let reviewed = submit_study_in_connection(
                    connection,
                    SubmitStudyReviewInput {
                        session_id: session.id.clone(),
                        card_id: card.id.clone(),
                        submission_id: "40000000000000000000000000000001".to_string(),
                        score: 0,
                        response_time_ms: 1200,
                        next_day_start_ms: revealed_at + 86_400_000,
                    },
                )?;
                assert_eq!(reviewed.score, 0);
                assert_eq!(reviewed.label, "Wrong");
                let after = load_schedule(connection, &card.id)?;
                assert_ne!(after.reps, before.reps);
                let snapshots: i64 = connection
                    .query_row(
                        "SELECT COUNT(*) FROM review_schedule_snapshots",
                        [],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(snapshots, 1);

                let undone = undo_last_review_in_connection(connection, &session.id)?;
                assert_eq!(undone.card_id, card.id);
                let restored = load_schedule(connection, &card.id)?;
                assert_eq!(restored.state, before.state);
                assert_eq!(restored.due_at, before.due_at);
                assert_eq!(restored.stability, before.stability);
                assert_eq!(restored.difficulty, before.difficulty);
                assert_eq!(restored.elapsed_days, before.elapsed_days);
                assert_eq!(restored.scheduled_days, before.scheduled_days);
                assert_eq!(restored.reps, before.reps);
                assert_eq!(restored.lapses, before.lapses);
                assert_eq!(restored.last_review_at, before.last_review_at);

                let visible_reviews: i64 = connection
                    .query_row(
                        "SELECT COUNT(*) FROM review_events r
                         LEFT JOIN review_event_reversals rr ON rr.review_event_id = r.id
                         WHERE rr.review_event_id IS NULL",
                        [],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                let immutable_reviews: i64 = connection
                    .query_row("SELECT COUNT(*) FROM review_events", [], |row| row.get(0))
                    .map_err(CommandError::database)?;
                assert_eq!(visible_reviews, 0);
                assert_eq!(immutable_reviews, 1);
                Ok(())
            })
            .expect("review and undo should succeed");

        let _ = std::fs::remove_dir_all(root);
    }

    fn plain_document(text: &str) -> serde_json::Value {
        serde_json::json!({
            "type": "doc",
            "content": [{
                "type": "paragraph",
                "content": [{"type": "text", "text": text}]
            }]
        })
    }

    fn temporary_test_directory(label: &str) -> std::path::PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "ostudy-study-{label}-{}-{nonce}",
            std::process::id()
        ))
    }
}
