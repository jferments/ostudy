use rusqlite::{params_from_iter, types::Value as SqlValue, Connection};

use crate::error::CommandError;

use super::{
    scope::{AnalyticsContext, HistoryPeriod},
    AttributionCoverageDto, HistoricalReviewSummaryDto, RatingDistributionDto,
};

pub struct HistoricalLoad {
    pub summary: HistoricalReviewSummaryDto,
    pub rating_distribution: RatingDistributionDto,
    pub attribution_coverage: AttributionCoverageDto,
    pub earliest_local_date: Option<String>,
}

pub fn load(
    connection: &Connection,
    context: &AnalyticsContext,
) -> Result<HistoricalLoad, CommandError> {
    let mut values = Vec::<SqlValue>::new();
    let conditions = context.historical_conditions(&mut values, HistoryPeriod::Current);
    let sql = format!(
        "SELECT
             COUNT(DISTINCT r.card_id),
             COUNT(*),
             COALESCE(SUM(r.answer_correct), 0),
             AVG(CAST(r.rating - 1 AS REAL)),
             AVG(CAST(r.response_time_ms AS REAL)),
             COALESCE(SUM(r.response_time_ms), 0),
             COUNT(DISTINCT r.session_id),
             COALESCE(SUM(CASE WHEN r.rating = 1 THEN 1 ELSE 0 END), 0),
             COALESCE(SUM(CASE WHEN r.rating = 2 THEN 1 ELSE 0 END), 0),
             COALESCE(SUM(CASE WHEN r.rating = 3 THEN 1 ELSE 0 END), 0),
             COALESCE(SUM(CASE WHEN r.rating = 4 THEN 1 ELSE 0 END), 0),
             COALESCE(SUM(CASE WHEN a.attribution_source = 'captured' THEN 1 ELSE 0 END), 0),
             COALESCE(SUM(CASE
                 WHEN a.attribution_source = 'legacy_current_state_backfill' THEN 1 ELSE 0
             END), 0),
             MIN(a.review_local_date)
         FROM review_events r
         JOIN review_event_attributions a ON a.review_event_id = r.id
         JOIN cards c ON c.id = r.card_id
         JOIN notes n ON n.id = c.note_id
         JOIN decks d ON d.id = c.deck_id
         WHERE {conditions}",
        conditions = conditions.join(" AND "),
    );
    connection
        .query_row(&sql, params_from_iter(values.iter()), |row| {
            let review_count: i64 = row.get(1)?;
            let correct_count: i64 = row.get(2)?;
            Ok(HistoricalLoad {
                summary: HistoricalReviewSummaryDto {
                    cards_practiced: row.get(0)?,
                    review_count,
                    correct_count,
                    correct_rate: rate(correct_count, review_count),
                    average_confidence: row.get(3)?,
                    average_response_time_ms: row.get(4)?,
                    total_study_time_ms: row.get(5)?,
                    session_count: row.get(6)?,
                },
                rating_distribution: RatingDistributionDto {
                    again: row.get(7)?,
                    hard: row.get(8)?,
                    good: row.get(9)?,
                    easy: row.get(10)?,
                },
                attribution_coverage: AttributionCoverageDto {
                    captured_review_count: row.get(11)?,
                    legacy_backfilled_review_count: row.get(12)?,
                },
                earliest_local_date: row.get(13)?,
            })
        })
        .map_err(CommandError::database)
}

pub fn rate(numerator: i64, denominator: i64) -> Option<f64> {
    if denominator == 0 {
        None
    } else {
        Some(numerator as f64 / denominator as f64)
    }
}
