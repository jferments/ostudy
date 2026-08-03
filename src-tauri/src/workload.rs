//! Daily workload accounting and authoritative scheduled-session planning.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{Local, TimeZone};
use rusqlite::{params_from_iter, types::Value as SqlValue, Connection};
use serde::Serialize;

use crate::{
    error::CommandError,
    learning_signals::{count_needing_more_practice, needs_more_practice},
    settings::AppSettingsDto,
    util::unix_time_millis,
};

const DAY_MS: i64 = 86_400_000;

#[derive(Clone, Debug)]
pub struct PlannedCard {
    pub deck_id: String,
    pub card_id: String,
    pub note_id: String,
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
    pub due_reviews: i64,
    pub due_new: i64,
    pub scheduled_reviews: i64,
    pub scheduled_new: i64,
    pub omitted_due_reviews: i64,
    pub omitted_due_new: i64,
    pub needs_more_practice: i64,
    pub extra_available: i64,
}

pub fn local_day_start_millis() -> Result<i64, CommandError> {
    let now = Local::now();
    let midnight = now
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| CommandError::new("invalid_local_day", "The local study day is invalid."))?;
    Local
        .from_local_datetime(&midnight)
        .earliest()
        .map(|value| value.timestamp_millis())
        .ok_or_else(|| {
            CommandError::new(
                "invalid_local_day",
                "The local study day could not be resolved for this time zone.",
            )
        })
}

pub fn next_local_day_start_millis() -> Result<i64, CommandError> {
    let now = Local::now();
    let tomorrow = now
        .date_naive()
        .succ_opt()
        .and_then(|date| date.and_hms_opt(0, 0, 0))
        .ok_or_else(|| {
            CommandError::new(
                "invalid_local_day",
                "Tomorrow is outside the supported date range.",
            )
        })?;
    Local
        .from_local_datetime(&tomorrow)
        .earliest()
        .map(|value| value.timestamp_millis())
        .ok_or_else(|| {
            CommandError::new(
                "invalid_local_day",
                "Tomorrow could not be resolved for this time zone.",
            )
        })
}

pub fn workload_overview(
    connection: &Connection,
    deck_ids: &[String],
    settings: &AppSettingsDto,
) -> Result<WorkloadOverviewDto, CommandError> {
    let now = unix_time_millis()?;
    let day_start = local_day_start_millis()?;
    let (new_completed, reviews_completed): (i64, i64) = connection
        .query_row(
            "SELECT
                 COALESCE(SUM(CASE WHEN state_before = 0 THEN 1 ELSE 0 END), 0),
                 COALESCE(SUM(CASE WHEN state_before <> 0 THEN 1 ELSE 0 END), 0)
             FROM review_events
             WHERE reviewed_at >= ?1",
            [day_start],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(CommandError::database)?;
    let new_remaining = (settings.daily_new_limit - new_completed).max(0);
    let reviews_remaining = (settings.daily_review_limit - reviews_completed).max(0);
    let candidates = load_candidates(connection, deck_ids, now, true)?;
    let due_reviews = candidates.iter().filter(|card| card.state != 0).count() as i64;
    let due_new = candidates.iter().filter(|card| card.state == 0).count() as i64;
    let scheduled_reviews = due_reviews.min(reviews_remaining);
    let scheduled_new = if due_reviews > reviews_remaining {
        0
    } else {
        due_new.min(new_remaining)
    };
    let extra_available = count_extra_available(connection, deck_ids, now)?;
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
        needs_more_practice: count_needing_more_practice(connection, deck_ids)?,
        extra_available,
    })
}

pub fn planned_scheduled_cards(
    connection: &Connection,
    deck_ids: &[String],
    settings: &AppSettingsDto,
) -> Result<(Vec<PlannedCard>, WorkloadOverviewDto), CommandError> {
    let now = unix_time_millis()?;
    let overview = workload_overview(connection, deck_ids, settings)?;
    let mut candidates = load_candidates(connection, deck_ids, now, true)?;
    candidates.sort_by_key(|card| {
        (
            card.due_at.div_euclid(DAY_MS),
            !card.needs_more_practice,
            card.due_at,
            card.card_id.clone(),
        )
    });

    let reviews = candidates
        .iter()
        .filter(|card| card.state != 0)
        .take(usize::try_from(overview.scheduled_reviews).unwrap_or(0))
        .cloned()
        .collect::<Vec<_>>();
    let mut selected = fair_interleave(reviews);
    if overview.scheduled_new > 0 {
        let new_cards = candidates
            .into_iter()
            .filter(|card| card.state == 0)
            .take(usize::try_from(overview.scheduled_new).unwrap_or(0))
            .collect::<Vec<_>>();
        selected.extend(fair_interleave(new_cards));
    }
    Ok((selected, overview))
}

pub fn planned_extra_cards(
    connection: &Connection,
    deck_ids: &[String],
) -> Result<Vec<PlannedCard>, CommandError> {
    let now = unix_time_millis()?;
    let mut cards = load_candidates(connection, deck_ids, now, false)?;
    cards.sort_by_key(|card| {
        (
            card.due_at > now,
            card.due_at.div_euclid(DAY_MS),
            !card.needs_more_practice,
            card.due_at,
            card.card_id.clone(),
        )
    });
    Ok(fair_interleave(cards))
}

fn load_candidates(
    connection: &Connection,
    deck_ids: &[String],
    now: i64,
    due_only: bool,
) -> Result<Vec<PlannedCard>, CommandError> {
    let mut values: Vec<SqlValue> = Vec::new();
    let mut conditions = vec![
        "c.deleted_at IS NULL".to_string(),
        "c.suspended_at IS NULL".to_string(),
        "(c.buried_until IS NULL OR c.buried_until <= ?1)".to_string(),
        "n.deleted_at IS NULL".to_string(),
        "d.deleted_at IS NULL".to_string(),
        "d.archived_at IS NULL".to_string(),
    ];
    values.push(now.into());
    if due_only {
        values.push(now.into());
        conditions.push("s.due_at <= ?2".to_string());
    }
    if !deck_ids.is_empty() {
        let placeholders = deck_ids
            .iter()
            .map(|deck_id| {
                values.push(deck_id.clone().into());
                format!("?{}", values.len())
            })
            .collect::<Vec<_>>()
            .join(", ");
        conditions.push(format!("c.deck_id IN ({placeholders})"));
    }
    let sql = format!(
        "SELECT c.deck_id, c.id, c.note_id, s.due_at, s.state
         FROM cards c
         JOIN notes n ON n.id = c.note_id
         JOIN decks d ON d.id = c.deck_id
         JOIN card_schedules s ON s.card_id = c.id
         WHERE {}",
        conditions.join(" AND ")
    );
    let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
    let rows = statement
        .query_map(params_from_iter(values.iter()), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
            ))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    rows.into_iter()
        .map(|(deck_id, card_id, note_id, due_at, state)| {
            Ok(PlannedCard {
                deck_id,
                needs_more_practice: needs_more_practice(connection, &card_id)?,
                card_id,
                note_id,
                due_at,
                state,
            })
        })
        .collect()
}

fn count_extra_available(
    connection: &Connection,
    deck_ids: &[String],
    now: i64,
) -> Result<i64, CommandError> {
    Ok(i64::try_from(load_candidates(connection, deck_ids, now, false)?.len()).unwrap_or(i64::MAX))
}

fn fair_interleave(cards: Vec<PlannedCard>) -> Vec<PlannedCard> {
    let mut by_deck: BTreeMap<String, Vec<PlannedCard>> = BTreeMap::new();
    for card in cards {
        by_deck.entry(card.deck_id.clone()).or_default().push(card);
    }
    let deck_ids = by_deck.keys().cloned().collect::<BTreeSet<_>>();
    let mut result = Vec::new();
    loop {
        let mut added = false;
        for deck_id in &deck_ids {
            if let Some(cards) = by_deck.get_mut(deck_id) {
                if !cards.is_empty() {
                    result.push(cards.remove(0));
                    added = true;
                }
            }
        }
        if !added {
            break;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fair_interleave_prevents_one_deck_from_monopolizing() {
        let make = |deck: &str, card: &str| PlannedCard {
            deck_id: deck.to_string(),
            card_id: card.to_string(),
            note_id: card.to_string(),
            due_at: 0,
            state: 2,
            needs_more_practice: false,
        };
        let result = fair_interleave(vec![make("a", "1"), make("a", "2"), make("b", "3")]);
        assert_eq!(result[0].deck_id, "a");
        assert_eq!(result[1].deck_id, "b");
        assert_eq!(result[2].card_id, "2");
    }
}
