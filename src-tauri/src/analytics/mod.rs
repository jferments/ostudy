//! Deterministic learning analytics over separate current-inventory and
//! historical-evidence domains.
//!
//! Current inventory follows current deck/tag organization and canonical
//! studyability. Historical evidence uses immutable review presentation/tag
//! snapshots while deck ownership and display names follow the card's current
//! owner. Moving a card therefore moves all of its history; deleting its current
//! owner removes that history from analytics immediately and the deletion
//! finalizer purges it after the undo window.

mod aggregates;
mod current_inventory;
mod daily;
mod history;
mod recent;
mod scope;
mod timeline;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{database::Database, error::CommandError, settings::load_settings};

use scope::AnalyticsContext;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnalyticsQueryDto {
    #[serde(default = "default_range")]
    pub range: String,
    #[serde(default)]
    pub deck_ids: Vec<String>,
    #[serde(default)]
    pub tag_ids: Vec<String>,
    #[serde(default)]
    pub highlighted_session_id: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsScopeDto {
    pub range: String,
    pub start_local_date: Option<String>,
    pub end_local_date: String,
    pub start_at: Option<i64>,
    pub end_at: i64,
    pub bucket_granularity: String,
    pub deck_ids: Vec<String>,
    pub tag_ids: Vec<String>,
    pub highlighted_session_id: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrentInventorySummaryDto {
    pub current_deck_count: i64,
    pub source_note_count: i64,
    pub generated_card_count: i64,
    pub studyable_card_count: i64,
    pub studyable_due_count: i64,
    pub needs_more_practice_count: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoricalReviewSummaryDto {
    pub cards_practiced: i64,
    pub review_count: i64,
    pub correct_count: i64,
    pub correct_rate: Option<f64>,
    pub average_confidence: Option<f64>,
    pub average_response_time_ms: Option<f64>,
    pub total_study_time_ms: i64,
    pub session_count: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyLimitUsageDto {
    pub new_limit: i64,
    pub review_limit: i64,
    pub new_completed: i64,
    pub reviews_completed: i64,
    pub new_remaining: i64,
    pub reviews_remaining: i64,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RatingDistributionDto {
    pub again: i64,
    pub hard: i64,
    pub good: i64,
    pub easy: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineBucketDto {
    pub start_local_date: String,
    pub end_local_date: String,
    pub start_at: i64,
    pub end_at: i64,
    pub cards_practiced: i64,
    pub review_count: i64,
    pub new_cards: i64,
    pub remembered_count: i64,
    pub correct_rate: Option<f64>,
    pub average_confidence: Option<f64>,
    pub average_response_time_ms: Option<f64>,
    pub study_time_ms: i64,
    pub session_count: i64,
    pub contains_highlighted_session: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeckAnalyticsDto {
    pub deck_id: String,
    pub deck_name: String,
    pub is_archived: bool,
    pub source_note_count: i64,
    pub generated_card_count: i64,
    pub studyable_card_count: i64,
    pub studyable_due_count: i64,
    pub needs_more_practice_count: i64,
    pub cards_practiced: i64,
    pub review_count: i64,
    pub correct_count: i64,
    pub correct_rate: Option<f64>,
    pub average_confidence: Option<f64>,
    pub average_response_time_ms: Option<f64>,
    pub total_study_time_ms: i64,
    pub last_practiced_at: Option<i64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TagAnalyticsDto {
    pub tag_id: String,
    pub tag_name: String,
    pub is_current: bool,
    pub source_note_count: i64,
    pub generated_card_count: i64,
    pub studyable_card_count: i64,
    pub studyable_due_count: i64,
    pub needs_more_practice_count: i64,
    pub cards_practiced: i64,
    pub review_count: i64,
    pub correct_count: i64,
    pub correct_rate: Option<f64>,
    pub average_confidence: Option<f64>,
    pub average_response_time_ms: Option<f64>,
    pub total_study_time_ms: i64,
    pub last_practiced_at: Option<i64>,
    pub trend_delta: Option<f64>,
    pub status: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentReviewDto {
    pub id: String,
    pub deck_name: String,
    pub captured_deck_name: String,
    pub card_id: String,
    pub card_label: String,
    pub question_excerpt: String,
    pub tags: Vec<String>,
    pub outcome: String,
    pub rating: i64,
    pub response_time_ms: i64,
    pub reviewed_at: i64,
    pub review_local_date: String,
    pub due_after: i64,
    pub attribution_source: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionAnalyticsDto {
    pub id: String,
    pub mode: String,
    pub deck_names: Vec<String>,
    pub started_at: i64,
    pub completed_at: Option<i64>,
    pub duration_ms: i64,
    pub card_count: i64,
    pub rated_count: i64,
    pub correct_count: i64,
    pub correct_rate: Option<f64>,
    pub study_time_ms: i64,
    pub highlighted: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttributionCoverageDto {
    pub captured_review_count: i64,
    pub legacy_backfilled_review_count: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsDeckOptionDto {
    pub id: String,
    pub name: String,
    pub is_archived: bool,
    pub has_current_cards: bool,
    pub has_history: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsTagOptionDto {
    pub id: String,
    pub name: String,
    pub is_current: bool,
    pub has_history: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsDimensionOptionsDto {
    pub decks: Vec<AnalyticsDeckOptionDto>,
    pub tags: Vec<AnalyticsTagOptionDto>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsDto {
    pub scope: AnalyticsScopeDto,
    pub current_inventory: CurrentInventorySummaryDto,
    pub history: HistoricalReviewSummaryDto,
    pub daily_limits: DailyLimitUsageDto,
    pub rating_distribution: RatingDistributionDto,
    pub timeline: Vec<TimelineBucketDto>,
    pub deck_stats: Vec<DeckAnalyticsDto>,
    pub tag_stats: Vec<TagAnalyticsDto>,
    pub recent_reviews: Vec<RecentReviewDto>,
    pub recent_sessions: Vec<SessionAnalyticsDto>,
    pub attribution_coverage: AttributionCoverageDto,
    pub dimension_options: AnalyticsDimensionOptionsDto,
}

#[tauri::command]
pub fn get_analytics(
    database: State<'_, Database>,
    input: AnalyticsQueryDto,
) -> Result<AnalyticsDto, CommandError> {
    database.with_connection(|connection| load_analytics(connection, input))
}

#[tauri::command]
pub fn get_recent_practice_reviews(
    database: State<'_, Database>,
) -> Result<Vec<RecentReviewDto>, CommandError> {
    database.with_connection(|connection| {
        let context = AnalyticsContext::from_query(AnalyticsQueryDto {
            range: "all".to_string(),
            deck_ids: Vec::new(),
            tag_ids: Vec::new(),
            highlighted_session_id: None,
        })?;
        recent::load_reviews(connection, &context)
    })
}

pub(crate) fn load_analytics(
    connection: &mut rusqlite::Connection,
    input: AnalyticsQueryDto,
) -> Result<AnalyticsDto, CommandError> {
    let context = AnalyticsContext::from_query(input)?;
    let settings = load_settings(connection)?;
    let current_inventory = current_inventory::load(connection, &context)?;
    let historical = history::load(connection, &context)?;
    let timeline = timeline::load(
        connection,
        &context,
        historical.earliest_local_date.as_deref(),
    )?;
    let deck_stats = aggregates::load_decks(connection, &context)?;
    let tag_stats = aggregates::load_tags(connection, &context, settings.desired_retention)?;
    let recent_reviews = recent::load_reviews(connection, &context)?;
    let recent_sessions = recent::load_sessions(connection, &context)?;
    let dimension_options = recent::load_dimension_options(connection)?;
    let daily_limits = daily::load(connection, &context, &settings)?;
    Ok(AnalyticsDto {
        scope: context.dto(historical.earliest_local_date.as_deref()),
        current_inventory,
        history: historical.summary,
        daily_limits,
        rating_distribution: historical.rating_distribution,
        timeline,
        deck_stats,
        tag_stats,
        recent_reviews,
        recent_sessions,
        attribution_coverage: historical.attribution_coverage,
        dimension_options,
    })
}

fn default_range() -> String {
    "7d".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_library_keeps_current_and_historical_domains_separate() {
        let database = Database::open_in_memory().expect("database should open");
        let analytics = database
            .with_connection(|connection| {
                load_analytics(
                    connection,
                    AnalyticsQueryDto {
                        range: default_range(),
                        deck_ids: Vec::new(),
                        tag_ids: Vec::new(),
                        highlighted_session_id: None,
                    },
                )
            })
            .expect("analytics should load");
        assert_eq!(analytics.current_inventory.generated_card_count, 0);
        assert_eq!(analytics.history.review_count, 0);
        assert_eq!(analytics.timeline.len(), 7);
    }
}
