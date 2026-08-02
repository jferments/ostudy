//! Deterministic review analytics derived from SQLite.
//!
//! These calculations deliberately remain ordinary code rather than AI output.
//! A later local model can interpret the aggregates, but the database remains
//! the authoritative source for due counts, correctness, timing, and ratings.

use rusqlite::Connection;
use serde::Serialize;
use tauri::State;

use crate::{
    cards::get_card_text_from_connection, database::Database, error::CommandError,
    util::unix_time_millis,
};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeckAnalyticsDto {
    pub deck_id: String,
    pub deck_name: String,
    pub card_count: i64,
    pub due_count: i64,
    pub review_count: i64,
    pub correct_count: i64,
    pub correct_rate: Option<f64>,
    pub average_response_time_ms: Option<f64>,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RatingDistributionDto {
    pub again: i64,
    pub hard: i64,
    pub good: i64,
    pub easy: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentReviewDto {
    pub id: String,
    pub deck_name: String,
    pub card_id: String,
    pub front_text: String,
    pub outcome: String,
    pub rating: i64,
    pub response_time_ms: i64,
    pub reviewed_at: i64,
    pub due_after: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsDto {
    pub deck_count: i64,
    pub card_count: i64,
    pub due_now: i64,
    pub review_count: i64,
    pub correct_count: i64,
    pub correct_rate: Option<f64>,
    pub average_response_time_ms: Option<f64>,
    pub rating_distribution: RatingDistributionDto,
    pub deck_stats: Vec<DeckAnalyticsDto>,
    pub recent_reviews: Vec<RecentReviewDto>,
}

#[tauri::command]
pub fn get_analytics(database: State<'_, Database>) -> Result<AnalyticsDto, CommandError> {
    database.with_connection(load_analytics)
}

fn load_analytics(connection: &mut Connection) -> Result<AnalyticsDto, CommandError> {
    let now = unix_time_millis()?;
    let deck_count = connection
        .query_row(
            "SELECT COUNT(*) FROM decks WHERE deleted_at IS NULL AND archived_at IS NULL",
            [],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    let card_count = active_card_count(connection, None)?;
    let due_now = due_card_count(connection, None, now)?;

    let (review_count, correct_count, average_response_time_ms): (i64, i64, Option<f64>) =
        connection
            .query_row(
                "SELECT COUNT(*),\n\
                        COALESCE(SUM(answer_correct), 0),\n\
                        AVG(response_time_ms)\n\
                 FROM review_events",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .map_err(CommandError::database)?;
    let correct_rate = rate(correct_count, review_count);

    let mut rating_distribution = RatingDistributionDto::default();
    let mut rating_statement = connection
        .prepare(
            "SELECT rating, COUNT(*) FROM review_events\n\
             GROUP BY rating ORDER BY rating",
        )
        .map_err(CommandError::database)?;
    let rating_rows = rating_statement
        .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)))
        .map_err(CommandError::database)?;
    for row in rating_rows {
        let (rating, count) = row.map_err(CommandError::database)?;
        match rating {
            1 => rating_distribution.again = count,
            2 => rating_distribution.hard = count,
            3 => rating_distribution.good = count,
            4 => rating_distribution.easy = count,
            _ => {}
        }
    }

    let mut deck_statement = connection
        .prepare(
            "SELECT d.id, d.name,\n\
                    (SELECT COUNT(*)\n\
                     FROM cards c\n\
                     JOIN notes n ON n.id = c.note_id\n\
                     WHERE c.deck_id = d.id\n\
                       AND c.deleted_at IS NULL\n\
                       AND c.suspended_at IS NULL\n\
                       AND n.deleted_at IS NULL) AS card_count,\n\
                    (SELECT COUNT(*)\n\
                     FROM cards c\n\
                     JOIN notes n ON n.id = c.note_id\n\
                     JOIN card_schedules s ON s.card_id = c.id\n\
                     WHERE c.deck_id = d.id\n\
                       AND c.deleted_at IS NULL\n\
                       AND c.suspended_at IS NULL\n\
                       AND n.deleted_at IS NULL\n\
                       AND s.due_at <= ?1) AS due_count,\n\
                    (SELECT COUNT(*)\n\
                     FROM review_events r\n\
                     JOIN cards c ON c.id = r.card_id\n\
                     WHERE c.deck_id = d.id) AS review_count,\n\
                    (SELECT COALESCE(SUM(r.answer_correct), 0)\n\
                     FROM review_events r\n\
                     JOIN cards c ON c.id = r.card_id\n\
                     WHERE c.deck_id = d.id) AS correct_count,\n\
                    (SELECT AVG(r.response_time_ms)\n\
                     FROM review_events r\n\
                     JOIN cards c ON c.id = r.card_id\n\
                     WHERE c.deck_id = d.id) AS average_response_time_ms\n\
             FROM decks d\n\
             WHERE d.deleted_at IS NULL AND d.archived_at IS NULL\n\
             ORDER BY due_count DESC, review_count DESC, d.name COLLATE NOCASE ASC",
        )
        .map_err(CommandError::database)?;
    let deck_rows = deck_statement
        .query_map([now], |row| {
            let review_count: i64 = row.get(4)?;
            let correct_count: i64 = row.get(5)?;
            Ok(DeckAnalyticsDto {
                deck_id: row.get(0)?,
                deck_name: row.get(1)?,
                card_count: row.get(2)?,
                due_count: row.get(3)?,
                review_count,
                correct_count,
                correct_rate: rate(correct_count, review_count),
                average_response_time_ms: row.get(6)?,
            })
        })
        .map_err(CommandError::database)?;
    let deck_stats = deck_rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;

    let recent_raw = {
        let mut recent_statement = connection
            .prepare(
                "SELECT r.id, d.name, r.card_id, r.confidence, r.rating,
                        r.response_time_ms, r.reviewed_at, r.due_after
                 FROM review_events r
                 JOIN cards c ON c.id = r.card_id
                 JOIN decks d ON d.id = c.deck_id
                 ORDER BY r.reviewed_at DESC
                 LIMIT 20",
            )
            .map_err(CommandError::database)?;
        recent_statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, i64>(7)?,
                ))
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?
    };
    let mut recent_reviews = Vec::with_capacity(recent_raw.len());
    for (id, deck_name, card_id, outcome, rating, response_time_ms, reviewed_at, due_after) in
        recent_raw
    {
        let front_text = get_card_text_from_connection(connection, &card_id)?.front_text;
        recent_reviews.push(RecentReviewDto {
            id,
            deck_name,
            card_id,
            front_text,
            outcome,
            rating,
            response_time_ms,
            reviewed_at,
            due_after,
        });
    }

    Ok(AnalyticsDto {
        deck_count,
        card_count,
        due_now,
        review_count,
        correct_count,
        correct_rate,
        average_response_time_ms,
        rating_distribution,
        deck_stats,
        recent_reviews,
    })
}

fn active_card_count(connection: &Connection, deck_id: Option<&str>) -> Result<i64, CommandError> {
    let base = "SELECT COUNT(*)\n\
                FROM cards c\n\
                JOIN notes n ON n.id = c.note_id\n\
                JOIN decks d ON d.id = c.deck_id\n\
                WHERE c.deleted_at IS NULL\n\
                  AND c.suspended_at IS NULL\n\
                  AND n.deleted_at IS NULL\n\
                  AND d.deleted_at IS NULL\n\
                  AND d.archived_at IS NULL";
    match deck_id {
        Some(deck_id) => connection
            .query_row(&format!("{base} AND c.deck_id = ?1"), [deck_id], |row| {
                row.get(0)
            })
            .map_err(CommandError::database),
        None => connection
            .query_row(base, [], |row| row.get(0))
            .map_err(CommandError::database),
    }
}

fn due_card_count(
    connection: &Connection,
    deck_id: Option<&str>,
    now: i64,
) -> Result<i64, CommandError> {
    let base = "SELECT COUNT(*)\n\
                FROM cards c\n\
                JOIN notes n ON n.id = c.note_id\n\
                JOIN decks d ON d.id = c.deck_id\n\
                JOIN card_schedules s ON s.card_id = c.id\n\
                WHERE c.deleted_at IS NULL\n\
                  AND c.suspended_at IS NULL\n\
                  AND n.deleted_at IS NULL\n\
                  AND d.deleted_at IS NULL\n\
                  AND d.archived_at IS NULL\n\
                  AND s.due_at <= ?1";
    match deck_id {
        Some(deck_id) => connection
            .query_row(
                &format!("{base} AND c.deck_id = ?2"),
                rusqlite::params![now, deck_id],
                |row| row.get(0),
            )
            .map_err(CommandError::database),
        None => connection
            .query_row(base, [now], |row| row.get(0))
            .map_err(CommandError::database),
    }
}

fn rate(numerator: i64, denominator: i64) -> Option<f64> {
    if denominator == 0 {
        None
    } else {
        Some(numerator as f64 / denominator as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_library_has_zeroed_analytics() {
        let database = Database::open_in_memory().expect("database should open");
        let analytics = database
            .with_connection(load_analytics)
            .expect("analytics should load");
        assert_eq!(analytics.deck_count, 0);
        assert_eq!(analytics.card_count, 0);
        assert_eq!(analytics.review_count, 0);
        assert_eq!(analytics.correct_rate, None);
    }

    #[test]
    fn rate_is_none_without_reviews() {
        assert_eq!(rate(0, 0), None);
        assert_eq!(rate(3, 4), Some(0.75));
    }
}
