//! Authoritative study-calendar semantics for study commands.
//!
//! FSRS continues to own exact due instants. This module owns the calendar
//! interpretation around those instants: study dates, burial boundaries,
//! date-only rescheduling, daily limits, and analytics windows. Every command
//! captures one [`StudyTimeContext`] and passes it through subordinate
//! calculations.
//!
//! A study day starts at the learner's "New day starts at" hour of local wall
//! time (4 a.m. unless changed in Settings), so a review at 01:30 still belongs
//! to the previous study date. Review attributions keep the calendar date,
//! zone and UTC offset of the moment they were saved (ADR 0010); a review's
//! study date is derived when it is read, from that stored instant and offset
//! (ADR 0054). Changing the hour therefore re-buckets history without
//! rewriting it, and a review saved while travelling keeps the offset of the
//! place where it was made.

use chrono::{DateTime, Datelike, Duration, NaiveDate, Utc};
use rusqlite::{types::Value as SqlValue, Connection};

use crate::{error::CommandError, projections::scope::push_value, util::unix_time_millis};

/// The "New day starts at" hour for a learner who has not chosen one.
pub const DEFAULT_NEW_DAY_START_HOUR: u32 = 4;

const MAX_CALENDAR_DAY_SPAN: i64 = 3_660;
const MILLIS_PER_MINUTE: i64 = 60_000;
const MILLIS_PER_HOUR: i64 = 3_600_000;
const MILLIS_PER_DAY: i64 = 86_400_000;
/// Farther from a local wall time, read as UTC, than any UTC offset reaches.
const OFFSET_PROBE_MILLIS: i64 = 15 * MILLIS_PER_HOUR;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StudyTimeContext {
    pub now_utc_millis: i64,
    pub time_zone_id: String,
    pub utc_offset_minutes: i32,
    /// The current study date: the local date of now, `new_day_start_hour`
    /// hours earlier. Between midnight and that hour it is yesterday's date.
    pub local_date: NaiveDate,
    /// When the current study date began: local wall time `H:00` on it.
    pub today_start_utc_millis: i64,
    /// When the next study date begins.
    pub tomorrow_start_utc_millis: i64,
    new_day_start_hour: u32,
    zone: StudyZoneResolver,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CalendarWindow {
    pub start_local_date: NaiveDate,
    pub end_local_date: NaiveDate,
    pub start_utc_millis: i64,
    pub end_utc_millis: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum StudyZoneResolver {
    System,
    #[cfg(test)]
    Pacific2026,
}

impl StudyZoneResolver {
    fn identifier(&self, now_utc_millis: i64) -> Result<String, CommandError> {
        match self {
            Self::System => crate::platform::time_zone_snapshot(now_utc_millis)
                .map(|snapshot| snapshot.identifier)
                .map_err(platform_calendar_error),
            #[cfg(test)]
            Self::Pacific2026 => Ok("America/Los_Angeles".to_string()),
        }
    }

    fn local_date_at(&self, utc_millis: i64) -> Result<NaiveDate, CommandError> {
        match self {
            Self::System => {
                crate::platform::local_date_at(utc_millis).map_err(platform_calendar_error)
            }
            #[cfg(test)]
            Self::Pacific2026 => synthetic_pacific_local_date(utc_millis),
        }
    }

    fn offset_minutes_at(&self, utc_millis: i64) -> Result<i32, CommandError> {
        match self {
            Self::System => crate::platform::time_zone_snapshot(utc_millis)
                .map(|snapshot| snapshot.utc_offset_minutes)
                .map_err(platform_calendar_error),
            #[cfg(test)]
            Self::Pacific2026 => Ok(synthetic_pacific_offset_at_utc(utc_millis)),
        }
    }

    fn first_valid_instant(&self, date: NaiveDate) -> Result<i64, CommandError> {
        match self {
            Self::System => {
                crate::platform::first_valid_instant(date).map_err(platform_calendar_error)
            }
            #[cfg(test)]
            Self::Pacific2026 => synthetic_pacific_first_instant(date),
        }
    }
}

impl StudyTimeContext {
    /// Captures now in the device's time zone with the learner's saved
    /// "New day starts at" hour. Taking the connection, rather than reading a
    /// process-wide value, keeps the setting tied to the library the command
    /// works on and makes every capture site visibly read it.
    pub fn capture(connection: &Connection) -> Result<Self, CommandError> {
        let new_day_start_hour = crate::settings::load_new_day_start_hour(connection)?;
        Self::capture_at(unix_time_millis()?, new_day_start_hour)
    }

    pub fn capture_at(now_utc_millis: i64, new_day_start_hour: u32) -> Result<Self, CommandError> {
        Self::from_zone(
            now_utc_millis,
            new_day_start_hour,
            StudyZoneResolver::System,
        )
    }

    /// A context in the synthetic 2026 Pacific zone, so calendar tests do not
    /// depend on the time zone of the machine running them.
    #[cfg(test)]
    pub(crate) fn pacific_2026_at(
        now_utc_millis: i64,
        new_day_start_hour: u32,
    ) -> Result<Self, CommandError> {
        Self::from_zone(
            now_utc_millis,
            new_day_start_hour,
            StudyZoneResolver::Pacific2026,
        )
    }

    fn from_zone(
        now_utc_millis: i64,
        new_day_start_hour: u32,
        zone: StudyZoneResolver,
    ) -> Result<Self, CommandError> {
        validate_new_day_start_hour(i64::from(new_day_start_hour))?;
        let local_date = study_date_at(&zone, new_day_start_hour, now_utc_millis)?;
        let today_start_utc_millis = study_day_start_in(&zone, new_day_start_hour, local_date)?;
        let tomorrow = checked_add_days(local_date, 1)?;
        let tomorrow_start_utc_millis = study_day_start_in(&zone, new_day_start_hour, tomorrow)?;
        Ok(Self {
            now_utc_millis,
            time_zone_id: zone.identifier(now_utc_millis)?,
            utc_offset_minutes: zone.offset_minutes_at(now_utc_millis)?,
            local_date,
            today_start_utc_millis,
            tomorrow_start_utc_millis,
            new_day_start_hour,
            zone,
        })
    }

    /// The local hour (0 to 23) at which a study date begins.
    pub fn new_day_start_hour(&self) -> u32 {
        self.new_day_start_hour
    }

    /// The instant study date `date` begins: local wall time `H:00` on `date`.
    /// A wall time skipped by a daylight-saving change resolves to the first
    /// instant after it, a repeated one to its first occurrence.
    pub fn study_day_start(&self, date: NaiveDate) -> Result<i64, CommandError> {
        study_day_start_in(&self.zone, self.new_day_start_hour, date)
    }

    /// The study date an instant belongs to in the captured zone.
    pub fn study_date_for_timestamp(&self, utc_millis: i64) -> Result<NaiveDate, CommandError> {
        study_date_at(&self.zone, self.new_day_start_hour, utc_millis)
    }

    /// The local calendar date of an instant, whatever hour the study day
    /// starts. Review attributions store this date.
    pub fn calendar_date_for_timestamp(&self, utc_millis: i64) -> Result<NaiveDate, CommandError> {
        self.zone.local_date_at(utc_millis)
    }

    pub fn calendar_date_string_for_timestamp(
        &self,
        utc_millis: i64,
    ) -> Result<String, CommandError> {
        Ok(self.calendar_date_for_timestamp(utc_millis)?.to_string())
    }

    pub fn add_local_days(&self, days: i64) -> Result<NaiveDate, CommandError> {
        if !(-MAX_CALENDAR_DAY_SPAN..=MAX_CALENDAR_DAY_SPAN).contains(&days) {
            return Err(invalid_calendar_date());
        }
        checked_add_days(self.local_date, days)
    }

    pub fn start_after_local_days(&self, days: i64) -> Result<i64, CommandError> {
        self.study_day_start(self.add_local_days(days)?)
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
        self.study_day_start(date)
    }

    pub fn trailing_local_dates(&self, count: u32) -> Result<CalendarWindow, CommandError> {
        if count == 0 || i64::from(count) > MAX_CALENDAR_DAY_SPAN {
            return Err(invalid_calendar_date());
        }
        let start_local_date = checked_add_days(self.local_date, 1 - i64::from(count))?;
        Ok(CalendarWindow {
            start_local_date,
            end_local_date: self.local_date,
            start_utc_millis: self.study_day_start(start_local_date)?,
            end_utc_millis: self.now_utc_millis.saturating_add(1),
        })
    }

    pub fn one_year_window(&self) -> Result<CalendarWindow, CommandError> {
        let start_local_date = previous_year_clamped(self.local_date)?;
        Ok(CalendarWindow {
            start_local_date,
            end_local_date: self.local_date,
            start_utc_millis: self.study_day_start(start_local_date)?,
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
            start_utc_millis: self.study_day_start(start_local_date)?,
            end_utc_millis: self.study_day_start(current.start_local_date)?,
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

    /// SQL limiting review attributions `alias` to study dates `start..=end`
    /// (an absent bound is open), appending its values. The stored calendar
    /// date selects rows through `review_event_attributions_local_date_idx`,
    /// widened by one date when the day starts after midnight because a study
    /// date also holds the first hours of the next calendar date. The stored
    /// wall-clock time then decides exactly. That comparison is the integer
    /// form of [`Self::review_study_date_sql`], so no date is formatted per row.
    pub fn review_study_dates_sql(
        &self,
        alias: &str,
        start: Option<NaiveDate>,
        end: Option<NaiveDate>,
        values: &mut Vec<SqlValue>,
    ) -> Result<String, CommandError> {
        let wall = review_wall_clock_sql(alias);
        let mut conditions = Vec::new();
        if let Some(start) = start {
            let date = push_value(values, start.to_string());
            let first = push_value(values, self.study_date_wall_clock_start(start)?);
            conditions.push(format!("{alias}.review_local_date >= ?{date}"));
            conditions.push(format!("{wall} >= ?{first}"));
        }
        if let Some(end) = end {
            let next = checked_add_days(end, 1)?;
            let last_calendar_date = if self.new_day_start_hour == 0 {
                end
            } else {
                next
            };
            let date = push_value(values, last_calendar_date.to_string());
            let limit = push_value(values, self.study_date_wall_clock_start(next)?);
            conditions.push(format!("{alias}.review_local_date <= ?{date}"));
            conditions.push(format!("{wall} < ?{limit}"));
        }
        Ok(if conditions.is_empty() {
            "1 = 1".to_string()
        } else {
            conditions.join(" AND ")
        })
    }

    /// SQL for the study date of review attribution `alias`, from its stored
    /// instant and offset. It equals
    /// `date(a.reviewed_at / 1000 + a.utc_offset_minutes * 60 - H * 3600, 'unixepoch')`.
    pub fn review_study_date_sql(&self, alias: &str) -> String {
        self.wall_clock_study_date_sql(&review_wall_clock_sql(alias))
    }

    /// SQL for the earliest study date among the aggregated review rows,
    /// formatting one date rather than one per row.
    pub fn earliest_review_study_date_sql(&self, alias: &str) -> String {
        self.wall_clock_study_date_sql(&format!("MIN({})", review_wall_clock_sql(alias)))
    }

    /// SQL for the study date of a wall-clock expression from
    /// [`review_wall_clock_sql`].
    pub fn wall_clock_study_date_sql(&self, wall_clock: &str) -> String {
        format!(
            "date(({wall_clock} - {}) / 1000, 'unixepoch')",
            self.hour_millis()
        )
    }

    /// SQL numbering the study date of review attribution `alias` in periods
    /// of `days` study dates counted from `start`, appending its value. Only
    /// meaningful for reviews on or after `start`.
    pub fn review_study_period_sql(
        &self,
        alias: &str,
        start: NaiveDate,
        days: i64,
        values: &mut Vec<SqlValue>,
    ) -> Result<String, CommandError> {
        let period = days
            .checked_mul(MILLIS_PER_DAY)
            .filter(|period| *period > 0)
            .ok_or_else(invalid_calendar_date)?;
        let first = push_value(values, self.study_date_wall_clock_start(start)?);
        Ok(format!(
            "({} - ?{first}) / {period}",
            review_wall_clock_sql(alias)
        ))
    }

    /// SQL for the study date of an instant column (UTC milliseconds), read
    /// with the offset in force now. It orders due cards only: a due instant
    /// across a daylight-saving change from now can land one study date off
    /// within an hour of the boundary, so counting never uses it.
    pub fn instant_study_date_sql(&self, column: &str) -> String {
        format!(
            "date(({column} + ({}) - {}) / 1000, 'unixepoch')",
            i64::from(self.utc_offset_minutes) * MILLIS_PER_MINUTE,
            self.hour_millis()
        )
    }

    fn hour_millis(&self) -> i64 {
        i64::from(self.new_day_start_hour) * MILLIS_PER_HOUR
    }

    /// The local wall-clock time, read as UTC milliseconds, at which study
    /// date `date` begins.
    fn study_date_wall_clock_start(&self, date: NaiveDate) -> Result<i64, CommandError> {
        wall_clock_millis(date, self.new_day_start_hour)
    }
}

/// Accepts a "New day starts at" hour: a whole hour from 0 (midnight) to 23.
pub fn validate_new_day_start_hour(value: i64) -> Result<u32, CommandError> {
    u32::try_from(value)
        .ok()
        .filter(|hour| *hour <= 23)
        .ok_or_else(|| {
            CommandError::new(
                "invalid_new_day_start_hour",
                "The new day must start on a whole hour from 12 a.m. through 11 p.m.",
            )
        })
}

pub fn parse_local_date(value: &str) -> Result<NaiveDate, CommandError> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| {
        CommandError::new(
            "invalid_reschedule_date",
            "Choose a valid calendar date in YYYY-MM-DD form.",
        )
    })
}

/// SQL for review attribution `alias`'s local wall-clock time, read as UTC
/// milliseconds: its stored instant plus the offset it was saved with.
pub fn review_wall_clock_sql(alias: &str) -> String {
    format!("({alias}.reviewed_at + {alias}.utc_offset_minutes * {MILLIS_PER_MINUTE})")
}

/// The study date of an instant: the local date of its wall time `hour`
/// hours earlier. Midnight keeps the platform's own local date.
fn study_date_at(
    zone: &StudyZoneResolver,
    hour: u32,
    utc_millis: i64,
) -> Result<NaiveDate, CommandError> {
    if hour == 0 {
        return zone.local_date_at(utc_millis);
    }
    let offset = i64::from(zone.offset_minutes_at(utc_millis)?) * MILLIS_PER_MINUTE;
    let wall = utc_millis
        .checked_add(offset)
        .and_then(|wall| wall.checked_sub(i64::from(hour) * MILLIS_PER_HOUR))
        .ok_or_else(invalid_calendar_date)?;
    DateTime::<Utc>::from_timestamp_millis(wall)
        .map(|value| value.date_naive())
        .ok_or_else(invalid_calendar_date)
}

/// The instant study date `date` begins. Midnight keeps the platform's own
/// first-valid-instant rule, so a midnight boundary behaves exactly as local
/// calendar dates always did.
fn study_day_start_in(
    zone: &StudyZoneResolver,
    hour: u32,
    date: NaiveDate,
) -> Result<i64, CommandError> {
    if hour == 0 {
        return zone.first_valid_instant(date);
    }
    first_instant_at_or_after(zone, wall_clock_millis(date, hour)?)
}

/// The first instant whose local wall time is at or after `wall` (a local
/// date and time read as UTC milliseconds): `wall` itself when it occurs
/// once, its first occurrence when a daylight-saving change repeats it, and
/// the moment the clock jumps past it when a change skips it.
fn first_instant_at_or_after(zone: &StudyZoneResolver, wall: i64) -> Result<i64, CommandError> {
    // Every instant that shows `wall` lies between these probes, and no zone
    // changes its offset twice within that span, so the offset in force at
    // any candidate instant is one of these two.
    let before = zone.offset_minutes_at(wall.saturating_sub(OFFSET_PROBE_MILLIS))?;
    let after = zone.offset_minutes_at(wall.saturating_add(OFFSET_PROBE_MILLIS))?;
    let instant_with = |offset: i32| wall.saturating_sub(i64::from(offset) * MILLIS_PER_MINUTE);
    let mut first: Option<i64> = None;
    for offset in [before, after] {
        let instant = instant_with(offset);
        if zone.offset_minutes_at(instant)? == offset {
            first = Some(first.map_or(instant, |current| current.min(instant)));
        }
    }
    if let Some(instant) = first {
        return Ok(instant);
    }
    // Skipped: the clock jumped forward across `wall`. The jump happened
    // after the instant read with the later offset and no later than the one
    // read with the earlier offset; bisect for it.
    let (mut low, mut high) = (instant_with(after), instant_with(before));
    if low >= high {
        return Err(CommandError::new(
            "invalid_study_calendar_date",
            "That study date has no valid start in the current time zone.",
        ));
    }
    while high - low > 1 {
        let middle = low + (high - low) / 2;
        if zone.offset_minutes_at(middle)? == before {
            low = middle;
        } else {
            high = middle;
        }
    }
    Ok(high)
}

/// Local wall time `hour:00` on `date`, read as UTC milliseconds.
fn wall_clock_millis(date: NaiveDate, hour: u32) -> Result<i64, CommandError> {
    date.and_hms_opt(hour, 0, 0)
        .map(|value| value.and_utc().timestamp_millis())
        .ok_or_else(invalid_calendar_date)
}

fn platform_calendar_error(message: String) -> CommandError {
    CommandError::new("invalid_study_calendar_date", message)
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
fn synthetic_pacific_offset_at_utc(utc_millis: i64) -> i32 {
    let spring = test_timestamp("2026-03-08T10:00:00Z");
    let autumn = test_timestamp("2026-11-01T09:00:00Z");
    if (spring..autumn).contains(&utc_millis) {
        -7 * 60
    } else {
        -8 * 60
    }
}

#[cfg(test)]
fn synthetic_pacific_local_date(utc_millis: i64) -> Result<NaiveDate, CommandError> {
    let utc =
        DateTime::<Utc>::from_timestamp_millis(utc_millis).ok_or_else(invalid_calendar_date)?;
    let offset = synthetic_pacific_offset_at_utc(utc_millis);
    utc.naive_utc()
        .checked_add_signed(Duration::minutes(i64::from(offset)))
        .map(|value| value.date())
        .ok_or_else(invalid_calendar_date)
}

#[cfg(test)]
fn synthetic_pacific_first_instant(date: NaiveDate) -> Result<i64, CommandError> {
    let spring_dst_date = NaiveDate::from_ymd_opt(2026, 3, 9).expect("valid test date");
    let autumn_standard_date = NaiveDate::from_ymd_opt(2026, 11, 2).expect("valid test date");
    let offset_minutes = if (spring_dst_date..autumn_standard_date).contains(&date) {
        -7 * 60
    } else {
        -8 * 60
    };
    let local_midnight_as_utc = date
        .and_hms_opt(0, 0, 0)
        .ok_or_else(invalid_calendar_date)?
        .and_utc()
        .timestamp_millis();
    Ok(local_midnight_as_utc.saturating_sub(i64::from(offset_minutes) * MILLIS_PER_MINUTE))
}

#[cfg(test)]
fn test_timestamp(value: &str) -> i64 {
    DateTime::parse_from_rfc3339(value)
        .expect("test timestamp should parse")
        .timestamp_millis()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pacific_context(value: &str) -> StudyTimeContext {
        pacific_at(value, 0)
    }

    fn pacific_at(value: &str, hour: u32) -> StudyTimeContext {
        StudyTimeContext::pacific_2026_at(test_timestamp(value), hour)
            .expect("synthetic timezone should resolve")
    }

    fn date(value: &str) -> NaiveDate {
        parse_local_date(value).expect("test date should parse")
    }

    #[test]
    fn seven_day_window_contains_today_and_six_preceding_dates() {
        let context = pacific_context("2026-08-06T20:00:00Z");
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
    fn spring_transition_has_a_twenty_three_hour_local_date() {
        let context = pacific_context("2026-03-08T18:00:00Z");
        assert_eq!(context.time_zone_id, "America/Los_Angeles");
        assert_eq!(context.utc_offset_minutes, -7 * 60);
        assert_eq!(
            context
                .tomorrow_start_utc_millis
                .saturating_sub(context.today_start_utc_millis),
            23 * 60 * 60 * 1_000
        );
    }

    #[test]
    fn autumn_transition_has_a_twenty_five_hour_local_date() {
        let context = pacific_context("2026-11-01T18:00:00Z");
        assert_eq!(context.utc_offset_minutes, -8 * 60);
        assert_eq!(
            context
                .tomorrow_start_utc_millis
                .saturating_sub(context.today_start_utc_millis),
            25 * 60 * 60 * 1_000
        );
    }

    #[test]
    fn current_custom_date_resolves_to_now_not_elapsed_midnight() {
        let context = pacific_context("2026-08-06T20:00:00Z");
        assert_eq!(
            context
                .resolve_calendar_date(context.local_date)
                .expect("today should resolve"),
            context.now_utc_millis
        );
    }

    #[test]
    fn calendar_addition_crosses_dst_without_fixed_duration_arithmetic() {
        let context = pacific_context("2026-03-07T20:00:00Z");
        let two_dates_later = context
            .start_after_local_days(2)
            .expect("future date should resolve");
        assert_eq!(
            two_dates_later.saturating_sub(context.today_start_utc_millis),
            47 * 60 * 60 * 1_000
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
    fn before_four_a_m_the_study_date_is_still_yesterday() {
        let night = pacific_at("2026-11-03T01:30:00-08:00", 4);
        assert_eq!(night.local_date, date("2026-11-02"));
        assert_eq!(
            night.today_start_utc_millis,
            test_timestamp("2026-11-02T04:00:00-08:00")
        );
        assert_eq!(
            night.tomorrow_start_utc_millis,
            test_timestamp("2026-11-03T04:00:00-08:00")
        );
        assert_eq!(
            night
                .calendar_date_for_timestamp(night.now_utc_millis)
                .unwrap(),
            date("2026-11-03")
        );

        let morning = pacific_at("2026-11-03T04:00:00-08:00", 4);
        assert_eq!(morning.local_date, date("2026-11-03"));
        assert_eq!(morning.today_start_utc_millis, morning.now_utc_millis);

        let midnight = pacific_at("2026-11-03T01:30:00-08:00", 0);
        assert_eq!(midnight.local_date, date("2026-11-03"));
    }

    #[test]
    fn date_only_reschedules_land_at_the_start_of_the_study_day() {
        let night = pacific_at("2026-11-03T01:30:00-08:00", 4);
        // Today's study date is 2 November, so 3 November is the future.
        assert_eq!(
            night.resolve_calendar_date(date("2026-11-03")).unwrap(),
            test_timestamp("2026-11-03T04:00:00-08:00")
        );
        assert_eq!(
            night.resolve_calendar_date(date("2026-11-02")).unwrap(),
            night.now_utc_millis
        );
        assert_eq!(
            night
                .resolve_calendar_date(date("2026-11-01"))
                .unwrap_err()
                .code,
            "reschedule_date_in_past"
        );
        assert_eq!(
            night.start_after_local_days(3).unwrap(),
            test_timestamp("2026-11-05T04:00:00-08:00")
        );
    }

    #[test]
    fn a_skipped_start_resolves_to_the_first_instant_after_it() {
        // Spring forward: 02:00 PST becomes 03:00 PDT on 8 March.
        let context = pacific_at("2026-03-08T12:00:00-07:00", 2);
        assert_eq!(context.local_date, date("2026-03-08"));
        assert_eq!(
            context.today_start_utc_millis,
            test_timestamp("2026-03-08T03:00:00-07:00")
        );
        assert_eq!(
            context
                .study_date_for_timestamp(test_timestamp("2026-03-08T01:59:59-08:00"))
                .unwrap(),
            date("2026-03-07")
        );
        // At 4 a.m. nothing is skipped: the study date is 23 hours long.
        let four = pacific_at("2026-03-08T12:00:00-07:00", 4);
        assert_eq!(
            four.today_start_utc_millis,
            test_timestamp("2026-03-08T04:00:00-07:00")
        );
        assert_eq!(
            four.tomorrow_start_utc_millis - four.today_start_utc_millis,
            24 * MILLIS_PER_HOUR
        );
        assert_eq!(
            four.study_day_start(date("2026-03-08")).unwrap()
                - four.study_day_start(date("2026-03-07")).unwrap(),
            23 * MILLIS_PER_HOUR
        );
    }

    #[test]
    fn a_repeated_start_resolves_to_its_first_occurrence() {
        // Fall back: 02:00 PDT becomes 01:00 PST on 1 November, so 01:00 to
        // 02:00 happens twice.
        let context = pacific_at("2026-11-01T12:00:00-08:00", 1);
        assert_eq!(
            context.today_start_utc_millis,
            test_timestamp("2026-11-01T01:00:00-07:00")
        );
        // The second 01:30 is still 1 November.
        assert_eq!(
            context
                .study_date_for_timestamp(test_timestamp("2026-11-01T01:30:00-08:00"))
                .unwrap(),
            date("2026-11-01")
        );
        assert_eq!(
            context
                .study_date_for_timestamp(test_timestamp("2026-11-01T00:59:59-07:00"))
                .unwrap(),
            date("2026-10-31")
        );
        let four = pacific_at("2026-11-01T12:00:00-08:00", 4);
        assert_eq!(
            four.study_day_start(date("2026-11-02")).unwrap()
                - four.study_day_start(date("2026-11-01")).unwrap(),
            24 * MILLIS_PER_HOUR
        );
        assert_eq!(
            four.study_day_start(date("2026-11-01")).unwrap()
                - four.study_day_start(date("2026-10-31")).unwrap(),
            25 * MILLIS_PER_HOUR
        );
    }

    #[test]
    fn every_study_day_of_2026_starts_where_the_previous_one_ends() {
        let now = test_timestamp("2026-06-01T12:00:00-07:00");
        for hour in 0..=23 {
            let at = StudyTimeContext::pacific_2026_at(now, hour).unwrap();
            let mut day = date("2026-01-02");
            while day < date("2026-12-31") {
                let start = at.study_day_start(day).unwrap();
                let next = at.study_day_start(day.succ_opt().unwrap()).unwrap();
                assert!(
                    (23 * MILLIS_PER_HOUR..=25 * MILLIS_PER_HOUR).contains(&(next - start)),
                    "{day} at {hour}:00 lasts {} ms",
                    next - start
                );
                assert_eq!(
                    at.study_date_for_timestamp(start).unwrap(),
                    day,
                    "{day} {hour}"
                );
                assert_eq!(
                    at.study_date_for_timestamp(start - 1).unwrap(),
                    day.pred_opt().unwrap(),
                    "{day} {hour}"
                );
                day = day.succ_opt().unwrap();
            }
        }
    }

    #[test]
    fn the_hour_must_be_a_whole_hour_of_the_day() {
        for (value, hour) in [(0, 0), (4, 4), (23, 23)] {
            assert_eq!(validate_new_day_start_hour(value).unwrap(), hour);
        }
        for hour in [-1, 24, i64::from(u32::MAX) + 1] {
            assert_eq!(
                validate_new_day_start_hour(hour).unwrap_err().code,
                "invalid_new_day_start_hour"
            );
        }
        assert_eq!(
            StudyTimeContext::pacific_2026_at(0, 24).unwrap_err().code,
            "invalid_new_day_start_hour"
        );
    }
}
