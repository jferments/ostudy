//! Progress page endpoints.
//!
//! Every native command waits in one queue, and one filter click on the old
//! Analytics screen recomputed everything. Each Progress section therefore has
//! its own small command that the page loads on its own request lane: changing
//! the range reloads the sections that depend on it and nothing else.

mod activity;
mod forecast;
mod hours;
mod review_log;
mod session;
mod strength;
mod subjects;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    database::Database, error::CommandError, settings::load_settings,
    study_calendar::StudyTimeContext,
};

use super::{
    aggregates,
    question_types::{self, QuestionTypeAnalyticsDto},
    recent,
    scope::AnalyticsContext,
    signals::RequestSignals,
    AnalyticsDimensionOptionsDto, AnalyticsQueryDto, AnalyticsScopeDto, SessionAnalyticsDto,
    TagAnalyticsDto,
};

pub use activity::ProgressActivityDto;
pub use forecast::ProgressForecastDto;
pub use hours::ProgressSummaryDto;
use hours::{DayRow, Totals};
pub use review_log::{ReviewLogCursorDto, ReviewLogPageDto};
pub use session::SessionSummaryDto;
pub use strength::CardStrengthDto;
pub use subjects::SubjectProgressDto;

/// The scope every scoped Progress section shares.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProgressQueryDto {
    #[serde(default = "default_range")]
    pub range: String,
    #[serde(default)]
    pub subject_ids: Vec<String>,
    #[serde(default)]
    pub deck_ids: Vec<String>,
    #[serde(default)]
    pub tag_ids: Vec<String>,
    #[serde(default)]
    pub custom_start_local_date: Option<String>,
    #[serde(default)]
    pub custom_end_local_date: Option<String>,
}

impl ProgressQueryDto {
    fn context(self, time: StudyTimeContext) -> Result<AnalyticsContext, CommandError> {
        AnalyticsContext::from_query(
            AnalyticsQueryDto {
                range: self.range,
                deck_ids: self.deck_ids,
                tag_ids: self.tag_ids,
                custom_start_local_date: self.custom_start_local_date,
                custom_end_local_date: self.custom_end_local_date,
                highlighted_session_id: None,
            },
            time,
        )?
        .with_subjects(self.subject_ids)
    }
}

fn default_range() -> String {
    "30d".to_string()
}

/// The summary tiles and the reviews and recall chart.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressOverviewDto {
    pub scope: AnalyticsScopeDto,
    pub summary: ProgressSummaryDto,
    /// The equal-length period just before, for "3 points better"; `None` for
    /// all time.
    pub previous_summary: Option<ProgressSummaryDto>,
    /// Study dates in the range with at least one review.
    pub study_day_count: i64,
    /// Buckets are cut back from today, so only the first can be partial.
    /// All time has one bucket per calendar month.
    pub timeline: Vec<ProgressBucketDto>,
    pub desired_retention: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressBucketDto {
    pub start_local_date: String,
    pub end_local_date: String,
    #[serde(flatten)]
    pub totals: ProgressSummaryDto,
}

#[tauri::command]
pub fn get_progress_overview(
    database: State<'_, Database>,
    input: ProgressQueryDto,
) -> Result<ProgressOverviewDto, CommandError> {
    database.with_connection(|connection| {
        let time = StudyTimeContext::capture(connection)?;
        load_overview(connection, &input.context(time)?)
    })
}

pub(crate) fn load_overview(
    connection: &mut rusqlite::Connection,
    context: &AnalyticsContext,
) -> Result<ProgressOverviewDto, CommandError> {
    let settings = load_settings(connection)?;
    let days = hours::load(connection, context, false)?;
    let current_start = context
        .window
        .current
        .as_ref()
        .map(|window| window.start_local_date);
    let in_current = |date: NaiveDate| current_start.is_none_or(|start| date >= start);

    let mut summary = Totals::default();
    let mut previous = Totals::default();
    let mut study_day_count = 0_i64;
    for day in &days {
        if in_current(day.study_date) {
            summary.add(&day.totals);
            if day.totals.review_count > 0 {
                study_day_count += 1;
            }
        } else {
            previous.add(&day.totals);
        }
    }
    let earliest = days
        .iter()
        .map(|day| day.study_date)
        .find(|date| in_current(*date));
    let timeline = timeline(context, earliest, &days)?;
    Ok(ProgressOverviewDto {
        scope: context.dto(earliest.map(|date| date.to_string()).as_deref()),
        summary: summary.summary(),
        previous_summary: context.previous_window().map(|_| previous.summary()),
        study_day_count,
        timeline,
        desired_retention: settings.desired_retention,
    })
}

/// Bucket the current period's days. Ranges step by days and are cut back
/// from the last date; all time steps by calendar month from the first review.
fn timeline(
    context: &AnalyticsContext,
    earliest: Option<NaiveDate>,
    days: &[DayRow],
) -> Result<Vec<ProgressBucketDto>, CommandError> {
    let end = context.window.end_local_date;
    let mut bounds = Vec::<(NaiveDate, NaiveDate)>::new();
    match (context.window.current.as_ref(), context.range.step_days()) {
        (Some(window), Some(step)) => {
            let date_count = (end - window.start_local_date).num_days() + 1;
            let bucket_count = (date_count + step - 1).div_euclid(step);
            let origin = add_days(end, 1 - bucket_count * step)?;
            for index in 0..bucket_count {
                let start = add_days(origin, index * step)?;
                bounds.push((
                    start.max(window.start_local_date),
                    add_days(start, step - 1)?,
                ));
            }
        }
        _ => {
            let last = context.time.month_start(end)?;
            let mut month = context.time.month_start(earliest.unwrap_or(end))?;
            while month <= last {
                let next = context.time.next_month_start(month)?;
                bounds.push((month, add_days(next, -1)?.min(end)));
                month = next;
            }
        }
    }
    Ok(bounds
        .into_iter()
        .map(|(start, last)| {
            let mut totals = Totals::default();
            for day in days
                .iter()
                .filter(|day| day.study_date >= start && day.study_date <= last)
            {
                totals.add(&day.totals);
            }
            ProgressBucketDto {
                start_local_date: start.to_string(),
                end_local_date: last.to_string(),
                totals: totals.summary(),
            }
        })
        .collect())
}

fn add_days(date: NaiveDate, days: i64) -> Result<NaiveDate, CommandError> {
    date.checked_add_signed(chrono::Duration::days(days))
        .ok_or_else(|| {
            CommandError::new(
                "invalid_analytics_date",
                "The analytics date range is outside the supported calendar.",
            )
        })
}

#[tauri::command]
pub fn get_progress_forecast(
    database: State<'_, Database>,
    input: ProgressQueryDto,
) -> Result<ProgressForecastDto, CommandError> {
    database.with_connection(|connection| {
        let time = StudyTimeContext::capture(connection)?;
        forecast::load(connection, &input.context(time)?)
    })
}

#[tauri::command]
pub fn get_progress_card_strength(
    database: State<'_, Database>,
    input: ProgressQueryDto,
) -> Result<CardStrengthDto, CommandError> {
    database.with_connection(|connection| {
        let time = StudyTimeContext::capture(connection)?;
        strength::load(connection, &input.context(time)?)
    })
}

/// The whole library's study calendar: streaks belong to the learner, not to
/// a filter.
#[tauri::command]
pub fn get_progress_activity(
    database: State<'_, Database>,
) -> Result<ProgressActivityDto, CommandError> {
    database.with_connection(|connection| {
        let time = StudyTimeContext::capture(connection)?;
        activity::load(connection, &time)
    })
}

/// What a finished practice session changed, with today's totals and streak.
#[tauri::command]
pub fn get_session_summary(
    database: State<'_, Database>,
    session_id: String,
) -> Result<SessionSummaryDto, CommandError> {
    crate::util::validate_entity_id(&session_id, "practice session")?;
    database.with_connection(|connection| {
        let time = StudyTimeContext::capture(connection)?;
        session::load(connection, time, &session_id)
    })
}

#[tauri::command]
pub fn get_progress_subjects(
    database: State<'_, Database>,
    input: ProgressQueryDto,
) -> Result<Vec<SubjectProgressDto>, CommandError> {
    database.with_connection(|connection| {
        let time = StudyTimeContext::capture(connection)?;
        subjects::load(connection, &input.context(time)?)
    })
}

#[tauri::command]
pub fn get_progress_question_types(
    database: State<'_, Database>,
    input: ProgressQueryDto,
) -> Result<Vec<QuestionTypeAnalyticsDto>, CommandError> {
    database.with_connection(|connection| {
        let time = StudyTimeContext::capture(connection)?;
        question_types::load(connection, &input.context(time)?)
    })
}

#[tauri::command]
pub fn get_progress_sessions(
    database: State<'_, Database>,
    input: ProgressQueryDto,
) -> Result<Vec<SessionAnalyticsDto>, CommandError> {
    database.with_connection(|connection| {
        let time = StudyTimeContext::capture(connection)?;
        recent::load_sessions(connection, &input.context(time)?)
    })
}

/// Every tag in scope, the ones that need attention first.
#[tauri::command]
pub fn get_progress_topics(
    database: State<'_, Database>,
    input: ProgressQueryDto,
) -> Result<Vec<TagAnalyticsDto>, CommandError> {
    database.with_connection(|connection| {
        let time = StudyTimeContext::capture(connection)?;
        load_topics(connection, &input.context(time)?)
    })
}

fn load_topics(
    connection: &mut rusqlite::Connection,
    context: &AnalyticsContext,
) -> Result<Vec<TagAnalyticsDto>, CommandError> {
    let settings = load_settings(connection)?;
    let signals = RequestSignals::materialize(connection)?;
    aggregates::load_tags(connection, context, &signals, settings.desired_retention)
}

/// The decks and tags the scope filter offers.
#[tauri::command]
pub fn get_progress_filter_options(
    database: State<'_, Database>,
) -> Result<AnalyticsDimensionOptionsDto, CommandError> {
    database.with_connection(|connection| recent::load_dimension_options(connection))
}

/// One page of the review log, newest first. `before` is the previous page's
/// `next` cursor.
#[tauri::command]
pub fn get_review_log(
    database: State<'_, Database>,
    input: ProgressQueryDto,
    before: Option<ReviewLogCursorDto>,
) -> Result<ReviewLogPageDto, CommandError> {
    database.with_connection(|connection| {
        let time = StudyTimeContext::capture(connection)?;
        review_log::load_page(connection, &input.context(time)?, before.as_ref())
    })
}

#[cfg(test)]
mod tests;
