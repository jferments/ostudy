//! Review totals by study date for the overview and subjects sections.
//!
//! Without a tag filter they come from the per-deck rollups
//! (`crate::study_activity`), so a year or all time costs a few thousand small
//! rows instead of a walk over every review. A tag filter needs each review's
//! tags, so it reads the history rows themselves. Both sources produce the same
//! hourly rows; `tests.rs` checks they agree.

use chrono::{Duration, NaiveDate};
use rusqlite::{params_from_iter, types::Value as SqlValue, Connection};
use serde::Serialize;

use crate::{
    error::CommandError,
    learning_metrics::{self, rate, STATE_NEW},
    projections::scope::{append_in_filter, push_value},
    study_activity::{self, WALL_DATE_SQL, WALL_HOUR_SQL},
};

use super::super::scope::{AnalyticsContext, HistoryPeriod};

/// Summed review counts; every field adds.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Totals {
    pub review_count: i64,
    pub new_card_count: i64,
    pub remembered_count: i64,
    pub again_count: i64,
    pub hard_count: i64,
    pub good_count: i64,
    pub easy_count: i64,
    pub response_time_ms: i64,
    pub retention_review_count: i64,
    pub retention_remembered_count: i64,
}

impl Totals {
    pub fn add(&mut self, other: &Self) {
        self.review_count += other.review_count;
        self.new_card_count += other.new_card_count;
        self.remembered_count += other.remembered_count;
        self.again_count += other.again_count;
        self.hard_count += other.hard_count;
        self.good_count += other.good_count;
        self.easy_count += other.easy_count;
        self.response_time_ms += other.response_time_ms;
        self.retention_review_count += other.retention_review_count;
        self.retention_remembered_count += other.retention_remembered_count;
    }

    pub fn summary(&self) -> ProgressSummaryDto {
        let reviews = self.review_count;
        let average = |sum: i64| (reviews > 0).then(|| sum as f64 / reviews as f64);
        ProgressSummaryDto {
            review_count: reviews,
            new_card_count: self.new_card_count,
            remembered_count: self.remembered_count,
            correct_rate: rate(self.remembered_count, reviews),
            // Confidence is the rating less one: Again 0 to Easy 3.
            average_confidence: average(
                self.hard_count + 2 * self.good_count + 3 * self.easy_count,
            ),
            average_response_time_ms: average(self.response_time_ms),
            study_time_ms: self.response_time_ms,
            retention_review_count: self.retention_review_count,
            retention_remembered_count: self.retention_remembered_count,
            true_retention: rate(self.retention_remembered_count, self.retention_review_count),
        }
    }
}

/// Totals for a period, with the rates `learning_metrics` defines.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressSummaryDto {
    pub review_count: i64,
    pub new_card_count: i64,
    pub remembered_count: i64,
    pub correct_rate: Option<f64>,
    pub average_confidence: Option<f64>,
    pub average_response_time_ms: Option<f64>,
    pub study_time_ms: i64,
    pub retention_review_count: i64,
    pub retention_remembered_count: i64,
    pub true_retention: Option<f64>,
}

/// One study date's totals, for one subject when grouped by subject.
#[derive(Clone, Debug, PartialEq)]
pub struct DayRow {
    pub subject_id: Option<String>,
    pub study_date: NaiveDate,
    pub totals: Totals,
}

/// Totals by study date (and subject when `by_subject`) for the context's
/// current and previous periods, oldest first. Rebuilds stale rollups first.
pub fn load(
    connection: &mut Connection,
    context: &AnalyticsContext,
    by_subject: bool,
) -> Result<Vec<DayRow>, CommandError> {
    let first = context
        .previous_window()
        .or(context.window.current.as_ref())
        .map(|window| window.start_local_date);
    let end = context.window.end_local_date;
    let rows = if context.tag_ids.is_empty() {
        study_activity::refresh_rollups(connection)?;
        rollup_rows(connection, context, by_subject, first, end)?
    } else {
        history_rows(connection, context, by_subject)?
    };
    let hour = context.time.new_day_start_hour();
    let mut days = std::collections::BTreeMap::<(NaiveDate, Option<String>), Totals>::new();
    for (subject_id, wall_date, wall_hour, totals) in rows {
        let date =
            study_activity::study_date(study_activity::parse_date(&wall_date)?, wall_hour, hour);
        if first.is_some_and(|first| date < first) || date > end {
            continue;
        }
        days.entry((date, subject_id)).or_default().add(&totals);
    }
    Ok(days
        .into_iter()
        .map(|((study_date, subject_id), totals)| DayRow {
            subject_id,
            study_date,
            totals,
        })
        .collect())
}

pub(super) type HourRow = (Option<String>, String, u32, Totals);

pub(super) fn rollup_rows(
    connection: &Connection,
    context: &AnalyticsContext,
    by_subject: bool,
    first: Option<NaiveDate>,
    end: NaiveDate,
) -> Result<Vec<HourRow>, CommandError> {
    let mut values = Vec::<SqlValue>::new();
    let mut conditions = vec!["d.deleted_at IS NULL".to_string()];
    append_in_filter(&mut conditions, &mut values, "x.deck_id", &context.deck_ids);
    append_in_filter(
        &mut conditions,
        &mut values,
        "d.subject_id",
        &context.subject_ids,
    );
    // A study date's reviews fall on its own wall date or the next one.
    if let Some(first) = first {
        let index = push_value(&mut values, first.to_string());
        conditions.push(format!("x.wall_date >= ?{index}"));
    }
    let index = push_value(&mut values, (end + Duration::days(1)).to_string());
    conditions.push(format!("x.wall_date <= ?{index}"));
    let subject = if by_subject { "d.subject_id" } else { "NULL" };
    let sql = format!(
        "SELECT {subject}, x.wall_date, x.wall_hour,
                SUM(x.review_count), SUM(x.new_card_count), SUM(x.remembered_count),
                SUM(x.again_count), SUM(x.hard_count), SUM(x.good_count), SUM(x.easy_count),
                SUM(x.response_time_ms),
                SUM(x.retention_review_count), SUM(x.retention_remembered_count)
         FROM review_day_rollups x
         CROSS JOIN decks d ON d.id = x.deck_id
         WHERE {conditions}
         GROUP BY 1, 2, 3",
        conditions = conditions.join(" AND "),
    );
    query_hour_rows(connection, &sql, &values)
}

pub(super) fn history_rows(
    connection: &Connection,
    context: &AnalyticsContext,
    by_subject: bool,
) -> Result<Vec<HourRow>, CommandError> {
    let mut values = Vec::<SqlValue>::new();
    let conditions =
        context.historical_conditions(&mut values, HistoryPeriod::CurrentAndPrevious)?;
    let subject = if by_subject { "d.subject_id" } else { "NULL" };
    let retention = learning_metrics::retention_review_sql("r");
    let sql = format!(
        "SELECT {subject}, {WALL_DATE_SQL}, {WALL_HOUR_SQL},
                COUNT(*), SUM(r.state_before = {STATE_NEW}), SUM(r.answer_correct),
                SUM(r.rating = 1), SUM(r.rating = 2), SUM(r.rating = 3), SUM(r.rating = 4),
                SUM(r.response_time_ms),
                SUM({retention}), SUM({retention} AND r.answer_correct = 1)
         FROM {history_from}
         WHERE {conditions}
         GROUP BY 1, 2, 3",
        history_from = context.history_from_sql(),
        conditions = conditions.join(" AND "),
    );
    query_hour_rows(connection, &sql, &values)
}

fn query_hour_rows(
    connection: &Connection,
    sql: &str,
    values: &[SqlValue],
) -> Result<Vec<HourRow>, CommandError> {
    let mut statement = connection.prepare(sql).map_err(CommandError::database)?;
    let rows = statement
        .query_map(params_from_iter(values.iter()), |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, u32>(2)?,
                Totals {
                    review_count: row.get(3)?,
                    new_card_count: row.get(4)?,
                    remembered_count: row.get(5)?,
                    again_count: row.get(6)?,
                    hard_count: row.get(7)?,
                    good_count: row.get(8)?,
                    easy_count: row.get(9)?,
                    response_time_ms: row.get(10)?,
                    retention_review_count: row.get(11)?,
                    retention_remembered_count: row.get(12)?,
                },
            ))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    Ok(rows)
}
