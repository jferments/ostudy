//! Bounded Analytics over current inventory and immutable review attribution.
//!
//! Current inventory uses the canonical studyability projection. Historical
//! metrics use only accepted review events plus immutable attribution. Retained
//! deck/tag identities display their current names; archived decks remain in
//! history; soft-deleted decks are excluded immediately and final deletion purges
//! their complete history.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{Datelike, Duration, NaiveDate};
use rusqlite::{params_from_iter, types::Value as SqlValue, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    database::Database,
    error::CommandError,
    projections::{learning_signals::LEARNING_SIGNAL_CTE, scope::placeholders},
    settings::load_settings,
    study_calendar::{CalendarWindow, StudyTimeContext},
    util::validate_entity_id,
};

const MAX_ANALYTICS_DECKS: usize = 256;
const MAX_ANALYTICS_TAGS: usize = 64;
const RECENT_REVIEW_LIMIT: i64 = 20;
const RECENT_SESSION_LIMIT: i64 = 12;

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
    pub start_at: Option<i64>,
    pub end_at: i64,
    pub bucket_granularity: String,
    pub deck_ids: Vec<String>,
    pub tag_ids: Vec<String>,
    pub highlighted_session_id: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrentInventorySummaryDto {
    pub current_deck_count: i64,
    pub source_note_count: i64,
    pub generated_card_count: i64,
    pub studyable_card_count: i64,
    pub studyable_due_count: i64,
    pub needs_more_practice_count: i64,
}

#[derive(Clone, Debug, Default, Serialize)]
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
    pub label: String,
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
    pub historical_only: bool,
    pub source_note_count: i64,
    pub generated_card_count: i64,
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
    pub deck_id: String,
    pub deck_name: String,
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
pub struct AnalyticsDimensionDto {
    pub id: String,
    pub name: String,
    pub historical_only: bool,
    pub archived: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsDimensionOptionsDto {
    pub decks: Vec<AnalyticsDimensionDto>,
    pub tags: Vec<AnalyticsDimensionDto>,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttributionCoverageDto {
    pub captured_review_count: i64,
    pub legacy_backfilled_review_count: i64,
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
    pub dimensions: AnalyticsDimensionOptionsDto,
    pub attribution_coverage: AttributionCoverageDto,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AnalyticsRange {
    Days7,
    Days30,
    Days90,
    Year1,
    All,
}

impl AnalyticsRange {
    fn parse(value: &str) -> Result<Self, CommandError> {
        match value {
            "7d" => Ok(Self::Days7),
            "30d" => Ok(Self::Days30),
            "90d" => Ok(Self::Days90),
            "1y" => Ok(Self::Year1),
            "all" => Ok(Self::All),
            _ => Err(CommandError::new(
                "invalid_analytics_range",
                "Choose 7 days, 30 days, 90 days, one year, or all time.",
            )),
        }
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::Days7 => "7d",
            Self::Days30 => "30d",
            Self::Days90 => "90d",
            Self::Year1 => "1y",
            Self::All => "all",
        }
    }

    const fn granularity(self) -> &'static str {
        match self {
            Self::Days7 | Self::Days30 => "day",
            Self::Days90 | Self::Year1 => "week",
            Self::All => "month",
        }
    }
}

#[derive(Clone, Debug)]
struct Scope {
    range: AnalyticsRange,
    deck_ids: Vec<String>,
    tag_ids: Vec<String>,
    highlighted_session_id: Option<String>,
    context: StudyTimeContext,
    current_window: Option<CalendarWindow>,
    previous_window: Option<CalendarWindow>,
}

#[derive(Clone, Debug, Default)]
struct HistoricalAggregate {
    cards_practiced: i64,
    review_count: i64,
    correct_count: i64,
    confidence_sum: i64,
    response_sum: i64,
    session_count: i64,
    last_practiced_at: Option<i64>,
}

impl HistoricalAggregate {
    fn correct_rate(&self) -> Option<f64> {
        ratio(self.correct_count, self.review_count)
    }

    fn average_confidence(&self) -> Option<f64> {
        ratio(self.confidence_sum, self.review_count)
    }

    fn average_response(&self) -> Option<f64> {
        ratio(self.response_sum, self.review_count)
    }
}

#[derive(Clone, Debug, Default)]
struct InventoryAggregate {
    source_notes: i64,
    generated_cards: i64,
    studyable_due: i64,
    needs_practice: i64,
}

#[derive(Clone, Debug)]
struct DailyHistoryRow {
    local_date: NaiveDate,
    cards_practiced: i64,
    review_count: i64,
    new_cards: i64,
    remembered_count: i64,
    confidence_sum: i64,
    response_sum: i64,
    study_time_ms: i64,
    session_count: i64,
    highlighted: bool,
}

#[derive(Clone, Debug, Default)]
struct BucketAccumulator {
    cards_practiced: i64,
    review_count: i64,
    new_cards: i64,
    remembered_count: i64,
    confidence_sum: i64,
    response_sum: i64,
    study_time_ms: i64,
    session_count: i64,
    highlighted: bool,
}

#[tauri::command]
pub fn get_analytics(
    database: State<'_, Database>,
    input: AnalyticsQueryDto,
) -> Result<AnalyticsDto, CommandError> {
    let scope = validate_scope(input)?;
    database.with_connection(|connection| analytics_in_connection(connection, &scope))
}

#[tauri::command]
pub fn get_recent_practice_reviews(
    database: State<'_, Database>,
) -> Result<Vec<RecentReviewDto>, CommandError> {
    let scope = validate_scope(AnalyticsQueryDto {
        range: "all".to_string(),
        deck_ids: Vec::new(),
        tag_ids: Vec::new(),
        highlighted_session_id: None,
    })?;
    database.with_connection(|connection| recent_reviews(connection, &scope, 12))
}

fn default_range() -> String {
    "7d".to_string()
}

fn validate_scope(input: AnalyticsQueryDto) -> Result<Scope, CommandError> {
    let range = AnalyticsRange::parse(&input.range)?;
    let deck_ids = normalized_ids(input.deck_ids, "deck", MAX_ANALYTICS_DECKS)?;
    let tag_ids = normalized_ids(input.tag_ids, "tag", MAX_ANALYTICS_TAGS)?;
    if let Some(session_id) = input.highlighted_session_id.as_deref() {
        validate_entity_id(session_id, "practice session")?;
    }
    let context = StudyTimeContext::capture()?;
    let current_window = match range {
        AnalyticsRange::Days7 => Some(context.calendar_days_window(7)?),
        AnalyticsRange::Days30 => Some(context.calendar_days_window(30)?),
        AnalyticsRange::Days90 => Some(context.calendar_days_window(90)?),
        AnalyticsRange::Year1 => Some(context.one_year_window()?),
        AnalyticsRange::All => None,
    };
    let previous_window = current_window
        .map(|window| context.previous_window(window))
        .transpose()?;
    Ok(Scope {
        range,
        deck_ids,
        tag_ids,
        highlighted_session_id: input.highlighted_session_id,
        context,
        current_window,
        previous_window,
    })
}

fn normalized_ids(
    values: Vec<String>,
    label: &str,
    maximum: usize,
) -> Result<Vec<String>, CommandError> {
    let mut unique = BTreeSet::new();
    for value in values {
        validate_entity_id(&value, label)?;
        unique.insert(value);
    }
    if unique.len() > maximum {
        return Err(CommandError::new(
            "analytics_scope_too_large",
            format!("Filter Analytics by at most {maximum} {label} values."),
        ));
    }
    Ok(unique.into_iter().collect())
}

fn analytics_in_connection(
    connection: &mut Connection,
    scope: &Scope,
) -> Result<AnalyticsDto, CommandError> {
    let current_inventory = current_inventory(connection, scope)?;
    let (history, rating_distribution) = historical_summary(connection, scope)?;
    let settings = load_settings(connection)?;
    let (new_completed, reviews_completed) = current_day_completed(connection, scope)?;
    Ok(AnalyticsDto {
        scope: AnalyticsScopeDto {
            range: scope.range.as_str().to_string(),
            start_at: scope.current_window.map(|window| window.start_utc_millis),
            end_at: scope.context.now_utc_millis,
            bucket_granularity: scope.range.granularity().to_string(),
            deck_ids: scope.deck_ids.clone(),
            tag_ids: scope.tag_ids.clone(),
            highlighted_session_id: scope.highlighted_session_id.clone(),
        },
        current_inventory,
        history,
        daily_limits: DailyLimitUsageDto {
            new_limit: settings.daily_new_limit,
            review_limit: settings.daily_review_limit,
            new_completed,
            reviews_completed,
            new_remaining: (settings.daily_new_limit - new_completed).max(0),
            reviews_remaining: (settings.daily_review_limit - reviews_completed).max(0),
        },
        rating_distribution,
        timeline: timeline(connection, scope)?,
        deck_stats: deck_stats(connection, scope)?,
        tag_stats: tag_stats(connection, scope)?,
        recent_reviews: recent_reviews(connection, scope, RECENT_REVIEW_LIMIT)?,
        recent_sessions: recent_sessions(connection, scope)?,
        dimensions: dimensions(connection)?,
        attribution_coverage: attribution_coverage(connection, scope)?,
    })
}

fn current_inventory(
    connection: &Connection,
    scope: &Scope,
) -> Result<CurrentInventorySummaryDto, CommandError> {
    let mut values = vec![scope.context.now_utc_millis.into()];
    let filters = current_filters(scope, &mut values, "c", "n");
    let sql = format!(
        "WITH {signals}, scoped AS (
             SELECT c.id, c.note_id, c.deck_id, s.due_at,
                    c.suspended_at, c.buried_until,
                    COALESCE(ls.needs_more_practice, 0) AS needs_more_practice
             FROM cards c
             JOIN notes n ON n.id = c.note_id
             JOIN decks d ON d.id = c.deck_id
             JOIN card_schedules s ON s.card_id = c.id
             LEFT JOIN learning_signals ls ON ls.card_id = c.id
             WHERE c.deleted_at IS NULL AND n.deleted_at IS NULL
               AND d.deleted_at IS NULL AND d.archived_at IS NULL {filters}
         )
         SELECT COUNT(DISTINCT deck_id), COUNT(DISTINCT note_id), COUNT(id),
                COALESCE(SUM(CASE
                    WHEN suspended_at IS NULL
                     AND (buried_until IS NULL OR buried_until <= ?1)
                    THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE
                    WHEN suspended_at IS NULL
                     AND (buried_until IS NULL OR buried_until <= ?1)
                     AND due_at <= ?1
                    THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE
                    WHEN suspended_at IS NULL
                     AND (buried_until IS NULL OR buried_until <= ?1)
                    THEN needs_more_practice ELSE 0 END), 0)
         FROM scoped",
        signals = LEARNING_SIGNAL_CTE,
    );
    connection
        .query_row(&sql, params_from_iter(values.iter()), |row| {
            Ok(CurrentInventorySummaryDto {
                current_deck_count: row.get(0)?,
                source_note_count: row.get(1)?,
                generated_card_count: row.get(2)?,
                studyable_card_count: row.get(3)?,
                studyable_due_count: row.get(4)?,
                needs_more_practice_count: row.get(5)?,
            })
        })
        .map_err(CommandError::database)
}

fn historical_summary(
    connection: &Connection,
    scope: &Scope,
) -> Result<(HistoricalReviewSummaryDto, RatingDistributionDto), CommandError> {
    let (where_sql, values) = historical_where(scope, None);
    let sql = format!(
        "SELECT COUNT(DISTINCT r.card_id), COUNT(*),
                COALESCE(SUM(r.answer_correct), 0),
                COALESCE(SUM(CASE r.confidence
                    WHEN 'wrong' THEN 0 WHEN 'not_confident' THEN 1
                    WHEN 'somewhat_confident' THEN 2 WHEN 'very_confident' THEN 3
                    ELSE 0 END), 0),
                COALESCE(SUM(r.response_time_ms), 0),
                COUNT(DISTINCT r.session_id),
                MAX(r.reviewed_at),
                COALESCE(SUM(CASE WHEN r.rating = 1 THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN r.rating = 2 THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN r.rating = 3 THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN r.rating = 4 THEN 1 ELSE 0 END), 0)
         FROM review_events r
         JOIN review_event_attributions a ON a.review_event_id = r.id
         JOIN decks d ON d.id = a.deck_id AND d.deleted_at IS NULL
         WHERE {where_sql}"
    );
    let result: (i64, i64, i64, i64, i64, i64, Option<i64>, i64, i64, i64, i64) =
        connection
            .query_row(&sql, params_from_iter(values.iter()), |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                    row.get(9)?,
                    row.get(10)?,
                ))
            })
            .map_err(CommandError::database)?;
    Ok((
        HistoricalReviewSummaryDto {
            cards_practiced: result.0,
            review_count: result.1,
            correct_count: result.2,
            correct_rate: ratio(result.2, result.1),
            average_confidence: ratio(result.3, result.1),
            average_response_time_ms: ratio(result.4, result.1),
            total_study_time_ms: result.4,
            session_count: result.5,
        },
        RatingDistributionDto {
            again: result.7,
            hard: result.8,
            good: result.9,
            easy: result.10,
        },
    ))
}

fn current_day_completed(
    connection: &Connection,
    scope: &Scope,
) -> Result<(i64, i64), CommandError> {
    let mut values = vec![scope.context.local_date_string().into()];
    let filters = retained_history_scope_filters(scope, &mut values, "a");
    connection
        .query_row(
            &format!(
                "SELECT
                     COALESCE(SUM(CASE WHEN r.state_before = 0 THEN 1 ELSE 0 END), 0),
                     COALESCE(SUM(CASE WHEN r.state_before <> 0 THEN 1 ELSE 0 END), 0)
                 FROM review_events r
                 JOIN review_event_attributions a ON a.review_event_id = r.id
                 JOIN decks d ON d.id = a.deck_id AND d.deleted_at IS NULL
                 WHERE a.review_local_date = ?1 {filters}"
            ),
            params_from_iter(values.iter()),
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(CommandError::database)
}

fn deck_stats(
    connection: &Connection,
    scope: &Scope,
) -> Result<Vec<DeckAnalyticsDto>, CommandError> {
    let inventory = deck_inventory(connection, scope)?;
    let historical = deck_history(connection, scope, scope.current_window)?;
    let ids = inventory
        .keys()
        .chain(historical.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut result = Vec::new();
    for deck_id in ids {
        let Some((name, archived)) = connection
            .query_row(
                "SELECT name, archived_at IS NOT NULL
                 FROM decks WHERE id = ?1 AND deleted_at IS NULL",
                [deck_id.as_str()],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? != 0)),
            )
            .optional()
            .map_err(CommandError::database)?
        else {
            continue;
        };
        let current = inventory.get(&deck_id).cloned().unwrap_or_default();
        let history = historical.get(&deck_id).cloned().unwrap_or_default();
        result.push(DeckAnalyticsDto {
            deck_id,
            deck_name: name,
            archived,
            source_note_count: current.source_notes,
            generated_card_count: current.generated_cards,
            studyable_due_count: current.studyable_due,
            needs_more_practice_count: current.needs_practice,
            cards_practiced: history.cards_practiced,
            review_count: history.review_count,
            correct_count: history.correct_count,
            correct_rate: history.correct_rate(),
            average_confidence: history.average_confidence(),
            average_response_time_ms: history.average_response(),
            total_study_time_ms: history.response_sum,
            last_practiced_at: history.last_practiced_at,
        });
    }
    result.sort_by(|left, right| {
        right
            .review_count
            .cmp(&left.review_count)
            .then_with(|| left.deck_name.to_lowercase().cmp(&right.deck_name.to_lowercase()))
    });
    Ok(result)
}

fn deck_inventory(
    connection: &Connection,
    scope: &Scope,
) -> Result<BTreeMap<String, InventoryAggregate>, CommandError> {
    let mut values = vec![scope.context.now_utc_millis.into()];
    let filters = current_filters(scope, &mut values, "c", "n");
    let sql = format!(
        "WITH {signals}
         SELECT c.deck_id, COUNT(DISTINCT c.note_id), COUNT(c.id),
                COALESCE(SUM(CASE
                    WHEN c.suspended_at IS NULL
                     AND (c.buried_until IS NULL OR c.buried_until <= ?1)
                     AND s.due_at <= ?1 THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE
                    WHEN c.suspended_at IS NULL
                     AND (c.buried_until IS NULL OR c.buried_until <= ?1)
                    THEN COALESCE(ls.needs_more_practice, 0) ELSE 0 END), 0)
         FROM cards c
         JOIN notes n ON n.id = c.note_id
         JOIN decks d ON d.id = c.deck_id
         JOIN card_schedules s ON s.card_id = c.id
         LEFT JOIN learning_signals ls ON ls.card_id = c.id
         WHERE c.deleted_at IS NULL AND n.deleted_at IS NULL
           AND d.deleted_at IS NULL AND d.archived_at IS NULL {filters}
         GROUP BY c.deck_id",
        signals = LEARNING_SIGNAL_CTE,
    );
    let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
    let rows = statement
        .query_map(params_from_iter(values.iter()), |row| {
            Ok((
                row.get::<_, String>(0)?,
                InventoryAggregate {
                    source_notes: row.get(1)?,
                    generated_cards: row.get(2)?,
                    studyable_due: row.get(3)?,
                    needs_practice: row.get(4)?,
                },
            ))
        })
        .map_err(CommandError::database)?;
    rows.collect::<Result<BTreeMap<_, _>, _>>()
        .map_err(CommandError::database)
}

fn deck_history(
    connection: &Connection,
    scope: &Scope,
    window: Option<CalendarWindow>,
) -> Result<BTreeMap<String, HistoricalAggregate>, CommandError> {
    let (where_sql, values) = historical_where(scope, window);
    let sql = format!(
        "SELECT a.deck_id, COUNT(DISTINCT r.card_id), COUNT(*),
                COALESCE(SUM(r.answer_correct), 0),
                COALESCE(SUM(CASE r.confidence
                    WHEN 'wrong' THEN 0 WHEN 'not_confident' THEN 1
                    WHEN 'somewhat_confident' THEN 2 WHEN 'very_confident' THEN 3
                    ELSE 0 END), 0),
                COALESCE(SUM(r.response_time_ms), 0),
                COUNT(DISTINCT r.session_id), MAX(r.reviewed_at)
         FROM review_events r
         JOIN review_event_attributions a ON a.review_event_id = r.id
         JOIN decks d ON d.id = a.deck_id AND d.deleted_at IS NULL
         WHERE {where_sql}
         GROUP BY a.deck_id"
    );
    let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
    let rows = statement
        .query_map(params_from_iter(values.iter()), |row| {
            Ok((
                row.get::<_, String>(0)?,
                HistoricalAggregate {
                    cards_practiced: row.get(1)?,
                    review_count: row.get(2)?,
                    correct_count: row.get(3)?,
                    confidence_sum: row.get(4)?,
                    response_sum: row.get(5)?,
                    session_count: row.get(6)?,
                    last_practiced_at: row.get(7)?,
                },
            ))
        })
        .map_err(CommandError::database)?;
    rows.collect::<Result<BTreeMap<_, _>, _>>()
        .map_err(CommandError::database)
}

fn tag_stats(
    connection: &Connection,
    scope: &Scope,
) -> Result<Vec<TagAnalyticsDto>, CommandError> {
    let inventory = tag_inventory(connection, scope)?;
    let historical = tag_history(connection, scope, scope.current_window)?;
    let previous = tag_history(connection, scope, scope.previous_window)?;
    let ids = inventory
        .keys()
        .chain(historical.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut result = Vec::new();
    for tag_id in ids {
        let current_name = connection
            .query_row(
                "SELECT name FROM tags WHERE id = ?1",
                [tag_id.as_str()],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(CommandError::database)?;
        let captured_name = connection
            .query_row(
                "SELECT captured_name
                 FROM review_event_tag_attributions
                 WHERE tag_id = ?1
                 ORDER BY review_event_id DESC LIMIT 1",
                [tag_id.as_str()],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(CommandError::database)?;
        let Some(name) = current_name.or(captured_name) else {
            continue;
        };
        let current = inventory.get(&tag_id).cloned().unwrap_or_default();
        let history = historical.get(&tag_id).cloned().unwrap_or_default();
        let prior = previous.get(&tag_id).cloned().unwrap_or_default();
        let trend_delta = trend(history.correct_rate(), prior.correct_rate());
        let status = if history.review_count == 0 {
            "not_practiced".to_string()
        } else if history.correct_rate().unwrap_or_default() >= 0.85 {
            "strong".to_string()
        } else if history.correct_rate().unwrap_or_default() >= 0.65 {
            "developing".to_string()
        } else {
            "needs_attention".to_string()
        };
        result.push(TagAnalyticsDto {
            tag_id,
            tag_name: name,
            historical_only: current.source_notes == 0,
            source_note_count: current.source_notes,
            generated_card_count: current.generated_cards,
            studyable_due_count: current.studyable_due,
            needs_more_practice_count: current.needs_practice,
            cards_practiced: history.cards_practiced,
            review_count: history.review_count,
            correct_count: history.correct_count,
            correct_rate: history.correct_rate(),
            average_confidence: history.average_confidence(),
            average_response_time_ms: history.average_response(),
            total_study_time_ms: history.response_sum,
            last_practiced_at: history.last_practiced_at,
            trend_delta,
            status,
        });
    }
    result.sort_by(|left, right| {
        right
            .review_count
            .cmp(&left.review_count)
            .then_with(|| left.tag_name.to_lowercase().cmp(&right.tag_name.to_lowercase()))
    });
    Ok(result)
}

fn tag_inventory(
    connection: &Connection,
    scope: &Scope,
) -> Result<BTreeMap<String, InventoryAggregate>, CommandError> {
    let mut values = vec![scope.context.now_utc_millis.into()];
    let filters = current_filters(scope, &mut values, "c", "n");
    let sql = format!(
        "WITH {signals}
         SELECT t.id, COUNT(DISTINCT n.id), COUNT(DISTINCT c.id),
                COUNT(DISTINCT CASE
                    WHEN c.suspended_at IS NULL
                     AND (c.buried_until IS NULL OR c.buried_until <= ?1)
                     AND s.due_at <= ?1 THEN c.id END),
                COALESCE(SUM(DISTINCT CASE
                    WHEN c.suspended_at IS NULL
                     AND (c.buried_until IS NULL OR c.buried_until <= ?1)
                     AND COALESCE(ls.needs_more_practice, 0) = 1
                    THEN 1 ELSE 0 END), 0)
         FROM tags t
         JOIN note_tags nt ON nt.tag_id = t.id
         JOIN notes n ON n.id = nt.note_id
         JOIN cards c ON c.note_id = n.id
         JOIN decks d ON d.id = c.deck_id
         JOIN card_schedules s ON s.card_id = c.id
         LEFT JOIN learning_signals ls ON ls.card_id = c.id
         WHERE c.deleted_at IS NULL AND n.deleted_at IS NULL
           AND d.deleted_at IS NULL AND d.archived_at IS NULL {filters}
         GROUP BY t.id",
        signals = LEARNING_SIGNAL_CTE,
    );
    let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
    let rows = statement
        .query_map(params_from_iter(values.iter()), |row| {
            Ok((
                row.get::<_, String>(0)?,
                InventoryAggregate {
                    source_notes: row.get(1)?,
                    generated_cards: row.get(2)?,
                    studyable_due: row.get(3)?,
                    needs_practice: row.get(4)?,
                },
            ))
        })
        .map_err(CommandError::database)?;
    rows.collect::<Result<BTreeMap<_, _>, _>>()
        .map_err(CommandError::database)
}

fn tag_history(
    connection: &Connection,
    scope: &Scope,
    window: Option<CalendarWindow>,
) -> Result<BTreeMap<String, HistoricalAggregate>, CommandError> {
    let (where_sql, values) = historical_where(scope, window);
    let sql = format!(
        "SELECT ta.tag_id, COUNT(DISTINCT r.card_id), COUNT(*),
                COALESCE(SUM(r.answer_correct), 0),
                COALESCE(SUM(CASE r.confidence
                    WHEN 'wrong' THEN 0 WHEN 'not_confident' THEN 1
                    WHEN 'somewhat_confident' THEN 2 WHEN 'very_confident' THEN 3
                    ELSE 0 END), 0),
                COALESCE(SUM(r.response_time_ms), 0),
                COUNT(DISTINCT r.session_id), MAX(r.reviewed_at)
         FROM review_events r
         JOIN review_event_attributions a ON a.review_event_id = r.id
         JOIN decks d ON d.id = a.deck_id AND d.deleted_at IS NULL
         JOIN review_event_tag_attributions ta ON ta.review_event_id = r.id
         WHERE {where_sql}
         GROUP BY ta.tag_id"
    );
    let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
    let rows = statement
        .query_map(params_from_iter(values.iter()), |row| {
            Ok((
                row.get::<_, String>(0)?,
                HistoricalAggregate {
                    cards_practiced: row.get(1)?,
                    review_count: row.get(2)?,
                    correct_count: row.get(3)?,
                    confidence_sum: row.get(4)?,
                    response_sum: row.get(5)?,
                    session_count: row.get(6)?,
                    last_practiced_at: row.get(7)?,
                },
            ))
        })
        .map_err(CommandError::database)?;
    rows.collect::<Result<BTreeMap<_, _>, _>>()
        .map_err(CommandError::database)
}

fn timeline(
    connection: &Connection,
    scope: &Scope,
) -> Result<Vec<TimelineBucketDto>, CommandError> {
    let rows = daily_history(connection, scope)?;
    let bounds = timeline_bounds(connection, scope)?;
    let Some((first, last)) = bounds else {
        return Ok(Vec::new());
    };
    let granularity = scope.range.granularity();
    let mut buckets = BTreeMap::<NaiveDate, BucketAccumulator>::new();
    for row in rows {
        let key = bucket_start(row.local_date, granularity)?;
        let bucket = buckets.entry(key).or_default();
        bucket.cards_practiced += row.cards_practiced;
        bucket.review_count += row.review_count;
        bucket.new_cards += row.new_cards;
        bucket.remembered_count += row.remembered_count;
        bucket.confidence_sum += row.confidence_sum;
        bucket.response_sum += row.response_sum;
        bucket.study_time_ms += row.study_time_ms;
        bucket.session_count += row.session_count;
        bucket.highlighted |= row.highlighted;
    }
    let mut cursor = bucket_start(first, granularity)?;
    let end = bucket_start(last, granularity)?;
    let mut result = Vec::new();
    while cursor <= end {
        let next = next_bucket(cursor, granularity)?;
        let item = buckets.remove(&cursor).unwrap_or_default();
        result.push(TimelineBucketDto {
            label: bucket_label(cursor, granularity),
            start_at: scope.context.resolve_date_start(cursor)?,
            end_at: scope
                .context
                .resolve_date_start(next)?
                .saturating_sub(1),
            cards_practiced: item.cards_practiced,
            review_count: item.review_count,
            new_cards: item.new_cards,
            remembered_count: item.remembered_count,
            correct_rate: ratio(item.remembered_count, item.review_count),
            average_confidence: ratio(item.confidence_sum, item.review_count),
            average_response_time_ms: ratio(item.response_sum, item.review_count),
            study_time_ms: item.study_time_ms,
            session_count: item.session_count,
            contains_highlighted_session: item.highlighted,
        });
        cursor = next;
    }
    Ok(result)
}

fn daily_history(
    connection: &Connection,
    scope: &Scope,
) -> Result<Vec<DailyHistoryRow>, CommandError> {
    let (where_sql, mut values) = historical_where(scope, None);
    let highlighted_index = scope.highlighted_session_id.as_ref().map(|session_id| {
        values.push(SqlValue::Text(session_id.clone()));
        values.len()
    });
    let highlighted_sql = highlighted_index
        .map(|index| format!("MAX(CASE WHEN r.session_id = ?{index} THEN 1 ELSE 0 END)"))
        .unwrap_or_else(|| "0".to_string());
    let sql = format!(
        "SELECT a.review_local_date, COUNT(DISTINCT r.card_id), COUNT(*),
                COALESCE(SUM(CASE WHEN r.state_before = 0 THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(r.answer_correct), 0),
                COALESCE(SUM(CASE r.confidence
                    WHEN 'wrong' THEN 0 WHEN 'not_confident' THEN 1
                    WHEN 'somewhat_confident' THEN 2 WHEN 'very_confident' THEN 3
                    ELSE 0 END), 0),
                COALESCE(SUM(r.response_time_ms), 0),
                COALESCE(SUM(r.response_time_ms), 0),
                COUNT(DISTINCT r.session_id), {highlighted_sql}
         FROM review_events r
         JOIN review_event_attributions a ON a.review_event_id = r.id
         JOIN decks d ON d.id = a.deck_id AND d.deleted_at IS NULL
         WHERE {where_sql}
         GROUP BY a.review_local_date
         ORDER BY a.review_local_date"
    );
    let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
    let rows = statement
        .query_map(params_from_iter(values.iter()), |row| {
            let date: String = row.get(0)?;
            Ok((
                date,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, i64>(7)?,
                row.get::<_, i64>(8)?,
                row.get::<_, i64>(9)?,
            ))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    rows.into_iter()
        .map(
            |(
                date,
                cards_practiced,
                review_count,
                new_cards,
                remembered_count,
                confidence_sum,
                response_sum,
                study_time_ms,
                session_count,
                highlighted,
            )| {
                Ok(DailyHistoryRow {
                    local_date: NaiveDate::parse_from_str(&date, "%Y-%m-%d").map_err(|_| {
                        CommandError::new(
                            "invalid_review_local_date",
                            "A historical review contains an invalid local date.",
                        )
                    })?,
                    cards_practiced,
                    review_count,
                    new_cards,
                    remembered_count,
                    confidence_sum,
                    response_sum,
                    study_time_ms,
                    session_count,
                    highlighted: highlighted != 0,
                })
            },
        )
        .collect()
}

fn timeline_bounds(
    connection: &Connection,
    scope: &Scope,
) -> Result<Option<(NaiveDate, NaiveDate)>, CommandError> {
    if let Some(window) = scope.current_window {
        return Ok(Some((window.start_local_date, window.end_local_date)));
    }
    let (where_sql, values) = historical_where(scope, None);
    let dates: Option<(String, String)> = connection
        .query_row(
            &format!(
                "SELECT MIN(a.review_local_date), MAX(a.review_local_date)
                 FROM review_events r
                 JOIN review_event_attributions a ON a.review_event_id = r.id
                 JOIN decks d ON d.id = a.deck_id AND d.deleted_at IS NULL
                 WHERE {where_sql}"
            ),
            params_from_iter(values.iter()),
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(CommandError::database)?;
    let Some((first, last)) = dates else {
        return Ok(None);
    };
    Ok(Some((
        NaiveDate::parse_from_str(&first, "%Y-%m-%d").map_err(|_| {
            CommandError::new(
                "invalid_review_local_date",
                "Historical Analytics contains an invalid first local date.",
            )
        })?,
        NaiveDate::parse_from_str(&last, "%Y-%m-%d").map_err(|_| {
            CommandError::new(
                "invalid_review_local_date",
                "Historical Analytics contains an invalid last local date.",
            )
        })?,
    )))
}

fn recent_reviews(
    connection: &Connection,
    scope: &Scope,
    limit: i64,
) -> Result<Vec<RecentReviewDto>, CommandError> {
    let (where_sql, mut values) = historical_where(scope, None);
    values.push(SqlValue::Integer(limit.min(RECENT_REVIEW_LIMIT).max(1)));
    let limit_index = values.len();
    let sql = format!(
        "SELECT r.id, a.deck_id, d.name, r.card_id, a.card_label,
                a.question_excerpt,
                COALESCE((
                    SELECT group_concat(name, char(31)) FROM (
                        SELECT COALESCE(t.name, ta.captured_name) AS name
                        FROM review_event_tag_attributions ta
                        LEFT JOIN tags t ON t.id = ta.tag_id
                        WHERE ta.review_event_id = r.id
                        ORDER BY name COLLATE NOCASE
                    )
                ), ''),
                r.confidence, r.rating, r.response_time_ms,
                r.reviewed_at, a.review_local_date, r.due_after,
                a.attribution_source
         FROM review_events r
         JOIN review_event_attributions a ON a.review_event_id = r.id
         JOIN decks d ON d.id = a.deck_id AND d.deleted_at IS NULL
         WHERE {where_sql}
         ORDER BY r.reviewed_at DESC, r.created_at DESC, r.id DESC
         LIMIT ?{limit_index}"
    );
    let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
    statement
        .query_map(params_from_iter(values.iter()), |row| {
            let tags: String = row.get(6)?;
            Ok(RecentReviewDto {
                id: row.get(0)?,
                deck_id: row.get(1)?,
                deck_name: row.get(2)?,
                card_id: row.get(3)?,
                card_label: row.get(4)?,
                question_excerpt: row.get(5)?,
                tags: split_names(&tags),
                outcome: row.get(7)?,
                rating: row.get(8)?,
                response_time_ms: row.get(9)?,
                reviewed_at: row.get(10)?,
                review_local_date: row.get(11)?,
                due_after: row.get(12)?,
                attribution_source: row.get(13)?,
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)
}

fn recent_sessions(
    connection: &Connection,
    scope: &Scope,
) -> Result<Vec<SessionAnalyticsDto>, CommandError> {
    let (where_sql, mut values) = historical_where(scope, None);
    values.push(SqlValue::Integer(RECENT_SESSION_LIMIT));
    let limit_index = values.len();
    let sql = format!(
        "SELECT r.session_id, MAX(ps.mode), MIN(ps.started_at),
                MAX(ps.completed_at), COUNT(*),
                COALESCE(SUM(r.answer_correct), 0),
                COALESCE(SUM(r.response_time_ms), 0),
                COALESCE((
                    SELECT group_concat(name, char(31)) FROM (
                        SELECT DISTINCT d2.name AS name
                        FROM review_events r2
                        JOIN review_event_attributions a2
                          ON a2.review_event_id = r2.id
                        JOIN decks d2 ON d2.id = a2.deck_id
                                     AND d2.deleted_at IS NULL
                        WHERE r2.session_id = r.session_id
                        ORDER BY name COLLATE NOCASE
                    )
                ), '')
         FROM review_events r
         JOIN review_event_attributions a ON a.review_event_id = r.id
         JOIN decks d ON d.id = a.deck_id AND d.deleted_at IS NULL
         JOIN practice_sessions ps ON ps.id = r.session_id
         WHERE {where_sql}
         GROUP BY r.session_id
         ORDER BY MIN(ps.started_at) DESC, r.session_id DESC
         LIMIT ?{limit_index}"
    );
    let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
    statement
        .query_map(params_from_iter(values.iter()), |row| {
            let session_id: String = row.get(0)?;
            let started_at: i64 = row.get(2)?;
            let completed_at: Option<i64> = row.get(3)?;
            let rated_count: i64 = row.get(4)?;
            let correct_count: i64 = row.get(5)?;
            let study_time_ms: i64 = row.get(6)?;
            let deck_names: String = row.get(7)?;
            Ok(SessionAnalyticsDto {
                highlighted: scope.highlighted_session_id.as_deref() == Some(&session_id),
                id: session_id,
                mode: row.get(1)?,
                deck_names: split_names(&deck_names),
                started_at,
                completed_at,
                duration_ms: completed_at
                    .unwrap_or(started_at.saturating_add(study_time_ms))
                    .saturating_sub(started_at)
                    .max(0),
                card_count: rated_count,
                rated_count,
                correct_count,
                correct_rate: ratio(correct_count, rated_count),
                study_time_ms,
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)
}

fn dimensions(connection: &Connection) -> Result<AnalyticsDimensionOptionsDto, CommandError> {
    let mut decks = connection
        .prepare(
            "SELECT d.id, d.name, d.archived_at IS NOT NULL,
                    NOT EXISTS(
                        SELECT 1 FROM notes n
                        WHERE n.deck_id = d.id AND n.deleted_at IS NULL
                    )
             FROM decks d
             WHERE d.deleted_at IS NULL
               AND (
                    EXISTS(
                        SELECT 1 FROM notes n
                        WHERE n.deck_id = d.id AND n.deleted_at IS NULL
                    )
                    OR EXISTS(
                        SELECT 1 FROM review_event_attributions a
                        WHERE a.deck_id = d.id
                    )
               )
             ORDER BY d.name COLLATE NOCASE, d.id",
        )
        .map_err(CommandError::database)?
        .query_map([], |row| {
            Ok(AnalyticsDimensionDto {
                id: row.get(0)?,
                name: row.get(1)?,
                archived: row.get::<_, i64>(2)? != 0,
                historical_only: row.get::<_, i64>(3)? != 0,
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    decks.sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));

    let tags = connection
        .prepare(
            "SELECT identity.tag_id,
                    COALESCE(t.name, identity.captured_name),
                    t.id IS NULL
             FROM (
                 SELECT tag_id, MAX(captured_name) AS captured_name
                 FROM review_event_tag_attributions
                 GROUP BY tag_id
                 UNION
                 SELECT id, name FROM tags
             ) identity
             LEFT JOIN tags t ON t.id = identity.tag_id
             WHERE EXISTS (
                 SELECT 1
                 FROM review_event_tag_attributions ta
                 JOIN review_event_attributions a
                   ON a.review_event_id = ta.review_event_id
                 JOIN decks d ON d.id = a.deck_id AND d.deleted_at IS NULL
                 WHERE ta.tag_id = identity.tag_id
             )
             OR EXISTS (
                 SELECT 1 FROM note_tags nt
                 JOIN notes n ON n.id = nt.note_id
                 JOIN decks d ON d.id = n.deck_id
                 WHERE nt.tag_id = identity.tag_id
                   AND n.deleted_at IS NULL
                   AND d.deleted_at IS NULL
                   AND d.archived_at IS NULL
             )
             ORDER BY 2 COLLATE NOCASE, identity.tag_id",
        )
        .map_err(CommandError::database)?
        .query_map([], |row| {
            Ok(AnalyticsDimensionDto {
                id: row.get(0)?,
                name: row.get(1)?,
                historical_only: row.get::<_, i64>(2)? != 0,
                archived: false,
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
    let (where_sql, values) = historical_where(scope, None);
    connection
        .query_row(
            &format!(
                "SELECT
                     COALESCE(SUM(CASE
                         WHEN a.attribution_source = 'captured' THEN 1 ELSE 0 END
                     ), 0),
                     COALESCE(SUM(CASE
                         WHEN a.attribution_source = 'legacy_current_state_backfill'
                         THEN 1 ELSE 0 END
                     ), 0)
                 FROM review_events r
                 JOIN review_event_attributions a ON a.review_event_id = r.id
                 JOIN decks d ON d.id = a.deck_id AND d.deleted_at IS NULL
                 WHERE {where_sql}"
            ),
            params_from_iter(values.iter()),
            |row| {
                Ok(AttributionCoverageDto {
                    captured_review_count: row.get(0)?,
                    legacy_backfilled_review_count: row.get(1)?,
                })
            },
        )
        .map_err(CommandError::database)
}

fn current_filters(
    scope: &Scope,
    values: &mut Vec<SqlValue>,
    card_alias: &str,
    note_alias: &str,
) -> String {
    let mut filters = Vec::new();
    if !scope.deck_ids.is_empty() {
        filters.push(format!(
            "AND {card_alias}.deck_id IN ({})",
            placeholders(values, scope.deck_ids.clone())
        ));
    }
    for tag_id in &scope.tag_ids {
        values.push(SqlValue::Text(tag_id.clone()));
        filters.push(format!(
            "AND EXISTS (
                 SELECT 1 FROM note_tags scope_nt
                 WHERE scope_nt.note_id = {note_alias}.id
                   AND scope_nt.tag_id = ?{}
             )",
            values.len()
        ));
    }
    filters.join(" ")
}

fn historical_where(
    scope: &Scope,
    window_override: Option<CalendarWindow>,
) -> (String, Vec<SqlValue>) {
    let mut values = Vec::<SqlValue>::new();
    let mut filters = vec!["1 = 1".to_string()];
    let window = window_override.or(scope.current_window);
    if let Some(window) = window {
        values.push(SqlValue::Integer(window.start_utc_millis));
        filters.push(format!("r.reviewed_at >= ?{}", values.len()));
        values.push(SqlValue::Integer(window.end_utc_millis));
        filters.push(format!("r.reviewed_at <= ?{}", values.len()));
    }
    if !scope.deck_ids.is_empty() {
        filters.push(format!(
            "a.deck_id IN ({})",
            placeholders(&mut values, scope.deck_ids.clone())
        ));
    }
    for tag_id in &scope.tag_ids {
        values.push(SqlValue::Text(tag_id.clone()));
        filters.push(format!(
            "EXISTS (
                 SELECT 1 FROM review_event_tag_attributions scope_ta
                 WHERE scope_ta.review_event_id = r.id
                   AND scope_ta.tag_id = ?{}
             )",
            values.len()
        ));
    }
    (filters.join(" AND "), values)
}

fn retained_history_scope_filters(
    scope: &Scope,
    values: &mut Vec<SqlValue>,
    attribution_alias: &str,
) -> String {
    let mut filters = Vec::new();
    if !scope.deck_ids.is_empty() {
        filters.push(format!(
            "AND {attribution_alias}.deck_id IN ({})",
            placeholders(values, scope.deck_ids.clone())
        ));
    }
    for tag_id in &scope.tag_ids {
        values.push(SqlValue::Text(tag_id.clone()));
        filters.push(format!(
            "AND EXISTS (
                 SELECT 1 FROM review_event_tag_attributions scope_ta
                 WHERE scope_ta.review_event_id = {attribution_alias}.review_event_id
                   AND scope_ta.tag_id = ?{}
             )",
            values.len()
        ));
    }
    filters.join(" ")
}

fn bucket_start(date: NaiveDate, granularity: &str) -> Result<NaiveDate, CommandError> {
    match granularity {
        "day" => Ok(date),
        "week" => date
            .checked_sub_signed(Duration::days(i64::from(date.weekday().num_days_from_monday())))
            .ok_or_else(|| {
                CommandError::new(
                    "invalid_analytics_bucket",
                    "A weekly Analytics bucket is outside the supported range.",
                )
            }),
        "month" => date.with_day(1).ok_or_else(|| {
            CommandError::new(
                "invalid_analytics_bucket",
                "A monthly Analytics bucket is invalid.",
            )
        }),
        _ => Err(CommandError::new(
            "invalid_analytics_bucket",
            "The Analytics bucket granularity is invalid.",
        )),
    }
}

fn next_bucket(date: NaiveDate, granularity: &str) -> Result<NaiveDate, CommandError> {
    match granularity {
        "day" => date.succ_opt().ok_or_else(|| {
            CommandError::new(
                "invalid_analytics_bucket",
                "The next Analytics day is outside the supported range.",
            )
        }),
        "week" => date.checked_add_signed(Duration::days(7)).ok_or_else(|| {
            CommandError::new(
                "invalid_analytics_bucket",
                "The next Analytics week is outside the supported range.",
            )
        }),
        "month" => {
            let (year, month) = if date.month() == 12 {
                (date.year().saturating_add(1), 1)
            } else {
                (date.year(), date.month() + 1)
            };
            NaiveDate::from_ymd_opt(year, month, 1).ok_or_else(|| {
                CommandError::new(
                    "invalid_analytics_bucket",
                    "The next Analytics month is outside the supported range.",
                )
            })
        }
        _ => Err(CommandError::new(
            "invalid_analytics_bucket",
            "The Analytics bucket granularity is invalid.",
        )),
    }
}

fn bucket_label(date: NaiveDate, granularity: &str) -> String {
    match granularity {
        "month" => date.format("%b %Y").to_string(),
        _ => date.format("%Y-%m-%d").to_string(),
    }
}

fn split_names(value: &str) -> Vec<String> {
    value
        .split('\u{001f}')
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .collect()
}

fn ratio(numerator: i64, denominator: i64) -> Option<f64> {
    (denominator > 0).then(|| numerator as f64 / denominator as f64)
}

fn trend(current: Option<f64>, previous: Option<f64>) -> Option<f64> {
    match (current, previous) {
        (Some(current), Some(previous)) => Some(current - previous),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_range_is_a_local_calendar_week() {
        assert_eq!(default_range(), "7d");
        assert_eq!(AnalyticsRange::parse("7d").expect("range").granularity(), "day");
    }

    #[test]
    fn months_advance_across_year_boundary() {
        let december = NaiveDate::from_ymd_opt(2026, 12, 1).expect("date");
        assert_eq!(
            next_bucket(december, "month").expect("next"),
            NaiveDate::from_ymd_opt(2027, 1, 1).expect("date")
        );
    }
}
