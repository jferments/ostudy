//! One captured local-calendar context for scheduling, workload, and Analytics.
//!
//! FSRS continues to own exact due instants. This module only resolves product
//! concepts expressed as local dates: study-day boundaries, burial deadlines,
//! date-only manual rescheduling, and historical event attribution.

use chrono::{
    DateTime, Datelike, Duration, FixedOffset, Local, LocalResult, NaiveDate, NaiveDateTime,
    Offset, TimeZone, Utc,
};
use chrono_tz::Tz;

use crate::{error::CommandError, util::unix_time_millis};

const MAX_DATE_START_SEARCH_SECONDS: i64 = 86_400;

#[derive(Clone, Debug)]
enum StudyZone {
    Named(Tz),
    Fixed(FixedOffset),
}

#[derive(Clone, Debug)]
pub struct StudyTimeContext {
    pub now_utc_millis: i64,
    pub time_zone_id: String,
    pub utc_offset_minutes: i32,
    pub local_date: NaiveDate,
    pub today_start_utc_millis: i64,
    pub tomorrow_start_utc_millis: i64,
    zone: StudyZone,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CalendarWindow {
    pub start_local_date: NaiveDate,
    pub end_local_date: NaiveDate,
    pub start_utc_millis: i64,
    pub end_utc_millis: i64,
}

impl StudyTimeContext {
    pub fn capture() -> Result<Self, CommandError> {
        Self::at(unix_time_millis()?)
    }

    pub fn at(now_utc_millis: i64) -> Result<Self, CommandError> {
        let named = iana_time_zone::get_timezone()
            .ok()
            .and_then(|name| name.parse::<Tz>().ok().map(|zone| (name, zone)));
        match named {
            Some((name, zone)) => Self::with_zone(now_utc_millis, name, StudyZone::Named(zone)),
            None => {
                let offset = Local::now().offset().fix();
                Self::with_zone(
                    now_utc_millis,
                    "unknown".to_string(),
                    StudyZone::Fixed(offset),
                )
            }
        }
    }

    #[cfg(test)]
    pub fn at_in_zone(now_utc_millis: i64, zone_id: &str) -> Result<Self, CommandError> {
        let zone = zone_id.parse::<Tz>().map_err(|_| {
            CommandError::new(
                "invalid_time_zone",
                format!("The test time zone {zone_id} is invalid."),
            )
        })?;
        Self::with_zone(
            now_utc_millis,
            zone_id.to_string(),
            StudyZone::Named(zone),
        )
    }

    fn with_zone(
        now_utc_millis: i64,
        time_zone_id: String,
        zone: StudyZone,
    ) -> Result<Self, CommandError> {
        let utc = utc_datetime(now_utc_millis)?;
        let (local_date, utc_offset_minutes) = local_date_and_offset(&zone, utc);
        let today_start_utc_millis = resolve_date_start_in_zone(&zone, local_date)?;
        let tomorrow = local_date.succ_opt().ok_or_else(|| {
            CommandError::new(
                "invalid_local_date",
                "Tomorrow is outside the supported calendar range.",
            )
        })?;
        let tomorrow_start_utc_millis = resolve_date_start_in_zone(&zone, tomorrow)?;
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

    pub fn local_date_string(&self) -> String {
        self.local_date.format("%Y-%m-%d").to_string()
    }

    pub fn local_date_at(&self, utc_millis: i64) -> Result<NaiveDate, CommandError> {
        let utc = utc_datetime(utc_millis)?;
        Ok(local_date_and_offset(&self.zone, utc).0)
    }

    pub fn event_attribution(
        &self,
        utc_millis: i64,
    ) -> Result<(String, i32, String), CommandError> {
        let utc = utc_datetime(utc_millis)?;
        let (date, offset) = local_date_and_offset(&self.zone, utc);
        Ok((
            date.format("%Y-%m-%d").to_string(),
            offset,
            self.time_zone_id.clone(),
        ))
    }

    pub fn resolve_date_start(&self, date: NaiveDate) -> Result<i64, CommandError> {
        resolve_date_start_in_zone(&self.zone, date)
    }

    pub fn resolve_date_or_now(&self, date: NaiveDate) -> Result<i64, CommandError> {
        if date < self.local_date {
            return Err(CommandError::new(
                "reschedule_date_in_past",
                "Choose today or a future local calendar date.",
            ));
        }
        if date == self.local_date {
            return Ok(self.now_utc_millis);
        }
        self.resolve_date_start(date)
    }

    pub fn parse_local_date(&self, value: &str) -> Result<NaiveDate, CommandError> {
        let date = NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| {
            CommandError::new(
                "invalid_calendar_date",
                "Choose a valid local calendar date in YYYY-MM-DD format.",
            )
        })?;
        if date.format("%Y-%m-%d").to_string() != value {
            return Err(CommandError::new(
                "invalid_calendar_date",
                "Choose a valid local calendar date in YYYY-MM-DD format.",
            ));
        }
        Ok(date)
    }

    pub fn date_after_days(&self, days: i64) -> Result<NaiveDate, CommandError> {
        self.local_date.checked_add_signed(Duration::days(days)).ok_or_else(|| {
            CommandError::new(
                "invalid_local_date",
                "The requested local calendar date is outside the supported range.",
            )
        })
    }

    pub fn calendar_days_window(&self, days: i64) -> Result<CalendarWindow, CommandError> {
        if days < 1 {
            return Err(CommandError::new(
                "invalid_calendar_window",
                "A calendar window must include at least one local date.",
            ));
        }
        let start_local_date = self
            .local_date
            .checked_sub_signed(Duration::days(days - 1))
            .ok_or_else(|| {
                CommandError::new(
                    "invalid_calendar_window",
                    "The requested calendar window is outside the supported range.",
                )
            })?;
        Ok(CalendarWindow {
            start_local_date,
            end_local_date: self.local_date,
            start_utc_millis: self.resolve_date_start(start_local_date)?,
            end_utc_millis: self.now_utc_millis,
        })
    }

    pub fn one_year_window(&self) -> Result<CalendarWindow, CommandError> {
        let previous_year = self.local_date.year().checked_sub(1).ok_or_else(|| {
            CommandError::new(
                "invalid_calendar_window",
                "The requested year is outside the supported range.",
            )
        })?;
        let start_local_date =
            clamped_date(previous_year, self.local_date.month(), self.local_date.day())?;
        Ok(CalendarWindow {
            start_local_date,
            end_local_date: self.local_date,
            start_utc_millis: self.resolve_date_start(start_local_date)?,
            end_utc_millis: self.now_utc_millis,
        })
    }

    pub fn previous_window(
        &self,
        current: CalendarWindow,
    ) -> Result<CalendarWindow, CommandError> {
        let date_count = current
            .end_local_date
            .signed_duration_since(current.start_local_date)
            .num_days()
            .saturating_add(1);
        let end_local_date = current.start_local_date.pred_opt().ok_or_else(|| {
            CommandError::new(
                "invalid_calendar_window",
                "The previous calendar window is outside the supported range.",
            )
        })?;
        let start_local_date = end_local_date
            .checked_sub_signed(Duration::days(date_count.saturating_sub(1)))
            .ok_or_else(|| {
                CommandError::new(
                    "invalid_calendar_window",
                    "The previous calendar window is outside the supported range.",
                )
            })?;
        let next_date = end_local_date.succ_opt().ok_or_else(|| {
            CommandError::new(
                "invalid_calendar_window",
                "The previous calendar window is outside the supported range.",
            )
        })?;
        Ok(CalendarWindow {
            start_local_date,
            end_local_date,
            start_utc_millis: self.resolve_date_start(start_local_date)?,
            end_utc_millis: self.resolve_date_start(next_date)?.saturating_sub(1),
        })
    }

    pub fn local_day_sort_key(&self, utc_millis: i64) -> Result<i32, CommandError> {
        let date = self.local_date_at(utc_millis)?;
        i32::try_from(date.signed_duration_since(self.local_date).num_days()).map_err(|_| {
            CommandError::new(
                "invalid_local_date",
                "The local calendar distance is outside the supported range.",
            )
        })
    }
}

fn utc_datetime(value: i64) -> Result<DateTime<Utc>, CommandError> {
    DateTime::<Utc>::from_timestamp_millis(value).ok_or_else(|| {
        CommandError::new(
            "invalid_timestamp",
            "A stored timestamp is outside the supported calendar range.",
        )
    })
}

fn local_date_and_offset(zone: &StudyZone, utc: DateTime<Utc>) -> (NaiveDate, i32) {
    match zone {
        StudyZone::Named(zone) => {
            let local = utc.with_timezone(zone);
            (local.date_naive(), local.offset().fix().local_minus_utc() / 60)
        }
        StudyZone::Fixed(offset) => {
            let local = utc.with_timezone(offset);
            (local.date_naive(), offset.local_minus_utc() / 60)
        }
    }
}

fn resolve_date_start_in_zone(
    zone: &StudyZone,
    date: NaiveDate,
) -> Result<i64, CommandError> {
    let midnight = date.and_hms_opt(0, 0, 0).ok_or_else(|| {
        CommandError::new("invalid_local_date", "The local calendar date is invalid.")
    })?;
    for seconds in 0..=MAX_DATE_START_SEARCH_SECONDS {
        let candidate = midnight
            .checked_add_signed(Duration::seconds(seconds))
            .ok_or_else(|| {
                CommandError::new(
                    "invalid_local_date",
                    "The local calendar date is outside the supported range.",
                )
            })?;
        match resolve_local(zone, candidate) {
            LocalResult::Single(value) => return Ok(value.timestamp_millis()),
            LocalResult::Ambiguous(first, second) => {
                return Ok(first.timestamp_millis().min(second.timestamp_millis()));
            }
            LocalResult::None => {}
        }
    }
    Err(CommandError::new(
        "invalid_local_date",
        "No valid instant exists for the requested local calendar date.",
    ))
}

fn resolve_local(
    zone: &StudyZone,
    candidate: NaiveDateTime,
) -> LocalResult<DateTime<FixedOffset>> {
    match zone {
        StudyZone::Named(zone) => match zone.from_local_datetime(&candidate) {
            LocalResult::Single(value) => LocalResult::Single(value.fixed_offset()),
            LocalResult::Ambiguous(first, second) => {
                LocalResult::Ambiguous(first.fixed_offset(), second.fixed_offset())
            }
            LocalResult::None => LocalResult::None,
        },
        StudyZone::Fixed(offset) => offset.from_local_datetime(&candidate),
    }
}

fn clamped_date(year: i32, month: u32, day: u32) -> Result<NaiveDate, CommandError> {
    for candidate_day in (1..=day).rev() {
        if let Some(date) = NaiveDate::from_ymd_opt(year, month, candidate_day) {
            return Ok(date);
        }
    }
    Err(CommandError::new(
        "invalid_calendar_date",
        "The requested local calendar date is invalid.",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn millis(value: &str) -> i64 {
        DateTime::parse_from_rfc3339(value)
            .expect("valid timestamp")
            .timestamp_millis()
    }

    #[test]
    fn spring_transition_uses_local_dates_not_twenty_four_hour_math() {
        let context =
            StudyTimeContext::at_in_zone(millis("2026-03-08T17:00:00Z"), "America/Los_Angeles")
                .expect("context");
        assert_eq!(context.local_date_string(), "2026-03-08");
        assert_eq!(
            context.tomorrow_start_utc_millis - context.today_start_utc_millis,
            23 * 60 * 60 * 1000
        );
    }

    #[test]
    fn autumn_transition_has_a_twenty_five_hour_local_date() {
        let context =
            StudyTimeContext::at_in_zone(millis("2026-11-01T18:00:00Z"), "America/Los_Angeles")
                .expect("context");
        assert_eq!(
            context.tomorrow_start_utc_millis - context.today_start_utc_millis,
            25 * 60 * 60 * 1000
        );
    }

    #[test]
    fn current_custom_date_resolves_to_now_and_past_dates_fail() {
        let context =
            StudyTimeContext::at_in_zone(millis("2026-06-10T18:00:00Z"), "America/Los_Angeles")
                .expect("context");
        assert_eq!(
            context
                .resolve_date_or_now(NaiveDate::from_ymd_opt(2026, 6, 10).expect("date"))
                .expect("today"),
            context.now_utc_millis
        );
        assert_eq!(
            context
                .resolve_date_or_now(NaiveDate::from_ymd_opt(2026, 6, 9).expect("date"))
                .expect_err("past date")
                .code,
            "reschedule_date_in_past"
        );
    }

    #[test]
    fn one_year_window_clamps_leap_day() {
        let context =
            StudyTimeContext::at_in_zone(millis("2028-02-29T20:00:00Z"), "UTC").expect("context");
        let window = context.one_year_window().expect("window");
        assert_eq!(
            window.start_local_date,
            NaiveDate::from_ymd_opt(2027, 2, 28).expect("date")
        );
    }
}
