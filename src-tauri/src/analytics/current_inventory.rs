use rusqlite::{params_from_iter, types::Value as SqlValue, Connection};

use crate::{
    error::CommandError,
    learning_metrics,
    projections::{learning_signals as signal_projection, scope::push_value},
    studyability,
};

use super::{scope::AnalyticsContext, signals::RequestSignals, CurrentInventorySummaryDto};

pub fn load(
    connection: &Connection,
    context: &AnalyticsContext,
    signals: &RequestSignals<'_>,
) -> Result<CurrentInventorySummaryDto, CommandError> {
    let mut values = vec![SqlValue::Integer(context.time.now_utc_millis)];
    let conditions = context.current_card_conditions(&mut values);
    let tomorrow = format!(
        "?{}",
        push_value(&mut values, context.time.tomorrow_start_utc_millis)
    );
    let sql = format!(
        "SELECT
             COUNT(DISTINCT d.id),
             COUNT(DISTINCT n.id),
             COUNT(DISTINCT c.id),
             COALESCE(SUM(CASE WHEN {studyable} THEN 1 ELSE 0 END), 0),
             COALESCE(SUM(CASE WHEN {studyable_due} THEN 1 ELSE 0 END), 0),
             COALESCE(SUM(CASE WHEN {signal_value} = 1 THEN 1 ELSE 0 END), 0),
             COALESCE(SUM(CASE WHEN {studyable} AND {ready_review} THEN 1 ELSE 0 END), 0),
             COALESCE(SUM(CASE WHEN {studyable} AND {ready_new} THEN 1 ELSE 0 END), 0),
             COALESCE(SUM(CASE WHEN {studyable} AND {due_today} THEN 1 ELSE 0 END), 0)
         FROM cards c
         JOIN notes n ON n.id = c.note_id
         JOIN decks d ON d.id = c.deck_id
         JOIN card_schedules s ON s.card_id = c.id
         {signal_join}
         WHERE {conditions}",
        studyable = studyability::studyable_card_sql("?1"),
        studyable_due = studyability::studyable_due_sql("?1"),
        signal_value = signal_projection::value_sql(),
        ready_review = learning_metrics::ready_review_sql("s", "?1"),
        ready_new = learning_metrics::ready_new_sql("s", "?1"),
        due_today = learning_metrics::due_today_review_sql("s", &tomorrow),
        signal_join = signals.join_sql(),
        conditions = conditions.join(" AND "),
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
                ready_review_count: row.get(6)?,
                ready_new_count: row.get(7)?,
                due_today_review_count: row.get(8)?,
            })
        })
        .map_err(CommandError::database)
}
