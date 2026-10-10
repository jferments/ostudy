//! What one finished practice session changed, for the session complete screen.
//!
//! The session's own counts come from its review events; today's totals and
//! the streak come from the study activity ledger, so they match the Progress
//! page's calendar.

use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;

use crate::{
    error::CommandError, learning_metrics::CardStrength, study_activity,
    study_calendar::StudyTimeContext, studyability::studyable_card_sql,
};

use super::{
    super::{scope::AnalyticsContext, AnalyticsQueryDto},
    activity::{self, StreakDto},
    forecast,
};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MissedTopicDto {
    pub tag_id: String,
    pub name: String,
    /// Missed cards carrying this tag.
    pub card_count: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudyTodayDto {
    pub local_date: String,
    pub review_count: i64,
    pub study_time_ms: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummaryDto {
    pub session_id: String,
    pub mode: String,
    pub rated_count: i64,
    pub remembered_count: i64,
    /// Active answering time over the session's reviews.
    pub study_time_ms: i64,
    /// Cards this session moved into Solid.
    pub now_solid_count: i64,
    /// Studyable cards forgotten at least once in this session, in the order
    /// they were first missed.
    pub missed_card_ids: Vec<String>,
    /// The tag the most missed cards share, other than the tags the session
    /// was filtered to.
    pub missed_topic: Option<MissedTopicDto>,
    /// Reviews still due today in the session's decks and tags. `None` for a
    /// session of chosen cards, which has no deck scope.
    pub due_today_count: Option<i64>,
    pub today: StudyTodayDto,
    pub current_streak: Option<StreakDto>,
    pub longest_streak: Option<StreakDto>,
    /// The most recent earlier streak at least as long as the current one;
    /// `None` when the current streak is the longest yet.
    pub last_as_long_streak: Option<StreakDto>,
}

struct SessionRow {
    mode: String,
    rated_count: i64,
    remembered_count: i64,
    deck_ids: Vec<String>,
    tag_ids: Vec<String>,
}

pub fn load(
    connection: &Connection,
    time: StudyTimeContext,
    session_id: &str,
) -> Result<SessionSummaryDto, CommandError> {
    let session = connection
        .query_row(
            "SELECT mode, rated_count, correct_count, selected_deck_ids_json, selected_tag_ids_json
             FROM practice_sessions WHERE id = ?1",
            [session_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            },
        )
        .optional()
        .map_err(CommandError::database)?
        .ok_or_else(|| {
            CommandError::new(
                "practice_session_not_found",
                "That practice session is no longer in your library.",
            )
        })
        .map(
            |(mode, rated_count, remembered_count, decks, tags)| SessionRow {
                mode,
                rated_count,
                remembered_count,
                deck_ids: serde_json::from_str(&decks).unwrap_or_default(),
                tag_ids: serde_json::from_str(&tags).unwrap_or_default(),
            },
        )?;

    let study_time_ms = connection
        .query_row(
            "SELECT COALESCE(SUM(response_time_ms), 0) FROM review_events WHERE session_id = ?1",
            [session_id],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    let now_solid_count = now_solid_count(connection, session_id)?;
    let missed_card_ids = missed_card_ids(connection, &time, session_id)?;
    let missed_topic = if missed_card_ids.is_empty() {
        None
    } else {
        missed_topic(connection, &time, session_id)?
    };

    let due_today_count = if session.mode == "custom" || session.deck_ids.is_empty() {
        None
    } else {
        let context = AnalyticsContext::from_query(
            AnalyticsQueryDto {
                range: "7d".to_string(),
                deck_ids: session.deck_ids.clone(),
                tag_ids: session.tag_ids.clone(),
                custom_start_local_date: None,
                custom_end_local_date: None,
                highlighted_session_id: None,
            },
            time.clone(),
        )?;
        forecast::load(connection, &context)?
            .days
            .first()
            .map(|day| day.due_count)
    };

    let day = study_activity::learner_days(
        connection,
        time.new_day_start_hour(),
        Some(time.local_date),
        Some(time.local_date),
    )?
    .remove(&time.local_date)
    .unwrap_or_default();
    let standing = activity::streak_standing(connection, &time)?;

    Ok(SessionSummaryDto {
        session_id: session_id.to_string(),
        mode: session.mode,
        rated_count: session.rated_count,
        remembered_count: session.remembered_count,
        study_time_ms,
        now_solid_count,
        missed_card_ids,
        missed_topic,
        due_today_count,
        today: StudyTodayDto {
            local_date: time.local_date.to_string(),
            review_count: day.review_count,
            study_time_ms: day.study_time_ms,
        },
        current_streak: standing.current,
        longest_streak: standing.longest,
        last_as_long_streak: standing.last_as_long,
    })
}

/// Cards that were not Solid before their first review in the session and
/// were Solid after their last.
fn now_solid_count(connection: &Connection, session_id: &str) -> Result<i64, CommandError> {
    let mut statement = connection
        .prepare(
            "WITH ordered AS (
                 SELECT card_id, state_before, stability_before, state_after, stability_after,
                        ROW_NUMBER() OVER (
                            PARTITION BY card_id ORDER BY reviewed_at, created_at, id
                        ) AS first_rank,
                        ROW_NUMBER() OVER (
                            PARTITION BY card_id
                            ORDER BY reviewed_at DESC, created_at DESC, id DESC
                        ) AS last_rank
                 FROM review_events WHERE session_id = ?1
             )
             SELECT f.state_before, f.stability_before, l.state_after, l.stability_after
             FROM ordered f
             JOIN ordered l ON l.card_id = f.card_id AND l.last_rank = 1
             WHERE f.first_rank = 1",
        )
        .map_err(CommandError::database)?;
    let cards = statement
        .query_map([session_id], |row| {
            Ok((
                CardStrength::of(row.get(0)?, row.get(1)?),
                CardStrength::of(row.get(2)?, row.get(3)?),
            ))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    Ok(cards
        .iter()
        .filter(|(before, after)| *before != CardStrength::Solid && *after == CardStrength::Solid)
        .count() as i64)
}

/// The session's forgotten reviews, joined to the cards that can still be
/// practiced.
fn missed_cards_sql(time_placeholder: &str) -> String {
    format!(
        "FROM review_events r
         JOIN cards c ON c.id = r.card_id
         JOIN notes n ON n.id = c.note_id
         JOIN decks d ON d.id = c.deck_id
         WHERE r.session_id = ?1 AND r.answer_correct = 0
           AND {studyable}",
        studyable = studyable_card_sql(time_placeholder),
    )
}

fn missed_card_ids(
    connection: &Connection,
    time: &StudyTimeContext,
    session_id: &str,
) -> Result<Vec<String>, CommandError> {
    let mut statement = connection
        .prepare(&format!(
            "SELECT c.id, MIN(r.reviewed_at) {from}
             GROUP BY c.id ORDER BY MIN(r.reviewed_at), c.id",
            from = missed_cards_sql("?2"),
        ))
        .map_err(CommandError::database)?;
    let ids = statement
        .query_map(rusqlite::params![session_id, time.now_utc_millis], |row| {
            row.get(0)
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    Ok(ids)
}

fn missed_topic(
    connection: &Connection,
    time: &StudyTimeContext,
    session_id: &str,
) -> Result<Option<MissedTopicDto>, CommandError> {
    connection
        .query_row(
            &format!(
                "SELECT t.id, t.name, COUNT(*)
                 FROM (SELECT DISTINCT c.id, c.note_id {from}) missed
                 JOIN note_tags nt ON nt.note_id = missed.note_id
                 JOIN tags t ON t.id = nt.tag_id
                 WHERE t.id NOT IN (
                     SELECT value FROM json_each(
                         (SELECT selected_tag_ids_json FROM practice_sessions WHERE id = ?1)
                     )
                 )
                 GROUP BY t.id, t.name
                 ORDER BY COUNT(*) DESC, t.name, t.id
                 LIMIT 1",
                from = missed_cards_sql("?2"),
            ),
            rusqlite::params![session_id, time.now_utc_millis],
            |row| {
                Ok(MissedTopicDto {
                    tag_id: row.get(0)?,
                    name: row.get(1)?,
                    card_count: row.get(2)?,
                })
            },
        )
        .optional()
        .map_err(CommandError::database)
}
