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

use super::{
    super::scope::AnalyticsContext,
    hours::{self, Totals},
};

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
    connection: &mut Connection,
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
    drop(statement);
    for (subject_id, card_count, solid_count, due_today_count) in current {
        let row = rows.entry(subject_id).or_default();
        row.card_count = card_count;
        row.solid_count = solid_count;
        row.due_today_count = due_today_count;
    }

    let current_start = context
        .window
        .current
        .as_ref()
        .map(|window| window.start_local_date);
    let mut history = BTreeMap::<Option<String>, (Totals, Totals)>::new();
    for day in hours::load(connection, context, true)? {
        let (current, previous) = history.entry(day.subject_id).or_default();
        if current_start.is_none_or(|start| day.study_date >= start) {
            current.add(&day.totals);
        } else {
            previous.add(&day.totals);
        }
    }
    for (subject_id, (current, previous)) in history {
        if current.review_count == 0 && !rows.contains_key(&subject_id) {
            continue;
        }
        let row = rows.entry(subject_id).or_default();
        row.review_count = current.review_count;
        row.remembered_count = current.remembered_count;
        row.correct_rate = rate(current.remembered_count, current.review_count);
        row.retention_review_count = current.retention_review_count;
        row.retention_remembered_count = current.retention_remembered_count;
        row.true_retention = rate(
            current.retention_remembered_count,
            current.retention_review_count,
        );
        row.study_time_ms = current.response_time_ms;
        if current.retention_review_count >= MIN_TOPIC_RETENTION_REVIEWS
            && previous.retention_review_count >= MIN_TOPIC_RETENTION_REVIEWS
        {
            row.retention_delta = row
                .true_retention
                .zip(rate(
                    previous.retention_remembered_count,
                    previous.retention_review_count,
                ))
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
