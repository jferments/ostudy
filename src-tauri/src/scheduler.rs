//! Stable adapter between the ostudy database schema and `rs-fsrs`.

use chrono::{DateTime, Utc};
use rs_fsrs::{Card as FsrsCard, Parameters, Rating, State, FSRS};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use sha2::{Digest, Sha256};

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
    pub context: SchedulerContext,
}

/// What FSRS knew when it scheduled one review. None of these values can be
/// rebuilt later from the before/after schedule, so the review records them.
#[derive(Clone, Debug, PartialEq)]
pub struct SchedulerContext {
    /// The previous review time. `None` for a New card, whose `last_review_at`
    /// holds its creation or reset time rather than a review.
    pub previous_review_at: Option<i64>,
    /// Whole days since the previous review as FSRS measured them for this
    /// review (0 for a New card). The stored `elapsed_days` before a review is
    /// stale: it describes the previous review.
    pub elapsed_days: i64,
    /// FSRS-predicted recall probability at review time. `None` for a New card
    /// (the crate reports 0.0, which is not a prediction) and for a schedule
    /// whose stability cannot produce a probability.
    pub retrievability: Option<f64>,
    pub reps_before: i64,
    pub lapses_before: i64,
    pub desired_retention: f64,
    /// SHA-256 of the canonical identity of every other parameter that
    /// scheduled this review (see `parameters_sha256`).
    pub parameters_sha256: String,
}

/// Canonical form of every FSRS parameter except the retention target, which
/// each review records on its own. The crate's fuzz seed is excluded too: the
/// crate replaces it on every call with a value derived from the review time
/// and card state, so it is a per-call nonce rather than configuration.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ParameterIdentity<'a> {
    format: &'static str,
    weights: &'a [f64],
    maximum_interval: i32,
    decay: f64,
    factor: f64,
    enable_short_term: bool,
    enable_fuzz: bool,
}

/// SHA-256 (lowercase hex) of the canonical JSON for the scheduler parameters,
/// so a later switch to personalized weights is visible on each review.
fn parameters_sha256(parameters: &Parameters) -> Result<String, CommandError> {
    let identity = ParameterIdentity {
        format: "ostudy-fsrs-parameters-v1",
        weights: &parameters.w,
        maximum_interval: parameters.maximum_interval,
        decay: parameters.decay,
        factor: parameters.factor,
        enable_short_term: parameters.enable_short_term,
        enable_fuzz: parameters.enable_fuzz,
    };
    let canonical = serde_json::to_vec(&identity).map_err(CommandError::serialization)?;
    Ok(format!("{:x}", Sha256::digest(canonical)))
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
            let transition =
                apply_review(current.clone(), outcome, previewed_at, desired_retention)?;
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
    // Read the prediction before FSRS consumes the pre-review card.
    let retrievability = (card.state != State::New)
        .then(|| card.get_retrievability(now))
        .filter(|value| value.is_finite() && (0.0..=1.0).contains(value));
    let parameters = scheduler_parameters(desired_retention);
    let parameters_sha256 = parameters_sha256(&parameters)?;
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
    let context = SchedulerContext {
        previous_review_at: (current.state != state_to_i64(State::New))
            .then_some(current.last_review_at),
        elapsed_days: after.elapsed_days,
        retrievability,
        reps_before: current.reps,
        lapses_before: current.lapses,
        desired_retention,
        parameters_sha256,
    };
    Ok(ScheduleTransition {
        before: current,
        after,
        outcome,
        reviewed_at,
        context,
    })
}

/// The FSRS parameters every review uses: the crate defaults with fuzz off and
/// the learner's retention target.
fn scheduler_parameters(desired_retention: f64) -> Parameters {
    Parameters {
        request_retention: desired_retention,
        enable_fuzz: false,
        ..Parameters::default()
    }
}

/// The parameter identity that reviews currently record. It does not depend on
/// the retention target, which each review stores separately.
#[cfg(test)]
pub(crate) fn scheduler_parameters_sha256() -> Result<String, CommandError> {
    parameters_sha256(&scheduler_parameters(MAX_DESIRED_RETENTION))
}

/// The recall probability rs-fsrs computes for the pre-review schedule and the
/// elapsed days its review log reports, computed independently of
/// `apply_review` so tests can compare the recorded context with the crate.
#[cfg(test)]
pub(crate) fn fsrs_review_view(schedule: &ScheduleRecord, reviewed_at: i64) -> (f64, i64) {
    let time = |millis: i64| DateTime::<Utc>::from_timestamp_millis(millis).unwrap();
    let now = time(reviewed_at);
    let card = FsrsCard {
        due: time(schedule.due_at),
        stability: schedule.stability,
        difficulty: schedule.difficulty,
        elapsed_days: schedule.elapsed_days,
        scheduled_days: schedule.scheduled_days,
        reps: i32::try_from(schedule.reps).unwrap(),
        lapses: i32::try_from(schedule.lapses).unwrap(),
        state: state_from_i64(schedule.state).unwrap(),
        last_review: time(schedule.last_review_at),
    };
    let retrievability = card.get_retrievability(now);
    let log = FSRS::new(scheduler_parameters(MAX_DESIRED_RETENTION))
        .next(card, now, Rating::Good)
        .review_log;
    (retrievability, log.elapsed_days)
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

/// FSRS's predicted probability of recalling a studied card at `now`, from
/// its stored stability and last review. `None` for a new card or a schedule
/// whose values give no valid prediction.
pub fn estimated_recall(
    state: i64,
    stability: f64,
    last_review_at: i64,
    now_utc_millis: i64,
) -> Option<f64> {
    let state = state_from_i64(state)
        .ok()
        .filter(|state| *state != State::New)?;
    let card = FsrsCard {
        state,
        stability,
        last_review: DateTime::<Utc>::from_timestamp_millis(last_review_at)?,
        ..FsrsCard::new()
    };
    let now = DateTime::<Utc>::from_timestamp_millis(now_utc_millis)?;
    Some(card.get_retrievability(now))
        .filter(|value| value.is_finite() && (0.0..=1.0).contains(value))
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

    const DAY: i64 = 86_400_000;
    /// SHA-256 of the canonical JSON for rs-fsrs 1.2.1 defaults with fuzz off:
    /// {"format":"ostudy-fsrs-parameters-v1","weights":[0.4072,...,0.6567],
    ///  "maximumInterval":36500,"decay":-0.5,"factor":0.2345679012345679,
    ///  "enableShortTerm":true,"enableFuzz":false}
    const DEFAULT_PARAMETERS_SHA256: &str =
        "c000fa95a9efa680ef0c360ce9e1e2fe6d0c1164aec88b5d40d8c5a017c72dac";

    fn reviewed(state: i64, stability: f64, last_review_at: i64, due_at: i64) -> ScheduleRecord {
        ScheduleRecord {
            state,
            due_at,
            stability,
            difficulty: 5.0,
            // Stale on purpose: this value describes the previous review.
            elapsed_days: 3,
            scheduled_days: 4,
            reps: 6,
            lapses: 2,
            revision: 7,
            ..new_schedule("a".repeat(32), last_review_at)
        }
    }

    #[test]
    fn new_card_context_has_no_previous_review_or_prediction() {
        let schedule = new_schedule("a".repeat(32), NOW - 5 * DAY);
        let transition =
            apply_review(schedule, ReviewOutcome::SomewhatConfident, NOW, 0.9).unwrap();
        assert_eq!(
            transition.context,
            SchedulerContext {
                previous_review_at: None,
                elapsed_days: 0,
                retrievability: None,
                reps_before: 0,
                lapses_before: 0,
                desired_retention: 0.9,
                parameters_sha256: DEFAULT_PARAMETERS_SHA256.to_string(),
            }
        );
    }

    #[test]
    fn review_context_records_the_prediction_and_elapsed_days_fsrs_used() {
        let last_review_at = NOW - 10 * DAY - 3_600_000;
        let schedule = reviewed(2, 12.0, last_review_at, NOW - 3_600_000);
        let (expected_retrievability, expected_elapsed) = fsrs_review_view(&schedule, NOW);
        assert_eq!(expected_elapsed, 10);
        assert_eq!(
            expected_retrievability,
            Parameters::forgetting_curve(10.0, 12.0)
        );

        let transition =
            apply_review(schedule, ReviewOutcome::SomewhatConfident, NOW, 0.85).unwrap();
        let context = &transition.context;
        assert_eq!(context.previous_review_at, Some(last_review_at));
        assert_eq!(context.elapsed_days, expected_elapsed);
        assert_ne!(context.elapsed_days, transition.before.elapsed_days);
        assert_eq!(context.retrievability, Some(expected_retrievability));
        assert_eq!((context.reps_before, context.lapses_before), (6, 2));
        assert_eq!(context.desired_retention, 0.85);
        assert_eq!(context.parameters_sha256, DEFAULT_PARAMETERS_SHA256);
    }

    #[test]
    fn relearning_context_keeps_the_short_term_prediction() {
        let last_review_at = NOW - 2 * DAY - 300_000;
        let schedule = reviewed(3, 2.0, last_review_at, NOW - 60_000);
        let (expected_retrievability, expected_elapsed) = fsrs_review_view(&schedule, NOW);
        let transition = apply_review(schedule, ReviewOutcome::Wrong, NOW, 0.9).unwrap();
        assert_eq!(transition.context.elapsed_days, expected_elapsed);
        assert_eq!(transition.context.elapsed_days, 2);
        assert_eq!(
            transition.context.retrievability,
            Some(expected_retrievability)
        );
        assert_eq!(transition.context.previous_review_at, Some(last_review_at));
    }

    #[test]
    fn a_schedule_without_a_usable_stability_records_no_prediction() {
        // Stability 0 reviewed on the same day is 0/0 inside the forgetting curve.
        let schedule = reviewed(1, 0.0, NOW - 60_000, NOW - 30_000);
        let transition = apply_review(schedule, ReviewOutcome::Wrong, NOW, 0.9).unwrap();
        assert_eq!(transition.context.retrievability, None);
        assert_eq!(transition.context.previous_review_at, Some(NOW - 60_000));
    }

    #[test]
    fn parameter_identity_ignores_retention_and_tracks_every_other_parameter() {
        let base = parameters_sha256(&scheduler_parameters(0.9)).unwrap();
        assert_eq!(base, DEFAULT_PARAMETERS_SHA256);
        assert_eq!(scheduler_parameters_sha256().unwrap(), base);
        assert_eq!(
            parameters_sha256(&scheduler_parameters(0.8)).unwrap(),
            base,
            "the retention target is recorded separately"
        );
        let changes: [fn(&mut Parameters); 6] = [
            |parameters| parameters.w[4] += 0.01,
            |parameters| parameters.maximum_interval = 365,
            |parameters| parameters.decay = -0.4,
            |parameters| parameters.factor = 0.25,
            |parameters| parameters.enable_short_term = false,
            |parameters| parameters.enable_fuzz = true,
        ];
        for change in changes {
            let mut parameters = scheduler_parameters(0.9);
            change(&mut parameters);
            assert_ne!(parameters_sha256(&parameters).unwrap(), base);
        }
    }
}
