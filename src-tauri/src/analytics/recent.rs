use rusqlite::{params_from_iter, types::Value as SqlValue, Connection};

use crate::error::CommandError;

use super::{
    scope::{AnalyticsContext, HistoryPeriod},
    AnalyticsDeckOptionDto, AnalyticsDimensionOptionsDto, AnalyticsTagOptionDto, RecentReviewDto,
    RecentReviewTagDto, SessionAnalyticsDto,
};

const RECENT_REVIEW_LIMIT: i64 = 20;
const RECENT_SESSION_LIMIT: i64 = 12;

pub fn load_reviews(
    connection: &Connection,
    context: &AnalyticsContext,
) -> Result<Vec<RecentReviewDto>, CommandError> {
    let mut values = Vec::<SqlValue>::new();
    let conditions = context.historical_conditions(&mut values, HistoryPeriod::Current);
    let sql = format!(
        "WITH recent_reviews AS (
             SELECT
                 r.id,
                 d.name AS deck_name,
                 a.captured_deck_name,
                 r.card_id,
                 a.card_label,
                 a.question_excerpt,
                 r.confidence,
                 r.rating,
                 r.response_time_ms,
                 r.reviewed_at,
                 a.review_local_date,
                 r.due_after,
                 a.attribution_source,
                 r.created_at
             FROM review_events r
             JOIN review_event_attributions a ON a.review_event_id = r.id
             JOIN cards c ON c.id = r.card_id
             JOIN notes n ON n.id = c.note_id
             JOIN decks d ON d.id = c.deck_id
             WHERE {conditions}
             ORDER BY r.reviewed_at DESC, r.created_at DESC, r.id DESC
             LIMIT {RECENT_REVIEW_LIMIT}
         ),
         current_tag_ids AS (
             SELECT DISTINCT nt.tag_id
             FROM note_tags nt
             JOIN notes current_note ON current_note.id = nt.note_id
             JOIN decks current_deck ON current_deck.id = current_note.deck_id
             WHERE current_note.deleted_at IS NULL
               AND current_deck.deleted_at IS NULL
               AND current_deck.archived_at IS NULL
         )
         SELECT
             recent.id,
             recent.deck_name,
             recent.captured_deck_name,
             recent.card_id,
             recent.card_label,
             recent.question_excerpt,
             recent.confidence,
             recent.rating,
             recent.response_time_ms,
             recent.reviewed_at,
             recent.review_local_date,
             recent.due_after,
             recent.attribution_source,
             rta.tag_id,
             CASE
                 WHEN current_tag_ids.tag_id IS NOT NULL THEN tags.name
                 ELSE rta.tag_name
             END AS resolved_tag_name,
             CASE
                 WHEN rta.tag_id IS NULL THEN NULL
                 ELSE current_tag_ids.tag_id IS NOT NULL
             END AS is_current,
             recent.created_at
         FROM recent_reviews recent
         LEFT JOIN review_event_tag_attributions rta
           ON rta.review_event_id = recent.id
         LEFT JOIN current_tag_ids ON current_tag_ids.tag_id = rta.tag_id
         LEFT JOIN tags ON tags.id = current_tag_ids.tag_id
         ORDER BY recent.reviewed_at DESC, recent.created_at DESC, recent.id DESC,
                  resolved_tag_name COLLATE NOCASE ASC, rta.tag_id ASC",
        conditions = conditions.join(" AND "),
    );
    let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
    let flattened = statement
        .query_map(params_from_iter(values.iter()), |row| {
            let tag_id = row.get::<_, Option<String>>(13)?;
            let tag_name = row.get::<_, Option<String>>(14)?;
            let is_current = row.get::<_, Option<i64>>(15)?;
            let tag = match (tag_id, tag_name, is_current) {
                (None, None, None) => None,
                (Some(tag_id), Some(tag_name), Some(is_current)) => Some(RecentReviewTagDto {
                    tag_id,
                    tag_name,
                    is_current: is_current != 0,
                }),
                _ => {
                    return Err(rusqlite::Error::InvalidColumnType(
                        13,
                        "recent review tag tuple".to_string(),
                        rusqlite::types::Type::Null,
                    ));
                }
            };
            Ok((
                RecentReviewDto {
                    id: row.get(0)?,
                    deck_name: row.get(1)?,
                    captured_deck_name: row.get(2)?,
                    card_id: row.get(3)?,
                    card_label: row.get(4)?,
                    question_excerpt: row.get(5)?,
                    tags: Vec::new(),
                    outcome: row.get(6)?,
                    rating: row.get(7)?,
                    response_time_ms: row.get(8)?,
                    reviewed_at: row.get(9)?,
                    review_local_date: row.get(10)?,
                    due_after: row.get(11)?,
                    attribution_source: row.get(12)?,
                },
                tag,
            ))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;

    let mut reviews = Vec::<RecentReviewDto>::new();
    for (mut review, tag) in flattened {
        if let Some(previous) = reviews.last_mut() {
            if previous.id == review.id {
                if let Some(tag) = tag {
                    previous.tags.push(tag);
                }
                continue;
            }
        }
        if let Some(tag) = tag {
            review.tags.push(tag);
        }
        reviews.push(review);
    }
    Ok(reviews)
}

pub fn load_sessions(
    connection: &Connection,
    context: &AnalyticsContext,
) -> Result<Vec<SessionAnalyticsDto>, CommandError> {
    let mut values = Vec::<SqlValue>::new();
    let conditions = context.historical_conditions(&mut values, HistoryPeriod::Current);
    let sql = format!(
        "SELECT
             ps.id,
             ps.mode,
             json_group_array(DISTINCT d.name),
             ps.started_at,
             ps.completed_at,
             ps.card_count,
             ps.rated_count,
             ps.correct_count,
             COALESCE(SUM(r.response_time_ms), 0),
             MAX(r.reviewed_at)
         FROM practice_sessions ps
         JOIN review_events r ON r.session_id = ps.id
         JOIN review_event_attributions a ON a.review_event_id = r.id
         JOIN cards c ON c.id = r.card_id
         JOIN notes n ON n.id = c.note_id
         JOIN decks d ON d.id = c.deck_id
         WHERE {conditions}
         GROUP BY ps.id, ps.mode, ps.started_at, ps.completed_at,
                  ps.card_count, ps.rated_count, ps.correct_count
         ORDER BY MAX(r.reviewed_at) DESC, ps.started_at DESC, ps.id DESC
         LIMIT {RECENT_SESSION_LIMIT}",
        conditions = conditions.join(" AND "),
    );
    let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
    let sessions = statement
        .query_map(params_from_iter(values.iter()), |row| {
            let id: String = row.get(0)?;
            let deck_names_json: String = row.get(2)?;
            let deck_names =
                serde_json::from_str::<Vec<String>>(&deck_names_json).unwrap_or_default();
            let started_at: i64 = row.get(3)?;
            let completed_at: Option<i64> = row.get(4)?;
            let rated_count: i64 = row.get(6)?;
            let correct_count: i64 = row.get(7)?;
            let last_reviewed_at: i64 = row.get(9)?;
            Ok(SessionAnalyticsDto {
                highlighted: context.highlighted_session_id.as_deref() == Some(id.as_str()),
                id,
                mode: row.get(1)?,
                deck_names,
                started_at,
                completed_at,
                duration_ms: completed_at
                    .unwrap_or(last_reviewed_at)
                    .saturating_sub(started_at)
                    .max(0),
                card_count: row.get(5)?,
                rated_count,
                correct_count,
                correct_rate: super::history::rate(correct_count, rated_count),
                study_time_ms: row.get(8)?,
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    Ok(sessions)
}

pub fn load_dimension_options(
    connection: &Connection,
) -> Result<AnalyticsDimensionOptionsDto, CommandError> {
    let decks = {
        let mut statement = connection
            .prepare(
                "SELECT
                     d.id,
                     d.name,
                     d.archived_at,
                     EXISTS(
                         SELECT 1 FROM cards c
                         JOIN notes n ON n.id = c.note_id
                         WHERE c.deck_id = d.id
                           AND c.deleted_at IS NULL AND n.deleted_at IS NULL
                     ),
                     EXISTS(
                         SELECT 1 FROM review_events r
                         JOIN cards c ON c.id = r.card_id
                         JOIN notes n ON n.id = c.note_id
                         WHERE c.deck_id = d.id
                           AND c.deleted_at IS NULL AND n.deleted_at IS NULL
                     )
                 FROM decks d
                 WHERE d.deleted_at IS NULL
                 ORDER BY CASE WHEN d.archived_at IS NULL THEN 0 ELSE 1 END,
                          d.name COLLATE NOCASE, d.id",
            )
            .map_err(CommandError::database)?;
        let deck_options = statement
            .query_map([], |row| {
                Ok(AnalyticsDeckOptionDto {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    is_archived: row.get::<_, Option<i64>>(2)?.is_some(),
                    has_current_cards: row.get::<_, i64>(3)? != 0,
                    has_history: row.get::<_, i64>(4)? != 0,
                })
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        deck_options
    };
    let tags = {
        let mut statement = connection
            .prepare(
                "SELECT
                     t.id,
                     t.name,
                     EXISTS(
                         SELECT 1 FROM note_tags nt
                         JOIN notes n ON n.id = nt.note_id
                         JOIN decks d ON d.id = n.deck_id
                         WHERE nt.tag_id = t.id
                           AND n.deleted_at IS NULL
                           AND d.deleted_at IS NULL
                           AND d.archived_at IS NULL
                     ),
                     EXISTS(
                         SELECT 1 FROM review_event_tag_attributions rta
                         JOIN review_events r ON r.id = rta.review_event_id
                         JOIN cards c ON c.id = r.card_id
                         JOIN notes n ON n.id = c.note_id
                         JOIN decks d ON d.id = c.deck_id
                         WHERE rta.tag_id = t.id
                           AND c.deleted_at IS NULL
                           AND n.deleted_at IS NULL
                           AND d.deleted_at IS NULL
                     )
                 FROM tags t
                 WHERE EXISTS(SELECT 1 FROM note_tags nt WHERE nt.tag_id = t.id)
                    OR EXISTS(
                         SELECT 1 FROM review_event_tag_attributions rta
                         WHERE rta.tag_id = t.id
                    )
                 ORDER BY t.name COLLATE NOCASE, t.id",
            )
            .map_err(CommandError::database)?;
        let tag_options = statement
            .query_map([], |row| {
                Ok(AnalyticsTagOptionDto {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    is_current: row.get::<_, i64>(2)? != 0,
                    has_history: row.get::<_, i64>(3)? != 0,
                })
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        tag_options
    };
    Ok(AnalyticsDimensionOptionsDto { decks, tags })
}
