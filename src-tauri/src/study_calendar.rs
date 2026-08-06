//! Authoritative local-calendar semantics for study commands.
//!
//! FSRS continues to own exact due instants. This module owns the calendar
//! interpretation around those instants: local study dates, burial boundaries,
//! date-only rescheduling, and analytics windows. Every command captures one
//! [`StudyTimeContext`] and passes it through subordinate calculations.

use std::{env, path::Path};

use chrono::{
    DateTime, Datelike, Duration, Local, LocalResult, NaiveDate, NaiveDateTime, Offset, TimeZone,
};

use crate::{error::CommandError, util::unix_time_millis};

const SECONDS_PER_LOCAL_DATE: i64 = 86_400;
const MAX_CALENDAR_DAY_SPAN: i64 = 3_660;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StudyTimeContext {
    pub now_utc_millis: i64,
    pub time_zone_id: String,
    pub utc_offset_minutes: i32,
    pub local_date: NaiveDate,
    pub today_start_utc_millis: i64,
    pub tomorrow_start_utc_millis: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CalendarWindow {
    pub start_local_date: NaiveDate,
    pub end_local_date: NaiveDate,
    pub start_utc_millis: i64,
    pub end_utc_millis: i64,
}

impl StudyTimeContext {
    pub fn capture() -> Result<Self, CommandError> {
        Self::capture_at(unix_time_millis()?)
    }

    pub fn capture_at(now_utc_millis: i64) -> Result<Self, CommandError> {
        let local = Local
            .timestamp_millis_opt(now_utc_millis)
            .earliest()
            .ok_or_else(invalid_calendar_date)?;
        let local_date = local.date_naive();
        let today_start_utc_millis = first_valid_instant_of_local_date(local_date)?;
        let tomorrow = checked_add_days(local_date, 1)?;
        let tomorrow_start_utc_millis = first_valid_instant_of_local_date(tomorrow)?;
        Ok(Self {
            now_utc_millis,
            time_zone_id: discover_system_time_zone_id(),
            utc_offset_minutes: local.offset().fix().local_minus_utc().div_euclid(60),
            local_date,
            today_start_utc_millis,
            tomorrow_start_utc_millis,
        })
    }

    pub fn first_valid_instant(&self, date: NaiveDate) -> Result<i64, CommandError> {
        first_valid_instant_of_local_date(date)
    }

    pub fn local_date_for_timestamp(&self, utc_millis: i64) -> Result<NaiveDate, CommandError> {
        Local
            .timestamp_millis_opt(utc_millis)
            .earliest()
            .map(|value| value.date_naive())
            .ok_or_else(invalid_calendar_date)
    }

    pub fn local_date_string_for_timestamp(&self, utc_millis: i64) -> Result<String, CommandError> {
        Ok(self.local_date_for_timestamp(utc_millis)?.to_string())
    }

    pub fn add_local_days(&self, days: i64) -> Result<NaiveDate, CommandError> {
        if !(-MAX_CALENDAR_DAY_SPAN..=MAX_CALENDAR_DAY_SPAN).contains(&days) {
            return Err(invalid_calendar_date());
        }
        checked_add_days(self.local_date, days)
    }

    pub fn start_after_local_days(&self, days: i64) -> Result<i64, CommandError> {
        self.first_valid_instant(self.add_local_days(days)?)
    }

    pub fn resolve_calendar_date(&self, date: NaiveDate) -> Result<i64, CommandError> {
        if date < self.local_date {
            return Err(CommandError::new(
                "reschedule_date_in_past",
                "Choose today or a future local calendar date.",
            ));
        }
        if date == self.local_date {
            return Ok(self.now_utc_millis);
        }
        self.first_valid_instant(date)
    }

    pub fn trailing_local_dates(&self, count: u32) -> Result<CalendarWindow, CommandError> {
        if count == 0 || i64::from(count) > MAX_CALENDAR_DAY_SPAN {
            return Err(invalid_calendar_date());
        }
        let start_local_date = checked_add_days(self.local_date, 1 - i64::from(count))?;
        Ok(CalendarWindow {
            start_local_date,
            end_local_date: self.local_date,
            start_utc_millis: self.first_valid_instant(start_local_date)?,
            end_utc_millis: self.now_utc_millis.saturating_add(1),
        })
    }

    pub fn one_year_window(&self) -> Result<CalendarWindow, CommandError> {
        let start_local_date = previous_year_clamped(self.local_date)?;
        Ok(CalendarWindow {
            start_local_date,
            end_local_date: self.local_date,
            start_utc_millis: self.first_valid_instant(start_local_date)?,
            end_utc_millis: self.now_utc_millis.saturating_add(1),
        })
    }

    pub fn adjacent_previous_window(
        &self,
        current: &CalendarWindow,
    ) -> Result<CalendarWindow, CommandError> {
        let local_date_count = current
            .end_local_date
            .signed_duration_since(current.start_local_date)
            .num_days()
            .saturating_add(1);
        let end_local_date = checked_add_days(current.start_local_date, -1)?;
        let start_local_date = checked_add_days(end_local_date, 1 - local_date_count)?;
        Ok(CalendarWindow {
            start_local_date,
            end_local_date,
            start_utc_millis: self.first_valid_instant(start_local_date)?,
            end_utc_millis: self.first_valid_instant(current.start_local_date)?,
        })
    }

    pub fn month_start(&self, date: NaiveDate) -> Result<NaiveDate, CommandError> {
        NaiveDate::from_ymd_opt(date.year(), date.month(), 1).ok_or_else(invalid_calendar_date)
    }

    pub fn next_month_start(&self, date: NaiveDate) -> Result<NaiveDate, CommandError> {
        let (year, month) = if date.month() == 12 {
            (
                date.year()
                    .checked_add(1)
                    .ok_or_else(invalid_calendar_date)?,
                1,
            )
        } else {
            (date.year(), date.month() + 1)
        };
        NaiveDate::from_ymd_opt(year, month, 1).ok_or_else(invalid_calendar_date)
    }
}

/// Resolve the first real instant that belongs to a local calendar date.
///
/// Midnight is normally unique. Historical time zones can make midnight
/// ambiguous or skip it completely, so the resolver chooses the earlier
/// ambiguous instant and otherwise advances to the first valid local second.
pub fn first_valid_instant_of_local_date(date: NaiveDate) -> Result<i64, CommandError> {
    let midnight = date
        .and_hms_opt(0, 0, 0)
        .ok_or_else(invalid_calendar_date)?;
    for seconds in 0..SECONDS_PER_LOCAL_DATE {
        let local = midnight
            .checked_add_signed(Duration::seconds(seconds))
            .ok_or_else(invalid_calendar_date)?;
        if local.date() != date {
            break;
        }
        if let Some(value) = resolve_local_result(Local.from_local_datetime(&local)) {
            return Ok(value);
        }
    }
    Err(CommandError::new(
        "nonexistent_local_date",
        "That local calendar date has no valid instant in the current time zone.",
    ))
}

fn resolve_local_result(result: LocalResult<DateTime<Local>>) -> Option<i64> {
    match result {
        LocalResult::Single(value) => Some(value.timestamp_millis()),
        LocalResult::Ambiguous(first, second) => {
            Some(first.timestamp_millis().min(second.timestamp_millis()))
        }
        LocalResult::None => None,
    }
}

pub fn parse_local_date(value: &str) -> Result<NaiveDate, CommandError> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| {
        CommandError::new(
            "invalid_reschedule_date",
            "Choose a valid calendar date in YYYY-MM-DD form.",
        )
    })
}

fn discover_system_time_zone_id() -> String {
    if let Ok(value) = env::var("TZ") {
        if let Some(normalized) = normalize_zone_identifier(&value) {
            return normalized;
        }
    }

    #[cfg(unix)]
    {
        if let Ok(target) = std::fs::read_link("/etc/localtime") {
            let rendered = target.to_string_lossy();
            if let Some((_, suffix)) = rendered.rsplit_once("/zoneinfo/") {
                if let Some(normalized) = normalize_zone_identifier(suffix) {
                    return normalized;
                }
            }
        }
        if let Ok(value) = std::fs::read_to_string("/etc/timezone") {
            if let Some(normalized) = normalize_zone_identifier(value.trim()) {
                return normalized;
            }
        }
    }

    "unknown".to_string()
}

fn normalize_zone_identifier(value: &str) -> Option<String> {
    let trimmed = value.trim().trim_start_matches(':');
    if trimmed.is_empty()
        || trimmed.len() > 128
        || Path::new(trimmed).is_absolute()
        || trimmed.chars().any(|character| {
            character.is_control()
                || !(character.is_ascii_alphanumeric()
                    || matches!(character, '/' | '_' | '-' | '+' | '.'))
        })
    {
        return None;
    }
    Some(trimmed.to_string())
}

fn checked_add_days(date: NaiveDate, days: i64) -> Result<NaiveDate, CommandError> {
    date.checked_add_signed(Duration::days(days))
        .ok_or_else(invalid_calendar_date)
}

fn previous_year_clamped(date: NaiveDate) -> Result<NaiveDate, CommandError> {
    let year = date
        .year()
        .checked_sub(1)
        .ok_or_else(invalid_calendar_date)?;
    let mut day = date.day();
    loop {
        if let Some(value) = NaiveDate::from_ymd_opt(year, date.month(), day) {
            return Ok(value);
        }
        day = day.checked_sub(1).ok_or_else(invalid_calendar_date)?;
    }
}

fn invalid_calendar_date() -> CommandError {
    CommandError::new(
        "invalid_study_calendar_date",
        "The requested study calendar date is outside the supported range.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seven_day_window_contains_today_and_six_preceding_dates() {
        let now = unix_time_millis().expect("clock should resolve");
        let context = StudyTimeContext::capture_at(now).expect("context should resolve");
        let window = context
            .trailing_local_dates(7)
            .expect("window should resolve");
        assert_eq!(
            window
                .end_local_date
                .signed_duration_since(window.start_local_date)
                .num_days(),
            6
        );
        assert_eq!(window.end_local_date, context.local_date);
    }

    #[test]
    fn current_custom_date_resolves_to_now_not_elapsed_midnight() {
        let now = unix_time_millis().expect("clock should resolve");
        let context = StudyTimeContext::capture_at(now).expect("context should resolve");
        assert_eq!(
            context
                .resolve_calendar_date(context.local_date)
                .expect("today should resolve"),
            now
        );
    }

    #[test]
    fn leap_day_one_year_window_clamps_to_february_twenty_eighth() {
        let date = NaiveDate::from_ymd_opt(2024, 2, 29).expect("valid leap date");
        assert_eq!(
            previous_year_clamped(date).expect("previous year should resolve"),
            NaiveDate::from_ymd_opt(2023, 2, 28).expect("valid date")
        );
    }

    #[test]
    fn zone_identifier_rejects_paths_and_control_characters() {
        assert_eq!(
            normalize_zone_identifier("America/Los_Angeles"),
            Some("America/Los_Angeles".to_string())
        );
        assert_eq!(normalize_zone_identifier("/usr/share/zoneinfo/UTC"), None);
        assert_eq!(normalize_zone_identifier("bad\nzone"), None);
    }
}
