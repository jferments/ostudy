//! The study calendar: days studied, streaks and the busiest weekday.
//!
//! Streaks and the calendar describe the learner, so they read every review
//! attribution, whatever filter the page shows and whether or not the card
//! still exists.

use chrono::{Datelike, Duration, NaiveDate};
use rusqlite::{params_from_iter, types::Value as SqlValue, Connection};
use serde::Serialize;

use crate::{error::CommandError, study_calendar::StudyTimeContext};

use super::super::scope::{AnalyticsContext, HistoryPeriod};

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

/// Study dates in the context's range with at least one review in scope.
pub fn study_day_count(
    connection: &Connection,
    context: &AnalyticsContext,
) -> Result<i64, CommandError> {
    let mut values = Vec::<SqlValue>::new();
    let conditions = context.historical_conditions(&mut values, HistoryPeriod::Current)?;
    connection
        .query_row(
            &format!(
                "SELECT COUNT(DISTINCT {study_date}) FROM {history_from} WHERE {conditions}",
                study_date = context.time.review_study_date_sql("a"),
                history_from = context.history_from_sql(),
                conditions = conditions.join(" AND "),
            ),
            params_from_iter(values.iter()),
            |row| row.get(0),
        )
        .map_err(CommandError::database)
}

pub fn load(
    connection: &Connection,
    time: &StudyTimeContext,
) -> Result<ProgressActivityDto, CommandError> {
    let start = time
        .local_date
        .checked_sub_signed(Duration::days(ACTIVITY_DAYS - 1))
        .ok_or_else(invalid_date)?;
    let mut values = Vec::<SqlValue>::new();
    let in_calendar =
        time.review_study_dates_sql("a", Some(start), Some(time.local_date), &mut values)?;
    let sql = format!(
        "SELECT {study_date}, COUNT(*), COALESCE(SUM(r.response_time_ms), 0)
         FROM review_event_attributions a
         CROSS JOIN review_events r ON r.id = a.review_event_id
         WHERE {in_calendar}
         GROUP BY 1
         ORDER BY 1",
        study_date = time.review_study_date_sql("a"),
    );
    let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
    let days = statement
        .query_map(params_from_iter(values.iter()), |row| {
            Ok(ActivityDayDto {
                local_date: row.get(0)?,
                review_count: row.get(1)?,
                study_time_ms: row.get(2)?,
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;

    let mut weekday_reviews = [0_i64; 7];
    for day in &days {
        let date = parse_date(&day.local_date)?;
        weekday_reviews[date.weekday().num_days_from_monday() as usize] += day.review_count;
    }
    let busiest_weekday = weekday_reviews
        .iter()
        .enumerate()
        .filter(|(_, count)| **count > 0)
        // The first of equal weekdays wins: max_by_key keeps the last.
        .max_by_key(|(weekday, count)| (**count, std::cmp::Reverse(*weekday)))
        .and_then(|(weekday, _)| u32::try_from(weekday).ok());

    let (current_streak, longest_streak) =
        streaks(&all_study_dates(connection, time)?, time.local_date);
    Ok(ProgressActivityDto {
        start_local_date: start.to_string(),
        today_local_date: time.local_date.to_string(),
        days,
        current_streak,
        longest_streak,
        busiest_weekday,
    })
}

/// Every study date with a review, oldest first.
fn all_study_dates(
    connection: &Connection,
    time: &StudyTimeContext,
) -> Result<Vec<NaiveDate>, CommandError> {
    let sql = format!(
        "SELECT DISTINCT {study_date} FROM review_event_attributions a ORDER BY 1",
        study_date = time.review_study_date_sql("a"),
    );
    let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
    let dates = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    dates.iter().map(|date| parse_date(date)).collect()
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

fn parse_date(value: &str) -> Result<NaiveDate, CommandError> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| invalid_date())
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
        parse_date(value).unwrap()
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
