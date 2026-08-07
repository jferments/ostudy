//! Daily workload accounting and bounded scheduled-session planning.

use rusqlite::{params_from_iter, types::Value as SqlValue, Connection};
use serde::Serialize;

use crate::{
    error::CommandError,
    projections::{
        learning_signals as signal_projection,
        scope::{append_in_filter, push_value},
    },
    settings::AppSettingsDto,
    study_calendar::StudyTimeContext,
    studyability,
};

const MAX_PLANNED_CARDS: i64 = 10_000;

#[derive(Clone, Copy, Debug)]
pub struct StudyScope<'a> {
    pub deck_ids: &'a [String],
    pub tag_ids: &'a [String],
}

#[derive(Clone, Debug)]
pub struct PlannedCard {
    pub deck_id: String,
    pub card_id: String,
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
    #[serde(rename = "studyableDueReviews")]
    pub due_reviews: i64,
    #[serde(rename = "studyableDueNew")]
    pub due_new: i64,
    pub scheduled_reviews: i64,
    pub scheduled_new: i64,
    pub omitted_due_reviews: i64,
    pub omitted_due_new: i64,
    pub needs_more_practice: i64,
    pub extra_available: i64,
}

pub fn workload_overview_for_scope(
    connection: &Connection,
    scope: StudyScope<'_>,
    settings: &AppSettingsDto,
    time: &StudyTimeContext,
) -> Result<WorkloadOverviewDto, CommandError> {
    let local_date = time.local_date.to_string();
    let (new_completed, reviews_completed): (i64, i64) = connection
        .query_row(
            "SELECT
                 COALESCE(SUM(CASE WHEN r.state_before = 0 THEN 1 ELSE 0 END), 0),
                 COALESCE(SUM(CASE WHEN r.state_before <> 0 THEN 1 ELSE 0 END), 0)
             FROM review_events r
             JOIN review_event_attributions a ON a.review_event_id = r.id
             WHERE a.review_local_date = ?1",
            [local_date],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(CommandError::database)?;

    let mut values = vec![SqlValue::Integer(time.now_utc_millis)];
    let conditions = scoped_studyable_conditions(scope, &mut values);
    let sql = format!(
        "WITH {signals}
         SELECT
             COALESCE(SUM(CASE WHEN s.due_at <= ?1 AND s.state <> 0 THEN 1 ELSE 0 END), 0),
             COALESCE(SUM(CASE WHEN s.due_at <= ?1 AND s.state = 0 THEN 1 ELSE 0 END), 0),
             COUNT(*),
             COALESCE(SUM(CASE WHEN {signal_value} = 1 THEN 1 ELSE 0 END), 0)
         FROM cards c
         JOIN notes n ON n.id = c.note_id
         JOIN decks d ON d.id = c.deck_id
         JOIN card_schedules s ON s.card_id = c.id
         {signal_join}
         WHERE {conditions}",
        signals = signal_projection::LEARNING_SIGNAL_CTES,
        signal_value = signal_projection::value_sql(),
        signal_join = signal_projection::join_sql(),
        conditions = conditions.join(" AND "),
    );
    let (due_reviews, due_new, extra_available, needs_more_practice): (i64, i64, i64, i64) =
        connection
            .query_row(&sql, params_from_iter(values.iter()), |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })
            .map_err(CommandError::database)?;

    let new_remaining = settings
        .daily_new_limit
        .saturating_sub(new_completed)
        .max(0);
    let reviews_remaining = settings
        .daily_review_limit
        .saturating_sub(reviews_completed)
        .max(0);
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
        needs_more_practice,
        extra_available,
    })
}

pub fn planned_scheduled_cards_for_scope(
    connection: &Connection,
    scope: StudyScope<'_>,
    settings: &AppSettingsDto,
    time: &StudyTimeContext,
) -> Result<(Vec<PlannedCard>, WorkloadOverviewDto), CommandError> {
    let overview = workload_overview_for_scope(connection, scope, settings, time)?;
    let mut selected = load_ranked_cards(
        connection,
        scope,
        time,
        Some(false),
        true,
        overview.scheduled_reviews,
    )?;
    if overview.scheduled_new > 0 {
        selected.extend(load_ranked_cards(
            connection,
            scope,
            time,
            Some(true),
            true,
            overview.scheduled_new,
        )?);
    }
    Ok((selected, overview))
}

pub fn planned_extra_cards_for_scope(
    connection: &Connection,
    scope: StudyScope<'_>,
    time: &StudyTimeContext,
) -> Result<Vec<PlannedCard>, CommandError> {
    load_ranked_cards(connection, scope, time, None, false, MAX_PLANNED_CARDS)
}

fn load_ranked_cards(
    connection: &Connection,
    scope: StudyScope<'_>,
    time: &StudyTimeContext,
    new_only: Option<bool>,
    due_only: bool,
    limit: i64,
) -> Result<Vec<PlannedCard>, CommandError> {
    if limit <= 0 {
        return Ok(Vec::new());
    }
    let mut values = vec![SqlValue::Integer(time.now_utc_millis)];
    let mut conditions = scoped_studyable_conditions(scope, &mut values);
    if due_only {
        conditions.push("s.due_at <= ?1".to_string());
    }
    match new_only {
        Some(true) => conditions.push("s.state = 0".to_string()),
        Some(false) => conditions.push("s.state <> 0".to_string()),
        None => {}
    }
    let limit_index = push_value(&mut values, limit.min(MAX_PLANNED_CARDS));
    let sql = format!(
        "WITH {signals},
         eligible AS (
             SELECT
                 c.deck_id,
                 c.id AS card_id,
                 s.due_at,
                 {signal_value} AS needs_more_practice,
                 date(s.due_at / 1000, 'unixepoch', 'localtime') AS due_local_date
             FROM cards c
             JOIN notes n ON n.id = c.note_id
             JOIN decks d ON d.id = c.deck_id
             JOIN card_schedules s ON s.card_id = c.id
             {signal_join}
             WHERE {conditions}
         ),
         ranked AS (
             SELECT *,
                    ROW_NUMBER() OVER (
                        PARTITION BY deck_id
                        ORDER BY due_local_date ASC,
                                 needs_more_practice DESC,
                                 due_at ASC,
                                 card_id ASC
                    ) AS deck_rank
             FROM eligible
         )
         SELECT deck_id, card_id
         FROM ranked
         ORDER BY due_local_date ASC,
                  deck_rank ASC,
                  deck_id ASC,
                  needs_more_practice DESC,
                  due_at ASC,
                  card_id ASC
         LIMIT ?{limit_index}",
        signals = signal_projection::LEARNING_SIGNAL_CTES,
        signal_value = signal_projection::value_sql(),
        signal_join = signal_projection::join_sql(),
        conditions = conditions.join(" AND "),
    );
    let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
    let planned = statement
        .query_map(params_from_iter(values.iter()), |row| {
            Ok(PlannedCard {
                deck_id: row.get(0)?,
                card_id: row.get(1)?,
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    Ok(planned)
}

fn scoped_studyable_conditions(scope: StudyScope<'_>, values: &mut Vec<SqlValue>) -> Vec<String> {
    let mut conditions = vec![studyability::studyable_card_sql("?1")];
    append_in_filter(&mut conditions, values, "c.deck_id", scope.deck_ids);
    for tag_id in scope.tag_ids {
        let tag_index = push_value(values, tag_id.clone());
        conditions.push(format!(
            "EXISTS (
                 SELECT 1 FROM note_tags filter_nt
                 WHERE filter_nt.note_id = n.id AND filter_nt.tag_id = ?{tag_index}
             )"
        ));
    }
    conditions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workload_field_names_distinguish_ready_inventory_from_daily_selection() {
        let overview = WorkloadOverviewDto {
            new_limit: 20,
            review_limit: 200,
            new_completed: 0,
            reviews_completed: 0,
            new_remaining: 20,
            reviews_remaining: 200,
            due_reviews: 250,
            due_new: 30,
            scheduled_reviews: 200,
            scheduled_new: 0,
            omitted_due_reviews: 50,
            omitted_due_new: 30,
            needs_more_practice: 4,
            extra_available: 400,
        };
        assert!(overview.due_reviews > overview.scheduled_reviews);
        assert_eq!(overview.scheduled_new, 0);
    }
}
