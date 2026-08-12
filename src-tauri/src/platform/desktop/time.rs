use std::{
    env,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use chrono::{DateTime, Duration, Local, LocalResult, NaiveDate, Offset, TimeZone};

use crate::platform::{Clock, TimeZoneProvider, TimeZoneSnapshot};

const SECONDS_PER_LOCAL_DATE: i64 = 86_400;

pub(super) struct SystemClock;

impl Clock for SystemClock {
    fn now_utc_millis(&self) -> Result<i64, String> {
        let duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("The system clock is before the Unix epoch: {error}"))?;
        i64::try_from(duration.as_millis()).map_err(|_| {
            "The current timestamp is outside the supported integer range.".to_string()
        })
    }
}

pub(super) struct SystemTimeZoneProvider;

impl TimeZoneProvider for SystemTimeZoneProvider {
    fn snapshot_at(&self, utc_millis: i64) -> Result<TimeZoneSnapshot, String> {
        let value = Local
            .timestamp_millis_opt(utc_millis)
            .earliest()
            .ok_or_else(invalid_calendar_date)?;
        Ok(TimeZoneSnapshot {
            identifier: discover_system_time_zone_id(),
            utc_offset_minutes: value.offset().fix().local_minus_utc().div_euclid(60),
        })
    }

    fn local_date_at(&self, utc_millis: i64) -> Result<NaiveDate, String> {
        Local
            .timestamp_millis_opt(utc_millis)
            .earliest()
            .map(|value| value.date_naive())
            .ok_or_else(invalid_calendar_date)
    }

    fn first_valid_instant(&self, date: NaiveDate) -> Result<i64, String> {
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
        Err("That local calendar date has no valid instant in the current time zone.".to_string())
    }
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

fn invalid_calendar_date() -> String {
    "The requested study calendar date is outside the supported range.".to_string()
}
