//! Stable adapter between the ostudy database schema and `rs-fsrs`.

use chrono::{DateTime, Utc};
use rs_fsrs::{Card as FsrsCard, Parameters, Rating, State, FSRS};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::error::CommandError;

pub const SCHEDULER_NAME: &str = "fsrs";
pub const SCHEDULER_VERSION: &str = "rs-fsrs-1.2.1";

const MIN_DESIRED_RETENTION: f64 = 0.80;
const MAX_DESIRED_RETENTION: f64 = 0.97;

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
    pub revision: i64,
}

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

#[derive(Clone, Debug)]
pub struct ScheduleTransition {
    pub before: ScheduleRecord,
    pub after: ScheduleRecord,
    pub outcome: ReviewOutcome,
    pub reviewed_at: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntervalPreviewDto {
    pub outcome: String,
    pub due_at: i64,
    pub interval_ms: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewPreviewSetDto {
    pub schedule_revision: i64,
    pub previewed_at: i64,
    pub previews: Vec<IntervalPreviewDto>,
}

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
        revision: 1,
    }
}

pub fn insert_new_schedule(
    connection: &Connection,
    card_id: &str,
    now: i64,
) -> Result<(), CommandError> {
    let schedule = new_schedule(card_id.to_string(), now);
    connection
        .execute(
            "INSERT INTO card_schedules (
                 card_id, state, due_at, stability, difficulty, elapsed_days,
                 scheduled_days, reps, lapses, last_review_at,
                 scheduler_name, scheduler_version, updated_at, revision
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
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
                schedule.revision,
            ],
        )
        .map_err(CommandError::database)?;
    Ok(())
}

pub fn load_schedule(
    connection: &Connection,
    card_id: &str,
) -> Result<ScheduleRecord, CommandError> {
    connection
        .query_row(
            "SELECT card_id, state, due_at, stability, difficulty, elapsed_days,
                    scheduled_days, reps, lapses, last_review_at,
                    scheduler_name, scheduler_version, updated_at, revision
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
                    revision: row.get(13)?,
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

pub fn update_schedule(
    connection: &Connection,
    schedule: &ScheduleRecord,
) -> Result<(), CommandError> {
    validate_schedule(schedule)?;
    let changed = connection
        .execute(
            "UPDATE card_schedules SET
                 state = ?1, due_at = ?2, stability = ?3, difficulty = ?4,
                 elapsed_days = ?5, scheduled_days = ?6, reps = ?7, lapses = ?8,
                 last_review_at = ?9, scheduler_name = ?10, scheduler_version = ?11,
                 updated_at = ?12, revision = ?13
             WHERE card_id = ?14",
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
                schedule.revision,
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

pub fn preview_intervals(
    current: &ScheduleRecord,
    previewed_at: i64,
    desired_retention: f64,
) -> Result<ReviewPreviewSetDto, CommandError> {
    let outcomes = [
        ReviewOutcome::Wrong,
        ReviewOutcome::NotConfident,
        ReviewOutcome::SomewhatConfident,
        ReviewOutcome::VeryConfident,
    ];
    let previews = outcomes
        .into_iter()
        .map(|outcome| {
            let transition = apply_review(current.clone(), outcome, previewed_at, desired_retention)?;
            Ok(IntervalPreviewDto {
                outcome: outcome.as_str().to_string(),
                due_at: transition.after.due_at,
                interval_ms: transition.after.due_at.saturating_sub(previewed_at),
            })
        })
        .collect::<Result<Vec<_>, CommandError>>()?;
    Ok(ReviewPreviewSetDto {
        schedule_revision: current.revision,
        previewed_at,
        previews,
    })
}

pub fn apply_review(
    current: ScheduleRecord,
    outcome: ReviewOutcome,
    reviewed_at: i64,
    desired_retention: f64,
) -> Result<ScheduleTransition, CommandError> {
    validate_desired_retention(desired_retention)?;
    validate_schedule(&current)?;

    let now = datetime_from_millis(reviewed_at, "review time")?;
    let card = FsrsCard {
        due: datetime_from_millis(current.due_at, "card due time")?,
        stability: current.stability,
        difficulty: current.difficulty,
        elapsed_days: current.elapsed_days,
        scheduled_days: current.scheduled_days,
        reps: i32::try_from(current.reps).map_err(|_| invalid_schedule())?,
        lapses: i32::try_from(current.lapses).map_err(|_| invalid_schedule())?,
        state: state_from_i64(current.state)?,
        last_review: datetime_from_millis(current.last_review_at, "last review time")?,
    };
    let parameters = Parameters {
        request_retention: desired_retention,
        enable_fuzz: false,
        ..Parameters::default()
    };
    let next = FSRS::new(parameters).next(card, now, outcome.rating()).card;
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
        revision: current.revision + 1,
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
        || schedule.revision < 1
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

    const NOW: i64 = 1_735_689_600_000;

    #[test]
    fn maps_product_wording_to_the_four_fsrs_ratings() {
        assert_eq!(ReviewOutcome::Wrong.rating_number(), 1);
        assert_eq!(ReviewOutcome::VeryConfident.rating_number(), 4);
        assert!(!ReviewOutcome::Wrong.answer_correct());
        assert!(ReviewOutcome::SomewhatConfident.answer_correct());
    }

    #[test]
    fn review_and_preview_advance_schedule_revisions() {
        let schedule = new_schedule("a".repeat(32), NOW);
        let previews = preview_intervals(&schedule, NOW, 0.90).expect("preview should work");
        assert_eq!(previews.schedule_revision, 1);
        assert_eq!(previews.previews.len(), 4);
        let transition = apply_review(schedule, ReviewOutcome::SomewhatConfident, NOW, 0.90)
            .expect("review should work");
        assert_eq!(transition.after.revision, 2);
        assert_eq!(transition.after.reps, 1);
    }

    #[test]
    fn rejects_retention_outside_the_supported_range() {
        assert!(validate_desired_retention(0.79).is_err());
        assert!(validate_desired_retention(0.90).is_ok());
        assert!(validate_desired_retention(0.98).is_err());
    }
}
