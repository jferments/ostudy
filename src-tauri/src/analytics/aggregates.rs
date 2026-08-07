use rusqlite::{params_from_iter, types::Value as SqlValue, Connection};

use crate::{
    error::CommandError,
    projections::{learning_signals as signal_projection, scope::push_value},
    studyability,
};

use super::{
    history::rate,
    scope::{AnalyticsContext, HistoryPeriod},
    DeckAnalyticsDto, TagAnalyticsDto,
};

const MIN_CLASSIFICATION_REVIEWS: i64 = 5;

pub fn load_decks(
    connection: &Connection,
    context: &AnalyticsContext,
) -> Result<Vec<DeckAnalyticsDto>, CommandError> {
    let mut values = vec![SqlValue::Integer(context.time.now_utc_millis)];
    let current_conditions = context.current_card_conditions(&mut values);
    let history_conditions = context.historical_conditions(&mut values, HistoryPeriod::Current);
    let sql = format!(
        "WITH {signals},
         current_stats AS (
             SELECT
                 c.deck_id,
                 COUNT(DISTINCT n.id) AS source_note_count,
                 COUNT(DISTINCT c.id) AS generated_card_count,
                 COALESCE(SUM(CASE WHEN {studyable} THEN 1 ELSE 0 END), 0)
                     AS studyable_card_count,
                 COALESCE(SUM(CASE WHEN {studyable_due} THEN 1 ELSE 0 END), 0)
                     AS studyable_due_count,
                 COALESCE(SUM(CASE WHEN {signal_value} = 1 THEN 1 ELSE 0 END), 0)
                     AS needs_more_practice_count
             FROM cards c
             JOIN notes n ON n.id = c.note_id
             JOIN decks d ON d.id = c.deck_id
             JOIN card_schedules s ON s.card_id = c.id
             {signal_join}
             WHERE {current_conditions}
             GROUP BY c.deck_id
         ),
         history_stats AS (
             SELECT
                 c.deck_id,
                 COUNT(DISTINCT r.card_id) AS cards_practiced,
                 COUNT(*) AS review_count,
                 COALESCE(SUM(r.answer_correct), 0) AS correct_count,
                 AVG(CAST(r.rating - 1 AS REAL)) AS average_confidence,
                 AVG(CAST(r.response_time_ms AS REAL)) AS average_response_time_ms,
                 COALESCE(SUM(r.response_time_ms), 0) AS total_study_time_ms,
                 MAX(r.reviewed_at) AS last_practiced_at
             FROM review_events r
             JOIN review_event_attributions a ON a.review_event_id = r.id
             JOIN cards c ON c.id = r.card_id
             JOIN notes n ON n.id = c.note_id
             JOIN decks d ON d.id = c.deck_id
             WHERE {history_conditions}
             GROUP BY c.deck_id
         )
         SELECT
             d.id,
             d.name,
             d.archived_at,
             COALESCE(cs.source_note_count, 0),
             COALESCE(cs.generated_card_count, 0),
             COALESCE(cs.studyable_card_count, 0),
             COALESCE(cs.studyable_due_count, 0),
             COALESCE(cs.needs_more_practice_count, 0),
             COALESCE(hs.cards_practiced, 0),
             COALESCE(hs.review_count, 0),
             COALESCE(hs.correct_count, 0),
             hs.average_confidence,
             hs.average_response_time_ms,
             COALESCE(hs.total_study_time_ms, 0),
             hs.last_practiced_at
         FROM decks d
         LEFT JOIN current_stats cs ON cs.deck_id = d.id
         LEFT JOIN history_stats hs ON hs.deck_id = d.id
         WHERE d.deleted_at IS NULL
           AND (cs.deck_id IS NOT NULL OR hs.deck_id IS NOT NULL)
         ORDER BY
             COALESCE(cs.needs_more_practice_count, 0) DESC,
             COALESCE(cs.studyable_due_count, 0) DESC,
             COALESCE(hs.review_count, 0) DESC,
             d.name COLLATE NOCASE,
             d.id",
        signals = signal_projection::LEARNING_SIGNAL_CTES,
        studyable = studyability::studyable_card_sql("?1"),
        studyable_due = studyability::studyable_due_sql("?1"),
        signal_value = signal_projection::value_sql(),
        signal_join = signal_projection::join_sql(),
        current_conditions = current_conditions.join(" AND "),
        history_conditions = history_conditions.join(" AND "),
    );
    let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
    let decks = statement
        .query_map(params_from_iter(values.iter()), |row| {
            let review_count: i64 = row.get(9)?;
            let correct_count: i64 = row.get(10)?;
            Ok(DeckAnalyticsDto {
                deck_id: row.get(0)?,
                deck_name: row.get(1)?,
                is_archived: row.get::<_, Option<i64>>(2)?.is_some(),
                source_note_count: row.get(3)?,
                generated_card_count: row.get(4)?,
                studyable_card_count: row.get(5)?,
                studyable_due_count: row.get(6)?,
                needs_more_practice_count: row.get(7)?,
                cards_practiced: row.get(8)?,
                review_count,
                correct_count,
                correct_rate: rate(correct_count, review_count),
                average_confidence: row.get(11)?,
                average_response_time_ms: row.get(12)?,
                total_study_time_ms: row.get(13)?,
                last_practiced_at: row.get(14)?,
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    Ok(decks)
}

pub fn load_tags(
    connection: &Connection,
    context: &AnalyticsContext,
    desired_retention: f64,
) -> Result<Vec<TagAnalyticsDto>, CommandError> {
    let mut values = vec![SqlValue::Integer(context.time.now_utc_millis)];
    let current_conditions = context.current_card_conditions(&mut values);
    let history_conditions =
        context.historical_conditions(&mut values, HistoryPeriod::CurrentAndPrevious);
    let current_predicate = if let Some(window) = context.window.current.as_ref() {
        let index = push_value(&mut values, window.start_local_date.to_string());
        format!("a.review_local_date >= ?{index}")
    } else {
        "1 = 1".to_string()
    };
    let previous_predicate = if let Some(window) = context.previous_window() {
        let start = push_value(&mut values, window.start_local_date.to_string());
        let end = push_value(&mut values, window.end_local_date.to_string());
        format!("a.review_local_date >= ?{start} AND a.review_local_date <= ?{end}")
    } else {
        "0 = 1".to_string()
    };

    let sql = format!(
        "WITH {signals},
         current_scoped_cards AS (
             SELECT
                 c.id AS card_id,
                 n.id AS note_id,
                 CASE WHEN {studyable} THEN 1 ELSE 0 END AS is_studyable,
                 CASE WHEN {studyable_due} THEN 1 ELSE 0 END AS is_studyable_due,
                 {signal_value} AS needs_more_practice
             FROM cards c
             JOIN notes n ON n.id = c.note_id
             JOIN decks d ON d.id = c.deck_id
             JOIN card_schedules s ON s.card_id = c.id
             {signal_join}
             WHERE {current_conditions}
         ),
         current_tag_stats AS (
             SELECT
                 nt.tag_id,
                 COUNT(DISTINCT scoped.note_id) AS source_note_count,
                 COUNT(DISTINCT scoped.card_id) AS generated_card_count,
                 COALESCE(SUM(scoped.is_studyable), 0) AS studyable_card_count,
                 COALESCE(SUM(scoped.is_studyable_due), 0) AS studyable_due_count,
                 COALESCE(SUM(scoped.needs_more_practice), 0)
                     AS needs_more_practice_count
             FROM current_scoped_cards scoped
             JOIN note_tags nt ON nt.note_id = scoped.note_id
             GROUP BY nt.tag_id
         ),
         historical_tag_stats AS (
             SELECT
                 rta.tag_id,
                 MAX(rta.tag_name) AS captured_name,
                 COUNT(DISTINCT CASE WHEN {current_predicate} THEN r.card_id END)
                     AS cards_practiced,
                 COALESCE(SUM(CASE WHEN {current_predicate} THEN 1 ELSE 0 END), 0)
                     AS review_count,
                 COALESCE(SUM(CASE WHEN {current_predicate} THEN r.answer_correct ELSE 0 END), 0)
                     AS correct_count,
                 AVG(CASE WHEN {current_predicate} THEN CAST(r.rating - 1 AS REAL) END)
                     AS average_confidence,
                 AVG(CASE WHEN {current_predicate} THEN CAST(r.response_time_ms AS REAL) END)
                     AS average_response_time_ms,
                 COALESCE(SUM(CASE WHEN {current_predicate} THEN r.response_time_ms ELSE 0 END), 0)
                     AS total_study_time_ms,
                 MAX(CASE WHEN {current_predicate} THEN r.reviewed_at END)
                     AS last_practiced_at,
                 COALESCE(SUM(CASE WHEN {previous_predicate} THEN 1 ELSE 0 END), 0)
                     AS previous_review_count,
                 COALESCE(SUM(CASE
                     WHEN {previous_predicate} THEN r.answer_correct ELSE 0
                 END), 0) AS previous_correct_count
             FROM review_events r
             JOIN review_event_attributions a ON a.review_event_id = r.id
             JOIN cards c ON c.id = r.card_id
             JOIN notes n ON n.id = c.note_id
             JOIN decks d ON d.id = c.deck_id
             JOIN review_event_tag_attributions rta ON rta.review_event_id = r.id
             WHERE {history_conditions}
             GROUP BY rta.tag_id
         ),
         keys AS (
             SELECT tag_id FROM current_tag_stats
             UNION
             SELECT tag_id FROM historical_tag_stats
         )
         SELECT
             keys.tag_id,
             COALESCE(t.name, hts.captured_name, 'Tag'),
             CASE WHEN cts.tag_id IS NULL THEN 0 ELSE 1 END,
             COALESCE(cts.source_note_count, 0),
             COALESCE(cts.generated_card_count, 0),
             COALESCE(cts.studyable_card_count, 0),
             COALESCE(cts.studyable_due_count, 0),
             COALESCE(cts.needs_more_practice_count, 0),
             COALESCE(hts.cards_practiced, 0),
             COALESCE(hts.review_count, 0),
             COALESCE(hts.correct_count, 0),
             hts.average_confidence,
             hts.average_response_time_ms,
             COALESCE(hts.total_study_time_ms, 0),
             hts.last_practiced_at,
             COALESCE(hts.previous_review_count, 0),
             COALESCE(hts.previous_correct_count, 0)
         FROM keys
         LEFT JOIN tags t ON t.id = keys.tag_id
         LEFT JOIN current_tag_stats cts ON cts.tag_id = keys.tag_id
         LEFT JOIN historical_tag_stats hts ON hts.tag_id = keys.tag_id",
        signals = signal_projection::LEARNING_SIGNAL_CTES,
        studyable = studyability::studyable_card_sql("?1"),
        studyable_due = studyability::studyable_due_sql("?1"),
        signal_value = signal_projection::value_sql(),
        signal_join = signal_projection::join_sql(),
        current_conditions = current_conditions.join(" AND "),
        current_predicate = current_predicate,
        previous_predicate = previous_predicate,
        history_conditions = history_conditions.join(" AND "),
    );
    let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
    let mut rows = statement
        .query_map(params_from_iter(values.iter()), |row| {
            let review_count: i64 = row.get(9)?;
            let correct_count: i64 = row.get(10)?;
            let previous_review_count: i64 = row.get(15)?;
            let previous_correct_count: i64 = row.get(16)?;
            let correct_rate = rate(correct_count, review_count);
            let previous_rate = rate(previous_correct_count, previous_review_count);
            let trend_delta = if review_count >= MIN_CLASSIFICATION_REVIEWS
                && previous_review_count >= MIN_CLASSIFICATION_REVIEWS
            {
                correct_rate
                    .zip(previous_rate)
                    .map(|(current, previous)| current - previous)
            } else {
                None
            };
            let needs_more_practice_count: i64 = row.get(7)?;
            Ok(TagAnalyticsDto {
                tag_id: row.get(0)?,
                tag_name: row.get(1)?,
                is_current: row.get::<_, i64>(2)? != 0,
                source_note_count: row.get(3)?,
                generated_card_count: row.get(4)?,
                studyable_card_count: row.get(5)?,
                studyable_due_count: row.get(6)?,
                needs_more_practice_count,
                cards_practiced: row.get(8)?,
                review_count,
                correct_count,
                correct_rate,
                average_confidence: row.get(11)?,
                average_response_time_ms: row.get(12)?,
                total_study_time_ms: row.get(13)?,
                last_practiced_at: row.get(14)?,
                trend_delta,
                status: classify_tag(
                    review_count,
                    correct_rate,
                    needs_more_practice_count,
                    desired_retention,
                ),
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    rows.sort_by(|left, right| {
        tag_status_rank(&left.status)
            .cmp(&tag_status_rank(&right.status))
            .then_with(|| {
                right
                    .needs_more_practice_count
                    .cmp(&left.needs_more_practice_count)
            })
            .then_with(|| right.studyable_due_count.cmp(&left.studyable_due_count))
            .then_with(|| {
                left.correct_rate
                    .unwrap_or(1.0)
                    .total_cmp(&right.correct_rate.unwrap_or(1.0))
            })
            .then_with(|| {
                left.tag_name
                    .to_lowercase()
                    .cmp(&right.tag_name.to_lowercase())
            })
    });
    Ok(rows)
}

fn classify_tag(
    review_count: i64,
    correct_rate: Option<f64>,
    needs_more_practice: i64,
    desired_retention: f64,
) -> String {
    if review_count < MIN_CLASSIFICATION_REVIEWS {
        return "building_data".to_string();
    }
    if needs_more_practice > 0
        || correct_rate.is_some_and(|value| value <= desired_retention - 0.10)
    {
        return "needs_attention".to_string();
    }
    if correct_rate.is_some_and(|value| value >= desired_retention + 0.03) {
        return "strong".to_string();
    }
    "on_track".to_string()
}

fn tag_status_rank(value: &str) -> i64 {
    match value {
        "needs_attention" => 0,
        "building_data" => 1,
        "on_track" => 2,
        "strong" => 3,
        _ => 4,
    }
}
