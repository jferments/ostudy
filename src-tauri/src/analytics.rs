//! Deterministic learning analytics derived from the local SQLite ledger.
//!
//! Analytics never modifies scheduling state. FSRS remains authoritative for
//! intervals, while this module groups immutable reviews and current card state
//! into scoped, learner-facing progress summaries.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use chrono::{Datelike, Duration, Local, NaiveDate, TimeZone};
use rusqlite::{params_from_iter, types::Value as SqlValue, Connection};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    database::Database,
    error::CommandError,
    settings::load_settings,
    util::{unix_time_millis, validate_entity_id},
    workload::local_day_start_millis,
};

const DAY_MS: i64 = 86_400_000;
const MAX_ANALYTICS_DECKS: usize = 256;
const MAX_ANALYTICS_TAGS: usize = 64;
const RECENT_REVIEW_LIMIT: usize = 20;
const RECENT_SESSION_LIMIT: usize = 12;
const MIN_CLASSIFICATION_REVIEWS: i64 = 5;

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

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsSummaryDto {
    pub deck_count: i64,
    pub card_count: i64,
    pub due_now: i64,
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
    pub card_count: i64,
    pub due_count: i64,
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
    pub card_count: i64,
    pub due_count: i64,
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
    pub deck_name: String,
    pub card_id: String,
    pub front_text: String,
    pub tags: Vec<String>,
    pub outcome: String,
    pub rating: i64,
    pub response_time_ms: i64,
    pub reviewed_at: i64,
    pub due_after: i64,
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
}

#[derive(Clone, Copy, Debug)]
enum AnalyticsRange {
    Days30,
    Days90,
    Year1,
    All,
}

impl AnalyticsRange {
    fn parse(value: &str) -> Result<Self, CommandError> {
        match value {
            "30d" => Ok(Self::Days30),
            "90d" => Ok(Self::Days90),
            "1y" => Ok(Self::Year1),
            "all" => Ok(Self::All),
            _ => Err(CommandError::new(
                "invalid_analytics_range",
                "Choose 30 days, 90 days, one year, or all time.",
            )),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Days30 => "30d",
            Self::Days90 => "90d",
            Self::Year1 => "1y",
            Self::All => "all",
        }
    }

    fn duration_ms(self) -> Option<i64> {
        match self {
            Self::Days30 => Some(30 * DAY_MS),
            Self::Days90 => Some(90 * DAY_MS),
            Self::Year1 => Some(365 * DAY_MS),
            Self::All => None,
        }
    }

    fn granularity(self) -> &'static str {
        match self {
            Self::Days30 => "day",
            Self::Days90 | Self::Year1 => "week",
            Self::All => "month",
        }
    }
}

#[derive(Clone, Debug)]
struct RangeWindow {
    current_start: Option<i64>,
    previous_start: Option<i64>,
}

#[derive(Clone, Debug)]
struct TagRef {
    id: String,
    name: String,
}

#[derive(Clone, Debug)]
struct CardSnapshot {
    card_id: String,
    deck_id: String,
    deck_name: String,
    due_at: i64,
    suspended_at: Option<i64>,
    buried_until: Option<i64>,
    needs_more_practice: bool,
    tags: Vec<TagRef>,
}

#[derive(Clone, Debug)]
struct CardRow {
    card_id: String,
    deck_id: String,
    deck_name: String,
    due_at: i64,
    suspended_at: Option<i64>,
    buried_until: Option<i64>,
    needs_more_practice: bool,
    tag_id: Option<String>,
    tag_name: Option<String>,
}

#[derive(Clone, Debug)]
struct ReviewRow {
    id: String,
    session_id: Option<String>,
    card_id: String,
    deck_id: String,
    deck_name: String,
    front_text: String,
    outcome: String,
    rating: i64,
    answer_correct: i64,
    response_time_ms: i64,
    reviewed_at: i64,
    due_after: i64,
    state_before: i64,
}

#[derive(Clone, Debug)]
struct BucketSpec {
    start_at: i64,
    end_at: i64,
}

#[derive(Default)]
struct ReviewAccumulator {
    review_count: i64,
    correct_count: i64,
    confidence_total: i64,
    response_time_total: i64,
    cards: HashSet<String>,
    sessions: HashSet<String>,
    last_practiced_at: Option<i64>,
}

impl ReviewAccumulator {
    fn add(&mut self, review: &ReviewRow) {
        self.review_count += 1;
        self.correct_count += review.answer_correct;
        self.confidence_total += review.rating - 1;
        self.response_time_total = self
            .response_time_total
            .saturating_add(review.response_time_ms);
        self.cards.insert(review.card_id.clone());
        if let Some(session_id) = &review.session_id {
            self.sessions.insert(session_id.clone());
        }
        self.last_practiced_at = Some(
            self.last_practiced_at
                .map_or(review.reviewed_at, |value| value.max(review.reviewed_at)),
        );
    }

    fn correct_rate(&self) -> Option<f64> {
        rate(self.correct_count, self.review_count)
    }

    fn average_confidence(&self) -> Option<f64> {
        average(self.confidence_total, self.review_count)
    }

    fn average_response_time_ms(&self) -> Option<f64> {
        average(self.response_time_total, self.review_count)
    }
}

#[derive(Default)]
struct ScopedAccumulator {
    card_count: i64,
    due_count: i64,
    needs_more_practice: i64,
    reviews: ReviewAccumulator,
}

#[derive(Default)]
struct TagAccumulator {
    scope: ScopedAccumulator,
    previous_review_count: i64,
    previous_correct_count: i64,
}

#[derive(Default)]
struct BucketAccumulator {
    reviews: ReviewAccumulator,
    new_cards: HashSet<String>,
    contains_highlighted_session: bool,
}

#[derive(Default)]
struct SessionEventAccumulator {
    deck_names: BTreeSet<String>,
    study_time_ms: i64,
    last_reviewed_at: i64,
}

#[derive(Clone, Debug)]
struct SessionRow {
    id: String,
    mode: String,
    selected_deck_ids_json: String,
    card_count: i64,
    rated_count: i64,
    correct_count: i64,
    started_at: i64,
    completed_at: Option<i64>,
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
    database.with_connection(load_recent_practice_reviews)
}

fn load_analytics(
    connection: &mut Connection,
    input: AnalyticsQueryDto,
) -> Result<AnalyticsDto, CommandError> {
    let input = normalize_query(input)?;
    let range = AnalyticsRange::parse(&input.range)?;
    let now = unix_time_millis()?;
    let window = range_window(range, now);
    let settings = load_settings(connection)?;
    let cards = load_card_snapshots(connection, &input)?;
    let reviews = load_review_rows(connection, &input, &window)?;
    let card_by_id = cards
        .iter()
        .map(|card| (card.card_id.clone(), card))
        .collect::<HashMap<_, _>>();

    let mut summary = ScopedAccumulator::default();
    let mut active_decks = HashSet::new();
    let mut deck_accumulators = BTreeMap::<String, ScopedAccumulator>::new();
    let mut deck_names = BTreeMap::<String, String>::new();
    let mut tag_accumulators = BTreeMap::<String, TagAccumulator>::new();
    let mut tag_names = BTreeMap::<String, String>::new();

    for card in &cards {
        deck_names.insert(card.deck_id.clone(), card.deck_name.clone());
        let active = card.suspended_at.is_none();
        let due =
            active && card.due_at <= now && card.buried_until.is_none_or(|value| value <= now);
        if active {
            summary.card_count += 1;
            active_decks.insert(card.deck_id.clone());
        }
        if due {
            summary.due_count += 1;
        }
        if active && card.needs_more_practice {
            summary.needs_more_practice += 1;
        }

        let deck = deck_accumulators.entry(card.deck_id.clone()).or_default();
        if active {
            deck.card_count += 1;
        }
        if due {
            deck.due_count += 1;
        }
        if active && card.needs_more_practice {
            deck.needs_more_practice += 1;
        }

        for tag in &card.tags {
            tag_names.insert(tag.id.clone(), tag.name.clone());
            let tag_accumulator = tag_accumulators.entry(tag.id.clone()).or_default();
            if active {
                tag_accumulator.scope.card_count += 1;
            }
            if due {
                tag_accumulator.scope.due_count += 1;
            }
            if active && card.needs_more_practice {
                tag_accumulator.scope.needs_more_practice += 1;
            }
        }
    }

    let current_reviews = reviews
        .iter()
        .filter(|review| {
            window
                .current_start
                .is_none_or(|start| review.reviewed_at >= start)
        })
        .collect::<Vec<_>>();
    let previous_reviews = reviews
        .iter()
        .filter(|review| {
            window.previous_start.is_some()
                && window
                    .current_start
                    .is_some_and(|start| review.reviewed_at < start)
        })
        .collect::<Vec<_>>();

    let mut rating_distribution = RatingDistributionDto::default();
    for &review in &current_reviews {
        summary.reviews.add(review);
        match review.rating {
            1 => rating_distribution.again += 1,
            2 => rating_distribution.hard += 1,
            3 => rating_distribution.good += 1,
            4 => rating_distribution.easy += 1,
            _ => {}
        }
        deck_accumulators
            .entry(review.deck_id.clone())
            .or_default()
            .reviews
            .add(review);
        if let Some(card) = card_by_id.get(&review.card_id) {
            for tag in &card.tags {
                tag_accumulators
                    .entry(tag.id.clone())
                    .or_default()
                    .scope
                    .reviews
                    .add(review);
            }
        }
    }
    for review in previous_reviews {
        if let Some(card) = card_by_id.get(&review.card_id) {
            for tag in &card.tags {
                let accumulator = tag_accumulators.entry(tag.id.clone()).or_default();
                accumulator.previous_review_count += 1;
                accumulator.previous_correct_count += review.answer_correct;
            }
        }
    }

    let first_current_review = current_reviews
        .iter()
        .map(|review| review.reviewed_at)
        .min();
    let bucket_specs = build_bucket_specs(range, now, first_current_review)?;
    let timeline = build_timeline(
        &bucket_specs,
        &current_reviews,
        input.highlighted_session_id.as_deref(),
    );
    let recent_reviews = current_reviews
        .iter()
        .copied()
        .take(RECENT_REVIEW_LIMIT)
        .map(|review| recent_review(review, card_by_id.get(&review.card_id).copied()))
        .collect::<Vec<_>>();
    let recent_sessions = load_recent_sessions(
        connection,
        &current_reviews,
        &deck_names,
        input.highlighted_session_id.as_deref(),
    )?;

    let mut deck_stats = deck_accumulators
        .into_iter()
        .map(|(deck_id, accumulator)| DeckAnalyticsDto {
            deck_name: deck_names
                .get(&deck_id)
                .cloned()
                .unwrap_or_else(|| "Deck".to_string()),
            deck_id,
            card_count: accumulator.card_count,
            due_count: accumulator.due_count,
            needs_more_practice: accumulator.needs_more_practice,
            cards_practiced: usize_to_i64(accumulator.reviews.cards.len()),
            review_count: accumulator.reviews.review_count,
            correct_count: accumulator.reviews.correct_count,
            correct_rate: accumulator.reviews.correct_rate(),
            average_confidence: accumulator.reviews.average_confidence(),
            average_response_time_ms: accumulator.reviews.average_response_time_ms(),
            total_study_time_ms: accumulator.reviews.response_time_total,
            last_practiced_at: accumulator.reviews.last_practiced_at,
        })
        .collect::<Vec<_>>();
    deck_stats.sort_by(|left, right| {
        right
            .needs_more_practice
            .cmp(&left.needs_more_practice)
            .then_with(|| right.due_count.cmp(&left.due_count))
            .then_with(|| right.review_count.cmp(&left.review_count))
            .then_with(|| {
                left.deck_name
                    .to_lowercase()
                    .cmp(&right.deck_name.to_lowercase())
            })
    });

    let desired_retention = settings.desired_retention;
    let mut tag_stats = tag_accumulators
        .into_iter()
        .map(|(tag_id, accumulator)| {
            let correct_rate = accumulator.scope.reviews.correct_rate();
            let previous_rate = rate(
                accumulator.previous_correct_count,
                accumulator.previous_review_count,
            );
            let trend_delta = if accumulator.scope.reviews.review_count
                >= MIN_CLASSIFICATION_REVIEWS
                && accumulator.previous_review_count >= MIN_CLASSIFICATION_REVIEWS
            {
                correct_rate
                    .zip(previous_rate)
                    .map(|(current, previous)| current - previous)
            } else {
                None
            };
            TagAnalyticsDto {
                tag_name: tag_names
                    .get(&tag_id)
                    .cloned()
                    .unwrap_or_else(|| "Tag".to_string()),
                tag_id,
                card_count: accumulator.scope.card_count,
                due_count: accumulator.scope.due_count,
                needs_more_practice: accumulator.scope.needs_more_practice,
                cards_practiced: usize_to_i64(accumulator.scope.reviews.cards.len()),
                review_count: accumulator.scope.reviews.review_count,
                correct_count: accumulator.scope.reviews.correct_count,
                correct_rate,
                average_confidence: accumulator.scope.reviews.average_confidence(),
                average_response_time_ms: accumulator.scope.reviews.average_response_time_ms(),
                total_study_time_ms: accumulator.scope.reviews.response_time_total,
                last_practiced_at: accumulator.scope.reviews.last_practiced_at,
                trend_delta,
                status: classify_tag(
                    accumulator.scope.reviews.review_count,
                    correct_rate,
                    accumulator.scope.needs_more_practice,
                    desired_retention,
                ),
            }
        })
        .collect::<Vec<_>>();
    tag_stats.sort_by(|left, right| {
        tag_status_rank(&left.status)
            .cmp(&tag_status_rank(&right.status))
            .then_with(|| right.needs_more_practice.cmp(&left.needs_more_practice))
            .then_with(|| right.due_count.cmp(&left.due_count))
            .then_with(|| {
                left.correct_rate
                    .unwrap_or(1.0)
                    .total_cmp(&right.correct_rate.unwrap_or(1.0))
            })
            .then_with(|| {
                left.tag_name
                    .to_lowercase()
                    .cmp(&right.tag_name.to_lowercase())
            })
    });

    let daily_limits = load_daily_limit_usage(connection, &settings, now)?;
    let summary_dto = AnalyticsSummaryDto {
        deck_count: usize_to_i64(active_decks.len()),
        card_count: summary.card_count,
        due_now: summary.due_count,
        needs_more_practice: summary.needs_more_practice,
        cards_practiced: usize_to_i64(summary.reviews.cards.len()),
        review_count: summary.reviews.review_count,
        correct_count: summary.reviews.correct_count,
        correct_rate: summary.reviews.correct_rate(),
        average_confidence: summary.reviews.average_confidence(),
        average_response_time_ms: summary.reviews.average_response_time_ms(),
        total_study_time_ms: summary.reviews.response_time_total,
        session_count: usize_to_i64(summary.reviews.sessions.len()),
    };
    let scope = AnalyticsScopeDto {
        range: range.as_str().to_string(),
        start_at: window.current_start,
        end_at: now,
        bucket_granularity: range.granularity().to_string(),
        deck_ids: input.deck_ids,
        tag_ids: input.tag_ids,
        highlighted_session_id: input.highlighted_session_id,
    };

    Ok(AnalyticsDto {
        scope,
        summary: summary_dto,
        daily_limits,
        rating_distribution,
        timeline,
        deck_stats,
        tag_stats,
        recent_reviews,
        recent_sessions,
    })
}

fn default_range() -> String {
    "30d".to_string()
}

fn normalize_query(mut input: AnalyticsQueryDto) -> Result<AnalyticsQueryDto, CommandError> {
    AnalyticsRange::parse(&input.range)?;
    input.deck_ids = normalize_ids(input.deck_ids, "deck", MAX_ANALYTICS_DECKS)?;
    input.tag_ids = normalize_ids(input.tag_ids, "tag", MAX_ANALYTICS_TAGS)?;
    if let Some(session_id) = input.highlighted_session_id.as_deref() {
        validate_entity_id(session_id, "practice session")?;
    }
    Ok(input)
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

fn range_window(range: AnalyticsRange, now: i64) -> RangeWindow {
    match range.duration_ms() {
        Some(duration) => {
            let current_start = now.saturating_sub(duration);
            RangeWindow {
                current_start: Some(current_start),
                previous_start: Some(current_start.saturating_sub(duration)),
            }
        }
        None => RangeWindow {
            current_start: None,
            previous_start: None,
        },
    }
}

fn load_card_snapshots(
    connection: &Connection,
    input: &AnalyticsQueryDto,
) -> Result<Vec<CardSnapshot>, CommandError> {
    let mut conditions = vec![
        "c.deleted_at IS NULL".to_string(),
        "n.deleted_at IS NULL".to_string(),
        "d.deleted_at IS NULL".to_string(),
        "d.archived_at IS NULL".to_string(),
    ];
    let mut values = Vec::<SqlValue>::new();
    apply_scope_filters(&mut conditions, &mut values, input);
    let sql = format!(
        "WITH latest_reset AS (
             SELECT card_id, MAX(occurred_at) AS reset_at
             FROM schedule_action_events
             WHERE action = 'reset'
             GROUP BY card_id
         ),
         ranked_reviews AS (
             SELECT r.card_id, r.answer_correct,
                    ROW_NUMBER() OVER (
                        PARTITION BY r.card_id
                        ORDER BY r.reviewed_at DESC, r.created_at DESC
                    ) AS review_rank
             FROM review_events r
             LEFT JOIN latest_reset lr ON lr.card_id = r.card_id
             WHERE r.reviewed_at > COALESCE(lr.reset_at, -1)
         ),
         signals AS (
             SELECT card_id,
                    MAX(CASE WHEN review_rank = 1 THEN answer_correct END) AS latest_answer,
                    MAX(CASE WHEN review_rank = 2 THEN answer_correct END) AS previous_answer,
                    SUM(CASE WHEN review_rank <= 5 AND answer_correct = 0 THEN 1 ELSE 0 END) AS wrong_last_five
             FROM ranked_reviews
             WHERE review_rank <= 5
             GROUP BY card_id
         )
         SELECT c.id, c.deck_id, d.name, s.due_at,
                c.suspended_at, c.buried_until,
                CASE
                    WHEN sig.latest_answer = 1 AND sig.previous_answer = 1 THEN 0
                    WHEN sig.latest_answer = 0 AND sig.previous_answer = 0 THEN 1
                    WHEN COALESCE(sig.wrong_last_five, 0) >= 3 THEN 1
                    ELSE 0
                END AS needs_more_practice,
                t.id, t.name
         FROM cards c
         JOIN notes n ON n.id = c.note_id
         JOIN decks d ON d.id = c.deck_id
         JOIN card_schedules s ON s.card_id = c.id
         LEFT JOIN signals sig ON sig.card_id = c.id
         LEFT JOIN note_tags nt ON nt.note_id = n.id
         LEFT JOIN tags t ON t.id = nt.tag_id
         WHERE {conditions}
         ORDER BY c.id, t.name COLLATE NOCASE ASC",
        conditions = conditions.join(" AND "),
    );
    let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
    let rows = statement
        .query_map(params_from_iter(values.iter()), |row| {
            Ok(CardRow {
                card_id: row.get(0)?,
                deck_id: row.get(1)?,
                deck_name: row.get(2)?,
                due_at: row.get(3)?,
                suspended_at: row.get(4)?,
                buried_until: row.get(5)?,
                needs_more_practice: row.get::<_, i64>(6)? == 1,
                tag_id: row.get(7)?,
                tag_name: row.get(8)?,
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;

    let mut cards = BTreeMap::<String, CardSnapshot>::new();
    for row in rows {
        let card = cards
            .entry(row.card_id.clone())
            .or_insert_with(|| CardSnapshot {
                card_id: row.card_id,
                deck_id: row.deck_id,
                deck_name: row.deck_name,
                due_at: row.due_at,
                suspended_at: row.suspended_at,
                buried_until: row.buried_until,
                needs_more_practice: row.needs_more_practice,
                tags: Vec::new(),
            });
        if let (Some(id), Some(name)) = (row.tag_id, row.tag_name) {
            card.tags.push(TagRef { id, name });
        }
    }
    Ok(cards.into_values().collect())
}

fn load_review_rows(
    connection: &Connection,
    input: &AnalyticsQueryDto,
    window: &RangeWindow,
) -> Result<Vec<ReviewRow>, CommandError> {
    let mut conditions = vec![
        "c.deleted_at IS NULL".to_string(),
        "n.deleted_at IS NULL".to_string(),
        "d.deleted_at IS NULL".to_string(),
        "d.archived_at IS NULL".to_string(),
    ];
    let mut values = Vec::<SqlValue>::new();
    apply_scope_filters(&mut conditions, &mut values, input);
    if let Some(start) = window.previous_start.or(window.current_start) {
        conditions.push(format!(
            "r.reviewed_at >= ?{}",
            push_value(&mut values, start)
        ));
    }
    let sql = format!(
        "SELECT r.id, r.session_id, r.card_id, c.deck_id, d.name,
                n.front_text, r.confidence, r.rating, r.answer_correct,
                r.response_time_ms, r.reviewed_at, r.due_after, r.state_before
         FROM review_events r
         JOIN cards c ON c.id = r.card_id
         JOIN notes n ON n.id = c.note_id
         JOIN decks d ON d.id = c.deck_id
         WHERE {conditions}
         ORDER BY r.reviewed_at DESC, r.created_at DESC",
        conditions = conditions.join(" AND "),
    );
    let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
    let reviews = statement
        .query_map(params_from_iter(values.iter()), |row| {
            Ok(ReviewRow {
                id: row.get(0)?,
                session_id: row.get(1)?,
                card_id: row.get(2)?,
                deck_id: row.get(3)?,
                deck_name: row.get(4)?,
                front_text: row.get(5)?,
                outcome: row.get(6)?,
                rating: row.get(7)?,
                answer_correct: row.get(8)?,
                response_time_ms: row.get(9)?,
                reviewed_at: row.get(10)?,
                due_after: row.get(11)?,
                state_before: row.get(12)?,
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    Ok(reviews)
}

fn apply_scope_filters(
    conditions: &mut Vec<String>,
    values: &mut Vec<SqlValue>,
    input: &AnalyticsQueryDto,
) {
    if !input.deck_ids.is_empty() {
        let placeholders = input
            .deck_ids
            .iter()
            .map(|deck_id| format!("?{}", push_value(values, deck_id.clone())))
            .collect::<Vec<_>>()
            .join(", ");
        conditions.push(format!("c.deck_id IN ({placeholders})"));
    }
    for tag_id in &input.tag_ids {
        let index = push_value(values, tag_id.clone());
        conditions.push(format!(
            "EXISTS (
                SELECT 1 FROM note_tags filter_nt
                WHERE filter_nt.note_id = n.id AND filter_nt.tag_id = ?{index}
             )"
        ));
    }
}

fn push_value(values: &mut Vec<SqlValue>, value: impl Into<SqlValue>) -> usize {
    values.push(value.into());
    values.len()
}

fn build_bucket_specs(
    range: AnalyticsRange,
    now: i64,
    first_review_at: Option<i64>,
) -> Result<Vec<BucketSpec>, CommandError> {
    let today = Local::now().date_naive();
    match range {
        AnalyticsRange::Days30 => date_buckets(today - Duration::days(29), today, 1, now),
        AnalyticsRange::Days90 => date_buckets(today - Duration::days(89), today, 7, now),
        AnalyticsRange::Year1 => date_buckets(today - Duration::days(364), today, 7, now),
        AnalyticsRange::All => monthly_buckets(first_review_at.unwrap_or(now), now),
    }
}

fn date_buckets(
    start_date: NaiveDate,
    today: NaiveDate,
    step_days: i64,
    now: i64,
) -> Result<Vec<BucketSpec>, CommandError> {
    let mut buckets = Vec::new();
    let mut date = start_date;
    while date <= today {
        let next = (date + Duration::days(step_days)).min(today + Duration::days(1));
        buckets.push(BucketSpec {
            start_at: local_midnight(date)?,
            end_at: if next > today {
                now.saturating_add(1)
            } else {
                local_midnight(next)?
            },
        });
        date = next;
    }
    Ok(buckets)
}

fn monthly_buckets(first_review_at: i64, now: i64) -> Result<Vec<BucketSpec>, CommandError> {
    let first_date = Local
        .timestamp_millis_opt(first_review_at)
        .earliest()
        .map(|value| value.date_naive())
        .unwrap_or_else(|| Local::now().date_naive());
    let today = Local::now().date_naive();
    let mut date = NaiveDate::from_ymd_opt(first_date.year(), first_date.month(), 1)
        .ok_or_else(invalid_analytics_date)?;
    let current_month = NaiveDate::from_ymd_opt(today.year(), today.month(), 1)
        .ok_or_else(invalid_analytics_date)?;
    let mut buckets = Vec::new();
    while date <= current_month {
        let next = next_month(date)?;
        buckets.push(BucketSpec {
            start_at: local_midnight(date)?,
            end_at: if date == current_month {
                now.saturating_add(1)
            } else {
                local_midnight(next)?
            },
        });
        date = next;
    }
    Ok(buckets)
}

fn local_midnight(date: NaiveDate) -> Result<i64, CommandError> {
    let midnight = date
        .and_hms_opt(0, 0, 0)
        .ok_or_else(invalid_analytics_date)?;
    Local
        .from_local_datetime(&midnight)
        .earliest()
        .map(|value| value.timestamp_millis())
        .ok_or_else(invalid_analytics_date)
}

fn next_month(date: NaiveDate) -> Result<NaiveDate, CommandError> {
    let (year, month) = if date.month() == 12 {
        (date.year().saturating_add(1), 1)
    } else {
        (date.year(), date.month() + 1)
    };
    NaiveDate::from_ymd_opt(year, month, 1).ok_or_else(invalid_analytics_date)
}

fn invalid_analytics_date() -> CommandError {
    CommandError::new(
        "invalid_analytics_date",
        "The analytics date range is outside the supported calendar.",
    )
}

fn build_timeline(
    specs: &[BucketSpec],
    reviews: &[&ReviewRow],
    highlighted_session_id: Option<&str>,
) -> Vec<TimelineBucketDto> {
    let mut accumulators = (0..specs.len())
        .map(|_| BucketAccumulator::default())
        .collect::<Vec<_>>();
    for &review in reviews {
        if let Some(index) = specs.iter().position(|bucket| {
            review.reviewed_at >= bucket.start_at && review.reviewed_at < bucket.end_at
        }) {
            let accumulator = &mut accumulators[index];
            accumulator.reviews.add(review);
            if review.state_before == 0 {
                accumulator.new_cards.insert(review.card_id.clone());
            }
            if highlighted_session_id
                .is_some_and(|value| review.session_id.as_deref() == Some(value))
            {
                accumulator.contains_highlighted_session = true;
            }
        }
    }
    specs
        .iter()
        .zip(accumulators)
        .map(|(spec, accumulator)| TimelineBucketDto {
            start_at: spec.start_at,
            end_at: spec.end_at,
            cards_practiced: usize_to_i64(accumulator.reviews.cards.len()),
            review_count: accumulator.reviews.review_count,
            new_cards: usize_to_i64(accumulator.new_cards.len()),
            remembered_count: accumulator.reviews.correct_count,
            correct_rate: accumulator.reviews.correct_rate(),
            average_confidence: accumulator.reviews.average_confidence(),
            average_response_time_ms: accumulator.reviews.average_response_time_ms(),
            study_time_ms: accumulator.reviews.response_time_total,
            session_count: usize_to_i64(accumulator.reviews.sessions.len()),
            contains_highlighted_session: accumulator.contains_highlighted_session,
        })
        .collect()
}

fn recent_review(review: &ReviewRow, card: Option<&CardSnapshot>) -> RecentReviewDto {
    let tags = card
        .map(|value| value.tags.iter().map(|tag| tag.name.clone()).collect())
        .unwrap_or_default();
    RecentReviewDto {
        id: review.id.clone(),
        deck_name: review.deck_name.clone(),
        card_id: review.card_id.clone(),
        front_text: review.front_text.clone(),
        tags,
        outcome: review.outcome.clone(),
        rating: review.rating,
        response_time_ms: review.response_time_ms,
        reviewed_at: review.reviewed_at,
        due_after: review.due_after,
    }
}

fn load_recent_sessions(
    connection: &Connection,
    reviews: &[&ReviewRow],
    deck_names: &BTreeMap<String, String>,
    highlighted_session_id: Option<&str>,
) -> Result<Vec<SessionAnalyticsDto>, CommandError> {
    let mut event_accumulators = HashMap::<String, SessionEventAccumulator>::new();
    for &review in reviews {
        if let Some(session_id) = &review.session_id {
            let accumulator = event_accumulators.entry(session_id.clone()).or_default();
            accumulator.deck_names.insert(review.deck_name.clone());
            accumulator.study_time_ms = accumulator
                .study_time_ms
                .saturating_add(review.response_time_ms);
            accumulator.last_reviewed_at = accumulator.last_reviewed_at.max(review.reviewed_at);
        }
    }
    let mut ids = event_accumulators
        .iter()
        .map(|(id, accumulator)| (id.clone(), accumulator.last_reviewed_at))
        .collect::<Vec<_>>();
    ids.sort_by_key(|(_, reviewed_at)| std::cmp::Reverse(*reviewed_at));
    ids.truncate(RECENT_SESSION_LIMIT);
    if let Some(highlighted) = highlighted_session_id {
        if !ids.iter().any(|(id, _)| id == highlighted) {
            ids.push((highlighted.to_string(), i64::MAX));
        }
    }
    if ids.is_empty() {
        return Ok(Vec::new());
    }

    let mut values = Vec::<SqlValue>::new();
    let placeholders = ids
        .iter()
        .map(|(id, _)| format!("?{}", push_value(&mut values, id.clone())))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT id, mode, selected_deck_ids_json, card_count, rated_count,
                correct_count, started_at, completed_at
         FROM practice_sessions
         WHERE id IN ({placeholders})"
    );
    let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
    let rows = statement
        .query_map(params_from_iter(values.iter()), |row| {
            Ok(SessionRow {
                id: row.get(0)?,
                mode: row.get(1)?,
                selected_deck_ids_json: row.get(2)?,
                card_count: row.get(3)?,
                rated_count: row.get(4)?,
                correct_count: row.get(5)?,
                started_at: row.get(6)?,
                completed_at: row.get(7)?,
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;

    let mut sessions = rows
        .into_iter()
        .map(|row| {
            let events = event_accumulators.get(&row.id);
            let mut names = events
                .map(|value| value.deck_names.iter().cloned().collect::<Vec<_>>())
                .unwrap_or_default();
            if names.is_empty() {
                let deck_ids = serde_json::from_str::<Vec<String>>(&row.selected_deck_ids_json)
                    .unwrap_or_default();
                names = deck_ids
                    .into_iter()
                    .filter_map(|deck_id| deck_names.get(&deck_id).cloned())
                    .collect();
            }
            let end = row
                .completed_at
                .or_else(|| events.map(|value| value.last_reviewed_at))
                .unwrap_or(row.started_at);
            SessionAnalyticsDto {
                highlighted: highlighted_session_id == Some(row.id.as_str()),
                id: row.id,
                mode: row.mode,
                deck_names: names,
                started_at: row.started_at,
                completed_at: row.completed_at,
                duration_ms: end.saturating_sub(row.started_at).max(0),
                card_count: row.card_count,
                rated_count: row.rated_count,
                correct_count: row.correct_count,
                correct_rate: rate(row.correct_count, row.rated_count),
                study_time_ms: events.map_or(0, |value| value.study_time_ms),
            }
        })
        .collect::<Vec<_>>();
    sessions.sort_by_key(|session| std::cmp::Reverse(session.started_at));
    sessions.truncate(RECENT_SESSION_LIMIT);
    Ok(sessions)
}

fn load_daily_limit_usage(
    connection: &Connection,
    settings: &crate::settings::AppSettingsDto,
    now: i64,
) -> Result<DailyLimitUsageDto, CommandError> {
    let day_start = local_day_start_millis()?.min(now);
    let (new_completed, reviews_completed): (i64, i64) = connection
        .query_row(
            "SELECT
                 COALESCE(SUM(CASE WHEN state_before = 0 THEN 1 ELSE 0 END), 0),
                 COALESCE(SUM(CASE WHEN state_before <> 0 THEN 1 ELSE 0 END), 0)
             FROM review_events
             WHERE reviewed_at >= ?1 AND reviewed_at <= ?2",
            [day_start, now],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(CommandError::database)?;
    Ok(DailyLimitUsageDto {
        new_limit: settings.daily_new_limit,
        review_limit: settings.daily_review_limit,
        new_completed,
        reviews_completed,
        new_remaining: settings
            .daily_new_limit
            .saturating_sub(new_completed)
            .max(0),
        reviews_remaining: settings
            .daily_review_limit
            .saturating_sub(reviews_completed)
            .max(0),
    })
}

fn classify_tag(
    review_count: i64,
    correct_rate: Option<f64>,
    needs_more_practice: i64,
    desired_retention: f64,
) -> String {
    if review_count < MIN_CLASSIFICATION_REVIEWS {
        return "building_data".to_string();
    }
    if needs_more_practice > 0
        || correct_rate.is_some_and(|value| value <= desired_retention - 0.10)
    {
        return "needs_attention".to_string();
    }
    if correct_rate.is_some_and(|value| value >= desired_retention + 0.03) {
        return "strong".to_string();
    }
    "on_track".to_string()
}

fn tag_status_rank(value: &str) -> i64 {
    match value {
        "needs_attention" => 0,
        "building_data" => 1,
        "on_track" => 2,
        "strong" => 3,
        _ => 4,
    }
}

fn load_recent_practice_reviews(
    connection: &mut Connection,
) -> Result<Vec<RecentReviewDto>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT r.id, d.name, r.card_id, n.front_text,
                    COALESCE((
                        SELECT group_concat(t.name, char(31))
                        FROM note_tags nt
                        JOIN tags t ON t.id = nt.tag_id
                        WHERE nt.note_id = n.id
                    ), ''),
                    r.confidence, r.rating, r.response_time_ms,
                    r.reviewed_at, r.due_after
             FROM review_events r
             JOIN cards c ON c.id = r.card_id
             JOIN notes n ON n.id = c.note_id
             JOIN decks d ON d.id = c.deck_id
             WHERE c.deleted_at IS NULL
               AND n.deleted_at IS NULL
               AND d.deleted_at IS NULL
               AND d.archived_at IS NULL
             ORDER BY r.reviewed_at DESC, r.created_at DESC
             LIMIT 20",
        )
        .map_err(CommandError::database)?;
    let reviews = statement
        .query_map([], |row| {
            let tags: String = row.get(4)?;
            Ok(RecentReviewDto {
                id: row.get(0)?,
                deck_name: row.get(1)?,
                card_id: row.get(2)?,
                front_text: row.get(3)?,
                tags: split_grouped(&tags),
                outcome: row.get(5)?,
                rating: row.get(6)?,
                response_time_ms: row.get(7)?,
                reviewed_at: row.get(8)?,
                due_after: row.get(9)?,
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    Ok(reviews)
}

fn split_grouped(value: &str) -> Vec<String> {
    value
        .split('\u{1f}')
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect()
}

fn rate(numerator: i64, denominator: i64) -> Option<f64> {
    if denominator == 0 {
        None
    } else {
        Some(numerator as f64 / denominator as f64)
    }
}

fn average(total: i64, count: i64) -> Option<f64> {
    if count == 0 {
        None
    } else {
        Some(total as f64 / count as f64)
    }
}

fn usize_to_i64(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_library_has_zeroed_scoped_analytics() {
        let database = Database::open_in_memory().expect("database should open");
        let analytics = database
            .with_connection(|connection| {
                load_analytics(
                    connection,
                    AnalyticsQueryDto {
                        range: "30d".to_string(),
                        deck_ids: Vec::new(),
                        tag_ids: Vec::new(),
                        highlighted_session_id: None,
                    },
                )
            })
            .expect("analytics should load");
        assert_eq!(analytics.summary.deck_count, 0);
        assert_eq!(analytics.summary.card_count, 0);
        assert_eq!(analytics.summary.review_count, 0);
        assert_eq!(analytics.summary.correct_rate, None);
        assert_eq!(analytics.timeline.len(), 30);
    }

    #[test]
    fn rate_and_average_are_none_without_reviews() {
        assert_eq!(rate(0, 0), None);
        assert_eq!(rate(3, 4), Some(0.75));
        assert_eq!(average(0, 0), None);
        assert_eq!(average(6, 3), Some(2.0));
    }

    #[test]
    fn tag_classification_uses_retention_and_learning_signals() {
        assert_eq!(classify_tag(3, Some(1.0), 0, 0.9), "building_data");
        assert_eq!(classify_tag(10, Some(0.75), 0, 0.9), "needs_attention");
        assert_eq!(classify_tag(10, Some(0.95), 0, 0.9), "strong");
        assert_eq!(classify_tag(10, Some(0.9), 0, 0.9), "on_track");
        assert_eq!(classify_tag(10, Some(1.0), 1, 0.9), "needs_attention");
    }
}
