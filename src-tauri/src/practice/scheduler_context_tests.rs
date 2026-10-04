//! Each review records what FSRS knew when it scheduled the card, plus the
//! visible study time the client measured, in the review's own transaction.

use rusqlite::{params, Connection};

use super::operation_tests::{create_card, create_session, review_input};
use super::*;
use crate::{
    database::Database,
    decks::{create_deck_in_connection, CreateDeckInput},
    scheduler::{fsrs_review_view, scheduler_parameters_sha256, ScheduleRecord},
    util::unix_time_millis,
};

const DAY: i64 = 86_400_000;

#[derive(Debug, PartialEq)]
struct StoredContext {
    previous_review_at: Option<i64>,
    elapsed_days: i64,
    retrievability: Option<f64>,
    reps_before: i64,
    lapses_before: i64,
    desired_retention: f64,
    parameters_sha256: String,
    active_time_ms: Option<i64>,
}

/// The review time and stored scheduler context for one submission.
fn stored(connection: &Connection, submission: char) -> Result<(i64, StoredContext), CommandError> {
    connection
        .query_row(
            "SELECT r.reviewed_at, c.previous_review_at, c.elapsed_days, c.retrievability,
                    c.reps_before, c.lapses_before, c.desired_retention,
                    c.scheduler_parameters_sha256, c.active_time_ms
             FROM review_events r
             JOIN review_event_scheduler_context c ON c.review_event_id = r.id
             WHERE r.submission_id = ?1",
            [submission.to_string().repeat(32)],
            |row| {
                Ok((
                    row.get(0)?,
                    StoredContext {
                        previous_review_at: row.get(1)?,
                        elapsed_days: row.get(2)?,
                        retrievability: row.get(3)?,
                        reps_before: row.get(4)?,
                        lapses_before: row.get(5)?,
                        desired_retention: row.get(6)?,
                        parameters_sha256: row.get(7)?,
                        active_time_ms: row.get(8)?,
                    },
                ))
            },
        )
        .map_err(CommandError::database)
}

fn count(connection: &Connection, sql: &str) -> Result<i64, CommandError> {
    connection
        .query_row(sql, [], |row| row.get(0))
        .map_err(CommandError::database)
}

fn deck(connection: &mut Connection) -> Result<String, CommandError> {
    Ok(create_deck_in_connection(
        connection,
        CreateDeckInput {
            name: "Scheduler context".to_string(),
            description: None,
        },
    )?
    .id)
}

/// Give a card a reviewed schedule, as an earlier review would have left it.
/// `elapsed_days` is deliberately stale: it describes that earlier review.
fn set_reviewed_schedule(
    connection: &Connection,
    card_id: &str,
    state: i64,
    stability: f64,
    last_review_at: i64,
    due_at: i64,
) -> Result<ScheduleRecord, CommandError> {
    connection
        .execute(
            "UPDATE card_schedules
             SET state = ?2, due_at = ?3, stability = ?4, difficulty = 5.5,
                 elapsed_days = 3, scheduled_days = 4, reps = 5, lapses = 1,
                 last_review_at = ?5, revision = 4
             WHERE card_id = ?1",
            params![card_id, state, due_at, stability, last_review_at],
        )
        .map_err(CommandError::database)?;
    crate::scheduler::load_schedule(connection, card_id)
}

#[test]
fn a_new_card_review_records_context_without_a_prediction() {
    let database = Database::open_in_memory().expect("database should open");
    database
        .with_connection(|connection| {
            connection
                .execute(
                    "UPDATE app_settings SET value_json = '0.85' WHERE key = 'desiredRetention'",
                    [],
                )
                .map_err(CommandError::database)?;
            let deck_id = deck(connection)?;
            let card_id = create_card(connection, &deck_id, "New", 0)?;
            let session_id = create_session(connection, std::slice::from_ref(&card_id))?;
            record_practice_event(connection, &session_id, &card_id, true)?;
            let mut input = review_input(&session_id, &card_id, 'a', "somewhat_confident");
            input.active_time_ms = Some(4_321);
            submit_review_in_connection(connection, input)?;

            let (_, context) = stored(connection, 'a')?;
            assert_eq!(
                context,
                StoredContext {
                    previous_review_at: None,
                    elapsed_days: 0,
                    retrievability: None,
                    reps_before: 0,
                    lapses_before: 0,
                    desired_retention: 0.85,
                    parameters_sha256: scheduler_parameters_sha256()?,
                    active_time_ms: Some(4_321),
                }
            );
            assert_eq!(
                count(
                    connection,
                    "SELECT COUNT(*) FROM review_event_scheduler_context"
                )?,
                1
            );
            Ok(())
        })
        .expect("a new card's review should record its scheduler context");
}

#[test]
fn a_due_review_card_records_the_prediction_and_elapsed_days_fsrs_used() {
    let database = Database::open_in_memory().expect("database should open");
    database
        .with_connection(|connection| {
            let deck_id = deck(connection)?;
            let card_id = create_card(connection, &deck_id, "Review", 0)?;
            let now = unix_time_millis()?;
            let last_review_at = now - 10 * DAY - 2 * 3_600_000;
            let before = set_reviewed_schedule(
                connection,
                &card_id,
                2,
                12.0,
                last_review_at,
                now - 3_600_000,
            )?;
            let session_id = create_session(connection, std::slice::from_ref(&card_id))?;
            record_practice_event(connection, &session_id, &card_id, true)?;
            let mut input = review_input(&session_id, &card_id, 'b', "somewhat_confident");
            input.schedule_revision = 4;
            submit_review_in_connection(connection, input)?;

            let (reviewed_at, context) = stored(connection, 'b')?;
            let (retrievability, elapsed_days) = fsrs_review_view(&before, reviewed_at);
            assert_eq!(elapsed_days, 10);
            assert!(retrievability > 0.0 && retrievability < 1.0);
            assert_eq!(
                context,
                StoredContext {
                    previous_review_at: Some(last_review_at),
                    elapsed_days,
                    retrievability: Some(retrievability),
                    reps_before: 5,
                    lapses_before: 1,
                    desired_retention: 0.9,
                    parameters_sha256: scheduler_parameters_sha256()?,
                    active_time_ms: None,
                }
            );
            Ok(())
        })
        .expect("a review card's review should record the FSRS prediction");
}

#[test]
fn a_relearning_card_records_its_short_term_prediction() {
    let database = Database::open_in_memory().expect("database should open");
    database
        .with_connection(|connection| {
            let deck_id = deck(connection)?;
            let card_id = create_card(connection, &deck_id, "Relearning", 0)?;
            let now = unix_time_millis()?;
            let last_review_at = now - 2 * DAY - 300_000;
            let before =
                set_reviewed_schedule(connection, &card_id, 3, 2.0, last_review_at, now - 60_000)?;
            let session_id = create_session(connection, std::slice::from_ref(&card_id))?;
            record_practice_event(connection, &session_id, &card_id, true)?;
            let mut input = review_input(&session_id, &card_id, 'c', "wrong");
            input.schedule_revision = 4;
            input.active_time_ms = Some(0);
            submit_review_in_connection(connection, input)?;

            let (reviewed_at, context) = stored(connection, 'c')?;
            let (retrievability, elapsed_days) = fsrs_review_view(&before, reviewed_at);
            assert_eq!(elapsed_days, 2);
            assert_eq!(context.previous_review_at, Some(last_review_at));
            assert_eq!(context.elapsed_days, elapsed_days);
            assert_eq!(context.retrievability, Some(retrievability));
            assert_eq!((context.reps_before, context.lapses_before), (5, 1));
            assert_eq!(context.active_time_ms, Some(0));
            Ok(())
        })
        .expect("a relearning card's review should record the FSRS prediction");
}

#[test]
fn active_time_is_stored_once_and_a_retry_cannot_change_it() {
    let database = Database::open_in_memory().expect("database should open");
    database
        .with_connection(|connection| {
            let deck_id = deck(connection)?;
            let first_card = create_card(connection, &deck_id, "First", 0)?;
            let second_card = create_card(connection, &deck_id, "Second", 1)?;
            let third_card = create_card(connection, &deck_id, "Third", 2)?;
            let session_id = create_session(
                connection,
                &[first_card.clone(), second_card.clone(), third_card.clone()],
            )?;

            record_practice_event(connection, &session_id, &first_card, true)?;
            let mut timed = review_input(&session_id, &first_card, 'd', "very_confident");
            timed.active_time_ms = Some(7_000);
            let first = submit_review_in_connection(connection, timed.clone())?;
            let mut untimed = timed.clone();
            untimed.active_time_ms = None;
            let fingerprint: String = connection
                .query_row(
                    "SELECT request_fingerprint FROM review_events WHERE submission_id = ?1",
                    ["d".repeat(32)],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            assert_eq!(fingerprint, review_request_fingerprint(&untimed)?);

            // A retry is the same request even if it reports another active time.
            let changes = connection.total_changes();
            let mut retry = timed;
            retry.active_time_ms = Some(9_999);
            let replay = submit_review_in_connection(connection, retry)?;
            assert_eq!(
                serde_json::to_value(&replay).map_err(CommandError::serialization)?,
                serde_json::to_value(&first).map_err(CommandError::serialization)?
            );
            assert_eq!(connection.total_changes(), changes);
            assert_eq!(stored(connection, 'd')?.1.active_time_ms, Some(7_000));

            record_practice_event(connection, &session_id, &second_card, true)?;
            submit_review_in_connection(
                connection,
                review_input(&session_id, &second_card, 'e', "wrong"),
            )?;
            assert_eq!(stored(connection, 'e')?.1.active_time_ms, None);

            record_practice_event(connection, &session_id, &third_card, true)?;
            let mut too_long = review_input(&session_id, &third_card, 'f', "wrong");
            too_long.active_time_ms = Some(MAX_ACTIVE_TIME_MS + 1);
            let error = submit_review_in_connection(connection, too_long)
                .expect_err("active time beyond a day is not a valid measurement");
            assert_eq!(error.code, "invalid_active_time");
            assert_eq!(count(connection, "SELECT COUNT(*) FROM review_events")?, 2);
            assert_eq!(
                count(
                    connection,
                    "SELECT COUNT(*) FROM review_event_scheduler_context"
                )?,
                2
            );
            Ok(())
        })
        .expect("active time should round-trip without changing retry identity");
}

#[test]
fn a_failed_context_write_rolls_back_the_whole_review() {
    let database = Database::open_in_memory().expect("database should open");
    database
        .with_connection(|connection| {
            let deck_id = deck(connection)?;
            let card_id = create_card(connection, &deck_id, "Atomic", 0)?;
            let session_id = create_session(connection, std::slice::from_ref(&card_id))?;
            record_practice_event(connection, &session_id, &card_id, true)?;
            connection
                .execute_batch(
                    "CREATE TEMP TRIGGER reject_scheduler_context
                     BEFORE INSERT ON review_event_scheduler_context
                     BEGIN SELECT RAISE(ABORT, 'injected context failure'); END;",
                )
                .map_err(CommandError::database)?;
            let input = review_input(&session_id, &card_id, '9', "somewhat_confident");
            assert!(submit_review_in_connection(connection, input.clone()).is_err());
            assert_eq!(count(connection, "SELECT COUNT(*) FROM review_events")?, 0);
            assert_eq!(
                count(connection, "SELECT COUNT(*) FROM review_event_attributions")?,
                0
            );
            assert_eq!(
                count(connection, "SELECT rated_count FROM practice_sessions")?,
                0
            );
            assert_eq!(
                crate::scheduler::load_schedule(connection, &card_id)?.reps,
                0
            );

            connection
                .execute_batch("DROP TRIGGER reject_scheduler_context;")
                .map_err(CommandError::database)?;
            submit_review_in_connection(connection, input)?;
            assert_eq!(stored(connection, '9')?.1.reps_before, 0);
            Ok(())
        })
        .expect("the review and its scheduler context should commit together");
}
