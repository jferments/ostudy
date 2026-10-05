//! One row per subject: its cards now and its reviews in the range.

use std::collections::BTreeMap;

use rusqlite::{params_from_iter, types::Value as SqlValue, Connection};
use serde::Serialize;

use crate::{
    error::CommandError,
    learning_metrics::{
        self, rate, MIN_TOPIC_RETENTION_REVIEWS, SOLID_STABILITY_DAYS, STATE_REVIEW,
    },
    projections::scope::push_value,
    studyability,
};

use super::super::scope::{AnalyticsContext, HistoryPeriod};

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubjectProgressDto {
    /// `None` is the decks filed under no subject.
    pub subject_id: Option<String>,
    pub name: Option<String>,
    pub card_count: i64,
    pub solid_count: i64,
    pub due_today_count: i64,
    pub review_count: i64,
    pub remembered_count: i64,
    pub correct_rate: Option<f64>,
    pub retention_review_count: i64,
    pub retention_remembered_count: i64,
    pub true_retention: Option<f64>,
    /// True retention minus the previous equal period's, when both periods
    /// have enough retention reviews to classify a topic.
    pub retention_delta: Option<f64>,
    pub study_time_ms: i64,
}

pub fn load(
    connection: &Connection,
    context: &AnalyticsContext,
) -> Result<Vec<SubjectProgressDto>, CommandError> {
    let mut rows = BTreeMap::<Option<String>, SubjectProgressDto>::new();

    let mut values = vec![SqlValue::Integer(context.time.now_utc_millis)];
    let mut conditions = context.current_card_conditions(&mut values);
    conditions.push("c.suspended_at IS NULL AND c.archive_suspended_at IS NULL".to_string());
    let tomorrow = format!(
        "?{}",
        push_value(&mut values, context.time.tomorrow_start_utc_millis)
    );
    let sql = format!(
        "SELECT d.subject_id, COUNT(*),
             COALESCE(SUM(CASE WHEN s.state = {STATE_REVIEW}
                 AND s.stability >= {SOLID_STABILITY_DAYS:.1} THEN 1 ELSE 0 END), 0),
             COALESCE(SUM(CASE WHEN {studyable} AND {due_today} THEN 1 ELSE 0 END), 0)
         FROM cards c
         JOIN notes n ON n.id = c.note_id
         JOIN decks d ON d.id = c.deck_id
         JOIN card_schedules s ON s.card_id = c.id
         WHERE {conditions}
         GROUP BY d.subject_id",
        studyable = studyability::studyable_card_sql("?1"),
        due_today = learning_metrics::due_today_review_sql("s", &tomorrow),
        conditions = conditions.join(" AND "),
    );
    let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
    let current = statement
        .query_map(params_from_iter(values.iter()), |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    for (subject_id, card_count, solid_count, due_today_count) in current {
        let row = rows.entry(subject_id).or_default();
        row.card_count = card_count;
        row.solid_count = solid_count;
        row.due_today_count = due_today_count;
    }

    let mut values = Vec::<SqlValue>::new();
    let conditions =
        context.historical_conditions(&mut values, HistoryPeriod::CurrentAndPrevious)?;
    let current_predicate = context.time.review_study_dates_sql(
        "a",
        context
            .window
            .current
            .as_ref()
            .map(|window| window.start_local_date),
        None,
        &mut values,
    )?;
    let previous_predicate = match context.previous_window() {
        Some(window) => context.time.review_study_dates_sql(
            "a",
            Some(window.start_local_date),
            Some(window.end_local_date),
            &mut values,
        )?,
        None => "0 = 1".to_string(),
    };
    let sql = format!(
        "SELECT d.subject_id,
             COALESCE(SUM(CASE WHEN {current_predicate} THEN 1 ELSE 0 END), 0),
             {remembered},
             {retention_reviews},
             {retention_remembered},
             COALESCE(SUM(CASE WHEN {current_predicate} THEN r.response_time_ms ELSE 0 END), 0),
             {previous_retention_reviews},
             {previous_retention_remembered}
         FROM {history_from}
         WHERE {conditions}
         GROUP BY d.subject_id",
        remembered = learning_metrics::remembered_count_where_sql("r", &current_predicate),
        retention_reviews = learning_metrics::retention_review_count_sql("r", &current_predicate),
        retention_remembered =
            learning_metrics::retention_remembered_count_sql("r", &current_predicate),
        previous_retention_reviews =
            learning_metrics::retention_review_count_sql("r", &previous_predicate),
        previous_retention_remembered =
            learning_metrics::retention_remembered_count_sql("r", &previous_predicate),
        history_from = context.history_from_sql(),
        conditions = conditions.join(" AND "),
    );
    let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
    let history = statement
        .query_map(params_from_iter(values.iter()), |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                [
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, i64>(7)?,
                ],
            ))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    for (
        subject_id,
        [reviews, remembered, retention, retained, time, previous, previous_retained],
    ) in history
    {
        if reviews == 0 && !rows.contains_key(&subject_id) {
            continue;
        }
        let row = rows.entry(subject_id).or_default();
        row.review_count = reviews;
        row.remembered_count = remembered;
        row.correct_rate = rate(remembered, reviews);
        row.retention_review_count = retention;
        row.retention_remembered_count = retained;
        row.true_retention = rate(retained, retention);
        row.study_time_ms = time;
        if retention >= MIN_TOPIC_RETENTION_REVIEWS && previous >= MIN_TOPIC_RETENTION_REVIEWS {
            row.retention_delta = rate(retained, retention)
                .zip(rate(previous_retained, previous))
                .map(|(now, before)| now - before);
        }
    }

    // Subjects in the library's order; there are few of them.
    let subjects = subject_order(connection)?;
    let position = |id: &Option<String>| {
        id.as_ref()
            .and_then(|id| subjects.iter().position(|(key, _)| key == id))
            .unwrap_or(usize::MAX)
    };
    let mut result: Vec<SubjectProgressDto> = rows
        .into_iter()
        .map(|(subject_id, mut row)| {
            row.name = subject_id.as_ref().and_then(|id| {
                subjects
                    .iter()
                    .find(|(key, _)| key == id)
                    .map(|(_, name)| name.clone())
            });
            row.subject_id = subject_id;
            row
        })
        .collect();
    // Most to review first, then the library's own subject order.
    result.sort_by(|left, right| {
        right
            .due_today_count
            .cmp(&left.due_today_count)
            .then_with(|| position(&left.subject_id).cmp(&position(&right.subject_id)))
    });
    Ok(result)
}

fn subject_order(connection: &Connection) -> Result<Vec<(String, String)>, CommandError> {
    let mut statement = connection
        .prepare("SELECT id, name FROM document_collections ORDER BY sort_order, id")
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    Ok(rows)
}
