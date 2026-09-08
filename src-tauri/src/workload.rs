//! Daily workload accounting and bounded scheduled-session planning.

use std::{
    collections::BTreeMap,
    hash::{Hash, Hasher},
};

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

#[derive(Clone, Copy, Debug)]
pub struct StudyScope<'a> {
    pub deck_ids: &'a [String],
    pub tag_ids: &'a [String],
}

#[derive(Clone, Debug)]
pub struct PlannedCard {
    pub deck_id: String,
    pub card_id: String,
    pub due_at: i64,
    pub state: i64,
    pub needs_more_practice: bool,
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
    selection_seed: &str,
) -> Result<(Vec<PlannedCard>, WorkloadOverviewDto), CommandError> {
    let overview = workload_overview_for_scope(connection, scope, settings, time)?;
    let mut selected = load_ranked_cards(
        connection,
        scope,
        time,
        Some(false),
        true,
        overview.scheduled_reviews,
        selection_seed,
    )?;
    if overview.scheduled_new > 0 {
        selected.extend(load_ranked_cards(
            connection,
            scope,
            time,
            Some(true),
            true,
            overview.scheduled_new,
            selection_seed,
        )?);
    }
    Ok((selected, overview))
}

pub fn planned_extra_cards_for_scope(
    connection: &Connection,
    scope: StudyScope<'_>,
    time: &StudyTimeContext,
    selection_seed: &str,
) -> Result<Vec<PlannedCard>, CommandError> {
    // Extra practice means the selected studyable scope, not an arbitrary
    // 10,000-card prefix. The durable practice layer below pages rich content
    // and snapshots in bounded batches, so large scopes do not require a giant
    // in-memory card payload.
    load_ranked_cards(
        connection,
        scope,
        time,
        None,
        false,
        i64::MAX,
        selection_seed,
    )
}

#[derive(Clone, Debug)]
struct RankedCandidate {
    deck_id: String,
    card_id: String,
    due_local_date: String,
    due_at: i64,
    state: i64,
    needs_more_practice: bool,
    is_new: bool,
    shuffle_key: u64,
    deck_rank: usize,
}

fn load_ranked_cards(
    connection: &Connection,
    scope: StudyScope<'_>,
    time: &StudyTimeContext,
    new_only: Option<bool>,
    due_only: bool,
    limit: i64,
    selection_seed: &str,
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
    let candidate_limit_index = push_value(&mut values, limit);
    let sql = format!(
        "WITH {signals}
         SELECT
             c.deck_id,
             c.id AS card_id,
             date(s.due_at / 1000, 'unixepoch', 'localtime') AS due_local_date,
             s.due_at,
             s.state,
             {signal_value} AS needs_more_practice,
             CASE WHEN s.state = 0 THEN 1 ELSE 0 END AS is_new
         FROM cards c
         JOIN notes n ON n.id = c.note_id
         JOIN decks d ON d.id = c.deck_id
         JOIN card_schedules s ON s.card_id = c.id
         {signal_join}
         WHERE {conditions}
         ORDER BY due_local_date ASC,
                  needs_more_practice DESC,
                  is_new ASC,
                  s.due_at ASC,
                  c.id ASC
         LIMIT ?{candidate_limit_index}",
        signals = signal_projection::LEARNING_SIGNAL_CTES,
        signal_value = signal_projection::value_sql(),
        signal_join = signal_projection::join_sql(),
        conditions = conditions.join(" AND "),
    );
    let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
    let candidates = statement
        .query_map(params_from_iter(values.iter()), |row| {
            Ok(RankedCandidate {
                deck_id: row.get(0)?,
                card_id: row.get(1)?,
                due_local_date: row.get(2)?,
                due_at: row.get(3)?,
                state: row.get(4)?,
                needs_more_practice: row.get::<_, i64>(5)? != 0,
                is_new: row.get::<_, i64>(6)? != 0,
                shuffle_key: 0,
                deck_rank: 0,
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    Ok(rank_candidates(candidates, selection_seed, limit))
}

fn rank_candidates(
    candidates: Vec<RankedCandidate>,
    selection_seed: &str,
    limit: i64,
) -> Vec<PlannedCard> {
    let mut by_deck = BTreeMap::<String, Vec<RankedCandidate>>::new();
    for mut candidate in candidates {
        candidate.shuffle_key = selection_shuffle_key(selection_seed, &candidate.card_id);
        by_deck
            .entry(candidate.deck_id.clone())
            .or_default()
            .push(candidate);
    }

    let mut ranked = Vec::new();
    for cards in by_deck.values_mut() {
        cards.sort_by_key(|card| {
            (
                card.due_local_date.clone(),
                !card.needs_more_practice,
                card.is_new,
                card.shuffle_key,
            )
        });
        for (deck_rank, card) in cards.iter_mut().enumerate() {
            card.deck_rank = deck_rank;
            ranked.push(card.clone());
        }
    }

    ranked.sort_by_key(|card| {
        (
            card.due_local_date.clone(),
            card.deck_rank,
            card.deck_id.clone(),
            !card.needs_more_practice,
            card.is_new,
            card.shuffle_key,
        )
    });
    let take = usize::try_from(limit.max(0)).unwrap_or(usize::MAX);
    ranked
        .into_iter()
        .take(take)
        .map(|card| PlannedCard {
            deck_id: card.deck_id,
            card_id: card.card_id,
            due_at: card.due_at,
            state: card.state,
            needs_more_practice: card.needs_more_practice,
        })
        .collect()
}

fn selection_shuffle_key(seed: &str, card_id: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    "practice-selection-v1".hash(&mut hasher);
    seed.hash(&mut hasher);
    card_id.hash(&mut hasher);
    hasher.finish()
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

    fn ranked_candidate(id: &str, date: &str, weak: bool, is_new: bool) -> RankedCandidate {
        RankedCandidate {
            deck_id: "deck".to_string(),
            card_id: id.to_string(),
            due_local_date: date.to_string(),
            due_at: 0,
            state: i64::from(!is_new),
            needs_more_practice: weak,
            is_new,
            shuffle_key: 0,
            deck_rank: 0,
        }
    }

    #[test]
    fn selection_seed_randomizes_equivalent_cards_without_overriding_due_date_priority() {
        let candidates = (0..12)
            .map(|index| ranked_candidate(&format!("card-{index:02}"), "2026-08-11", true, false))
            .collect::<Vec<_>>();
        let first = rank_candidates(candidates.clone(), "operation-a", 12);
        let replay = rank_candidates(candidates.clone(), "operation-a", 12);
        let next = rank_candidates(candidates, "operation-b", 12);
        assert_eq!(
            first.iter().map(|card| &card.card_id).collect::<Vec<_>>(),
            replay.iter().map(|card| &card.card_id).collect::<Vec<_>>()
        );
        assert_ne!(
            first.iter().map(|card| &card.card_id).collect::<Vec<_>>(),
            next.iter().map(|card| &card.card_id).collect::<Vec<_>>()
        );

        let priority = rank_candidates(
            vec![
                ranked_candidate("newer-weak", "2026-08-11", true, false),
                ranked_candidate("older-normal", "2026-08-10", false, false),
            ],
            "operation-c",
            2,
        );
        assert_eq!(priority[0].card_id, "older-normal");
    }

    #[test]
    fn extra_ranking_does_not_truncate_large_scopes_at_ten_thousand_cards() {
        let candidates = (0..10_050)
            .map(|index| {
                ranked_candidate(
                    &format!("card-{index:05}"),
                    "2026-08-11",
                    index % 7 == 0,
                    false,
                )
            })
            .collect::<Vec<_>>();
        let selected = rank_candidates(candidates, "large-extra-session", i64::MAX);
        assert_eq!(selected.len(), 10_050);
    }
}
