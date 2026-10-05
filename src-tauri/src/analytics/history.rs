use rusqlite::{params_from_iter, types::Value as SqlValue, Connection};

use crate::{
    error::CommandError,
    learning_metrics::{self, rate},
};

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
    let conditions = context.historical_conditions(&mut values, HistoryPeriod::Current)?;
    let sql = format!(
        "SELECT
             COUNT(DISTINCT c.id),
             COUNT(*),
             {remembered},
             {average_confidence},
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
             {earliest_study_date},
             {retention}
         FROM {history_from}
         WHERE {conditions}",
        remembered = learning_metrics::remembered_count_sql("r"),
        average_confidence = learning_metrics::average_confidence_sql("r"),
        retention = learning_metrics::retention_counts_sql("r"),
        earliest_study_date = context.time.earliest_review_study_date_sql("a"),
        history_from = context.history_from_sql(),
        conditions = conditions.join(" AND "),
    );
    connection
        .query_row(&sql, params_from_iter(values.iter()), |row| {
            let review_count: i64 = row.get(1)?;
            let correct_count: i64 = row.get(2)?;
            let retention_review_count: i64 = row.get(14)?;
            let retention_remembered_count: i64 = row.get(15)?;
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
                    retention_review_count,
                    retention_remembered_count,
                    true_retention: rate(retention_remembered_count, retention_review_count),
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
