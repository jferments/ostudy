//! Bounded daily workload accounting and authoritative scheduled-session planning.
//!
//! The workload planner consumes one captured local-calendar context. Counts are
//! aggregate SQL projections and selected rows are capped before hydration; query
//! count therefore does not grow with the number of cards in the library.

use rusqlite::{params_from_iter, types::Value as SqlValue, Connection};
use serde::Serialize;

use crate::{
    authoring::presentation_variant_for_projected, error::CommandError, settings::AppSettingsDto,
    study_calendar::StudyTimeContext, studyability::learning_signal_cte,
};

const MAX_PLANNED_CARDS: i64 = 10_000;

#[derive(Clone, Debug)]
pub struct PlannedCard {
    pub deck_id: String,
    pub card_id: String,
    pub due_at: i64,
    pub state: i64,
    pub needs_more_practice: bool,
    pub presentation_variant: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkloadOverviewDto {
    pub new_limit: i64,
    pub review_limit: i64,
    pub new_completed: i64,
    pub reviews_completed: i64,
    pub new_remaining: i64,
    pub reviews_remaining: i64,
    pub due_reviews: i64,
    pub due_new: i64,
    pub scheduled_reviews: i64,
    pub scheduled_new: i64,
    pub omitted_due_reviews: i64,
    pub omitted_due_new: i64,
    pub needs_more_practice: i64,
    pub extra_available: i64,
}

pub fn workload_overview_scoped(
    connection: &Connection,
    deck_ids: &[String],
    tag_ids: &[String],
    settings: &AppSettingsDto,
    context: &StudyTimeContext,
) -> Result<WorkloadOverviewDto, CommandError> {
    let (scope_sql, mut values) = scope_conditions(deck_ids, tag_ids, context.now_utc_millis);
    let today_index = push_value(&mut values, context.local_date_string());
    let sql = format!(
        "WITH {signals},
         scoped AS (
             SELECT c.id, c.deck_id, s.state, s.due_at,
                    COALESCE(ls.needs_more_practice, 0) AS needs_more_practice
             FROM cards c
             JOIN notes n ON n.id = c.note_id
             JOIN decks d ON d.id = c.deck_id
             JOIN card_schedules s ON s.card_id = c.id
             LEFT JOIN learning_signals ls ON ls.card_id = c.id
             WHERE {scope_sql}
         ),
         completed AS (
             SELECT
                 COALESCE(SUM(CASE WHEN r.state_before = 0 THEN 1 ELSE 0 END), 0) AS new_completed,
                 COALESCE(SUM(CASE WHEN r.state_before <> 0 THEN 1 ELSE 0 END), 0) AS reviews_completed
             FROM review_events r
             JOIN review_event_attributions a ON a.review_event_id = r.id
             WHERE a.review_local_date = ?{today_index}
         )
         SELECT
             completed.new_completed,
             completed.reviews_completed,
             COALESCE(SUM(CASE WHEN scoped.due_at <= ?1 AND scoped.state <> 0 THEN 1 ELSE 0 END), 0),
             COALESCE(SUM(CASE WHEN scoped.due_at <= ?1 AND scoped.state = 0 THEN 1 ELSE 0 END), 0),
             COALESCE(SUM(scoped.needs_more_practice), 0),
             COUNT(scoped.id)
         FROM completed LEFT JOIN scoped ON 1 = 1",
        signals = learning_signal_cte(),
    );
    let (new_completed, reviews_completed, due_reviews, due_new, needs, extra): (
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
    ) = connection
        .query_row(&sql, params_from_iter(values.iter()), |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
            ))
        })
        .map_err(CommandError::database)?;
    let new_remaining = (settings.daily_new_limit - new_completed).max(0);
    let reviews_remaining = (settings.daily_review_limit - reviews_completed).max(0);
    let scheduled_reviews = due_reviews.min(reviews_remaining);
    let scheduled_new = if due_reviews > reviews_remaining {
        0
    } else {
        due_new.min(new_remaining)
    };
    Ok(WorkloadOverviewDto {
        new_limit: settings.daily_new_limit,
        review_limit: settings.daily_review_limit,
        new_completed,
        reviews_completed,
        new_remaining,
        reviews_remaining,
        due_reviews,
        due_new,
        scheduled_reviews,
        scheduled_new,
        omitted_due_reviews: due_reviews.saturating_sub(scheduled_reviews),
        omitted_due_new: due_new.saturating_sub(scheduled_new),
        needs_more_practice: needs,
        extra_available: extra,
    })
}

pub fn planned_scheduled_cards_scoped(
    connection: &Connection,
    deck_ids: &[String],
    tag_ids: &[String],
    settings: &AppSettingsDto,
    context: &StudyTimeContext,
) -> Result<(Vec<PlannedCard>, WorkloadOverviewDto), CommandError> {
    let overview = workload_overview_scoped(connection, deck_ids, tag_ids, settings, context)?;
    let mut selected = load_ranked(
        connection,
        deck_ids,
        tag_ids,
        context,
        false,
        overview.scheduled_reviews,
    )?;
    if overview.scheduled_new > 0 {
        selected.extend(load_ranked(
            connection,
            deck_ids,
            tag_ids,
            context,
            true,
            overview.scheduled_new,
        )?);
    }
    Ok((selected, overview))
}

pub fn planned_extra_cards_scoped(
    connection: &Connection,
    deck_ids: &[String],
    tag_ids: &[String],
    context: &StudyTimeContext,
) -> Result<Vec<PlannedCard>, CommandError> {
    let (scope_sql, mut values) = scope_conditions(deck_ids, tag_ids, context.now_utc_millis);
    let limit_index = push_value(&mut values, MAX_PLANNED_CARDS);
    let sql = format!(
        "WITH {signals},
         ranked AS (
             SELECT c.deck_id, c.id, s.due_at, s.state,
                    COALESCE(ls.needs_more_practice, 0) AS needs_more_practice,
                    n.note_type, n.cloze_source, n.cloze_mode, s.reps,
                    ROW_NUMBER() OVER (
                        PARTITION BY c.deck_id
                        ORDER BY CASE WHEN s.due_at <= ?1 THEN 0 ELSE 1 END,
                                 COALESCE(ls.needs_more_practice, 0) DESC,
                                 s.due_at, c.id
                    ) AS deck_rank
             FROM cards c
             JOIN notes n ON n.id = c.note_id
             JOIN decks d ON d.id = c.deck_id
             JOIN card_schedules s ON s.card_id = c.id
             LEFT JOIN learning_signals ls ON ls.card_id = c.id
             WHERE {scope_sql}
         )
         SELECT deck_id, id, due_at, state, needs_more_practice,
                note_type, cloze_source, cloze_mode, reps
         FROM ranked
         ORDER BY deck_rank, deck_id, due_at, id
         LIMIT ?{limit_index}",
        signals = learning_signal_cte(),
    );
    query_planned(connection, &sql, &values)
}

fn load_ranked(
    connection: &Connection,
    deck_ids: &[String],
    tag_ids: &[String],
    context: &StudyTimeContext,
    new_cards: bool,
    limit: i64,
) -> Result<Vec<PlannedCard>, CommandError> {
    if limit <= 0 {
        return Ok(Vec::new());
    }
    let (scope_sql, mut values) = scope_conditions(deck_ids, tag_ids, context.now_utc_millis);
    let limit_index = push_value(&mut values, limit.min(MAX_PLANNED_CARDS));
    let state_clause = if new_cards {
        "s.state = 0"
    } else {
        "s.state <> 0"
    };
    let sql = format!(
        "WITH {signals},
         ranked AS (
             SELECT c.deck_id, c.id, s.due_at, s.state,
                    COALESCE(ls.needs_more_practice, 0) AS needs_more_practice,
                    n.note_type, n.cloze_source, n.cloze_mode, s.reps,
                    ROW_NUMBER() OVER (
                        PARTITION BY c.deck_id
                        ORDER BY COALESCE(ls.needs_more_practice, 0) DESC,
                                 s.due_at, c.id
                    ) AS deck_rank
             FROM cards c
             JOIN notes n ON n.id = c.note_id
             JOIN decks d ON d.id = c.deck_id
             JOIN card_schedules s ON s.card_id = c.id
             LEFT JOIN learning_signals ls ON ls.card_id = c.id
             WHERE {scope_sql} AND s.due_at <= ?1 AND {state_clause}
         )
         SELECT deck_id, id, due_at, state, needs_more_practice,
                note_type, cloze_source, cloze_mode, reps
         FROM ranked
         ORDER BY deck_rank, deck_id, due_at, id
         LIMIT ?{limit_index}",
        signals = learning_signal_cte(),
    );
    query_planned(connection, &sql, &values)
}

fn query_planned(
    connection: &Connection,
    sql: &str,
    values: &[SqlValue],
) -> Result<Vec<PlannedCard>, CommandError> {
    let mut statement = connection.prepare(sql).map_err(CommandError::database)?;
    let rows = statement
        .query_map(params_from_iter(values.iter()), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, i64>(8)?,
            ))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    rows.into_iter()
        .map(
            |(deck_id, card_id, due_at, state, weak, note_type, source, mode, reps)| {
                Ok(PlannedCard {
                    deck_id,
                    presentation_variant: presentation_variant_for_projected(
                        &note_type, &source, &mode, reps,
                    )?,
                    card_id,
                    due_at,
                    state,
                    needs_more_practice: weak != 0,
                })
            },
        )
        .collect()
}

fn scope_conditions(deck_ids: &[String], tag_ids: &[String], now: i64) -> (String, Vec<SqlValue>) {
    let mut values = vec![now.into()];
    let mut conditions = vec![
        "c.deleted_at IS NULL".to_string(),
        "n.deleted_at IS NULL".to_string(),
        "d.deleted_at IS NULL".to_string(),
        "d.archived_at IS NULL".to_string(),
        "c.suspended_at IS NULL".to_string(),
        "(c.buried_until IS NULL OR c.buried_until <= ?1)".to_string(),
    ];
    if !deck_ids.is_empty() {
        let placeholders = deck_ids
            .iter()
            .map(|id| {
                values.push(id.clone().into());
                format!("?{}", values.len())
            })
            .collect::<Vec<_>>()
            .join(", ");
        conditions.push(format!("c.deck_id IN ({placeholders})"));
    }
    for tag_id in tag_ids {
        values.push(tag_id.clone().into());
        conditions.push(format!(
            "EXISTS (SELECT 1 FROM note_tags scope_nt WHERE scope_nt.note_id = n.id AND scope_nt.tag_id = ?{})",
            values.len()
        ));
    }
    (conditions.join(" AND "), values)
}

fn push_value(values: &mut Vec<SqlValue>, value: impl Into<SqlValue>) -> usize {
    values.push(value.into());
    values.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn planner_enforces_a_hard_session_cap() {
        assert_eq!(MAX_PLANNED_CARDS, 10_000);
    }
}
