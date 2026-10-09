//! The study calendar: days studied, streaks and the busiest weekday.
//!
//! Streaks and the calendar describe the learner, so they read the study
//! activity ledger: every stored review, whatever filter the page shows and
//! whether or not its card still exists, plus the totals that deletions kept
//! when they purged review history.

use chrono::{Datelike, Duration, NaiveDate};
use rusqlite::Connection;
use serde::Serialize;

use crate::{error::CommandError, study_activity, study_calendar::StudyTimeContext};

/// Study days in the activity calendar, today included (26 weeks).
pub const ACTIVITY_DAYS: i64 = 182;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityDayDto {
    pub local_date: String,
    pub review_count: i64,
    pub study_time_ms: i64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StreakDto {
    pub length: i64,
    pub start_local_date: String,
    pub end_local_date: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressActivityDto {
    pub start_local_date: String,
    pub today_local_date: String,
    /// Study days with at least one review, oldest first.
    pub days: Vec<ActivityDayDto>,
    /// The run of study days ending today, or yesterday while today has no
    /// review yet. `None` once a whole study day was missed.
    pub current_streak: Option<StreakDto>,
    pub longest_streak: Option<StreakDto>,
    /// Monday is 0. The weekday with the most reviews in the calendar.
    pub busiest_weekday: Option<u32>,
}

pub fn load(
    connection: &Connection,
    time: &StudyTimeContext,
) -> Result<ProgressActivityDto, CommandError> {
    let start = time
        .local_date
        .checked_sub_signed(Duration::days(ACTIVITY_DAYS - 1))
        .ok_or_else(invalid_date)?;
    let hour = time.new_day_start_hour();
    let calendar =
        study_activity::learner_days(connection, hour, Some(start), Some(time.local_date))?;
    let days: Vec<ActivityDayDto> = calendar
        .iter()
        .filter(|(_, day)| day.review_count > 0)
        .map(|(date, day)| ActivityDayDto {
            local_date: date.to_string(),
            review_count: day.review_count,
            study_time_ms: day.study_time_ms,
        })
        .collect();

    let mut weekday_reviews = [0_i64; 7];
    for (date, day) in &calendar {
        weekday_reviews[date.weekday().num_days_from_monday() as usize] += day.review_count;
    }
    let busiest_weekday = weekday_reviews
        .iter()
        .enumerate()
        .filter(|(_, count)| **count > 0)
        // The first of equal weekdays wins: max_by_key keeps the last.
        .max_by_key(|(weekday, count)| (**count, std::cmp::Reverse(*weekday)))
        .and_then(|(weekday, _)| u32::try_from(weekday).ok());

    let study_dates = study_activity::learner_study_dates(connection, hour)?;
    let (current_streak, longest_streak) = streaks(&study_dates, time.local_date);
    Ok(ProgressActivityDto {
        start_local_date: start.to_string(),
        today_local_date: time.local_date.to_string(),
        days,
        current_streak,
        longest_streak,
        busiest_weekday,
    })
}

/// The current and longest runs of consecutive study dates. `dates` is sorted
/// and distinct; a later date than `today` (a clock moved back) is ignored.
fn streaks(dates: &[NaiveDate], today: NaiveDate) -> (Option<StreakDto>, Option<StreakDto>) {
    let mut longest: Option<(NaiveDate, NaiveDate)> = None;
    let mut run: Option<(NaiveDate, NaiveDate)> = None;
    for &date in dates.iter().filter(|date| **date <= today) {
        run = match run {
            Some((start, end)) if end.succ_opt() == Some(date) => Some((start, date)),
            _ => Some((date, date)),
        };
        if let Some((start, end)) = run {
            let longer = longest.is_none_or(|(best_start, best_end)| {
                (end - start).num_days() > (best_end - best_start).num_days()
            });
            if longer {
                longest = Some((start, end));
            }
        }
    }
    let current = run.filter(|(_, end)| *end == today || end.succ_opt() == Some(today));
    (current.map(streak_dto), longest.map(streak_dto))
}

fn streak_dto((start, end): (NaiveDate, NaiveDate)) -> StreakDto {
    StreakDto {
        length: (end - start).num_days() + 1,
        start_local_date: start.to_string(),
        end_local_date: end.to_string(),
    }
}

fn invalid_date() -> CommandError {
    CommandError::new(
        "invalid_analytics_date",
        "The activity calendar is outside the supported calendar.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(value: &str) -> NaiveDate {
        study_activity::parse_date(value).unwrap()
    }

    fn run(length: i64, start: &str, end: &str) -> Option<StreakDto> {
        Some(StreakDto {
            length,
            start_local_date: start.to_string(),
            end_local_date: end.to_string(),
        })
    }

    #[test]
    fn a_streak_survives_until_a_whole_study_day_is_missed() {
        let dates = [
            date("2026-09-01"),
            date("2026-09-02"),
            date("2026-09-03"),
            date("2026-09-05"),
            date("2026-09-06"),
        ];
        // Today has no review yet: yesterday's run is still current.
        assert_eq!(
            streaks(&dates, date("2026-09-07")),
            (
                run(2, "2026-09-05", "2026-09-06"),
                run(3, "2026-09-01", "2026-09-03")
            )
        );
        assert_eq!(streaks(&dates, date("2026-09-08")).0, None);
        assert_eq!(streaks(&[], date("2026-09-08")), (None, None));
    }

    #[test]
    fn the_earliest_of_equal_longest_runs_is_kept() {
        let dates = [
            date("2026-09-01"),
            date("2026-09-02"),
            date("2026-09-04"),
            date("2026-09-05"),
        ];
        assert_eq!(
            streaks(&dates, date("2026-09-05")),
            (
                run(2, "2026-09-04", "2026-09-05"),
                run(2, "2026-09-01", "2026-09-02")
            )
        );
    }
}
