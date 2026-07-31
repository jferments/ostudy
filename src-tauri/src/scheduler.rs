//! Stable adapter between the ostudy database schema and `rs-fsrs`.
//!
//! The external crate is intentionally isolated in this module. Database rows,
//! Tauri commands, and frontend DTOs use application-owned types so that a
//! future scheduler upgrade cannot silently change the durable file format.
//! Review history records both the adapter version and the before/after state.

use chrono::{DateTime, Utc};
use rs_fsrs::{Card as FsrsCard, FSRS, Parameters, Rating, State};
use rusqlite::{params, Connection, OptionalExtension};

use crate::error::CommandError;

pub const SCHEDULER_NAME: &str = "fsrs";
pub const SCHEDULER_VERSION: &str = "rs-fsrs-1.2.1";

const MIN_DESIRED_RETENTION: f64 = 0.80;
const MAX_DESIRED_RETENTION: f64 = 0.97;

/// Application-owned representation of one card's mutable scheduling state.
#[derive(Clone, Debug, PartialEq)]
pub struct ScheduleRecord {
    pub card_id: String,
    pub state: i64,
    pub due_at: i64,
    pub stability: f64,
    pub difficulty: f64,
    pub elapsed_days: i64,
    pub scheduled_days: i64,
    pub reps: i64,
    pub lapses: i64,
    pub last_review_at: i64,
    pub scheduler_name: String,
    pub scheduler_version: String,
    pub updated_at: i64,
}

/// User-facing answer choices mapped onto FSRS's four canonical ratings.
///
/// The wording is intentionally about recall confidence rather than exposing
/// algorithm terminology to learners:
/// * wrong -> Again
/// * right, not confident -> Hard
/// * right, somewhat confident -> Good
/// * right, very confident -> Easy
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReviewOutcome {
    Wrong,
    NotConfident,
    SomewhatConfident,
    VeryConfident,
}

impl ReviewOutcome {
    pub fn parse(value: &str) -> Result<Self, CommandError> {
        match value {
            "wrong" => Ok(Self::Wrong),
            "not_confident" => Ok(Self::NotConfident),
            "somewhat_confident" => Ok(Self::SomewhatConfident),
            "very_confident" => Ok(Self::VeryConfident),
            _ => Err(CommandError::new(
                "invalid_review_outcome",
                "Choose whether the answer was wrong or how confident you were when correct.",
            )),
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Wrong => "wrong",
            Self::NotConfident => "not_confident",
            Self::SomewhatConfident => "somewhat_confident",
            Self::VeryConfident => "very_confident",
        }
    }

    pub const fn rating(self) -> Rating {
        match self {
            Self::Wrong => Rating::Again,
            Self::NotConfident => Rating::Hard,
            Self::SomewhatConfident => Rating::Good,
            Self::VeryConfident => Rating::Easy,
        }
    }

    pub const fn rating_number(self) -> i64 {
        match self {
            Self::Wrong => 1,
            Self::NotConfident => 2,
            Self::SomewhatConfident => 3,
            Self::VeryConfident => 4,
        }
    }

    pub const fn answer_correct(self) -> bool {
        !matches!(self, Self::Wrong)
    }
}

/// Complete result of applying one accepted review.
#[derive(Clone, Debug)]
pub struct ScheduleTransition {
    pub before: ScheduleRecord,
    pub after: ScheduleRecord,
    pub outcome: ReviewOutcome,
    pub reviewed_at: i64,
}

/// Creates the initial schedule for a newly created or imported card.
/// New cards are due immediately and become non-new only after a graded review.
pub fn new_schedule(card_id: String, now: i64) -> ScheduleRecord {
    ScheduleRecord {
        card_id,
        state: state_to_i64(State::New),
        due_at: now,
        stability: 0.0,
        difficulty: 0.0,
        elapsed_days: 0,
        scheduled_days: 0,
        reps: 0,
        lapses: 0,
        last_review_at: now,
        scheduler_name: SCHEDULER_NAME.to_string(),
        scheduler_version: SCHEDULER_VERSION.to_string(),
        updated_at: now,
    }
}

/// Inserts the initial schedule inside the caller's transaction.
pub fn insert_new_schedule(
    connection: &Connection,
    card_id: &str,
    now: i64,
) -> Result<(), CommandError> {
    let schedule = new_schedule(card_id.to_string(), now);
    connection
        .execute(
            "INSERT INTO card_schedules (\n\
                 card_id, state, due_at, stability, difficulty, elapsed_days,\n\
                 scheduled_days, reps, lapses, last_review_at,\n\
                 scheduler_name, scheduler_version, updated_at\n\
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                schedule.card_id,
                schedule.state,
                schedule.due_at,
                schedule.stability,
                schedule.difficulty,
                schedule.elapsed_days,
                schedule.scheduled_days,
                schedule.reps,
                schedule.lapses,
                schedule.last_review_at,
                schedule.scheduler_name,
                schedule.scheduler_version,
                schedule.updated_at,
            ],
        )
        .map_err(CommandError::database)?;
    Ok(())
}

/// Loads one active schedule. A missing row indicates a migration or integrity
/// problem; silently inventing a replacement would discard review history.
pub fn load_schedule(
    connection: &Connection,
    card_id: &str,
) -> Result<ScheduleRecord, CommandError> {
    connection
        .query_row(
            "SELECT card_id, state, due_at, stability, difficulty, elapsed_days,\n\
                    scheduled_days, reps, lapses, last_review_at,\n\
                    scheduler_name, scheduler_version, updated_at\n\
             FROM card_schedules WHERE card_id = ?1",
            [card_id],
            |row| {
                Ok(ScheduleRecord {
                    card_id: row.get(0)?,
                    state: row.get(1)?,
                    due_at: row.get(2)?,
                    stability: row.get(3)?,
                    difficulty: row.get(4)?,
                    elapsed_days: row.get(5)?,
                    scheduled_days: row.get(6)?,
                    reps: row.get(7)?,
                    lapses: row.get(8)?,
                    last_review_at: row.get(9)?,
                    scheduler_name: row.get(10)?,
                    scheduler_version: row.get(11)?,
                    updated_at: row.get(12)?,
                })
            },
        )
        .optional()
        .map_err(CommandError::database)?
        .ok_or_else(|| {
            CommandError::new(
                "schedule_not_found",
                "The flashcard has no review schedule. Restart the application and try again.",
            )
        })
}

/// Persists the result of `apply_review` inside the caller's transaction.
pub fn update_schedule(
    connection: &Connection,
    schedule: &ScheduleRecord,
) -> Result<(), CommandError> {
    validate_schedule(schedule)?;
    let changed = connection
        .execute(
            "UPDATE card_schedules SET\n\
                 state = ?1, due_at = ?2, stability = ?3, difficulty = ?4,\n\
                 elapsed_days = ?5, scheduled_days = ?6, reps = ?7, lapses = ?8,\n\
                 last_review_at = ?9, scheduler_name = ?10, scheduler_version = ?11,\n\
                 updated_at = ?12\n\
             WHERE card_id = ?13",
            params![
                schedule.state,
                schedule.due_at,
                schedule.stability,
                schedule.difficulty,
                schedule.elapsed_days,
                schedule.scheduled_days,
                schedule.reps,
                schedule.lapses,
                schedule.last_review_at,
                schedule.scheduler_name,
                schedule.scheduler_version,
                schedule.updated_at,
                schedule.card_id,
            ],
        )
        .map_err(CommandError::database)?;
    if changed != 1 {
        return Err(CommandError::new(
            "schedule_not_found",
            "The flashcard review schedule no longer exists.",
        ));
    }
    Ok(())
}

/// Applies FSRS to one card without mutating the database.
///
/// The caller owns the surrounding SQLite transaction. Keeping calculation
/// pure allows the review event and updated schedule to commit atomically.
pub fn apply_review(
    current: ScheduleRecord,
    outcome: ReviewOutcome,
    reviewed_at: i64,
    desired_retention: f64,
) -> Result<ScheduleTransition, CommandError> {
    validate_desired_retention(desired_retention)?;
    validate_schedule(&current)?;

    let now = datetime_from_millis(reviewed_at, "review time")?;
    let due = datetime_from_millis(current.due_at, "card due time")?;
    let last_review = datetime_from_millis(current.last_review_at, "last review time")?;

    let card = FsrsCard {
        due,
        stability: current.stability,
        difficulty: current.difficulty,
        elapsed_days: current.elapsed_days,
        scheduled_days: current.scheduled_days,
        reps: i32::try_from(current.reps).map_err(|_| invalid_schedule())?,
        lapses: i32::try_from(current.lapses).map_err(|_| invalid_schedule())?,
        state: state_from_i64(current.state)?,
        last_review,
    };

    let mut parameters = Parameters::default();
    parameters.request_retention = desired_retention;
    // Fuzz is useful for smoothing large real-world workloads, but disabling it
    // makes this early scheduler deterministic and therefore easier to test,
    // migrate, and explain. It can be made configurable later.
    parameters.enable_fuzz = false;

    let result = FSRS::new(parameters).next(card, now, outcome.rating());
    let next = result.card;
    let after = ScheduleRecord {
        card_id: current.card_id.clone(),
        state: state_to_i64(next.state),
        due_at: next.due.timestamp_millis(),
        stability: next.stability,
        difficulty: next.difficulty,
        elapsed_days: next.elapsed_days,
        scheduled_days: next.scheduled_days,
        reps: i64::from(next.reps),
        lapses: i64::from(next.lapses),
        last_review_at: next.last_review.timestamp_millis(),
        scheduler_name: SCHEDULER_NAME.to_string(),
        scheduler_version: SCHEDULER_VERSION.to_string(),
        updated_at: reviewed_at,
    };
    validate_schedule(&after)?;

    Ok(ScheduleTransition {
        before: current,
        after,
        outcome,
        reviewed_at,
    })
}

pub fn validate_desired_retention(value: f64) -> Result<(), CommandError> {
    if !value.is_finite() || !(MIN_DESIRED_RETENTION..=MAX_DESIRED_RETENTION).contains(&value) {
        return Err(CommandError::new(
            "invalid_desired_retention",
            "Desired retention must be between 80% and 97%.",
        ));
    }
    Ok(())
}

fn validate_schedule(schedule: &ScheduleRecord) -> Result<(), CommandError> {
    state_from_i64(schedule.state)?;
    if schedule.due_at < 0
        || schedule.last_review_at < 0
        || schedule.elapsed_days < 0
        || schedule.scheduled_days < 0
        || schedule.reps < 0
        || schedule.lapses < 0
        || !schedule.stability.is_finite()
        || schedule.stability < 0.0
        || !schedule.difficulty.is_finite()
        || schedule.difficulty < 0.0
    {
        return Err(invalid_schedule());
    }
    Ok(())
}

fn datetime_from_millis(value: i64, label: &str) -> Result<DateTime<Utc>, CommandError> {
    DateTime::<Utc>::from_timestamp_millis(value).ok_or_else(|| {
        CommandError::new(
            "invalid_schedule_time",
            format!("The stored {label} is outside the supported date range."),
        )
    })
}

fn state_from_i64(value: i64) -> Result<State, CommandError> {
    match value {
        0 => Ok(State::New),
        1 => Ok(State::Learning),
        2 => Ok(State::Review),
        3 => Ok(State::Relearning),
        _ => Err(invalid_schedule()),
    }
}

const fn state_to_i64(value: State) -> i64 {
    match value {
        State::New => 0,
        State::Learning => 1,
        State::Review => 2,
        State::Relearning => 3,
    }
}

fn invalid_schedule() -> CommandError {
    CommandError::new(
        "invalid_schedule",
        "The saved review schedule is invalid. The card was not changed.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_735_689_600_000; // 2025-01-01T00:00:00Z

    #[test]
    fn maps_product_wording_to_the_four_fsrs_ratings() {
        assert_eq!(ReviewOutcome::Wrong.rating_number(), 1);
        assert_eq!(ReviewOutcome::NotConfident.rating_number(), 2);
        assert_eq!(ReviewOutcome::SomewhatConfident.rating_number(), 3);
        assert_eq!(ReviewOutcome::VeryConfident.rating_number(), 4);
        assert!(!ReviewOutcome::Wrong.answer_correct());
        assert!(ReviewOutcome::SomewhatConfident.answer_correct());
    }

    #[test]
    fn new_cards_are_due_immediately() {
        let schedule = new_schedule("a".repeat(32), NOW);
        assert_eq!(schedule.state, 0);
        assert_eq!(schedule.due_at, NOW);
        assert_eq!(schedule.reps, 0);
    }

    #[test]
    fn applying_a_review_produces_a_future_schedule() {
        let schedule = new_schedule("a".repeat(32), NOW);
        let transition = apply_review(
            schedule,
            ReviewOutcome::SomewhatConfident,
            NOW,
            0.90,
        )
        .expect("FSRS should schedule a valid new card");

        assert_eq!(transition.after.reps, 1);
        assert!(transition.after.due_at >= NOW);
        assert_eq!(transition.after.scheduler_version, SCHEDULER_VERSION);
    }

    #[test]
    fn rejects_retention_outside_the_supported_range() {
        assert!(validate_desired_retention(0.79).is_err());
        assert!(validate_desired_retention(0.90).is_ok());
        assert!(validate_desired_retention(0.98).is_err());
        assert!(validate_desired_retention(f64::NAN).is_err());
    }
}
