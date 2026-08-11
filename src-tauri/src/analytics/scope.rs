use std::collections::BTreeSet;

use chrono::NaiveDate;
use rusqlite::types::Value as SqlValue;

use crate::{
    error::CommandError,
    projections::scope::{append_in_filter, push_value},
    study_calendar::{CalendarWindow, StudyTimeContext},
    studyability,
    util::validate_entity_id,
};

use super::{AnalyticsQueryDto, AnalyticsScopeDto};

const MAX_ANALYTICS_DECKS: usize = 256;
const MAX_ANALYTICS_TAGS: usize = 64;
const MAX_CUSTOM_RANGE_DAYS: i64 = 3_660;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AnalyticsRange {
    Days7,
    Days30,
    Days90,
    Year1,
    All,
    Custom,
}

impl AnalyticsRange {
    pub fn parse(value: &str) -> Result<Self, CommandError> {
        match value {
            "7d" => Ok(Self::Days7),
            "30d" => Ok(Self::Days30),
            "90d" => Ok(Self::Days90),
            "1y" => Ok(Self::Year1),
            "all" => Ok(Self::All),
            "custom" => Ok(Self::Custom),
            _ => Err(CommandError::new(
                "invalid_analytics_range",
                "Choose 7 days, 30 days, 90 days, one year, all time, or a custom range.",
            )),
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Days7 => "7d",
            Self::Days30 => "30d",
            Self::Days90 => "90d",
            Self::Year1 => "1y",
            Self::All => "all",
            Self::Custom => "custom",
        }
    }

    pub const fn granularity(self) -> &'static str {
        match self {
            Self::Days7 | Self::Days30 => "day",
            Self::Days90 | Self::Year1 => "week",
            Self::All => "month",
            Self::Custom => "day",
        }
    }

    pub const fn step_days(self) -> Option<i64> {
        match self {
            Self::Days7 | Self::Days30 => Some(1),
            Self::Days90 | Self::Year1 => Some(7),
            Self::All => None,
            Self::Custom => Some(1),
        }
    }
}

#[derive(Clone, Debug)]
pub struct AnalyticsWindow {
    pub current: Option<CalendarWindow>,
    pub previous: Option<CalendarWindow>,
    pub end_local_date: NaiveDate,
}

#[derive(Clone, Debug)]
pub struct AnalyticsContext {
    pub range: AnalyticsRange,
    pub deck_ids: Vec<String>,
    pub tag_ids: Vec<String>,
    pub highlighted_session_id: Option<String>,
    pub time: StudyTimeContext,
    pub window: AnalyticsWindow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HistoryPeriod {
    Current,
    CurrentAndPrevious,
}

impl AnalyticsContext {
    pub fn from_query(mut input: AnalyticsQueryDto) -> Result<Self, CommandError> {
        let range = AnalyticsRange::parse(&input.range)?;
        input.deck_ids = normalize_ids(input.deck_ids, "deck", MAX_ANALYTICS_DECKS)?;
        input.tag_ids = normalize_ids(input.tag_ids, "tag", MAX_ANALYTICS_TAGS)?;
        if let Some(session_id) = input.highlighted_session_id.as_deref() {
            validate_entity_id(session_id, "practice session")?;
        }
        let time = StudyTimeContext::capture()?;
        let current = match range {
            AnalyticsRange::Days7 => Some(time.trailing_local_dates(7)?),
            AnalyticsRange::Days30 => Some(time.trailing_local_dates(30)?),
            AnalyticsRange::Days90 => Some(time.trailing_local_dates(90)?),
            AnalyticsRange::Year1 => Some(time.one_year_window()?),
            AnalyticsRange::All => None,
            AnalyticsRange::Custom => Some(custom_window(
                &time,
                input.custom_start_local_date.as_deref(),
                input.custom_end_local_date.as_deref(),
            )?),
        };
        let previous = current
            .as_ref()
            .map(|window| time.adjacent_previous_window(window))
            .transpose()?;
        let end_local_date = current
            .as_ref()
            .map_or(time.local_date, |window| window.end_local_date);
        Ok(Self {
            range,
            deck_ids: input.deck_ids,
            tag_ids: input.tag_ids,
            highlighted_session_id: input.highlighted_session_id,
            time: time.clone(),
            window: AnalyticsWindow {
                current,
                previous,
                end_local_date,
            },
        })
    }

    pub fn dto(&self, earliest_local_date: Option<&str>) -> AnalyticsScopeDto {
        let start_local_date = self
            .window
            .current
            .as_ref()
            .map(|window| window.start_local_date.to_string())
            .or_else(|| earliest_local_date.map(str::to_string));
        AnalyticsScopeDto {
            range: self.range.as_str().to_string(),
            start_local_date,
            end_local_date: self.window.end_local_date.to_string(),
            start_at: self
                .window
                .current
                .as_ref()
                .map(|window| window.start_utc_millis),
            end_at: self
                .window
                .current
                .as_ref()
                .map_or(self.time.now_utc_millis, |window| {
                    window.end_utc_millis.saturating_sub(1)
                }),
            bucket_granularity: self.range.granularity().to_string(),
            deck_ids: self.deck_ids.clone(),
            tag_ids: self.tag_ids.clone(),
            highlighted_session_id: self.highlighted_session_id.clone(),
        }
    }

    pub fn current_card_conditions(&self, values: &mut Vec<SqlValue>) -> Vec<String> {
        let mut conditions = vec![studyability::current_card_sql("c", "n", "d").to_string()];
        append_in_filter(&mut conditions, values, "c.deck_id", &self.deck_ids);
        for tag_id in &self.tag_ids {
            let index = push_value(values, tag_id.clone());
            conditions.push(format!(
                "EXISTS (
                     SELECT 1 FROM note_tags filter_nt
                     WHERE filter_nt.note_id = n.id AND filter_nt.tag_id = ?{index}
                 )"
            ));
        }
        conditions
    }

    pub fn historical_conditions(
        &self,
        values: &mut Vec<SqlValue>,
        period: HistoryPeriod,
    ) -> Vec<String> {
        let mut conditions = vec![
            "c.deleted_at IS NULL".to_string(),
            "n.deleted_at IS NULL".to_string(),
            "d.deleted_at IS NULL".to_string(),
        ];
        append_in_filter(&mut conditions, values, "c.deck_id", &self.deck_ids);
        for tag_id in &self.tag_ids {
            let index = push_value(values, tag_id.clone());
            conditions.push(format!(
                "EXISTS (
                     SELECT 1 FROM review_event_tag_attributions filter_rta
                     WHERE filter_rta.review_event_id = r.id
                       AND filter_rta.tag_id = ?{index}
                 )"
            ));
        }
        match period {
            HistoryPeriod::Current => {
                if let Some(window) = &self.window.current {
                    let start = push_value(values, window.start_local_date.to_string());
                    conditions.push(format!("a.review_local_date >= ?{start}"));
                }
                let end = push_value(values, self.window.end_local_date.to_string());
                conditions.push(format!("a.review_local_date <= ?{end}"));
            }
            HistoryPeriod::CurrentAndPrevious => {
                if let Some(window) = self
                    .window
                    .previous
                    .as_ref()
                    .or(self.window.current.as_ref())
                {
                    let start = push_value(values, window.start_local_date.to_string());
                    conditions.push(format!("a.review_local_date >= ?{start}"));
                }
                let end = push_value(values, self.window.end_local_date.to_string());
                conditions.push(format!("a.review_local_date <= ?{end}"));
            }
        }
        conditions
    }

    pub fn previous_window(&self) -> Option<&CalendarWindow> {
        self.window.previous.as_ref()
    }
}

fn custom_window(
    time: &StudyTimeContext,
    start: Option<&str>,
    end: Option<&str>,
) -> Result<CalendarWindow, CommandError> {
    let parse = |value: Option<&str>| -> Result<NaiveDate, CommandError> {
        let value = value.ok_or_else(|| {
            CommandError::new(
                "invalid_analytics_custom_range",
                "Choose both a start date and an end date for the custom analytics range.",
            )
        })?;
        NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| {
            CommandError::new(
                "invalid_analytics_custom_range",
                "Choose valid custom analytics dates.",
            )
        })
    };
    let start_local_date = parse(start)?;
    let end_local_date = parse(end)?;
    if start_local_date > end_local_date || end_local_date > time.local_date {
        return Err(CommandError::new(
            "invalid_analytics_custom_range",
            "The custom analytics start date must be on or before the end date, and the end date cannot be in the future.",
        ));
    }
    let day_count = end_local_date
        .signed_duration_since(start_local_date)
        .num_days()
        .saturating_add(1);
    if !(1..=MAX_CUSTOM_RANGE_DAYS).contains(&day_count) {
        return Err(CommandError::new(
            "invalid_analytics_custom_range",
            "Choose a custom analytics range no longer than 3660 local calendar days.",
        ));
    }
    let end_utc_millis = if end_local_date == time.local_date {
        time.now_utc_millis.saturating_add(1)
    } else {
        let next = end_local_date.succ_opt().ok_or_else(|| {
            CommandError::new(
                "invalid_analytics_custom_range",
                "The custom analytics end date is outside the supported calendar.",
            )
        })?;
        time.first_valid_instant(next)?
    };
    Ok(CalendarWindow {
        start_local_date,
        end_local_date,
        start_utc_millis: time.first_valid_instant(start_local_date)?,
        end_utc_millis,
    })
}

fn normalize_ids(
    values: Vec<String>,
    label: &str,
    maximum: usize,
) -> Result<Vec<String>, CommandError> {
    if values.len() > maximum {
        return Err(CommandError::new(
            "too_many_analytics_filters",
            format!("Filter analytics by at most {maximum} {label}s."),
        ));
    }
    let mut unique = BTreeSet::new();
    for value in values {
        validate_entity_id(&value, label)?;
        unique.insert(value);
    }
    Ok(unique.into_iter().collect())
}
