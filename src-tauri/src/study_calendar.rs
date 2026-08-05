//! Local-calendar boundaries shared by workload, scheduling, and Analytics.
//!
//! FSRS schedules remain exact UTC millisecond instants. This module only owns
//! concepts that are explicitly calendar-based: the current study date, daily
//! limits, burial expiry, date-only manual rescheduling, and Analytics buckets.
//! Each command captures one context and passes it through subordinate queries so
//! a request cannot straddle midnight with inconsistent interpretations.

use chrono::{DateTime, Datelike, Duration, Local, LocalResult, NaiveDate, TimeZone};

use crate::{error::CommandError, util::unix_time_millis};

#[derive(Clone, Debug)]
pub struct StudyTimeContext {
    pub now_utc_millis: i64,
    pub local_date: NaiveDate,
    pub today_start_utc_millis: i64,
    pub tomorrow_start_utc_millis: i64,
    pub utc_offset_minutes: i32,
    pub time_zone_id: String,
}

impl StudyTimeContext {
    pub fn capture() -> Result<Self, CommandError> {
        Self::at(unix_time_millis()?)
    }

    pub fn at(now_utc_millis: i64) -> Result<Self, CommandError> {
        let local = Local
            .timestamp_millis_opt(now_utc_millis)
            .earliest()
            .ok_or_else(invalid_calendar_time)?;
        let local_date = local.date_naive();
        let today_start_utc_millis = local_date_start_millis(local_date)?;
        let tomorrow = local_date.succ_opt().ok_or_else(invalid_calendar_date)?;
        let tomorrow_start_utc_millis = local_date_start_millis(tomorrow)?;
        let offset_seconds = local.offset().local_minus_utc();
        let utc_offset_minutes = offset_seconds / 60;
        Ok(Self {
            now_utc_millis,
            local_date,
            today_start_utc_millis,
            tomorrow_start_utc_millis,
            utc_offset_minutes,
            // `chrono::Local` does not expose a portable IANA identifier. Store an
            // explicit system-local label plus the event-time offset instead of
            // pretending a fixed offset is a complete time-zone rule set.
            time_zone_id: "system-local".to_string(),
        })
    }

    pub fn local_date_string(&self) -> String {
        self.local_date.format("%Y-%m-%d").to_string()
    }

    pub fn local_date_for_millis(&self, value: i64) -> Result<NaiveDate, CommandError> {
        local_date_for_millis(value)
    }

    pub fn local_day_sort_key(&self, value: i64) -> Result<i32, CommandError> {
        Ok(self.local_date_for_millis(value)?.num_days_from_ce())
    }

    pub fn date_after_days(&self, days: i64) -> Result<NaiveDate, CommandError> {
        self.local_date
            .checked_add_signed(Duration::days(days))
            .ok_or_else(invalid_calendar_date)
    }

    pub fn resolve_date_start(&self, date: NaiveDate) -> Result<i64, CommandError> {
        local_date_start_millis(date)
    }

    pub fn resolve_date_or_now(&self, date: NaiveDate) -> Result<i64, CommandError> {
        if date < self.local_date {
            return Err(CommandError::new(
                "invalid_reschedule_date",
                "Choose today or a future local calendar date.",
            ));
        }
        if date == self.local_date {
            Ok(self.now_utc_millis)
        } else {
            self.resolve_date_start(date)
        }
    }
}

pub fn local_date_for_millis(value: i64) -> Result<NaiveDate, CommandError> {
    Local
        .timestamp_millis_opt(value)
        .earliest()
        .map(|date_time| date_time.date_naive())
        .ok_or_else(invalid_calendar_time)
}

pub fn local_date_string_for_millis(value: i64) -> Result<String, CommandError> {
    Ok(local_date_for_millis(value)?.format("%Y-%m-%d").to_string())
}

/// Resolve the first valid local instant belonging to `date`.
///
/// Most zones have an unambiguous midnight. A few historical/current time-zone
/// rules can skip or repeat midnight, so the resolver advances by one minute
/// until it finds the first valid instant of the requested date. Ambiguous times
/// use the earlier occurrence.
pub fn local_date_start_millis(date: NaiveDate) -> Result<i64, CommandError> {
    let mut candidate = date
        .and_hms_opt(0, 0, 0)
        .ok_or_else(invalid_calendar_date)?;
    for _ in 0..=(26 * 60) {
        match Local.from_local_datetime(&candidate) {
            LocalResult::Single(value) => return Ok(value.timestamp_millis()),
            LocalResult::Ambiguous(earlier, later) => {
                return Ok(earlier.min(later).timestamp_millis())
            }
            LocalResult::None => {
                candidate = candidate
                    .checked_add_signed(Duration::minutes(1))
                    .ok_or_else(invalid_calendar_date)?;
                if candidate.date() != date {
                    break;
                }
            }
        }
    }
    Err(CommandError::new(
        "invalid_local_day",
        "The beginning of that local calendar date could not be resolved.",
    ))
}

pub fn parse_local_date(value: &str) -> Result<NaiveDate, CommandError> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| {
        CommandError::new(
            "invalid_reschedule_date",
            "Choose a valid local calendar date.",
        )
    })
}

pub fn range_start_date(
    context: &StudyTimeContext,
    range: &str,
) -> Result<Option<NaiveDate>, CommandError> {
    match range {
        "7d" => context.date_after_days(-6).map(Some),
        "30d" => context.date_after_days(-29).map(Some),
        "90d" => context.date_after_days(-89).map(Some),
        "1y" => Ok(Some(one_year_before(context.local_date)?)),
        "all" => Ok(None),
        _ => Err(CommandError::new(
            "invalid_analytics_range",
            "Choose 7 days, 30 days, 90 days, one year, or all time.",
        )),
    }
}

pub fn previous_range_start_date(
    context: &StudyTimeContext,
    range: &str,
) -> Result<Option<NaiveDate>, CommandError> {
    let current = range_start_date(context, range)?;
    let Some(current) = current else {
        return Ok(None);
    };
    let length_days = context
        .local_date
        .signed_duration_since(current)
        .num_days()
        .saturating_add(1);
    current
        .checked_sub_signed(Duration::days(length_days))
        .map(Some)
        .ok_or_else(invalid_calendar_date)
}

fn one_year_before(date: NaiveDate) -> Result<NaiveDate, CommandError> {
    if let Some(value) = date.with_year(date.year().saturating_sub(1)) {
        return Ok(value);
    }
    // February 29 clamps to February 28 in the previous non-leap year.
    NaiveDate::from_ymd_opt(date.year().saturating_sub(1), 2, 28).ok_or_else(invalid_calendar_date)
}

fn invalid_calendar_date() -> CommandError {
    CommandError::new(
        "invalid_local_day",
        "The local calendar date is outside the supported range.",
    )
}

fn invalid_calendar_time() -> CommandError {
    CommandError::new(
        "invalid_local_day",
        "The local calendar time is outside the supported range.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_ranges_include_today_as_a_calendar_date() {
        let now = unix_time_millis().expect("clock");
        let context = StudyTimeContext::at(now).expect("context");
        assert_eq!(
            context
                .local_date
                .signed_duration_since(range_start_date(&context, "7d").unwrap().unwrap())
                .num_days(),
            6
        );
        assert_eq!(
            context
                .local_date
                .signed_duration_since(range_start_date(&context, "30d").unwrap().unwrap())
                .num_days(),
            29
        );
    }

    #[test]
    fn current_date_resolves_to_now_and_future_date_to_day_start() {
        let context = StudyTimeContext::capture().expect("context");
        assert_eq!(
            context.resolve_date_or_now(context.local_date).unwrap(),
            context.now_utc_millis
        );
        let tomorrow = context.date_after_days(1).unwrap();
        assert_eq!(
            context.resolve_date_or_now(tomorrow).unwrap(),
            context.tomorrow_start_utc_millis
        );
    }

    #[test]
    fn leap_day_year_range_clamps_safely() {
        let date = NaiveDate::from_ymd_opt(2024, 2, 29).unwrap();
        assert_eq!(
            one_year_before(date).unwrap(),
            NaiveDate::from_ymd_opt(2023, 2, 28).unwrap()
        );
    }
}
