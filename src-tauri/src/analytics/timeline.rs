use std::collections::BTreeMap;

use chrono::{Duration, NaiveDate};
use rusqlite::{params_from_iter, types::Value as SqlValue, Connection};

use crate::{error::CommandError, projections::scope::push_value};

use super::{
    history::rate,
    scope::{AnalyticsContext, AnalyticsRange, HistoryPeriod},
    TimelineBucketDto,
};

#[derive(Clone, Default)]
struct BucketValues {
    cards_practiced: i64,
    review_count: i64,
    new_cards: i64,
    remembered_count: i64,
    average_confidence: Option<f64>,
    average_response_time_ms: Option<f64>,
    study_time_ms: i64,
    session_count: i64,
    highlighted: bool,
}

pub fn load(
    connection: &Connection,
    context: &AnalyticsContext,
    earliest_local_date: Option<&str>,
) -> Result<Vec<TimelineBucketDto>, CommandError> {
    match context.range {
        AnalyticsRange::All => load_months(connection, context, earliest_local_date),
        _ => load_local_date_buckets(connection, context),
    }
}

fn load_local_date_buckets(
    connection: &Connection,
    context: &AnalyticsContext,
) -> Result<Vec<TimelineBucketDto>, CommandError> {
    let window = context.window.current.as_ref().ok_or_else(|| {
        CommandError::new(
            "invalid_analytics_window",
            "The selected analytics range has no local-date window.",
        )
    })?;
    let step_days = context.range.step_days().unwrap_or(1);
    let mut values = Vec::<SqlValue>::new();
    let conditions = context.historical_conditions(&mut values, HistoryPeriod::Current);
    let start_index = push_value(&mut values, window.start_local_date.to_string());
    let highlighted_index = context
        .highlighted_session_id
        .as_ref()
        .map(|session_id| push_value(&mut values, session_id.clone()));
    let highlighted_sql = highlighted_index.map_or_else(
        || "0".to_string(),
        |index| format!("MAX(CASE WHEN r.session_id = ?{index} THEN 1 ELSE 0 END)"),
    );
    let sql = format!(
        "SELECT
             CAST((julianday(a.review_local_date) - julianday(?{start_index})) / {step_days} AS INTEGER),
             COUNT(DISTINCT r.card_id),
             COUNT(*),
             COALESCE(SUM(CASE WHEN r.state_before = 0 THEN 1 ELSE 0 END), 0),
             COALESCE(SUM(r.answer_correct), 0),
             AVG(CAST(r.rating - 1 AS REAL)),
             AVG(CAST(r.response_time_ms AS REAL)),
             COALESCE(SUM(r.response_time_ms), 0),
             COUNT(DISTINCT r.session_id),
             {highlighted_sql}
         FROM review_events r
         JOIN review_event_attributions a ON a.review_event_id = r.id
         JOIN cards c ON c.id = r.card_id
         JOIN notes n ON n.id = c.note_id
         JOIN decks d ON d.id = c.deck_id
         WHERE {conditions}
         GROUP BY 1
         ORDER BY 1",
        conditions = conditions.join(" AND "),
    );
    let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
    let rows = statement
        .query_map(params_from_iter(values.iter()), |row| {
            Ok((
                row.get::<_, i64>(0)?,
                BucketValues {
                    cards_practiced: row.get(1)?,
                    review_count: row.get(2)?,
                    new_cards: row.get(3)?,
                    remembered_count: row.get(4)?,
                    average_confidence: row.get(5)?,
                    average_response_time_ms: row.get(6)?,
                    study_time_ms: row.get(7)?,
                    session_count: row.get(8)?,
                    highlighted: row.get::<_, i64>(9)? != 0,
                },
            ))
        })
        .map_err(CommandError::database)?
        .collect::<Result<BTreeMap<_, _>, _>>()
        .map_err(CommandError::database)?;

    let date_count = window
        .end_local_date
        .signed_duration_since(window.start_local_date)
        .num_days()
        .saturating_add(1);
    let bucket_count = date_count
        .saturating_add(step_days - 1)
        .div_euclid(step_days);
    let mut timeline = Vec::with_capacity(usize::try_from(bucket_count).unwrap_or_default());
    for index in 0..bucket_count {
        let start_date = checked_add_days(window.start_local_date, index * step_days)?;
        let end_date = checked_add_days(start_date, step_days - 1)?.min(window.end_local_date);
        let values = rows.get(&index).cloned().unwrap_or_default();
        timeline.push(to_dto(context, start_date, end_date, values)?);
    }
    Ok(timeline)
}

fn load_months(
    connection: &Connection,
    context: &AnalyticsContext,
    earliest_local_date: Option<&str>,
) -> Result<Vec<TimelineBucketDto>, CommandError> {
    let first_review_date = earliest_local_date
        .map(|value| NaiveDate::parse_from_str(value, "%Y-%m-%d"))
        .transpose()
        .map_err(|_| invalid_date())?
        .unwrap_or(context.time.local_date);
    let first_month = context.time.month_start(first_review_date)?;
    let last_month = context.time.month_start(context.time.local_date)?;

    let mut values = Vec::<SqlValue>::new();
    let conditions = context.historical_conditions(&mut values, HistoryPeriod::Current);
    let highlighted_index = context
        .highlighted_session_id
        .as_ref()
        .map(|session_id| push_value(&mut values, session_id.clone()));
    let highlighted_sql = highlighted_index.map_or_else(
        || "0".to_string(),
        |index| format!("MAX(CASE WHEN r.session_id = ?{index} THEN 1 ELSE 0 END)"),
    );
    let sql = format!(
        "SELECT
             substr(a.review_local_date, 1, 7),
             COUNT(DISTINCT r.card_id),
             COUNT(*),
             COALESCE(SUM(CASE WHEN r.state_before = 0 THEN 1 ELSE 0 END), 0),
             COALESCE(SUM(r.answer_correct), 0),
             AVG(CAST(r.rating - 1 AS REAL)),
             AVG(CAST(r.response_time_ms AS REAL)),
             COALESCE(SUM(r.response_time_ms), 0),
             COUNT(DISTINCT r.session_id),
             {highlighted_sql}
         FROM review_events r
         JOIN review_event_attributions a ON a.review_event_id = r.id
         JOIN cards c ON c.id = r.card_id
         JOIN notes n ON n.id = c.note_id
         JOIN decks d ON d.id = c.deck_id
         WHERE {conditions}
         GROUP BY 1
         ORDER BY 1",
        conditions = conditions.join(" AND "),
    );
    let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
    let rows = statement
        .query_map(params_from_iter(values.iter()), |row| {
            Ok((
                row.get::<_, String>(0)?,
                BucketValues {
                    cards_practiced: row.get(1)?,
                    review_count: row.get(2)?,
                    new_cards: row.get(3)?,
                    remembered_count: row.get(4)?,
                    average_confidence: row.get(5)?,
                    average_response_time_ms: row.get(6)?,
                    study_time_ms: row.get(7)?,
                    session_count: row.get(8)?,
                    highlighted: row.get::<_, i64>(9)? != 0,
                },
            ))
        })
        .map_err(CommandError::database)?
        .collect::<Result<BTreeMap<_, _>, _>>()
        .map_err(CommandError::database)?;

    let mut timeline = Vec::new();
    let mut month = first_month;
    while month <= last_month {
        let next = context.time.next_month_start(month)?;
        let end_date = if month == last_month {
            context.time.local_date
        } else {
            checked_add_days(next, -1)?
        };
        let key = month.format("%Y-%m").to_string();
        timeline.push(to_dto(
            context,
            month,
            end_date,
            rows.get(&key).cloned().unwrap_or_default(),
        )?);
        month = next;
    }
    Ok(timeline)
}

fn to_dto(
    context: &AnalyticsContext,
    start_date: NaiveDate,
    end_date: NaiveDate,
    values: BucketValues,
) -> Result<TimelineBucketDto, CommandError> {
    let next_date = checked_add_days(end_date, 1)?;
    let end_at = if end_date >= context.time.local_date {
        context.time.now_utc_millis.saturating_add(1)
    } else {
        context.time.first_valid_instant(next_date)?
    };
    Ok(TimelineBucketDto {
        start_local_date: start_date.to_string(),
        end_local_date: end_date.to_string(),
        start_at: context.time.first_valid_instant(start_date)?,
        end_at,
        cards_practiced: values.cards_practiced,
        review_count: values.review_count,
        new_cards: values.new_cards,
        remembered_count: values.remembered_count,
        correct_rate: rate(values.remembered_count, values.review_count),
        average_confidence: values.average_confidence,
        average_response_time_ms: values.average_response_time_ms,
        study_time_ms: values.study_time_ms,
        session_count: values.session_count,
        contains_highlighted_session: values.highlighted,
    })
}

fn checked_add_days(date: NaiveDate, days: i64) -> Result<NaiveDate, CommandError> {
    date.checked_add_signed(Duration::days(days))
        .ok_or_else(invalid_date)
}

fn invalid_date() -> CommandError {
    CommandError::new(
        "invalid_analytics_date",
        "The analytics date range is outside the supported calendar.",
    )
}
