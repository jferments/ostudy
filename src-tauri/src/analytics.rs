//! Fixed-query current inventory and immutable historical learning Analytics.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{Datelike, Duration, NaiveDate};
use rusqlite::{params_from_iter, types::Value as SqlValue, Connection};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    database::Database,
    error::CommandError,
    settings::load_settings,
    study_calendar::{previous_range_start_date, range_start_date, StudyTimeContext},
    studyability::learning_signal_cte,
    util::validate_entity_id,
};

const MAX_SCOPE_DECKS: usize = 100;
const MAX_SCOPE_TAGS: usize = 32;
const TAG_SEPARATOR: char = '\u{001f}';

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnalyticsInput {
    pub range: String,
    #[serde(default)]
    pub deck_ids: Vec<String>,
    #[serde(default)]
    pub tag_ids: Vec<String>,
    pub highlighted_session_id: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsScopeDto {
    pub range: String,
    pub start_at: Option<i64>,
    pub end_at: i64,
    pub bucket_granularity: String,
    pub deck_ids: Vec<String>,
    pub tag_ids: Vec<String>,
    pub highlighted_session_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsSummaryDto {
    pub current_deck_count: i64,
    pub source_note_count: i64,
    pub generated_card_count: i64,
    pub studyable_card_count: i64,
    pub studyable_due_count: i64,
    pub needs_more_practice: i64,
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

#[derive(Clone, Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RatingDistributionDto {
    pub again: i64,
    pub hard: i64,
    pub good: i64,
    pub easy: i64,
}

#[derive(Clone, Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TimelineBucketDto {
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
    pub archived: bool,
    pub source_note_count: i64,
    pub generated_card_count: i64,
    pub studyable_due_count: i64,
    pub needs_more_practice: i64,
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
    pub source_note_count: i64,
    pub generated_card_count: i64,
    pub studyable_due_count: i64,
    pub needs_more_practice: i64,
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
    pub deck_id: String,
    pub deck_name: String,
    pub card_id: String,
    pub front_text: String,
    pub tags: Vec<String>,
    pub outcome: String,
    pub rating: i64,
    pub response_time_ms: i64,
    pub reviewed_at: i64,
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
pub struct AnalyticsDimensionOptionDto {
    pub id: String,
    pub name: String,
    pub archived: bool,
    pub historical_only: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsDimensionOptionsDto {
    pub decks: Vec<AnalyticsDimensionOptionDto>,
    pub tags: Vec<AnalyticsDimensionOptionDto>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttributionCoverageDto {
    pub captured_review_count: i64,
    pub legacy_backfilled_review_count: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsDto {
    pub scope: AnalyticsScopeDto,
    pub summary: AnalyticsSummaryDto,
    pub daily_limits: DailyLimitUsageDto,
    pub rating_distribution: RatingDistributionDto,
    pub timeline: Vec<TimelineBucketDto>,
    pub deck_stats: Vec<DeckAnalyticsDto>,
    pub tag_stats: Vec<TagAnalyticsDto>,
    pub recent_reviews: Vec<RecentReviewDto>,
    pub recent_sessions: Vec<SessionAnalyticsDto>,
    pub dimensions: AnalyticsDimensionOptionsDto,
    pub attribution_coverage: AttributionCoverageDto,
}

#[derive(Clone)]
struct Scope {
    range: String,
    deck_ids: Vec<String>,
    tag_ids: Vec<String>,
    highlighted: Option<String>,
    start_date: Option<NaiveDate>,
    previous_start: Option<NaiveDate>,
    context: StudyTimeContext,
}

#[tauri::command]
pub fn get_analytics(
    database: State<'_, Database>,
    input: AnalyticsInput,
) -> Result<AnalyticsDto, CommandError> {
    let scope = validate_scope(input)?;
    database.with_connection(|connection| analytics_in_connection(connection, &scope))
}

#[tauri::command]
pub fn get_recent_practice_reviews(
    database: State<'_, Database>,
) -> Result<Vec<RecentReviewDto>, CommandError> {
    database.with_connection(|connection| {
        recent_reviews(
            connection,
            &Scope {
                range: "all".to_string(),
                deck_ids: vec![],
                tag_ids: vec![],
                highlighted: None,
                start_date: None,
                previous_start: None,
                context: StudyTimeContext::capture()?,
            },
            30,
        )
    })
}

fn analytics_in_connection(
    connection: &mut Connection,
    scope: &Scope,
) -> Result<AnalyticsDto, CommandError> {
    let mut summary = current_inventory(connection, scope)?;
    let (history, ratings) = historical_summary(connection, scope)?;
    summary.cards_practiced = history.0;
    summary.review_count = history.1;
    summary.correct_count = history.2;
    summary.correct_rate = ratio(history.2, history.1);
    summary.average_confidence = history.3;
    summary.average_response_time_ms = history.4;
    summary.total_study_time_ms = history.5;
    summary.session_count = history.6;
    let settings = load_settings(connection)?;
    let (new_completed, reviews_completed): (i64, i64) = connection
        .query_row(
            "SELECT COALESCE(SUM(CASE WHEN r.state_before = 0 THEN 1 ELSE 0 END),0),
                COALESCE(SUM(CASE WHEN r.state_before <> 0 THEN 1 ELSE 0 END),0)
         FROM review_events r JOIN review_event_attributions a ON a.review_event_id = r.id
         WHERE a.review_local_date = ?1",
            [scope.context.local_date_string()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(CommandError::database)?;
    Ok(AnalyticsDto {
        scope: AnalyticsScopeDto {
            range: scope.range.clone(),
            start_at: scope
                .start_date
                .map(|date| scope.context.resolve_date_start(date))
                .transpose()?,
            end_at: scope.context.now_utc_millis,
            bucket_granularity: granularity(&scope.range).to_string(),
            deck_ids: scope.deck_ids.clone(),
            tag_ids: scope.tag_ids.clone(),
            highlighted_session_id: scope.highlighted.clone(),
        },
        summary,
        daily_limits: DailyLimitUsageDto {
            new_limit: settings.daily_new_limit,
            review_limit: settings.daily_review_limit,
            new_completed,
            reviews_completed,
            new_remaining: (settings.daily_new_limit - new_completed).max(0),
            reviews_remaining: (settings.daily_review_limit - reviews_completed).max(0),
        },
        rating_distribution: ratings,
        timeline: timeline(connection, scope)?,
        deck_stats: deck_stats(connection, scope)?,
        tag_stats: tag_stats(connection, scope)?,
        recent_reviews: recent_reviews(connection, scope, 20)?,
        recent_sessions: recent_sessions(connection, scope)?,
        dimensions: dimensions(connection)?,
        attribution_coverage: attribution_coverage(connection, scope)?,
    })
}

fn current_inventory(
    connection: &Connection,
    scope: &Scope,
) -> Result<AnalyticsSummaryDto, CommandError> {
    let mut values = vec![scope.context.now_utc_millis.into()];
    let filters = current_filters(scope, &mut values);
    let sql = format!(
        "WITH {signals}, scoped AS (
             SELECT c.id, c.note_id, c.deck_id, s.due_at,
                    c.suspended_at, c.buried_until,
                    COALESCE(ls.needs_more_practice,0) AS needs_more_practice
             FROM cards c JOIN notes n ON n.id=c.note_id JOIN decks d ON d.id=c.deck_id
             JOIN card_schedules s ON s.card_id=c.id
             LEFT JOIN learning_signals ls ON ls.card_id=c.id
             WHERE c.deleted_at IS NULL AND n.deleted_at IS NULL
               AND d.deleted_at IS NULL AND d.archived_at IS NULL {filters}
         )
         SELECT COUNT(DISTINCT deck_id), COUNT(DISTINCT note_id), COUNT(id),
                COALESCE(SUM(CASE WHEN suspended_at IS NULL AND (buried_until IS NULL OR buried_until <= ?1) THEN 1 ELSE 0 END),0),
                COALESCE(SUM(CASE WHEN suspended_at IS NULL AND (buried_until IS NULL OR buried_until <= ?1) AND due_at <= ?1 THEN 1 ELSE 0 END),0),
                COALESCE(SUM(CASE WHEN suspended_at IS NULL AND (buried_until IS NULL OR buried_until <= ?1) THEN needs_more_practice ELSE 0 END),0)
         FROM scoped", signals=learning_signal_cte());
    let row: (i64, i64, i64, i64, i64, i64) = connection
        .query_row(&sql, params_from_iter(values.iter()), |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
            ))
        })
        .map_err(CommandError::database)?;
    Ok(AnalyticsSummaryDto {
        current_deck_count: row.0,
        source_note_count: row.1,
        generated_card_count: row.2,
        studyable_card_count: row.3,
        studyable_due_count: row.4,
        needs_more_practice: row.5,
        ..Default::default()
    })
}

fn historical_summary(
    connection: &Connection,
    scope: &Scope,
) -> Result<
    (
        (i64, i64, i64, Option<f64>, Option<f64>, i64, i64),
        RatingDistributionDto,
    ),
    CommandError,
> {
    let (where_sql, values) = history_where(scope);
    let sql = format!(
        "SELECT COUNT(DISTINCT r.card_id), COUNT(*), COALESCE(SUM(r.answer_correct),0),
                AVG(CASE r.confidence WHEN 'wrong' THEN 0.0 WHEN 'not_confident' THEN 1.0 WHEN 'somewhat_confident' THEN 2.0 WHEN 'very_confident' THEN 3.0 END),
                AVG(r.response_time_ms), COALESCE(SUM(r.response_time_ms),0), COUNT(DISTINCT r.session_id),
                COALESCE(SUM(CASE WHEN r.rating=1 THEN 1 ELSE 0 END),0),
                COALESCE(SUM(CASE WHEN r.rating=2 THEN 1 ELSE 0 END),0),
                COALESCE(SUM(CASE WHEN r.rating=3 THEN 1 ELSE 0 END),0),
                COALESCE(SUM(CASE WHEN r.rating=4 THEN 1 ELSE 0 END),0)
         FROM review_events r JOIN review_event_attributions a ON a.review_event_id=r.id
         JOIN decks d ON d.id=a.deck_id AND d.deleted_at IS NULL
         WHERE {where_sql}");
    let row = connection
        .query_row(&sql, params_from_iter(values.iter()), |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
                r.get(7)?,
                r.get(8)?,
                r.get(9)?,
                r.get(10)?,
            ))
        })
        .map_err(CommandError::database)?;
    Ok((
        (row.0, row.1, row.2, row.3, row.4, row.5, row.6),
        RatingDistributionDto {
            again: row.7,
            hard: row.8,
            good: row.9,
            easy: row.10,
        },
    ))
}

fn deck_stats(
    connection: &Connection,
    scope: &Scope,
) -> Result<Vec<DeckAnalyticsDto>, CommandError> {
    let (history_where, mut values) = history_where(scope);
    let now_index = push(&mut values, scope.context.now_utc_millis);
    let sql = format!(
        "WITH {signals},
         current_stats(deck_id, source_note_count, generated_card_count,
                       studyable_due_count, needs_more_practice) AS (
             SELECT c.deck_id, COUNT(DISTINCT c.note_id), COUNT(c.id),
                    COALESCE(SUM(CASE WHEN c.suspended_at IS NULL
                         AND (c.buried_until IS NULL OR c.buried_until <= ?{now_index})
                         AND s.due_at <= ?{now_index} THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN c.suspended_at IS NULL
                         AND (c.buried_until IS NULL OR c.buried_until <= ?{now_index})
                         THEN COALESCE(ls.needs_more_practice,0) ELSE 0 END), 0)
             FROM cards c JOIN notes n ON n.id=c.note_id
             JOIN decks cd ON cd.id=c.deck_id
             JOIN card_schedules s ON s.card_id=c.id
             LEFT JOIN learning_signals ls ON ls.card_id=c.id
             WHERE c.deleted_at IS NULL AND n.deleted_at IS NULL
               AND cd.deleted_at IS NULL AND cd.archived_at IS NULL
             GROUP BY c.deck_id
         ),
         history(deck_id, cards_practiced, review_count, correct_count,
                 average_confidence, average_response_time_ms,
                 total_study_time_ms, last_practiced_at) AS (
             SELECT a.deck_id, COUNT(DISTINCT r.card_id), COUNT(*),
                    COALESCE(SUM(r.answer_correct),0),
                    AVG(CASE r.confidence WHEN 'wrong' THEN 0.0
                        WHEN 'not_confident' THEN 1.0
                        WHEN 'somewhat_confident' THEN 2.0
                        WHEN 'very_confident' THEN 3.0 END),
                    AVG(r.response_time_ms), COALESCE(SUM(r.response_time_ms),0),
                    MAX(r.reviewed_at)
             FROM review_events r
             JOIN review_event_attributions a ON a.review_event_id=r.id
             JOIN decks hd ON hd.id=a.deck_id AND hd.deleted_at IS NULL
             WHERE {history_where}
             GROUP BY a.deck_id
         )
         SELECT d.id,d.name,d.archived_at,
                COALESCE(c.source_note_count,0),COALESCE(c.generated_card_count,0),
                COALESCE(c.studyable_due_count,0),COALESCE(c.needs_more_practice,0),
                COALESCE(h.cards_practiced,0),COALESCE(h.review_count,0),
                COALESCE(h.correct_count,0),h.average_confidence,
                h.average_response_time_ms,COALESCE(h.total_study_time_ms,0),
                h.last_practiced_at
         FROM decks d
         LEFT JOIN current_stats c ON c.deck_id=d.id
         LEFT JOIN history h ON h.deck_id=d.id
         WHERE d.deleted_at IS NULL
           AND (c.deck_id IS NOT NULL OR h.deck_id IS NOT NULL)
         ORDER BY COALESCE(h.last_practiced_at,0) DESC,d.name COLLATE NOCASE",
        signals = learning_signal_cte()
    );
    let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
    let rows = statement
        .query_map(params_from_iter(values.iter()), |r| {
            let reviews: i64 = r.get(8)?;
            let correct: i64 = r.get(9)?;
            Ok(DeckAnalyticsDto {
                deck_id: r.get(0)?,
                deck_name: r.get(1)?,
                archived: r.get::<_, Option<i64>>(2)?.is_some(),
                source_note_count: r.get(3)?,
                generated_card_count: r.get(4)?,
                studyable_due_count: r.get(5)?,
                needs_more_practice: r.get(6)?,
                cards_practiced: r.get(7)?,
                review_count: reviews,
                correct_count: correct,
                correct_rate: ratio(correct, reviews),
                average_confidence: r.get(10)?,
                average_response_time_ms: r.get(11)?,
                total_study_time_ms: r.get(12)?,
                last_practiced_at: r.get(13)?,
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    Ok(rows)
}

fn tag_stats(connection: &Connection, scope: &Scope) -> Result<Vec<TagAnalyticsDto>, CommandError> {
    let (history_where, mut values) = history_where(scope);
    let current_start_index = scope
        .start_date
        .map(|date| push(&mut values, date.format("%Y-%m-%d").to_string()));
    let previous_start_index = scope
        .previous_start
        .map(|date| push(&mut values, date.format("%Y-%m-%d").to_string()));
    let now_index = push(&mut values, scope.context.now_utc_millis);
    let previous_condition = match (previous_start_index, current_start_index) {
        (Some(previous), Some(current)) => {
            format!("a.review_local_date >= ?{previous} AND a.review_local_date < ?{current}")
        }
        _ => "0=1".to_string(),
    };
    let sql = format!(
        "WITH {signals},
         current_stats(tag_id, source_note_count, generated_card_count,
                       studyable_due_count, needs_more_practice) AS (
             SELECT nt.tag_id, COUNT(DISTINCT n.id), COUNT(DISTINCT c.id),
                    COALESCE(SUM(CASE WHEN c.suspended_at IS NULL
                         AND (c.buried_until IS NULL OR c.buried_until <= ?{now_index})
                         AND s.due_at <= ?{now_index} THEN 1 ELSE 0 END),0),
                    COALESCE(SUM(CASE WHEN c.suspended_at IS NULL
                         AND (c.buried_until IS NULL OR c.buried_until <= ?{now_index})
                         THEN COALESCE(ls.needs_more_practice,0) ELSE 0 END),0)
             FROM note_tags nt JOIN notes n ON n.id=nt.note_id
             JOIN cards c ON c.note_id=n.id JOIN decks d ON d.id=c.deck_id
             JOIN card_schedules s ON s.card_id=c.id
             LEFT JOIN learning_signals ls ON ls.card_id=c.id
             WHERE n.deleted_at IS NULL AND c.deleted_at IS NULL
               AND d.deleted_at IS NULL AND d.archived_at IS NULL
             GROUP BY nt.tag_id
         ),
         history(tag_id,captured_name,cards_practiced,review_count,correct_count,
                 average_confidence,average_response_time_ms,total_study_time_ms,
                 last_practiced_at) AS (
             SELECT ta.tag_id,MIN(ta.captured_name),COUNT(DISTINCT r.card_id),COUNT(*),
                    COALESCE(SUM(r.answer_correct),0),
                    AVG(CASE r.confidence WHEN 'wrong' THEN 0.0
                        WHEN 'not_confident' THEN 1.0
                        WHEN 'somewhat_confident' THEN 2.0
                        WHEN 'very_confident' THEN 3.0 END),
                    AVG(r.response_time_ms),COALESCE(SUM(r.response_time_ms),0),
                    MAX(r.reviewed_at)
             FROM review_events r
             JOIN review_event_attributions a ON a.review_event_id=r.id
             JOIN decks d ON d.id=a.deck_id AND d.deleted_at IS NULL
             JOIN review_event_tag_attributions ta ON ta.review_event_id=r.id
             WHERE {history_where}
             GROUP BY ta.tag_id
         ),
         previous(tag_id,correct_rate) AS (
             SELECT ta.tag_id,1.0*SUM(r.answer_correct)/NULLIF(COUNT(*),0)
             FROM review_events r
             JOIN review_event_attributions a ON a.review_event_id=r.id
             JOIN decks d ON d.id=a.deck_id AND d.deleted_at IS NULL
             JOIN review_event_tag_attributions ta ON ta.review_event_id=r.id
             WHERE {previous_condition}
             GROUP BY ta.tag_id
         ),
         dimensions(tag_id) AS (
             SELECT tag_id FROM current_stats UNION SELECT tag_id FROM history
         )
         SELECT dim.tag_id,COALESCE(t.name,h.captured_name),
                COALESCE(c.source_note_count,0),COALESCE(c.generated_card_count,0),
                COALESCE(c.studyable_due_count,0),COALESCE(c.needs_more_practice,0),
                COALESCE(h.cards_practiced,0),COALESCE(h.review_count,0),
                COALESCE(h.correct_count,0),h.average_confidence,
                h.average_response_time_ms,COALESCE(h.total_study_time_ms,0),
                h.last_practiced_at,p.correct_rate
         FROM dimensions dim LEFT JOIN tags t ON t.id=dim.tag_id
         LEFT JOIN current_stats c ON c.tag_id=dim.tag_id
         LEFT JOIN history h ON h.tag_id=dim.tag_id
         LEFT JOIN previous p ON p.tag_id=dim.tag_id
         ORDER BY COALESCE(h.last_practiced_at,0) DESC,
                  COALESCE(t.name,h.captured_name) COLLATE NOCASE",
        signals = learning_signal_cte()
    );
    let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
    let rows = statement
        .query_map(params_from_iter(values.iter()), |r| {
            let reviews: i64 = r.get(7)?;
            let correct: i64 = r.get(8)?;
            let rate = ratio(correct, reviews);
            let previous: Option<f64> = r.get(13)?;
            let status = if reviews < 3 {
                "building_data"
            } else if rate.unwrap_or(0.0) < 0.6 {
                "needs_attention"
            } else if rate.unwrap_or(0.0) >= 0.85 {
                "strong"
            } else {
                "on_track"
            };
            Ok(TagAnalyticsDto {
                tag_id: r.get(0)?,
                tag_name: r.get(1)?,
                source_note_count: r.get(2)?,
                generated_card_count: r.get(3)?,
                studyable_due_count: r.get(4)?,
                needs_more_practice: r.get(5)?,
                cards_practiced: r.get(6)?,
                review_count: reviews,
                correct_count: correct,
                correct_rate: rate,
                average_confidence: r.get(9)?,
                average_response_time_ms: r.get(10)?,
                total_study_time_ms: r.get(11)?,
                last_practiced_at: r.get(12)?,
                trend_delta: match (rate, previous) {
                    (Some(current), Some(old)) => Some(current - old),
                    _ => None,
                },
                status: status.to_string(),
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    Ok(rows)
}

fn timeline(
    connection: &Connection,
    scope: &Scope,
) -> Result<Vec<TimelineBucketDto>, CommandError> {
    let (where_sql, values) = history_where(scope);
    let sql=format!("SELECT a.review_local_date,COUNT(DISTINCT r.card_id),COUNT(*),SUM(CASE WHEN r.state_before=0 THEN 1 ELSE 0 END),SUM(r.answer_correct),AVG(CASE r.confidence WHEN 'wrong' THEN 0.0 WHEN 'not_confident' THEN 1.0 WHEN 'somewhat_confident' THEN 2.0 WHEN 'very_confident' THEN 3.0 END),AVG(r.response_time_ms),SUM(r.response_time_ms),COUNT(DISTINCT r.session_id) FROM review_events r JOIN review_event_attributions a ON a.review_event_id=r.id JOIN decks d ON d.id=a.deck_id AND d.deleted_at IS NULL WHERE {where_sql} GROUP BY a.review_local_date ORDER BY a.review_local_date");
    let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
    let rows = statement
        .query_map(params_from_iter(values.iter()), |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, i64>(4)?,
                r.get::<_, Option<f64>>(5)?,
                r.get::<_, Option<f64>>(6)?,
                r.get::<_, i64>(7)?,
                r.get::<_, i64>(8)?,
            ))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    let mut grouped: BTreeMap<String, TimelineBucketDto> = BTreeMap::new();
    for (date, cards, reviews, new_cards, remembered, confidence, response, time, sessions) in rows
    {
        let parsed = NaiveDate::parse_from_str(&date, "%Y-%m-%d").map_err(|_| {
            CommandError::new("invalid_analytics_date", "Stored review date is invalid.")
        })?;
        let key = match granularity(&scope.range) {
            "month" => format!("{:04}-{:02}", parsed.year(), parsed.month()),
            "week" => {
                let start = scope.start_date.unwrap_or(parsed);
                format!(
                    "w{}",
                    parsed.signed_duration_since(start).num_days().div_euclid(7)
                )
            }
            _ => date.clone(),
        };
        let start_date = match granularity(&scope.range) {
            "month" => NaiveDate::from_ymd_opt(parsed.year(), parsed.month(), 1).unwrap_or(parsed),
            "week" => parsed
                .checked_sub_signed(Duration::days(
                    parsed.weekday().num_days_from_monday().into(),
                ))
                .unwrap_or(parsed),
            _ => parsed,
        };
        let start = scope.context.resolve_date_start(start_date)?;
        let end_date = match granularity(&scope.range) {
            "month" => {
                if parsed.month() == 12 {
                    NaiveDate::from_ymd_opt(parsed.year() + 1, 1, 1)
                } else {
                    NaiveDate::from_ymd_opt(parsed.year(), parsed.month() + 1, 1)
                }
            }
            "week" => start_date.checked_add_signed(Duration::days(7)),
            _ => start_date.succ_opt(),
        }
        .ok_or_else(|| {
            CommandError::new(
                "invalid_analytics_date",
                "Analytics bucket is outside supported range.",
            )
        })?;
        let entry = grouped.entry(key).or_insert(TimelineBucketDto {
            start_at: start,
            end_at: scope.context.resolve_date_start(end_date)?,
            ..Default::default()
        });
        entry.cards_practiced += cards;
        entry.review_count += reviews;
        entry.new_cards += new_cards;
        entry.remembered_count += remembered;
        entry.study_time_ms += time;
        entry.session_count += sessions;
        entry.correct_rate = ratio(entry.remembered_count, entry.review_count);
        entry.average_confidence = confidence;
        entry.average_response_time_ms = response;
    }
    Ok(grouped.into_values().collect())
}

fn recent_reviews(
    connection: &Connection,
    scope: &Scope,
    limit: i64,
) -> Result<Vec<RecentReviewDto>, CommandError> {
    let (where_sql, mut values) = history_where(scope);
    let limit_index = push(&mut values, limit);
    let sql=format!("SELECT r.id,a.deck_id,d.name,r.card_id,a.question_excerpt,COALESCE((SELECT group_concat(name,char(31)) FROM (SELECT DISTINCT COALESCE(t.name,ta.captured_name) name FROM review_event_tag_attributions ta LEFT JOIN tags t ON t.id=ta.tag_id WHERE ta.review_event_id=r.id ORDER BY name COLLATE NOCASE)),''),r.confidence,r.rating,r.response_time_ms,r.reviewed_at,r.due_after,a.attribution_source FROM review_events r JOIN review_event_attributions a ON a.review_event_id=r.id JOIN decks d ON d.id=a.deck_id AND d.deleted_at IS NULL WHERE {where_sql} ORDER BY r.reviewed_at DESC,r.created_at DESC,r.id DESC LIMIT ?{limit_index}");
    let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
    let rows = statement
        .query_map(params_from_iter(values.iter()), |r| {
            Ok(RecentReviewDto {
                id: r.get(0)?,
                deck_id: r.get(1)?,
                deck_name: r.get(2)?,
                card_id: r.get(3)?,
                front_text: r.get(4)?,
                tags: split(r.get::<_, String>(5)?),
                outcome: r.get(6)?,
                rating: r.get(7)?,
                response_time_ms: r.get(8)?,
                reviewed_at: r.get(9)?,
                due_after: r.get(10)?,
                attribution_source: r.get(11)?,
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    Ok(rows)
}

fn recent_sessions(
    connection: &Connection,
    scope: &Scope,
) -> Result<Vec<SessionAnalyticsDto>, CommandError> {
    let (where_sql, mut values) = history_where(scope);
    let highlighted_index = push(&mut values, scope.highlighted.clone().unwrap_or_default());
    let sql=format!("SELECT ps.id,ps.mode,COALESCE((SELECT group_concat(name,char(31)) FROM (SELECT DISTINCT d2.name name FROM review_events r2 JOIN review_event_attributions a2 ON a2.review_event_id=r2.id JOIN decks d2 ON d2.id=a2.deck_id AND d2.deleted_at IS NULL WHERE r2.session_id=ps.id ORDER BY name COLLATE NOCASE)),''),ps.started_at,ps.completed_at,ps.card_count,ps.rated_count,ps.correct_count,COALESCE(SUM(r.response_time_ms),0),CASE WHEN ps.id=?{highlighted_index} THEN 1 ELSE 0 END FROM practice_sessions ps JOIN review_events r ON r.session_id=ps.id JOIN review_event_attributions a ON a.review_event_id=r.id JOIN decks d ON d.id=a.deck_id AND d.deleted_at IS NULL WHERE {where_sql} GROUP BY ps.id ORDER BY ps.started_at DESC LIMIT 12");
    let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
    let rows = statement
        .query_map(params_from_iter(values.iter()), |r| {
            let rated: i64 = r.get(6)?;
            let correct: i64 = r.get(7)?;
            let start: i64 = r.get(3)?;
            let completed: Option<i64> = r.get(4)?;
            Ok(SessionAnalyticsDto {
                id: r.get(0)?,
                mode: r.get(1)?,
                deck_names: split(r.get::<_, String>(2)?),
                started_at: start,
                completed_at: completed,
                duration_ms: completed
                    .unwrap_or(scope.context.now_utc_millis)
                    .saturating_sub(start),
                card_count: r.get(5)?,
                rated_count: rated,
                correct_count: correct,
                correct_rate: ratio(correct, rated),
                study_time_ms: r.get(8)?,
                highlighted: r.get::<_, i64>(9)? != 0,
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    Ok(rows)
}

fn dimensions(connection: &Connection) -> Result<AnalyticsDimensionOptionsDto, CommandError> {
    let mut ds=connection.prepare("SELECT id,name,archived_at FROM decks WHERE deleted_at IS NULL ORDER BY name COLLATE NOCASE").map_err(CommandError::database)?;
    let decks = ds
        .query_map([], |r| {
            Ok(AnalyticsDimensionOptionDto {
                id: r.get(0)?,
                name: r.get(1)?,
                archived: r.get::<_, Option<i64>>(2)?.is_some(),
                historical_only: false,
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    let mut ts=connection.prepare("SELECT dim.id,COALESCE(t.name,dim.name) FROM (SELECT tag_id id,MIN(captured_name) name FROM review_event_tag_attributions GROUP BY tag_id UNION SELECT id,name FROM tags) dim LEFT JOIN tags t ON t.id=dim.id GROUP BY dim.id ORDER BY COALESCE(t.name,dim.name) COLLATE NOCASE").map_err(CommandError::database)?;
    let tags = ts
        .query_map([], |r| {
            Ok(AnalyticsDimensionOptionDto {
                id: r.get(0)?,
                name: r.get(1)?,
                archived: false,
                historical_only: false,
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    Ok(AnalyticsDimensionOptionsDto { decks, tags })
}

fn attribution_coverage(
    connection: &Connection,
    scope: &Scope,
) -> Result<AttributionCoverageDto, CommandError> {
    let (where_sql, values) = history_where(scope);
    let sql=format!("SELECT SUM(CASE WHEN a.attribution_source='captured' THEN 1 ELSE 0 END),SUM(CASE WHEN a.attribution_source='legacy_current_state_backfill' THEN 1 ELSE 0 END) FROM review_events r JOIN review_event_attributions a ON a.review_event_id=r.id JOIN decks d ON d.id=a.deck_id AND d.deleted_at IS NULL WHERE {where_sql}");
    connection
        .query_row(&sql, params_from_iter(values.iter()), |r| {
            Ok(AttributionCoverageDto {
                captured_review_count: r.get::<_, Option<i64>>(0)?.unwrap_or(0),
                legacy_backfilled_review_count: r.get::<_, Option<i64>>(1)?.unwrap_or(0),
            })
        })
        .map_err(CommandError::database)
}

fn validate_scope(input: AnalyticsInput) -> Result<Scope, CommandError> {
    let mut decks = BTreeSet::new();
    for id in input.deck_ids {
        validate_entity_id(&id, "deck")?;
        decks.insert(id);
    }
    let mut tags = BTreeSet::new();
    for id in input.tag_ids {
        validate_entity_id(&id, "tag")?;
        tags.insert(id);
    }
    if decks.len() > MAX_SCOPE_DECKS || tags.len() > MAX_SCOPE_TAGS {
        return Err(CommandError::new(
            "analytics_scope_too_large",
            "The Analytics scope contains too many decks or tags.",
        ));
    }
    if let Some(id) = input.highlighted_session_id.as_deref() {
        validate_entity_id(id, "practice session")?;
    }
    let context = StudyTimeContext::capture()?;
    Ok(Scope {
        start_date: range_start_date(&context, &input.range)?,
        previous_start: previous_range_start_date(&context, &input.range)?,
        range: input.range,
        deck_ids: decks.into_iter().collect(),
        tag_ids: tags.into_iter().collect(),
        highlighted: input.highlighted_session_id,
        context,
    })
}

fn history_where(scope: &Scope) -> (String, Vec<SqlValue>) {
    let mut values = Vec::new();
    let mut c = vec!["1=1".to_string()];
    if let Some(date) = scope.start_date {
        values.push(date.format("%Y-%m-%d").to_string().into());
        c.push(format!("a.review_local_date >= ?{}", values.len()));
    }
    if !scope.deck_ids.is_empty() {
        let p = scope
            .deck_ids
            .iter()
            .map(|id| {
                values.push(id.clone().into());
                format!("?{}", values.len())
            })
            .collect::<Vec<_>>()
            .join(",");
        c.push(format!("a.deck_id IN ({p})"));
    }
    for id in &scope.tag_ids {
        values.push(id.clone().into());
        c.push(format!("EXISTS(SELECT 1 FROM review_event_tag_attributions fta WHERE fta.review_event_id=r.id AND fta.tag_id=?{})",values.len()));
    }
    (c.join(" AND "), values)
}
fn current_filters(scope: &Scope, values: &mut Vec<SqlValue>) -> String {
    let mut c = Vec::new();
    if !scope.deck_ids.is_empty() {
        let p = scope
            .deck_ids
            .iter()
            .map(|id| {
                values.push(id.clone().into());
                format!("?{}", values.len())
            })
            .collect::<Vec<_>>()
            .join(",");
        c.push(format!("c.deck_id IN ({p})"));
    }
    for id in &scope.tag_ids {
        values.push(id.clone().into());
        c.push(format!(
            "EXISTS(SELECT 1 FROM note_tags nt WHERE nt.note_id=n.id AND nt.tag_id=?{})",
            values.len()
        ));
    }
    if c.is_empty() {
        String::new()
    } else {
        format!(" AND {}", c.join(" AND "))
    }
}
fn granularity(range: &str) -> &'static str {
    match range {
        "90d" | "1y" => "week",
        "all" => "month",
        _ => "day",
    }
}
fn ratio(n: i64, d: i64) -> Option<f64> {
    (d > 0).then_some(n as f64 / d as f64)
}
fn split(value: String) -> Vec<String> {
    if value.is_empty() {
        vec![]
    } else {
        value.split(TAG_SEPARATOR).map(str::to_string).collect()
    }
}
fn push(values: &mut Vec<SqlValue>, value: impl Into<SqlValue>) -> usize {
    values.push(value.into());
    values.len()
}

#[cfg(test)]
mod stage36_tests;
