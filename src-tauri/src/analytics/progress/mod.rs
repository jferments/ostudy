//! Progress page endpoints.
//!
//! Every native command waits in one queue, and one filter click on the old
//! Analytics screen recomputed everything. Each Progress section therefore has
//! its own small command that the page loads on its own request lane: changing
//! the range reloads the sections that depend on it and nothing else.

mod activity;
mod forecast;
mod review_log;
mod strength;
mod subjects;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    database::Database, error::CommandError, settings::load_settings,
    study_calendar::StudyTimeContext,
};

use super::{
    history,
    question_types::{self, QuestionTypeAnalyticsDto},
    recent,
    scope::AnalyticsContext,
    timeline, AnalyticsQueryDto, AnalyticsScopeDto, HistoricalReviewSummaryDto,
    SessionAnalyticsDto, TimelineBucketDto,
};

pub use activity::ProgressActivityDto;
pub use forecast::ProgressForecastDto;
pub use review_log::{ReviewLogCursorDto, ReviewLogPageDto};
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
    pub summary: HistoricalReviewSummaryDto,
    /// The equal-length period just before, for "3 points better".
    pub previous_summary: Option<HistoricalReviewSummaryDto>,
    /// Study dates in the range with at least one review.
    pub study_day_count: i64,
    pub timeline: Vec<TimelineBucketDto>,
    pub desired_retention: f64,
}

#[tauri::command]
pub fn get_progress_overview(
    database: State<'_, Database>,
    input: ProgressQueryDto,
) -> Result<ProgressOverviewDto, CommandError> {
    database.with_connection(|connection| {
        let time = StudyTimeContext::capture(connection)?;
        load_overview(connection, input.context(time)?)
    })
}

pub(crate) fn load_overview(
    connection: &mut rusqlite::Connection,
    context: AnalyticsContext,
) -> Result<ProgressOverviewDto, CommandError> {
    let settings = load_settings(connection)?;
    let current = history::load(connection, &context)?;
    let previous_summary = match context.previous_window() {
        Some(window) => {
            let previous = AnalyticsContext::from_query(
                AnalyticsQueryDto {
                    range: "custom".to_string(),
                    deck_ids: context.deck_ids.clone(),
                    tag_ids: context.tag_ids.clone(),
                    custom_start_local_date: Some(window.start_local_date.to_string()),
                    custom_end_local_date: Some(window.end_local_date.to_string()),
                    highlighted_session_id: None,
                },
                context.time.clone(),
            )?
            .with_subjects(context.subject_ids.clone())?;
            Some(history::load(connection, &previous)?.summary)
        }
        None => None,
    };
    let timeline = timeline::load(connection, &context, current.earliest_local_date.as_deref())?;
    // Daily buckets already say which study dates have reviews.
    let study_day_count = if context.range.step_days() == Some(1) {
        i64::try_from(
            timeline
                .iter()
                .filter(|bucket| bucket.review_count > 0)
                .count(),
        )
        .unwrap_or(i64::MAX)
    } else {
        activity::study_day_count(connection, &context)?
    };
    Ok(ProgressOverviewDto {
        scope: context.dto(current.earliest_local_date.as_deref()),
        summary: current.summary,
        previous_summary,
        study_day_count,
        timeline,
        desired_retention: settings.desired_retention,
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
