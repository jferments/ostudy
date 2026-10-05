//! Card strength buckets and estimated recall.

use rusqlite::{params_from_iter, types::Value as SqlValue, Connection};
use serde::Serialize;

use crate::{
    error::CommandError,
    learning_metrics::{CardStrength, SOLID_STABILITY_DAYS},
    scheduler,
};

use super::super::scope::AnalyticsContext;

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardStrengthDto {
    pub new_count: i64,
    pub learning_count: i64,
    pub getting_there_count: i64,
    pub solid_count: i64,
    /// Stability in days at which a card counts as solid.
    pub solid_stability_days: f64,
    /// Mean predicted recall right now over studied cards with a prediction.
    pub estimated_recall: Option<f64>,
    pub estimated_recall_card_count: i64,
}

/// Current, unsuspended cards in scope; a buried card is still in the library.
pub fn load(
    connection: &Connection,
    context: &AnalyticsContext,
) -> Result<CardStrengthDto, CommandError> {
    let mut values = Vec::<SqlValue>::new();
    let mut conditions = context.current_card_conditions(&mut values);
    conditions.push("c.suspended_at IS NULL AND c.archive_suspended_at IS NULL".to_string());
    let sql = format!(
        "SELECT s.state, s.stability, s.last_review_at
         FROM cards c
         JOIN notes n ON n.id = c.note_id
         JOIN decks d ON d.id = c.deck_id
         JOIN card_schedules s ON s.card_id = c.id
         WHERE {}",
        conditions.join(" AND ")
    );
    let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
    let mut rows = statement
        .query(params_from_iter(values.iter()))
        .map_err(CommandError::database)?;
    let mut result = CardStrengthDto {
        solid_stability_days: SOLID_STABILITY_DAYS,
        ..CardStrengthDto::default()
    };
    let mut recall_sum = 0.0;
    while let Some(row) = rows.next().map_err(CommandError::database)? {
        let state: i64 = row.get(0).map_err(CommandError::database)?;
        let stability: f64 = row.get(1).map_err(CommandError::database)?;
        let last_review_at: i64 = row.get(2).map_err(CommandError::database)?;
        match CardStrength::of(state, stability) {
            CardStrength::New => result.new_count += 1,
            CardStrength::Learning => result.learning_count += 1,
            CardStrength::GettingThere => result.getting_there_count += 1,
            CardStrength::Solid => result.solid_count += 1,
        }
        if let Some(recall) = scheduler::estimated_recall(
            state,
            stability,
            last_review_at,
            context.time.now_utc_millis,
        ) {
            recall_sum += recall;
            result.estimated_recall_card_count += 1;
        }
    }
    if result.estimated_recall_card_count > 0 {
        result.estimated_recall = Some(recall_sum / result.estimated_recall_card_count as f64);
    }
    Ok(result)
}
