//! Authoritative local-calendar semantics for study commands.
//!
//! FSRS owns exact UTC due instants. This module owns the calendar interpretation
//! around those instants: local study dates, burial boundaries, date-only
//! rescheduling, and analytics windows. Every command captures one
//! [`StudyTimeContext`] and passes it through subordinate calculations so midnight
//! cannot split one logical operation into two study days.

use std::{env, fs, path::Path};

use chrono::{
    DateTime, Datelike, Duration, FixedOffset, Local, LocalResult, NaiveDate, NaiveDateTime,
    Offset, TimeZone, Utc,
};

use crate::{error::CommandError, util::unix_time_millis};

const SECONDS_PER_DAY: i64 = 86_400;
const MAX_CALENDAR_DAYS: i64 = 3_660;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StudyTimeContext {
    pub now_utc_millis: i64,
    pub time_zone_id: String,
    pub utc_offset_minutes: i32,
    pub local_date: NaiveDate,
    pub today_start_utc_millis: i64,
    pub tomorrow_start_utc_millis: i64,
    zone: CalendarZone,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CalendarWindow {
    pub start_local_date: NaiveDate,
    pub end_local_date: NaiveDate,
    pub start_utc_millis: i64,
    pub end_utc_millis: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum CalendarZone {
    System,
    Fixed(FixedOffset),
}

impl StudyTimeContext {
    pub fn capture() -> Result<Self, CommandError> {
        Self::capture_at(unix_time_millis()?)
    }

    pub fn capture_at(now_utc_millis: i64) -> Result<Self, CommandError> {
        Self::from_zone(
            now_utc_millis,
            discover_time_zone_id(),
            CalendarZone::System,
        )
    }

    #[cfg(test)]
    fn for_fixed_offset(
        now_utc_millis: i64,
        offset_seconds: i32,
    ) -> Result<Self, CommandError> {
        let offset = FixedOffset::east_opt(offset_seconds).ok_or_else(invalid_calendar_date)?;
        Self::from_zone(
            now_utc_millis,
            format!("fixed:{offset_seconds}"),
            CalendarZone::Fixed(offset),
        )
    }

    fn from_zone(
        now_utc_millis: i64,
        time_zone_id: String,
        zone: CalendarZone,
    ) -> Result<Self, CommandError> {
        let local_date = zone.local_date_at(now_utc_millis)?;
        let today_start_utc_millis = zone.first_valid_instant(local_date)?;
        let tomorrow = checked_add_days(local_date, 1)?;
        let tomorrow_start_utc_millis = zone.first_valid_instant(tomorrow)?;
        let utc_offset_minutes = zone.offset_minutes_at(now_utc_millis)?;
        Ok(Self {
            now_utc_millis,
            time_zone_id,
            utc_offset_minutes,
            local_date,
            today_start_utc_millis,
            tomorrow_start_utc_millis,
            zone,
        })
    }

    pub fn first_valid_instant(&self, date: NaiveDate) -> Result<i64, CommandError> {
        self.zone.first_valid_instant(date)
    }

    pub fn local_date_for_timestamp(
        &self,
        utc_millis: i64,
    ) -> Result<NaiveDate, CommandError> {
        self.zone.local_date_at(utc_millis)
    }

    pub fn local_date_string_for_timestamp(
        &self,
        utc_millis: i64,
    ) -> Result<String, CommandError> {
        Ok(self.local_date_for_timestamp(utc_millis)?.to_string())
    }

    pub fn add_local_days(&self, days: i64) -> Result<NaiveDate, CommandError> {
        if !(-MAX_CALENDAR_DAYS..=MAX_CALENDAR_DAYS).contains(&days) {
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
        if count == 0 || i64::from(count) > MAX_CALENDAR_DAYS {
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
        let local_day_count = current
            .end_local_date
            .signed_duration_since(current.start_local_date)
            .num_days()
            .saturating_add(1);
        let end_local_date = checked_add_days(current.start_local_date, -1)?;
        let start_local_date = checked_add_days(end_local_date, 1 - local_day_count)?;
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
            (date.year().checked_add(1).ok_or_else(invalid_calendar_date)?, 1)
        } else {
            (date.year(), date.month() + 1)
        };
        NaiveDate::from_ymd_opt(year, month, 1).ok_or_else(invalid_calendar_date)
    }
}

impl CalendarZone {
    fn utc_datetime(utc_millis: i64) -> Result<DateTime<Utc>, CommandError> {
        DateTime::<Utc>::from_timestamp_millis(utc_millis).ok_or_else(invalid_calendar_date)
    }

    fn local_date_at(&self, utc_millis: i64) -> Result<NaiveDate, CommandError> {
        let utc = Self::utc_datetime(utc_millis)?;
        Ok(match self {
            Self::System => utc.with_timezone(&Local).date_naive(),
            Self::Fixed(offset) => utc.with_timezone(offset).date_naive(),
        })
    }

    fn offset_minutes_at(&self, utc_millis: i64) -> Result<i32, CommandError> {
        let utc = Self::utc_datetime(utc_millis)?;
        let seconds = match self {
            Self::System => utc.with_timezone(&Local).offset().fix().local_minus_utc(),
            Self::Fixed(offset) => offset.local_minus_utc(),
        };
        Ok(seconds.div_euclid(60))
    }

    fn first_valid_instant(&self, date: NaiveDate) -> Result<i64, CommandError> {
        match self {
            Self::System => first_valid_instant_with(date, |local| {
                Local.from_local_datetime(&local).map(|value| value.timestamp_millis())
            }),
            Self::Fixed(offset) => first_valid_instant_with(date, |local| {
                offset
                    .from_local_datetime(&local)
                    .map(|value| value.timestamp_millis())
            }),
        }
    }
}

fn first_valid_instant_with(
    date: NaiveDate,
    mut resolve: impl FnMut(NaiveDateTime) -> LocalResult<i64>,
) -> Result<i64, CommandError> {
    let midnight = date
        .and_hms_opt(0, 0, 0)
        .ok_or_else(invalid_calendar_date)?;
    // Most zones resolve on the first iteration. The bounded scan is exercised
    // only for historical zones whose midnight was skipped and correctly handles
    // non-hour transitions without substituting noon as a fake day boundary.
    for seconds in 0..SECONDS_PER_DAY {
        let local = midnight
            .checked_add_signed(Duration::seconds(seconds))
            .ok_or_else(invalid_calendar_date)?;
        if local.date() != date {
            break;
        }
        match resolve(local) {
            LocalResult::Single(value) => return Ok(value),
            LocalResult::Ambiguous(first, second) => return Ok(first.min(second)),
            LocalResult::None => {}
        }
    }
    Err(CommandError::new(
        "nonexistent_local_date",
        "That local calendar date has no valid instant in the current time zone.",
    ))
}

pub fn parse_local_date(value: &str) -> Result<NaiveDate, CommandError> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| {
        CommandError::new(
            "invalid_reschedule_date",
            "Choose a valid calendar date in YYYY-MM-DD form.",
        )
    })
}

fn checked_add_days(date: NaiveDate, days: i64) -> Result<NaiveDate, CommandError> {
    date.checked_add_signed(Duration::days(days))
        .ok_or_else(invalid_calendar_date)
}

fn previous_year_clamped(date: NaiveDate) -> Result<NaiveDate, CommandError> {
    let year = date.year().checked_sub(1).ok_or_else(invalid_calendar_date)?;
    let mut day = date.day();
    loop {
        if let Some(value) = NaiveDate::from_ymd_opt(year, date.month(), day) {
            return Ok(value);
        }
        day = day.checked_sub(1).ok_or_else(invalid_calendar_date)?;
    }
}

fn discover_time_zone_id() -> String {
    if let Ok(value) = env::var("TZ") {
        let trimmed = value.trim().trim_start_matches(':');
        if is_plausible_zone_id(trimmed) {
            return trimmed.to_string();
        }
    }

    #[cfg(unix)]
    {
        if let Ok(value) = fs::read_to_string("/etc/timezone") {
            let trimmed = value.trim();
            if is_plausible_zone_id(trimmed) {
                return trimmed.to_string();
            }
        }
        if let Ok(target) = fs::read_link("/etc/localtime") {
            if let Some(value) = zone_id_from_zoneinfo_path(&target) {
                return value;
            }
        }
    }

    "unknown".to_string()
}

fn is_plausible_zone_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 255
        && value.contains('/')
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "_+-/".contains(character))
}

#[cfg(unix)]
fn zone_id_from_zoneinfo_path(path: &Path) -> Option<String> {
    let value = path.to_string_lossy();
    let (_, suffix) = value.split_once("/zoneinfo/")?;
    is_plausible_zone_id(suffix).then(|| suffix.to_string())
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

    fn timestamp(value: &str) -> i64 {
        DateTime::parse_from_rfc3339(value)
            .expect("test timestamp should parse")
            .timestamp_millis()
    }

    #[test]
    fn current_date_resolves_to_now_not_elapsed_midnight() {
        let now = timestamp("2026-08-05T18:30:00Z");
        let context = StudyTimeContext::for_fixed_offset(now, -7 * 3_600)
            .expect("fixed offset should resolve");
        assert_eq!(
            context
                .resolve_calendar_date(context.local_date)
                .expect("today should resolve"),
            now
        );
    }

    #[test]
    fn leap_day_one_year_window_clamps_to_previous_february() {
        let context = StudyTimeContext::for_fixed_offset(
            timestamp("2024-02-29T20:00:00Z"),
            -8 * 3_600,
        )
        .expect("fixed offset should resolve");
        assert_eq!(
            context
                .one_year_window()
                .expect("window should resolve")
                .start_local_date,
            NaiveDate::from_ymd_opt(2023, 2, 28).expect("valid date")
        );
    }

    #[test]
    fn first_valid_instant_uses_earlier_ambiguous_occurrence() {
        let date = NaiveDate::from_ymd_opt(2026, 11, 1).expect("valid date");
        let instant = first_valid_instant_with(date, |_| LocalResult::Ambiguous(200, 100))
            .expect("ambiguous instant should resolve");
        assert_eq!(instant, 100);
    }

    #[test]
    fn first_valid_instant_advances_through_a_nonexistent_midnight() {
        let date = NaiveDate::from_ymd_opt(2026, 3, 8).expect("valid date");
        let instant = first_valid_instant_with(date, |local| {
            if local.time() < chrono::NaiveTime::from_hms_opt(1, 0, 0).expect("valid time") {
                LocalResult::None
            } else {
                LocalResult::Single(123)
            }
        })
        .expect("first valid instant should resolve");
        assert_eq!(instant, 123);
    }

    #[test]
    fn adjacent_previous_window_has_the_same_local_date_count() {
        let context = StudyTimeContext::for_fixed_offset(
            timestamp("2026-08-05T18:30:00Z"),
            -7 * 3_600,
        )
        .expect("fixed offset should resolve");
        let current = context.trailing_local_dates(7).expect("window should resolve");
        let previous = context
            .adjacent_previous_window(&current)
            .expect("previous window should resolve");
        assert_eq!(
            current
                .end_local_date
                .signed_duration_since(current.start_local_date)
                .num_days(),
            previous
                .end_local_date
                .signed_duration_since(previous.start_local_date)
                .num_days()
        );
    }
}
