//! Reviews due on each of the next study days.

use rusqlite::{params_from_iter, types::Value as SqlValue, Connection};
use serde::Serialize;

use crate::{
    error::CommandError,
    learning_metrics::{self, STATE_NEW},
    projections::scope::push_value,
};

use super::super::scope::AnalyticsContext;

/// Study days in the forecast, today included.
pub const FORECAST_DAYS: i64 = 14;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ForecastDayDto {
    pub local_date: String,
    pub due_count: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressForecastDto {
    /// Reviews due on each study day from today on. Today includes overdue
    /// reviews; a card's due instant decides its day exactly.
    pub days: Vec<ForecastDayDto>,
    /// Reviews already overdue before today started, included in today.
    pub overdue_count: i64,
    /// New cards available to learn now; they have no due day of their own.
    pub new_available_count: i64,
}

pub fn load(
    connection: &Connection,
    context: &AnalyticsContext,
) -> Result<ProgressForecastDto, CommandError> {
    let time = &context.time;
    let mut values = vec![SqlValue::Integer(time.now_utc_millis)];
    let mut conditions = context.current_card_conditions(&mut values);
    // Burial ends within a day, so a buried card still counts on its due day.
    conditions.push("c.suspended_at IS NULL AND c.archive_suspended_at IS NULL".to_string());
    let today_start = push_value(&mut values, time.today_start_utc_millis);
    let mut ends = Vec::with_capacity(usize::try_from(FORECAST_DAYS).unwrap_or_default());
    let mut dates = Vec::with_capacity(ends.capacity());
    for offset in 0..FORECAST_DAYS {
        let date = time
            .local_date
            .checked_add_signed(chrono::Duration::days(offset))
            .ok_or_else(invalid_date)?;
        let next = date.succ_opt().ok_or_else(invalid_date)?;
        dates.push(date);
        ends.push(push_value(&mut values, time.study_day_start(next)?));
    }
    let day_case = ends
        .iter()
        .enumerate()
        .map(|(day, end)| format!("WHEN s.due_at < ?{end} THEN {day}"))
        .collect::<Vec<_>>()
        .join(" ");
    let last_end = ends.last().copied().ok_or_else(invalid_date)?;
    let sql = format!(
        "SELECT
             CASE WHEN s.state = {STATE_NEW} THEN -1 ELSE CASE {day_case} END END AS day,
             COUNT(*),
             COALESCE(SUM(CASE WHEN s.state <> {STATE_NEW} AND s.due_at < ?{today_start}
                 THEN 1 ELSE 0 END), 0)
         FROM cards c
         JOIN notes n ON n.id = c.note_id
         JOIN decks d ON d.id = c.deck_id
         JOIN card_schedules s ON s.card_id = c.id
         WHERE {conditions}
           AND ({ready_new} OR (s.state <> {STATE_NEW} AND s.due_at < ?{last_end}))
         GROUP BY 1",
        ready_new = learning_metrics::ready_new_sql("s", "?1"),
        conditions = conditions.join(" AND "),
    );
    let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
    let rows = statement
        .query_map(params_from_iter(values.iter()), |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    let mut counts = vec![0_i64; dates.len()];
    let mut overdue_count = 0;
    let mut new_available_count = 0;
    for (day, count, overdue) in rows {
        match usize::try_from(day) {
            Ok(index) => {
                if let Some(slot) = counts.get_mut(index) {
                    *slot = count;
                }
                overdue_count += overdue;
            }
            Err(_) => new_available_count = count,
        }
    }
    Ok(ProgressForecastDto {
        days: dates
            .into_iter()
            .zip(counts)
            .map(|(date, due_count)| ForecastDayDto {
                local_date: date.to_string(),
                due_count,
            })
            .collect(),
        overdue_count,
        new_available_count,
    })
}

fn invalid_date() -> CommandError {
    CommandError::new(
        "invalid_analytics_date",
        "The forecast dates are outside the supported calendar.",
    )
}
