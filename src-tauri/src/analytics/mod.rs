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
#[cfg(test)]
mod query_plan_tests;
mod question_types;
mod recent;
mod scope;
mod signals;
mod timeline;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    database::Database, error::CommandError, settings::load_settings,
    study_calendar::StudyTimeContext,
};

use question_types::QuestionTypeAnalyticsDto;
use scope::AnalyticsContext;
use signals::RequestSignals;

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
    pub custom_start_local_date: Option<String>,
    #[serde(default)]
    pub custom_end_local_date: Option<String>,
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
    /// The current study date, so date inputs need no clock of their own.
    pub today_local_date: String,
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
    pub wrong_count: i64,
    pub hard_count: i64,
    pub medium_count: i64,
    pub easy_count: i64,
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
pub struct RecentReviewTagDto {
    pub tag_id: String,
    pub tag_name: String,
    pub is_current: bool,
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
    pub tags: Vec<RecentReviewTagDto>,
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
    pub question_types: Vec<QuestionTypeAnalyticsDto>,
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
        let context = AnalyticsContext::from_query(
            AnalyticsQueryDto {
                range: "all".to_string(),
                deck_ids: Vec::new(),
                tag_ids: Vec::new(),
                custom_start_local_date: None,
                custom_end_local_date: None,
                highlighted_session_id: None,
            },
            StudyTimeContext::capture(connection)?,
        )?;
        recent::load_reviews(connection, &context)
    })
}

pub(crate) fn load_analytics(
    connection: &mut rusqlite::Connection,
    input: AnalyticsQueryDto,
) -> Result<AnalyticsDto, CommandError> {
    let time = StudyTimeContext::capture(connection)?;
    load_analytics_at(connection, input, time)
}

/// Analytics for one captured study time; `load_analytics` captures it.
pub(crate) fn load_analytics_at(
    connection: &mut rusqlite::Connection,
    input: AnalyticsQueryDto,
    time: StudyTimeContext,
) -> Result<AnalyticsDto, CommandError> {
    let context = AnalyticsContext::from_query(input, time)?;
    let settings = load_settings(connection)?;
    // One learning-signal pass shared by the inventory, deck and tag queries.
    let signals = RequestSignals::materialize(connection)?;
    let current_inventory = current_inventory::load(connection, &context, &signals)?;
    let historical = history::load(connection, &context)?;
    let timeline = timeline::load(
        connection,
        &context,
        historical.earliest_local_date.as_deref(),
    )?;
    let deck_stats = aggregates::load_decks(connection, &context, &signals)?;
    let tag_stats =
        aggregates::load_tags(connection, &context, &signals, settings.desired_retention)?;
    drop(signals);
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
        question_types: question_types::load(connection, &context)?,
    })
}

fn default_range() -> String {
    "7d".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        cards::validate_content,
        decks::{create_deck_in_connection, CreateDeckInput},
        practice::{record_practice_event, submit_review_in_connection, SubmitReviewInput},
        scheduler::insert_new_schedule,
        util::generate_id,
    };
    use rusqlite::params;
    use serde_json::json;

    fn paragraph(text: &str) -> serde_json::Value {
        json!({
            "type": "doc",
            "content": [{
                "type": "paragraph",
                "content": [{"type": "text", "text": text}]
            }]
        })
    }

    fn create_card(
        connection: &mut rusqlite::Connection,
        deck_id: &str,
        label: &str,
        now: i64,
    ) -> Result<String, CommandError> {
        let note_id = generate_id(connection)?;
        let card_id = generate_id(connection)?;
        let content = validate_content(
            paragraph(label),
            paragraph("Answer"),
            label.to_string(),
            "Answer".to_string(),
        )?;
        connection
            .execute(
                "INSERT INTO notes (
                     id,deck_id,note_type,content_format,content_schema_version,
                     front_doc,back_doc,front_text,back_text,duplicate_hash,
                     created_at,updated_at,revision
                 ) VALUES(
                     ?1,?2,'basic','tiptap-json',1,?3,?4,?5,?6,?7,?8,?8,1
                 )",
                params![
                    note_id,
                    deck_id,
                    content.front_doc_json,
                    content.back_doc_json,
                    content.front_text,
                    content.back_text,
                    format!("analytics-{label}"),
                    now,
                ],
            )
            .map_err(CommandError::database)?;
        connection
            .execute(
                "INSERT INTO cards(
                     id,note_id,deck_id,direction,template_ordinal,created_at,updated_at
                 ) VALUES(?1,?2,?3,'forward',0,?4,?4)",
                params![card_id, note_id, deck_id, now],
            )
            .map_err(CommandError::database)?;
        insert_new_schedule(connection, &card_id, now)?;
        Ok(card_id)
    }

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
                        custom_start_local_date: None,
                        custom_end_local_date: None,
                        highlighted_session_id: None,
                    },
                )
            })
            .expect("analytics should load");
        assert_eq!(analytics.current_inventory.generated_card_count, 0);
        assert_eq!(analytics.history.review_count, 0);
        assert_eq!(analytics.timeline.len(), 7);
    }

    #[test]
    fn accepted_reviews_from_an_early_ended_session_remain_analytics_evidence() {
        let database = Database::open_in_memory().expect("database should open");
        database
            .with_connection(|connection| {
                let now = StudyTimeContext::capture(connection)?.now_utc_millis;
                let deck_id = create_deck_in_connection(
                    connection,
                    CreateDeckInput {
                        name: "Early stop analytics".to_string(),
                        description: None,
                    },
                )?
                .id;
                let first_card = create_card(connection, &deck_id, "First", now)?;
                let second_card = create_card(connection, &deck_id, "Second", now)?;
                let session_id = generate_id(connection)?;
                connection
                    .execute(
                        "INSERT INTO practice_sessions(
                             id,selected_deck_ids_json,selected_tag_ids_json,shuffle,
                             card_count,mode,started_at
                         ) VALUES(?1,?2,'[]',1,2,'extra',?3)",
                        params![
                            session_id,
                            serde_json::to_string(&vec![deck_id.clone()])
                                .map_err(CommandError::serialization)?,
                            now,
                        ],
                    )
                    .map_err(CommandError::database)?;
                for (position, card_id) in [&first_card, &second_card].into_iter().enumerate() {
                    connection
                        .execute(
                            "INSERT INTO practice_session_cards(
                                 session_id,card_id,position,presentation_variant
                             ) VALUES(?1,?2,?3,'')",
                            params![
                                session_id,
                                card_id,
                                i64::try_from(position).unwrap_or(i64::MAX),
                            ],
                        )
                        .map_err(CommandError::database)?;
                }

                record_practice_event(connection, &session_id, &first_card, true)?;
                let result = submit_review_in_connection(
                    connection,
                    SubmitReviewInput {
                        session_id: session_id.clone(),
                        card_id: first_card.clone(),
                        submission_id: "a".repeat(32),
                        outcome: "somewhat_confident".to_string(),
                        response_time_ms: 1_250,
                        schedule_revision: 1,
                        active_time_ms: None,
                    },
                )?;
                assert_eq!(result.rated_count, 1);
                assert!(!result.completed);

                // This is the same durable terminal state written by the End
                // session command. The second card remains unrated.
                connection
                    .execute(
                        "UPDATE practice_sessions SET completed_at=?1 WHERE id=?2",
                        params![now.saturating_add(2_000), session_id],
                    )
                    .map_err(CommandError::database)?;

                let analytics = load_analytics(
                    connection,
                    AnalyticsQueryDto {
                        range: default_range(),
                        deck_ids: vec![deck_id],
                        tag_ids: Vec::new(),
                        custom_start_local_date: None,
                        custom_end_local_date: None,
                        highlighted_session_id: Some(session_id.clone()),
                    },
                )?;
                assert_eq!(analytics.history.review_count, 1);
                assert_eq!(analytics.history.cards_practiced, 1);
                assert_eq!(analytics.history.session_count, 1);
                assert_eq!(analytics.current_inventory.generated_card_count, 2);
                assert_eq!(analytics.recent_reviews.len(), 1);
                let session = analytics
                    .recent_sessions
                    .iter()
                    .find(|candidate| candidate.id == session_id)
                    .expect("early-ended reviewed session should remain visible");
                assert_eq!(session.rated_count, 1);
                assert!(session.completed_at.is_some());
                assert!(session.highlighted);
                assert!(analytics
                    .timeline
                    .iter()
                    .any(|bucket| bucket.review_count == 1 && bucket.contains_highlighted_session));
                let second_reviews: i64 = connection
                    .query_row(
                        "SELECT COUNT(*) FROM review_events WHERE card_id=?1",
                        [&second_card],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(second_reviews, 0);
                Ok(())
            })
            .expect("ending early must retain accepted review analytics");
    }
}
