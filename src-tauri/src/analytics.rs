//! Deterministic review analytics derived from SQLite.
//!
//! Phase 2 exposes the same single-step labels used during practice: Wrong,
//! Hard, OK, and Easy. Reviews reversed by Undo remain in the append-only
//! ledger but are excluded from active analytics.

use rusqlite::Connection;
use serde::Serialize;
use tauri::State;

use crate::{database::Database, error::CommandError, util::unix_time_millis};

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
    pub struggling_count: i64,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RatingDistributionDto {
    pub wrong: i64,
    pub hard: i64,
    pub ok: i64,
    pub easy: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentDeckPracticeDto {
    pub deck_id: String,
    pub deck_name: String,
    pub session_id: String,
    pub practiced_at: i64,
    pub review_count: i64,
    pub rating_distribution: RatingDistributionDto,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentReviewDto {
    pub id: String,
    pub deck_name: String,
    pub card_id: String,
    pub front_text: String,
    pub score: i64,
    pub label: String,
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
    pub struggling_count: i64,
    pub suspended_count: i64,
    pub rating_distribution: RatingDistributionDto,
    pub deck_stats: Vec<DeckAnalyticsDto>,
    pub recent_deck_practice: Vec<RecentDeckPracticeDto>,
    pub recent_reviews: Vec<RecentReviewDto>,
}

#[tauri::command]
pub fn get_analytics(database: State<'_, Database>) -> Result<AnalyticsDto, CommandError> {
    database.with_connection(load_analytics)
}

fn load_analytics(connection: &mut Connection) -> Result<AnalyticsDto, CommandError> {
    let now = unix_time_millis()?;
    let deck_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM decks WHERE deleted_at IS NULL AND archived_at IS NULL",
            [],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    let card_count = active_card_count(connection, None)?;
    let due_now = due_card_count(connection, None, now)?;
    let struggling_count: i64 = connection
        .query_row(
            "SELECT COUNT(DISTINCT c.note_id) FROM cards c\n\
             JOIN notes n ON n.id = c.note_id\n\
             JOIN decks d ON d.id = c.deck_id\n\
             WHERE c.deleted_at IS NULL AND c.struggling_since IS NOT NULL\n\
               AND n.deleted_at IS NULL AND d.deleted_at IS NULL AND d.archived_at IS NULL",
            [],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    let suspended_count: i64 = connection
        .query_row(
            "SELECT COUNT(DISTINCT c.note_id) FROM cards c\n\
             JOIN notes n ON n.id = c.note_id\n\
             JOIN decks d ON d.id = c.deck_id\n\
             WHERE c.deleted_at IS NULL AND c.suspended_at IS NOT NULL\n\
               AND n.deleted_at IS NULL AND d.deleted_at IS NULL AND d.archived_at IS NULL",
            [],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;

    let (review_count, correct_count, average_response_time_ms): (i64, i64, Option<f64>) =
        connection
            .query_row(
                "SELECT COUNT(*),\n\
                        COALESCE(SUM(r.answer_correct), 0),\n\
                        AVG(r.response_time_ms)\n\
                 FROM review_events r\n\
                 LEFT JOIN review_event_reversals rr ON rr.review_event_id = r.id\n\
                 WHERE rr.review_event_id IS NULL",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .map_err(CommandError::database)?;
    let correct_rate = rate(correct_count, review_count);

    let mut rating_distribution = RatingDistributionDto::default();
    let mut rating_statement = connection
        .prepare(
            "SELECT r.rating, COUNT(*) FROM review_events r\n\
             LEFT JOIN review_event_reversals rr ON rr.review_event_id = r.id\n\
             WHERE rr.review_event_id IS NULL\n\
             GROUP BY r.rating ORDER BY r.rating",
        )
        .map_err(CommandError::database)?;
    let rating_rows = rating_statement
        .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)))
        .map_err(CommandError::database)?;
    for row in rating_rows {
        let (rating, count) = row.map_err(CommandError::database)?;
        match rating {
            1 => rating_distribution.wrong = count,
            2 => rating_distribution.hard = count,
            3 => rating_distribution.ok = count,
            4 => rating_distribution.easy = count,
            _ => {}
        }
    }

    let mut deck_statement = connection
        .prepare(
            "SELECT d.id, d.name,\n\
                    (SELECT COUNT(*) FROM cards c JOIN notes n ON n.id = c.note_id\n\
                     WHERE c.deck_id = d.id AND c.deleted_at IS NULL AND n.deleted_at IS NULL),\n\
                    (SELECT COUNT(*) FROM cards c JOIN notes n ON n.id = c.note_id\n\
                     JOIN card_schedules s ON s.card_id = c.id\n\
                     WHERE c.deck_id = d.id AND c.deleted_at IS NULL\n\
                       AND c.suspended_at IS NULL\n\
                       AND (c.buried_until IS NULL OR c.buried_until <= ?1)\n\
                       AND n.deleted_at IS NULL AND s.due_at <= ?1),\n\
                    (SELECT COUNT(*) FROM review_events r\n\
                     JOIN cards c ON c.id = r.card_id\n\
                     LEFT JOIN review_event_reversals rr ON rr.review_event_id = r.id\n\
                     WHERE c.deck_id = d.id AND rr.review_event_id IS NULL),\n\
                    (SELECT COALESCE(SUM(r.answer_correct), 0) FROM review_events r\n\
                     JOIN cards c ON c.id = r.card_id\n\
                     LEFT JOIN review_event_reversals rr ON rr.review_event_id = r.id\n\
                     WHERE c.deck_id = d.id AND rr.review_event_id IS NULL),\n\
                    (SELECT AVG(r.response_time_ms) FROM review_events r\n\
                     JOIN cards c ON c.id = r.card_id\n\
                     LEFT JOIN review_event_reversals rr ON rr.review_event_id = r.id\n\
                     WHERE c.deck_id = d.id AND rr.review_event_id IS NULL),\n\
                    (SELECT COUNT(DISTINCT c.note_id) FROM cards c\n\
                     JOIN notes n ON n.id = c.note_id\n\
                     WHERE c.deck_id = d.id AND c.deleted_at IS NULL\n\
                       AND c.struggling_since IS NOT NULL AND n.deleted_at IS NULL)\n\
             FROM decks d\n\
             WHERE d.deleted_at IS NULL AND d.archived_at IS NULL\n\
             ORDER BY 4 DESC, 5 DESC, d.name COLLATE NOCASE ASC",
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
                struggling_count: row.get(7)?,
            })
        })
        .map_err(CommandError::database)?;
    let deck_stats = deck_rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;

    let mut recent_statement = connection
        .prepare(
            "SELECT r.id, d.name, r.card_id, n.front_text, r.rating - 1,\n\
                    r.response_time_ms, r.reviewed_at, r.due_after\n\
             FROM review_events r\n\
             JOIN cards c ON c.id = r.card_id\n\
             JOIN notes n ON n.id = c.note_id\n\
             JOIN decks d ON d.id = c.deck_id\n\
             LEFT JOIN review_event_reversals rr ON rr.review_event_id = r.id\n\
             WHERE rr.review_event_id IS NULL\n\
             ORDER BY r.reviewed_at DESC, r.created_at DESC\n\
             LIMIT 20",
        )
        .map_err(CommandError::database)?;
    let recent_reviews = recent_statement
        .query_map([], |row| {
            let score: i64 = row.get(4)?;
            Ok(RecentReviewDto {
                id: row.get(0)?,
                deck_name: row.get(1)?,
                card_id: row.get(2)?,
                front_text: row.get(3)?,
                score,
                label: score_label(score).to_string(),
                response_time_ms: row.get(5)?,
                reviewed_at: row.get(6)?,
                due_after: row.get(7)?,
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;

    let mut recent_deck_statement = connection
        .prepare(
            r#"WITH session_decks AS (
                 SELECT c.deck_id, r.session_id, MAX(r.reviewed_at) AS practiced_at
                 FROM review_events r
                 JOIN cards c ON c.id = r.card_id
                 LEFT JOIN review_event_reversals rr ON rr.review_event_id = r.id
                 WHERE r.session_id IS NOT NULL AND rr.review_event_id IS NULL
                 GROUP BY c.deck_id, r.session_id
             ), ranked AS (
                 SELECT deck_id, session_id, practiced_at,
                        ROW_NUMBER() OVER (
                            PARTITION BY deck_id
                            ORDER BY practiced_at DESC, session_id DESC
                        ) AS deck_rank
                 FROM session_decks
             )
             SELECT d.id, d.name, ranked.session_id, ranked.practiced_at,
                    COUNT(r.id),
                    COALESCE(SUM(CASE WHEN r.rating = 1 THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN r.rating = 2 THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN r.rating = 3 THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN r.rating = 4 THEN 1 ELSE 0 END), 0)
             FROM ranked
             JOIN decks d ON d.id = ranked.deck_id
             JOIN cards c ON c.deck_id = ranked.deck_id
             JOIN review_events r
               ON r.card_id = c.id AND r.session_id = ranked.session_id
             LEFT JOIN review_event_reversals rr ON rr.review_event_id = r.id
             WHERE ranked.deck_rank = 1
               AND d.deleted_at IS NULL AND d.archived_at IS NULL
               AND rr.review_event_id IS NULL
             GROUP BY d.id, d.name, ranked.session_id, ranked.practiced_at
             ORDER BY ranked.practiced_at DESC, d.name COLLATE NOCASE ASC
             LIMIT 5"#,
        )
        .map_err(CommandError::database)?;
    let recent_deck_practice = recent_deck_statement
        .query_map([], |row| {
            Ok(RecentDeckPracticeDto {
                deck_id: row.get(0)?,
                deck_name: row.get(1)?,
                session_id: row.get(2)?,
                practiced_at: row.get(3)?,
                review_count: row.get(4)?,
                rating_distribution: RatingDistributionDto {
                    wrong: row.get(5)?,
                    hard: row.get(6)?,
                    ok: row.get(7)?,
                    easy: row.get(8)?,
                },
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;

    Ok(AnalyticsDto {
        deck_count,
        card_count,
        due_now,
        review_count,
        correct_count,
        correct_rate,
        average_response_time_ms,
        struggling_count,
        suspended_count,
        rating_distribution,
        deck_stats,
        recent_deck_practice,
        recent_reviews,
    })
}

fn active_card_count(connection: &Connection, deck_id: Option<&str>) -> Result<i64, CommandError> {
    let base = "SELECT COUNT(*) FROM cards c\n\
                JOIN notes n ON n.id = c.note_id\n\
                JOIN decks d ON d.id = c.deck_id\n\
                WHERE c.deleted_at IS NULL AND n.deleted_at IS NULL\n\
                  AND d.deleted_at IS NULL AND d.archived_at IS NULL";
    match deck_id {
        Some(deck_id) => connection
            .query_row(&format!("{base} AND c.deck_id = ?1"), [deck_id], |row| row.get(0))
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
    let base = "SELECT COUNT(*) FROM cards c\n\
                JOIN notes n ON n.id = c.note_id\n\
                JOIN decks d ON d.id = c.deck_id\n\
                JOIN card_schedules s ON s.card_id = c.id\n\
                WHERE c.deleted_at IS NULL AND c.suspended_at IS NULL\n\
                  AND (c.buried_until IS NULL OR c.buried_until <= ?1)\n\
                  AND n.deleted_at IS NULL AND d.deleted_at IS NULL\n\
                  AND d.archived_at IS NULL AND s.due_at <= ?1";
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

const fn score_label(score: i64) -> &'static str {
    match score {
        0 => "Wrong",
        1 => "Hard",
        2 => "OK",
        3 => "Easy",
        _ => "Unknown",
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
        assert_eq!(analytics.struggling_count, 0);
        assert!(analytics.recent_deck_practice.is_empty());
    }

    #[test]
    fn recent_deck_practice_uses_the_latest_active_session() {
        let database = Database::open_in_memory().expect("database should open");
        database
            .with_connection(|connection| {
                let deck_id = "dddddddddddddddddddddddddddddddd";
                let note_id = "nnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnn";
                let card_id = "cccccccccccccccccccccccccccccccc";
                let older_session_id = "11111111111111111111111111111111";
                let latest_session_id = "22222222222222222222222222222222";
                let document = r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Question"}]}]}"#;

                connection
                    .execute(
                        "INSERT INTO decks (id, name, description, created_at, updated_at, sort_order, revision) VALUES (?1, 'Dog facts', '', 1, 1, 0, 1)",
                        [deck_id],
                    )
                    .map_err(CommandError::database)?;
                connection
                    .execute(
                        "INSERT INTO notes (id, deck_id, note_type, content_format, content_schema_version, front_doc, back_doc, front_text, back_text, created_at, updated_at, revision) VALUES (?1, ?2, 'basic', 'tiptap-json', 1, ?3, ?3, 'Question', 'Answer', 1, 1, 1)",
                        rusqlite::params![note_id, deck_id, document],
                    )
                    .map_err(CommandError::database)?;
                connection
                    .execute(
                        "INSERT INTO cards (id, note_id, deck_id, direction, template_ordinal, created_at, updated_at, display_name) VALUES (?1, ?2, ?3, 'forward', 0, 1, 1, 'Forward')",
                        rusqlite::params![card_id, note_id, deck_id],
                    )
                    .map_err(CommandError::database)?;
                connection
                    .execute(
                        "INSERT INTO card_schedules (card_id, state, due_at, stability, difficulty, elapsed_days, scheduled_days, reps, lapses, last_review_at, scheduler_name, scheduler_version, updated_at) VALUES (?1, 0, 1, 0.0, 0.0, 0, 0, 0, 0, 1, 'fsrs', 'rs-fsrs-1.2.1', 1)",
                        [card_id],
                    )
                    .map_err(CommandError::database)?;

                for (session_id, reviewed_at, rating, correct) in [
                    (older_session_id, 100_i64, 1_i64, 0_i64),
                    (latest_session_id, 200_i64, 4_i64, 1_i64),
                ] {
                    connection
                        .execute(
                            "INSERT INTO practice_sessions (id, selected_deck_ids_json, shuffle, card_count, started_at, rated_count, correct_count) VALUES (?1, '[]', 0, 1, ?2, 1, ?3)",
                            rusqlite::params![session_id, reviewed_at - 10, correct],
                        )
                        .map_err(CommandError::database)?;
                    connection
                        .execute(
                            "INSERT INTO review_events (id, submission_id, session_id, card_id, reviewed_at, rating, answer_correct, confidence, response_time_ms, state_before, state_after, due_before, due_after, stability_before, stability_after, difficulty_before, difficulty_after, scheduled_days_before, scheduled_days_after, scheduler_name, scheduler_version, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1000, 0, 1, 1, 1000, 0.0, 1.0, 0.0, 5.0, 0, 1, 'fsrs', 'rs-fsrs-1.2.1', ?5)",
                            rusqlite::params![
                                if session_id == older_session_id { "rrrrrrrrrrrrrrrrrrrrrrrrrrrrrrr1" } else { "rrrrrrrrrrrrrrrrrrrrrrrrrrrrrrr2" },
                                if session_id == older_session_id { "uuuuuuuuuuuuuuuuuuuuuuuuuuuuuuu1" } else { "uuuuuuuuuuuuuuuuuuuuuuuuuuuuuuu2" },
                                session_id,
                                card_id,
                                reviewed_at,
                                rating,
                                correct,
                                if rating == 1 { "wrong" } else { "very_confident" },
                            ],
                        )
                        .map_err(CommandError::database)?;
                }
                Ok(())
            })
            .expect("analytics fixture should load");

        let analytics = database
            .with_connection(load_analytics)
            .expect("analytics should load");
        assert_eq!(analytics.recent_deck_practice.len(), 1);
        let recent = &analytics.recent_deck_practice[0];
        assert_eq!(recent.deck_name, "Dog facts");
        assert_eq!(recent.session_id, "22222222222222222222222222222222");
        assert_eq!(recent.review_count, 1);
        assert_eq!(recent.rating_distribution.wrong, 0);
        assert_eq!(recent.rating_distribution.easy, 1);
    }

    #[test]
    fn direct_rating_labels_match_practice() {
        assert_eq!(score_label(0), "Wrong");
        assert_eq!(score_label(2), "OK");
        assert_eq!(rate(3, 4), Some(0.75));
    }
}
