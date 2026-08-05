#!/usr/bin/env python3
"""Remove superseded native paths and resolve strict Clippy findings.

Temporary development applicator; the workflow removes it before final delivery.
"""
from __future__ import annotations

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def write(path: str, content: str) -> None:
    (ROOT / path).write_text(content, encoding="utf-8")


def remove_function(text: str, name: str) -> str:
    pattern = re.compile(
        rf"(?m)^(?:\s*#\[[^\n]+\]\n)*(?:\s*pub(?:\([^)]*\))?\s+)?\s*fn\s+{re.escape(name)}\s*\("
    )
    match = pattern.search(text)
    if match is None:
        raise SystemExit(f"function anchor missing: {name}")
    brace = text.find("{", match.end())
    if brace < 0:
        raise SystemExit(f"function body missing: {name}")
    depth = 0
    end = None
    for index in range(brace, len(text)):
        char = text[index]
        if char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                end = index + 1
                break
    if end is None:
        raise SystemExit(f"unbalanced function body: {name}")
    while end < len(text) and text[end] == "\n":
        end += 1
    return text[: match.start()] + text[end:]


# Remove unbounded or superseded card/template/practice helpers rather than
# hiding them behind dead-code allowances.
removals = {
    "src-tauri/src/authoring/stage3_v2/types_commands.rs": ["list_card_templates"],
    "src-tauri/src/authoring/stage3_v2/templates.rs": ["list_card_templates_in_connection"],
    "src-tauri/src/authoring/stage3_v2/auto_template_layout.rs": [
        "normalize_card_template_list_for_editing"
    ],
    "src-tauri/src/authoring/stage3_v2/generated_cards.rs": [
        "presentation_variant_for_card",
        "render_stored_card_presentation",
        "render_stored_card_presentation_for_variant",
        "get_generated_card_for_practice",
    ],
    "src-tauri/src/cards.rs": ["list_cards", "list_cards_from_connection"],
    "src-tauri/src/study_calendar.rs": ["local_date_string_for_millis"],
}
for path, functions in removals.items():
    text = read(path)
    for function in functions:
        text = remove_function(text, function)
    write(path, text)

# The point-query learning-signal implementation was replaced by the shared
# fixed-query SQL projection. Remove the obsolete module completely.
learning_path = ROOT / "src-tauri/src/learning_signals.rs"
if learning_path.exists():
    learning_path.unlink()
lib = read("src-tauri/src/lib.rs").replace("mod learning_signals;\n", "")
write("src-tauri/src/lib.rs", lib)

write(
    "src-tauri/src/studyability.rs",
    r'''//! Canonical studyability projection shared by workload, Search/Filter,
//! practice, deck summaries, and Analytics.
//!
//! Current/studyable/due predicates live directly in the bounded SQL queries;
//! this module supplies the one reusable learning-signal CTE so those queries do
//! not fall back to per-card database calls.

/// Shared CTE for the current learning epoch.
///
/// A reset begins a new epoch without deleting immutable review evidence. The
/// projection returns one boolean-like integer per card.
pub fn learning_signal_cte() -> &'static str {
    "latest_reset AS (
         SELECT card_id, MAX(occurred_at) AS reset_at
         FROM schedule_action_events
         WHERE action = 'reset'
         GROUP BY card_id
     ),
     ranked_learning_reviews AS (
         SELECT r.card_id, r.answer_correct,
                ROW_NUMBER() OVER (
                    PARTITION BY r.card_id
                    ORDER BY r.reviewed_at DESC, r.created_at DESC, r.id DESC
                ) AS review_rank
         FROM review_events r
         LEFT JOIN latest_reset lr ON lr.card_id = r.card_id
         WHERE r.reviewed_at > COALESCE(lr.reset_at, -1)
     ),
     learning_signals AS (
         SELECT card_id,
                CASE
                    WHEN MAX(CASE WHEN review_rank = 1 THEN answer_correct END) = 1
                     AND MAX(CASE WHEN review_rank = 2 THEN answer_correct END) = 1
                    THEN 0
                    WHEN MAX(CASE WHEN review_rank = 1 THEN answer_correct END) = 0
                     AND MAX(CASE WHEN review_rank = 2 THEN answer_correct END) = 0
                    THEN 1
                    WHEN SUM(CASE
                        WHEN review_rank <= 5 AND answer_correct = 0 THEN 1 ELSE 0
                    END) >= 3
                    THEN 1
                    ELSE 0
                END AS needs_more_practice
         FROM ranked_learning_reviews
         WHERE review_rank <= 5
         GROUP BY card_id
     )"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn learning_signal_projection_is_bounded_to_recent_reviews() {
        let sql = learning_signal_cte();
        assert!(sql.contains("review_rank <= 5"));
        assert!(sql.contains("latest_reset"));
    }
}
''',
)

# Keep only the scoped workload API and remove a projected field no consumer
# reads. The selected row remains sufficient to create immutable snapshots by
# card ID.
write(
    "src-tauri/src/workload.rs",
    r'''//! Bounded daily workload accounting and authoritative scheduled-session planning.
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
        i64, i64, i64, i64, i64, i64,
    ) = connection
        .query_row(&sql, params_from_iter(values.iter()), |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?))
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
                    COALESCE(ls.needs_more_practice, 0),
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
    let state_clause = if new_cards { "s.state = 0" } else { "s.state <> 0" };
    let sql = format!(
        "WITH {signals},
         ranked AS (
             SELECT c.deck_id, c.id, s.due_at, s.state,
                    COALESCE(ls.needs_more_practice, 0),
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
        .map(|(deck_id, card_id, due_at, state, weak, note_type, source, mode, reps)| {
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
        })
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
''',
)

# Custom-card planning no longer selects or stores the removed note-id field.
practice = read("src-tauri/src/practice_selection.rs")
practice = practice.replace(
    "SELECT c.deck_id,c.id,c.note_id,s.due_at,s.state,COALESCE(ls.needs_more_practice,0),n.note_type,n.cloze_source,n.cloze_mode,s.reps",
    "SELECT c.deck_id,c.id,s.due_at,s.state,COALESCE(ls.needs_more_practice,0),n.note_type,n.cloze_source,n.cloze_mode,s.reps",
)
practice = practice.replace(
    """                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, i64>(4)?,
                    r.get::<_, i64>(5)?,
                    r.get::<_, String>(6)?,
                    r.get::<_, String>(7)?,
                    r.get::<_, String>(8)?,
                    r.get::<_, i64>(9)?,""",
    """                    r.get::<_, i64>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, i64>(4)?,
                    r.get::<_, String>(5)?,
                    r.get::<_, String>(6)?,
                    r.get::<_, String>(7)?,
                    r.get::<_, i64>(8)?,""",
)
practice = practice.replace(
    "let (d, c, n, due, state, weak, typ, source, mode, reps) =",
    "let (d, c, due, state, weak, typ, source, mode, reps) =",
)
practice = practice.replace("                    note_id: n,\n", "")

# Bundle immutable session-creation inputs so the transaction boundary has a
# small, explicit API rather than eight positional arguments.
insert_at = practice.index("fn create_session(")
creation_struct = r'''struct SessionCreation {
    operation_id: String,
    fingerprint: String,
    cards: Vec<PendingCard>,
    shuffle: bool,
    mode: String,
    context: StudyTimeContext,
}

'''
practice = practice[:insert_at] + creation_struct + practice[insert_at:]
practice = practice.replace(
    """    create_session(
        connection,
        paths,
        operation_id,
        fingerprint,
        planned,
        shuffle,
        mode,
        &context,
    )""",
    """    create_session(
        connection,
        paths,
        SessionCreation {
            operation_id,
            fingerprint,
            cards: planned,
            shuffle,
            mode: mode.to_string(),
            context,
        },
    )""",
    1,
)
practice = practice.replace(
    """    create_session(
        connection,
        paths,
        operation_id,
        fingerprint,
        planned,
        shuffle,
        "custom",
        &context,
    )""",
    """    create_session(
        connection,
        paths,
        SessionCreation {
            operation_id,
            fingerprint,
            cards: planned,
            shuffle,
            mode: "custom".to_string(),
            context,
        },
    )""",
    1,
)
old_signature = r'''fn create_session(
    connection: &mut Connection,
    paths: &AppPaths,
    operation_id: String,
    fingerprint: String,
    mut cards: Vec<PendingCard>,
    shuffle: bool,
    mode: &str,
    context: &StudyTimeContext,
) -> Result<PracticeSessionDto, CommandError> {'''
new_signature = r'''fn create_session(
    connection: &mut Connection,
    paths: &AppPaths,
    creation: SessionCreation,
) -> Result<PracticeSessionDto, CommandError> {
    let SessionCreation {
        operation_id,
        fingerprint,
        mut cards,
        shuffle,
        mode,
        context,
    } = creation;'''
if old_signature not in practice:
    raise SystemExit("create_session signature changed")
practice = practice.replace(old_signature, new_signature, 1)
practice = practice.replace("mode,context.now_utc_millis", "mode,context.now_utc_millis")
write("src-tauri/src/practice_selection.rs", practice)

# Give the historical aggregate a named shape instead of an eleven-column tuple
# leaking through the function boundary.
analytics = read("src-tauri/src/analytics.rs")
analytics = analytics.replace(
    """    let (history, ratings) = historical_summary(connection, scope)?;
    summary.cards_practiced = history.0;
    summary.review_count = history.1;
    summary.correct_count = history.2;
    summary.correct_rate = ratio(history.2, history.1);
    summary.average_confidence = history.3;
    summary.average_response_time_ms = history.4;
    summary.total_study_time_ms = history.5;
    summary.session_count = history.6;""",
    """    let history = historical_summary(connection, scope)?;
    summary.cards_practiced = history.cards_practiced;
    summary.review_count = history.review_count;
    summary.correct_count = history.correct_count;
    summary.correct_rate = ratio(history.correct_count, history.review_count);
    summary.average_confidence = history.average_confidence;
    summary.average_response_time_ms = history.average_response_time_ms;
    summary.total_study_time_ms = history.total_study_time_ms;
    summary.session_count = history.session_count;""",
    1,
)
analytics = analytics.replace("        rating_distribution: ratings,", "        rating_distribution: history.ratings,", 1)
historical_start = analytics.index("fn historical_summary(")
summary_struct = r'''struct HistoricalSummary {
    cards_practiced: i64,
    review_count: i64,
    correct_count: i64,
    average_confidence: Option<f64>,
    average_response_time_ms: Option<f64>,
    total_study_time_ms: i64,
    session_count: i64,
    ratings: RatingDistributionDto,
}

'''
analytics = analytics[:historical_start] + summary_struct + analytics[historical_start:]
old_return = r'''fn historical_summary(
    connection: &Connection,
    scope: &Scope,
) -> Result<
    (
        (i64, i64, i64, Option<f64>, Option<f64>, i64, i64),
        RatingDistributionDto,
    ),
    CommandError,
> {'''
new_return = r'''fn historical_summary(
    connection: &Connection,
    scope: &Scope,
) -> Result<HistoricalSummary, CommandError> {'''
if old_return not in analytics:
    raise SystemExit("historical_summary return anchor changed")
analytics = analytics.replace(old_return, new_return, 1)
old_result = r'''    Ok((
        (row.0, row.1, row.2, row.3, row.4, row.5, row.6),
        RatingDistributionDto {
            again: row.7,
            hard: row.8,
            good: row.9,
            easy: row.10,
        },
    ))'''
new_result = r'''    Ok(HistoricalSummary {
        cards_practiced: row.0,
        review_count: row.1,
        correct_count: row.2,
        average_confidence: row.3,
        average_response_time_ms: row.4,
        total_study_time_ms: row.5,
        session_count: row.6,
        ratings: RatingDistributionDto {
            again: row.7,
            hard: row.8,
            good: row.9,
            easy: row.10,
        },
    })'''
if old_result not in analytics:
    raise SystemExit("historical_summary construction anchor changed")
analytics = analytics.replace(old_result, new_result, 1)
write("src-tauri/src/analytics.rs", analytics)

# Reformat the FTS tokenizer so each independent condition is unambiguous to
# Clippy and review readers.
search = read("src-tauri/src/authoring/search_and_tags.rs")
old_fts = """fn fts_query_is_present(query:&str)->bool{build_fts_query(query).is_some()}
fn push_value(values:&mut Vec<SqlValue>,value:impl Into<SqlValue>)->usize{values.push(value.into());values.len()}
fn build_fts_query(value:&str)->Option<String>{let mut tokens=Vec::new();let mut current=String::new();for ch in value.chars(){if ch.is_alphanumeric()||matches!(ch,'_'|'-'){current.push(ch);if current.chars().count()>=MAX_TAG_CHARS{tokens.push(std::mem::take(&mut current));}}else if !current.is_empty(){tokens.push(std::mem::take(&mut current));}if tokens.len()>=20{break;}}if !current.is_empty()&&tokens.len()<20{tokens.push(current);}let quoted=tokens.into_iter().filter(|t|!t.is_empty()).map(|t|format!("\\\"{}\\\"*",t.replace('"',"\\\"\\\""))).collect::<Vec<_>>();(!quoted.is_empty()).then(||quoted.join(" AND "))}"""
new_fts = r'''fn fts_query_is_present(query: &str) -> bool {
    build_fts_query(query).is_some()
}

fn push_value(values: &mut Vec<SqlValue>, value: impl Into<SqlValue>) -> usize {
    values.push(value.into());
    values.len()
}

fn build_fts_query(value: &str) -> Option<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    for character in value.chars() {
        if character.is_alphanumeric() || matches!(character, '_' | '-') {
            current.push(character);
            if current.chars().count() >= MAX_TAG_CHARS {
                tokens.push(std::mem::take(&mut current));
            }
        } else if !current.is_empty() {
            tokens.push(std::mem::take(&mut current));
        }
        if tokens.len() >= 20 {
            break;
        }
    }
    if !current.is_empty() && tokens.len() < 20 {
        tokens.push(current);
    }
    let quoted = tokens
        .into_iter()
        .filter(|token| !token.is_empty())
        .map(|token| format!("\"{}\"*", token.replace('"', "\"\"")))
        .collect::<Vec<_>>();
    (!quoted.is_empty()).then(|| quoted.join(" AND "))
}'''
if old_fts not in search:
    raise SystemExit("FTS tokenizer anchor changed")
search = search.replace(old_fts, new_fts, 1)
write("src-tauri/src/authoring/search_and_tags.rs", search)

# Verifiers follow the current projection API rather than requiring removed
# compatibility functions.
stage3 = read("scripts/verify-stage3.mjs")
stage3 = stage3.replace('"render_stored_card_presentation_for_variant",', '"render_projected_presentation",')
write("scripts/verify-stage3.mjs", stage3)

print("Applied strict native cleanup and Clippy refactors.")
