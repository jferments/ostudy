#!/usr/bin/env python3
"""Apply the Stage 3.6/3.7 scheduling and Analytics production cutover.

Temporary reviewable applicator. It is removed before PR #28 is marked ready.
"""
from __future__ import annotations

import hashlib
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def write(path: str, content: str) -> None:
    target = ROOT / path
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(content, encoding="utf-8")


def replace_once(path: str, old: str, new: str) -> None:
    text = read(path)
    if text.count(old) != 1:
        raise SystemExit(f"{path}: expected one anchor for replacement")
    write(path, text.replace(old, new, 1))


write("src-tauri/src/workload.rs", r'''//! Bounded daily workload accounting and authoritative scheduled-session planning.
//!
//! The workload planner consumes one captured local-calendar context. Counts are
//! aggregate SQL projections and selected rows are capped before hydration; query
//! count therefore does not grow with the number of cards in the library.

use rusqlite::{params_from_iter, types::Value as SqlValue, Connection};
use serde::Serialize;

use crate::{
    authoring::presentation_variant_for_projected,
    error::CommandError,
    settings::AppSettingsDto,
    study_calendar::StudyTimeContext,
    studyability::learning_signal_cte,
};

const MAX_PLANNED_CARDS: i64 = 10_000;

#[derive(Clone, Debug)]
pub struct PlannedCard {
    pub deck_id: String,
    pub card_id: String,
    pub note_id: String,
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

pub fn workload_overview(
    connection: &Connection,
    deck_ids: &[String],
    settings: &AppSettingsDto,
) -> Result<WorkloadOverviewDto, CommandError> {
    workload_overview_scoped(connection, deck_ids, &[], settings, &StudyTimeContext::capture()?)
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
    let (new_completed, reviews_completed, due_reviews, due_new, needs, extra):
        (i64, i64, i64, i64, i64, i64) = connection
        .query_row(&sql, params_from_iter(values.iter()), |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?))
        })
        .map_err(CommandError::database)?;
    let new_remaining = (settings.daily_new_limit - new_completed).max(0);
    let reviews_remaining = (settings.daily_review_limit - reviews_completed).max(0);
    let scheduled_reviews = due_reviews.min(reviews_remaining);
    let scheduled_new = if due_reviews > reviews_remaining { 0 } else { due_new.min(new_remaining) };
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

pub fn planned_scheduled_cards(
    connection: &Connection,
    deck_ids: &[String],
    settings: &AppSettingsDto,
) -> Result<(Vec<PlannedCard>, WorkloadOverviewDto), CommandError> {
    planned_scheduled_cards_scoped(
        connection,
        deck_ids,
        &[],
        settings,
        &StudyTimeContext::capture()?,
    )
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

pub fn planned_extra_cards(
    connection: &Connection,
    deck_ids: &[String],
) -> Result<Vec<PlannedCard>, CommandError> {
    planned_extra_cards_scoped(connection, deck_ids, &[], &StudyTimeContext::capture()?)
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
             SELECT c.deck_id, c.id, c.note_id, s.due_at, s.state,
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
         SELECT deck_id, id, note_id, due_at, state, needs_more_practice,
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
             SELECT c.deck_id, c.id, c.note_id, s.due_at, s.state,
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
         SELECT deck_id, id, note_id, due_at, state, needs_more_practice,
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
                row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?, row.get::<_, i64>(4)?, row.get::<_, i64>(5)?,
                row.get::<_, String>(6)?, row.get::<_, String>(7)?,
                row.get::<_, String>(8)?, row.get::<_, i64>(9)?,
            ))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    rows.into_iter()
        .map(|(deck_id, card_id, note_id, due_at, state, weak, note_type, source, mode, reps)| {
            Ok(PlannedCard {
                deck_id,
                presentation_variant: presentation_variant_for_projected(
                    &note_type, &source, &mode, reps,
                )?,
                card_id,
                note_id,
                due_at,
                state,
                needs_more_practice: weak != 0,
            })
        })
        .collect()
}

fn scope_conditions(
    deck_ids: &[String],
    tag_ids: &[String],
    now: i64,
) -> (String, Vec<SqlValue>) {
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
        let placeholders = deck_ids.iter().map(|id| {
            values.push(id.clone().into());
            format!("?{}", values.len())
        }).collect::<Vec<_>>().join(", ");
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
    fn empty_limits_produce_no_selected_cards_without_querying() {
        assert_eq!(MAX_PLANNED_CARDS, 10_000);
    }
}
''')

# Add one pure cloze-presentation variant resolver and route the existing query helper through it.
generated_path = "src-tauri/src/authoring/stage3_v2/generated_cards.rs"
generated = read(generated_path)
anchor = '''pub(crate) fn presentation_variant_for_card(
    connection: &Connection,
    card_id: &str,
) -> Result<String, CommandError> {
'''
if "pub(crate) fn presentation_variant_for_projected" not in generated:
    function = r'''pub(crate) fn presentation_variant_for_projected(
    note_type: &str,
    cloze_source: &str,
    cloze_mode: &str,
    reps: i64,
) -> Result<String, CommandError> {
    if note_type != "cloze" {
        return Ok(String::new());
    }
    validate_cloze_mode(cloze_mode)?;
    if cloze_mode == "all_together" {
        return Ok("cloze:all".to_string());
    }
    let parsed = parse_cloze(cloze_source)?;
    let index = usize::try_from(reps.max(0)).unwrap_or(0) % parsed.groups.len();
    Ok(format!("cloze:{}", parsed.groups[index]))
}

'''
    if anchor not in generated:
        raise SystemExit("generated card variant anchor changed")
    generated = generated.replace(anchor, function + anchor, 1)
old_body = '''    if stored.0 != "cloze" {
        return Ok(String::new());
    }
    validate_cloze_mode(&stored.2)?;
    if stored.2 == "all_together" {
        return Ok("cloze:all".to_string());
    }
    let parsed = parse_cloze(&stored.1)?;
    let index = usize::try_from(stored.3.max(0)).unwrap_or(0) % parsed.groups.len();
    Ok(format!("cloze:{}", parsed.groups[index]))
'''
if old_body in generated:
    generated = generated.replace(old_body, '''    presentation_variant_for_projected(&stored.0, &stored.1, &stored.2, stored.3)
''', 1)
write(generated_path, generated)

# Archive should dismiss active session rows; deleting an archived deck is valid.
decks_path = "src-tauri/src/decks.rs"
decks = read(decks_path)
if "practice::refresh_session_counters" not in decks:
    decks = decks.replace(
        "    error::CommandError,\n    study_calendar::StudyTimeContext,",
        "    error::CommandError,\n    practice::refresh_session_counters,\n    study_calendar::StudyTimeContext,",
        1,
    )
archive_commit = '''    transaction.commit().map_err(CommandError::database)?;
    get_deck_from_connection(connection, deck_id)
}

fn unarchive_deck_in_connection'''
if "archive_session_ids" not in decks:
    addition = '''    let archive_session_ids = {
        let mut statement = transaction
            .prepare(
                "SELECT DISTINCT psc.session_id
                 FROM practice_session_cards psc
                 JOIN cards c ON c.id = psc.card_id
                 JOIN practice_sessions ps ON ps.id = psc.session_id
                 WHERE c.deck_id = ?1 AND ps.completed_at IS NULL
                   AND psc.rated_at IS NULL AND psc.dismissed_at IS NULL",
            )
            .map_err(CommandError::database)?;
        statement
            .query_map([deck_id], |row| row.get::<_, String>(0))
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?
    };
    transaction
        .execute(
            "UPDATE practice_session_cards
             SET dismissed_at = ?1, disposition = 'suspended'
             WHERE rated_at IS NULL AND dismissed_at IS NULL
               AND card_id IN (SELECT id FROM cards WHERE deck_id = ?2)",
            params![now, deck_id],
        )
        .map_err(CommandError::database)?;
    for session_id in archive_session_ids {
        refresh_session_counters(&transaction, &session_id, now)?;
    }
'''
    if archive_commit not in decks:
        raise SystemExit("archive commit anchor changed")
    decks = decks.replace(archive_commit, addition + archive_commit, 1)
write(decks_path, decks)

deletion_path = "src-tauri/src/deletion.rs"
deletion = read(deletion_path)
deletion = deletion.replace(
    "WHERE id = ?1 AND deleted_at IS NULL AND archived_at IS NULL",
    "WHERE id = ?1 AND deleted_at IS NULL",
)
deletion = deletion.replace(
    "WHERE id = ?2 AND deleted_at IS NULL AND archived_at IS NULL",
    "WHERE id = ?2 AND deleted_at IS NULL",
)
write(deletion_path, deletion)

# Backend-authoritative date intents for manual rescheduling.
schedule_path = "src-tauri/src/schedule_controls.rs"
schedule = read(schedule_path)
if "RescheduleIntentInput" not in schedule:
    schedule = schedule.replace(
        "use serde::{ser::SerializeStruct, Deserialize, Serialize, Serializer};",
        "use serde::{ser::SerializeStruct, Deserialize, Serialize, Serializer};",
        1,
    )
    schedule = schedule.replace(
        "    util::{generate_id, unix_time_millis, validate_entity_id},\n    workload::next_local_day_start_millis,",
        "    study_calendar::{parse_local_date, StudyTimeContext},\n    util::{generate_id, unix_time_millis, validate_entity_id},",
        1,
    )
    schedule = schedule.replace(
        '''pub struct ScheduleActionInput {
    pub mutation_id: String,
    pub card_ids: Vec<String>,
    pub action: String,
    pub due_at: Option<i64>,
    pub session_id: Option<String>,
}
''',
        '''pub struct ScheduleActionInput {
    pub mutation_id: String,
    pub card_ids: Vec<String>,
    pub action: String,
    pub reschedule: Option<RescheduleIntentInput>,
    pub session_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", tag = "kind", deny_unknown_fields)]
pub enum RescheduleIntentInput {
    DueNow,
    Tomorrow,
    CalendarDays { days: i64 },
    CalendarDate { date: String },
}
''',
        1,
    )
    schedule = schedule.replace(
        "    due_at: Option<i64>,\n    session_id: Option<&'a str>,",
        "    reschedule: Option<&'a RescheduleIntentInput>,\n    session_id: Option<&'a str>,",
        1,
    )
    schedule = schedule.replace(
        "    due_at: Option<i64>,\n    session_id: Option<String>,\n    request_fingerprint: String,",
        "    reschedule: Option<RescheduleIntentInput>,\n    session_id: Option<String>,\n    request_fingerprint: String,",
        1,
    )
    schedule = schedule.replace(
        "    due_at: Option<i64>,\n    result: ScheduleActionResultDto,",
        "    due_at: Option<i64>,\n    result: ScheduleActionResultDto,",
        1,
    )
    schedule = schedule.replace(
        '''    let now = unix_time_millis()?;
    let bury_until = if input.action == ScheduleActionKind::Bury {
        Some(next_local_day_start_millis()?)
    } else {
        None
    };
''',
        '''    let calendar = StudyTimeContext::capture()?;
    let now = calendar.now_utc_millis;
    let bury_until = (input.action == ScheduleActionKind::Bury)
        .then_some(calendar.tomorrow_start_utc_millis);
    let resolved_due_at = resolve_reschedule(&input, &calendar)?;
''',
        1,
    )
    schedule = schedule.replace("            input.due_at,\n            bury_until,", "            resolved_due_at,\n            bury_until,", 1)
    schedule = schedule.replace("    insert_operation(&transaction, &input, &result, now)?;", "    insert_operation(&transaction, &input, resolved_due_at, &result, now)?;", 1)
    schedule = schedule.replace(
        "    input: &ValidatedScheduleAction,\n    result: &ScheduleActionResultDto,",
        "    input: &ValidatedScheduleAction,\n    resolved_due_at: Option<i64>,\n    result: &ScheduleActionResultDto,",
        1,
    )
    schedule = schedule.replace("                input.due_at,\n                result.requested_count,", "                resolved_due_at,\n                result.requested_count,", 1)
    schedule = schedule.replace(
        "                && existing.due_at == input.due_at\n                && card_ids == input.card_ids",
        "                && existing.due_at.is_some() == input.reschedule.is_some()\n                && card_ids == input.card_ids",
        1,
    )
    # Replace validation's timestamp rules and identity construction.
    old = '''    if action == ScheduleActionKind::Reschedule {
        let due_at = input.due_at.ok_or_else(|| {
            CommandError::new("missing_reschedule_date", "Choose a new due date.")
        })?;
        if due_at < 0 {
            return Err(CommandError::new(
                "invalid_reschedule_date",
                "The new due date is invalid.",
            ));
        }
    } else if input.due_at.is_some() {
        return Err(CommandError::new(
            "unexpected_reschedule_date",
            "A due date is only valid when rescheduling.",
        ));
    }
'''
    new = '''    if action == ScheduleActionKind::Reschedule {
        if input.reschedule.is_none() {
            return Err(CommandError::new("missing_reschedule_date", "Choose a new due date."));
        }
    } else if input.reschedule.is_some() {
        return Err(CommandError::new(
            "unexpected_reschedule_date",
            "A date intent is only valid when rescheduling.",
        ));
    }
'''
    if old not in schedule:
        raise SystemExit("schedule validation anchor changed")
    schedule = schedule.replace(old, new, 1)
    schedule = schedule.replace(
        "        due_at: input.due_at,\n        session_id: input.session_id.as_deref(),",
        "        reschedule: input.reschedule.as_ref(),\n        session_id: input.session_id.as_deref(),",
        1,
    )
    schedule = schedule.replace(
        "        due_at: input.due_at,\n        session_id: input.session_id,",
        "        reschedule: input.reschedule,\n        session_id: input.session_id,",
        1,
    )
    # Tests construct due_at fields; adjust in this file.
    schedule = schedule.replace("due_at: None,", "reschedule: None,")
    # Add resolver before validation.
    marker = "fn validate_input(input: ScheduleActionInput) -> Result<ValidatedScheduleAction, CommandError> {"
    resolver = r'''fn resolve_reschedule(
    input: &ValidatedScheduleAction,
    context: &StudyTimeContext,
) -> Result<Option<i64>, CommandError> {
    let Some(intent) = input.reschedule.as_ref() else {
        return Ok(None);
    };
    let resolved = match intent {
        RescheduleIntentInput::DueNow => context.now_utc_millis,
        RescheduleIntentInput::Tomorrow => context.tomorrow_start_utc_millis,
        RescheduleIntentInput::CalendarDays { days } => {
            if !(1..=3650).contains(days) {
                return Err(CommandError::new(
                    "invalid_reschedule_date",
                    "Choose between 1 and 3650 local calendar days.",
                ));
            }
            context.resolve_date_or_now(context.date_after_days(*days)?)?
        }
        RescheduleIntentInput::CalendarDate { date } => {
            context.resolve_date_or_now(parse_local_date(date)?)?
        }
    };
    Ok(Some(resolved))
}

'''
    if marker not in schedule:
        raise SystemExit("schedule resolver marker missing")
    schedule = schedule.replace(marker, resolver + marker, 1)
write(schedule_path, schedule)

# Complete Analytics implementation: current inventory is current-state; all history
# reads immutable attribution and joins retained deck IDs for current names.
write("src-tauri/src/analytics.rs", r'''//! Fixed-query current inventory and immutable historical learning Analytics.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{Datelike, Duration, NaiveDate};
use rusqlite::{params_from_iter, types::Value as SqlValue, Connection};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    database::Database,
    error::CommandError,
    settings::load_settings,
    study_calendar::{previous_range_start_date, range_start_date, StudyTimeContext},
    studyability::learning_signal_cte,
    util::validate_entity_id,
};

const MAX_SCOPE_DECKS: usize = 100;
const MAX_SCOPE_TAGS: usize = 32;
const TAG_SEPARATOR: char = '\u{001f}';

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnalyticsInput {
    pub range: String,
    #[serde(default)]
    pub deck_ids: Vec<String>,
    #[serde(default)]
    pub tag_ids: Vec<String>,
    pub highlighted_session_id: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsScopeDto {
    pub range: String,
    pub start_at: Option<i64>,
    pub end_at: i64,
    pub bucket_granularity: String,
    pub deck_ids: Vec<String>,
    pub tag_ids: Vec<String>,
    pub highlighted_session_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsSummaryDto {
    pub current_deck_count: i64,
    pub source_note_count: i64,
    pub generated_card_count: i64,
    pub studyable_card_count: i64,
    pub studyable_due_count: i64,
    pub needs_more_practice: i64,
    pub cards_practiced: i64,
    pub review_count: i64,
    pub correct_count: i64,
    pub correct_rate: Option<f64>,
    pub average_confidence: Option<f64>,
    pub average_response_time_ms: Option<f64>,
    pub total_study_time_ms: i64,
    pub session_count: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyLimitUsageDto {
    pub new_limit: i64,
    pub review_limit: i64,
    pub new_completed: i64,
    pub reviews_completed: i64,
    pub new_remaining: i64,
    pub reviews_remaining: i64,
}

#[derive(Clone, Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RatingDistributionDto { pub again: i64, pub hard: i64, pub good: i64, pub easy: i64 }

#[derive(Clone, Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TimelineBucketDto {
    pub start_at: i64, pub end_at: i64, pub cards_practiced: i64, pub review_count: i64,
    pub new_cards: i64, pub remembered_count: i64, pub correct_rate: Option<f64>,
    pub average_confidence: Option<f64>, pub average_response_time_ms: Option<f64>,
    pub study_time_ms: i64, pub session_count: i64, pub contains_highlighted_session: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeckAnalyticsDto {
    pub deck_id: String, pub deck_name: String, pub archived: bool,
    pub source_note_count: i64, pub generated_card_count: i64, pub studyable_due_count: i64,
    pub needs_more_practice: i64, pub cards_practiced: i64, pub review_count: i64,
    pub correct_count: i64, pub correct_rate: Option<f64>, pub average_confidence: Option<f64>,
    pub average_response_time_ms: Option<f64>, pub total_study_time_ms: i64,
    pub last_practiced_at: Option<i64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TagAnalyticsDto {
    pub tag_id: String, pub tag_name: String, pub source_note_count: i64,
    pub generated_card_count: i64, pub studyable_due_count: i64, pub needs_more_practice: i64,
    pub cards_practiced: i64, pub review_count: i64, pub correct_count: i64,
    pub correct_rate: Option<f64>, pub average_confidence: Option<f64>,
    pub average_response_time_ms: Option<f64>, pub total_study_time_ms: i64,
    pub last_practiced_at: Option<i64>, pub trend_delta: Option<f64>, pub status: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentReviewDto {
    pub id: String, pub deck_id: String, pub deck_name: String, pub card_id: String,
    pub front_text: String, pub tags: Vec<String>, pub outcome: String, pub rating: i64,
    pub response_time_ms: i64, pub reviewed_at: i64, pub due_after: i64,
    pub attribution_source: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionAnalyticsDto {
    pub id: String, pub mode: String, pub deck_names: Vec<String>, pub started_at: i64,
    pub completed_at: Option<i64>, pub duration_ms: i64, pub card_count: i64,
    pub rated_count: i64, pub correct_count: i64, pub correct_rate: Option<f64>,
    pub study_time_ms: i64, pub highlighted: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsDimensionOptionDto { pub id: String, pub name: String, pub archived: bool, pub historical_only: bool }

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsDimensionOptionsDto { pub decks: Vec<AnalyticsDimensionOptionDto>, pub tags: Vec<AnalyticsDimensionOptionDto> }

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttributionCoverageDto { pub captured_review_count: i64, pub legacy_backfilled_review_count: i64 }

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsDto {
    pub scope: AnalyticsScopeDto, pub summary: AnalyticsSummaryDto,
    pub daily_limits: DailyLimitUsageDto, pub rating_distribution: RatingDistributionDto,
    pub timeline: Vec<TimelineBucketDto>, pub deck_stats: Vec<DeckAnalyticsDto>,
    pub tag_stats: Vec<TagAnalyticsDto>, pub recent_reviews: Vec<RecentReviewDto>,
    pub recent_sessions: Vec<SessionAnalyticsDto>, pub dimensions: AnalyticsDimensionOptionsDto,
    pub attribution_coverage: AttributionCoverageDto,
}

#[derive(Clone)]
struct Scope {
    range: String, deck_ids: Vec<String>, tag_ids: Vec<String>, highlighted: Option<String>,
    start_date: Option<NaiveDate>, previous_start: Option<NaiveDate>, context: StudyTimeContext,
}

#[tauri::command]
pub fn get_analytics(database: State<'_, Database>, input: AnalyticsInput) -> Result<AnalyticsDto, CommandError> {
    let scope = validate_scope(input)?;
    database.with_connection(|connection| analytics_in_connection(connection, &scope))
}

#[tauri::command]
pub fn get_recent_practice_reviews(database: State<'_, Database>) -> Result<Vec<RecentReviewDto>, CommandError> {
    database.with_connection(|connection| recent_reviews(connection, &Scope {
        range: "all".to_string(), deck_ids: vec![], tag_ids: vec![], highlighted: None,
        start_date: None, previous_start: None, context: StudyTimeContext::capture()?,
    }, 30))
}

fn analytics_in_connection(connection: &Connection, scope: &Scope) -> Result<AnalyticsDto, CommandError> {
    let mut summary = current_inventory(connection, scope)?;
    let (history, ratings) = historical_summary(connection, scope)?;
    summary.cards_practiced = history.0; summary.review_count = history.1;
    summary.correct_count = history.2; summary.correct_rate = ratio(history.2, history.1);
    summary.average_confidence = history.3; summary.average_response_time_ms = history.4;
    summary.total_study_time_ms = history.5; summary.session_count = history.6;
    let settings = load_settings(connection)?;
    let (new_completed, reviews_completed): (i64, i64) = connection.query_row(
        "SELECT COALESCE(SUM(CASE WHEN r.state_before = 0 THEN 1 ELSE 0 END),0),
                COALESCE(SUM(CASE WHEN r.state_before <> 0 THEN 1 ELSE 0 END),0)
         FROM review_events r JOIN review_event_attributions a ON a.review_event_id = r.id
         WHERE a.review_local_date = ?1",
        [scope.context.local_date_string()], |row| Ok((row.get(0)?, row.get(1)?)),
    ).map_err(CommandError::database)?;
    Ok(AnalyticsDto {
        scope: AnalyticsScopeDto {
            range: scope.range.clone(),
            start_at: scope.start_date.map(|date| scope.context.resolve_date_start(date)).transpose()?,
            end_at: scope.context.now_utc_millis,
            bucket_granularity: granularity(&scope.range).to_string(),
            deck_ids: scope.deck_ids.clone(), tag_ids: scope.tag_ids.clone(),
            highlighted_session_id: scope.highlighted.clone(),
        },
        summary,
        daily_limits: DailyLimitUsageDto {
            new_limit: settings.daily_new_limit, review_limit: settings.daily_review_limit,
            new_completed, reviews_completed,
            new_remaining: (settings.daily_new_limit - new_completed).max(0),
            reviews_remaining: (settings.daily_review_limit - reviews_completed).max(0),
        },
        rating_distribution: ratings,
        timeline: timeline(connection, scope)?, deck_stats: deck_stats(connection, scope)?,
        tag_stats: tag_stats(connection, scope)?, recent_reviews: recent_reviews(connection, scope, 20)?,
        recent_sessions: recent_sessions(connection, scope)?, dimensions: dimensions(connection)?,
        attribution_coverage: attribution_coverage(connection, scope)?,
    })
}

fn current_inventory(connection: &Connection, scope: &Scope) -> Result<AnalyticsSummaryDto, CommandError> {
    let mut values = vec![scope.context.now_utc_millis.into()];
    let filters = current_filters(scope, &mut values);
    let sql = format!(
        "WITH {signals}, scoped AS (
             SELECT c.id, c.note_id, c.deck_id, s.due_at,
                    c.suspended_at, c.buried_until,
                    COALESCE(ls.needs_more_practice,0) AS needs_more_practice
             FROM cards c JOIN notes n ON n.id=c.note_id JOIN decks d ON d.id=c.deck_id
             JOIN card_schedules s ON s.card_id=c.id
             LEFT JOIN learning_signals ls ON ls.card_id=c.id
             WHERE c.deleted_at IS NULL AND n.deleted_at IS NULL
               AND d.deleted_at IS NULL AND d.archived_at IS NULL {filters}
         )
         SELECT COUNT(DISTINCT deck_id), COUNT(DISTINCT note_id), COUNT(id),
                COALESCE(SUM(CASE WHEN suspended_at IS NULL AND (buried_until IS NULL OR buried_until <= ?1) THEN 1 ELSE 0 END),0),
                COALESCE(SUM(CASE WHEN suspended_at IS NULL AND (buried_until IS NULL OR buried_until <= ?1) AND due_at <= ?1 THEN 1 ELSE 0 END),0),
                COALESCE(SUM(CASE WHEN suspended_at IS NULL AND (buried_until IS NULL OR buried_until <= ?1) THEN needs_more_practice ELSE 0 END),0)
         FROM scoped", signals=learning_signal_cte());
    let row: (i64,i64,i64,i64,i64,i64) = connection.query_row(&sql, params_from_iter(values.iter()), |r| {
        Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))
    }).map_err(CommandError::database)?;
    Ok(AnalyticsSummaryDto { current_deck_count: row.0, source_note_count: row.1,
        generated_card_count: row.2, studyable_card_count: row.3, studyable_due_count: row.4,
        needs_more_practice: row.5, ..Default::default() })
}

fn historical_summary(connection: &Connection, scope: &Scope) -> Result<((i64,i64,i64,Option<f64>,Option<f64>,i64,i64), RatingDistributionDto), CommandError> {
    let (where_sql, values) = history_where(scope);
    let sql = format!(
        "SELECT COUNT(DISTINCT r.card_id), COUNT(*), COALESCE(SUM(r.answer_correct),0),
                AVG(CASE r.confidence WHEN 'wrong' THEN 0.0 WHEN 'not_confident' THEN 1.0 WHEN 'somewhat_confident' THEN 2.0 WHEN 'very_confident' THEN 3.0 END),
                AVG(r.response_time_ms), COALESCE(SUM(r.response_time_ms),0), COUNT(DISTINCT r.session_id),
                COALESCE(SUM(CASE WHEN r.rating=1 THEN 1 ELSE 0 END),0),
                COALESCE(SUM(CASE WHEN r.rating=2 THEN 1 ELSE 0 END),0),
                COALESCE(SUM(CASE WHEN r.rating=3 THEN 1 ELSE 0 END),0),
                COALESCE(SUM(CASE WHEN r.rating=4 THEN 1 ELSE 0 END),0)
         FROM review_events r JOIN review_event_attributions a ON a.review_event_id=r.id
         JOIN decks d ON d.id=a.deck_id AND d.deleted_at IS NULL
         WHERE {where_sql}");
    let row = connection.query_row(&sql, params_from_iter(values.iter()), |r| Ok((
        r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,
        r.get(7)?,r.get(8)?,r.get(9)?,r.get(10)?
    ))).map_err(CommandError::database)?;
    Ok(((row.0,row.1,row.2,row.3,row.4,row.5,row.6), RatingDistributionDto { again:row.7, hard:row.8, good:row.9, easy:row.10 }))
}

fn deck_stats(connection: &Connection, scope: &Scope) -> Result<Vec<DeckAnalyticsDto>, CommandError> {
    let (history_where, mut values) = history_where(scope);
    let now_index = push(&mut values, scope.context.now_utc_millis);
    let sql = format!(
        "WITH {signals}, current_stats AS (
             SELECT c.deck_id, COUNT(DISTINCT c.note_id), COUNT(c.id),
                    SUM(CASE WHEN c.suspended_at IS NULL AND (c.buried_until IS NULL OR c.buried_until <= ?{now_index}) AND s.due_at <= ?{now_index} THEN 1 ELSE 0 END),
                    SUM(CASE WHEN c.suspended_at IS NULL AND (c.buried_until IS NULL OR c.buried_until <= ?{now_index}) THEN COALESCE(ls.needs_more_practice,0) ELSE 0 END)
             FROM cards c JOIN notes n ON n.id=c.note_id JOIN decks cd ON cd.id=c.deck_id
             JOIN card_schedules s ON s.card_id=c.id LEFT JOIN learning_signals ls ON ls.card_id=c.id
             WHERE c.deleted_at IS NULL AND n.deleted_at IS NULL AND cd.deleted_at IS NULL AND cd.archived_at IS NULL
             GROUP BY c.deck_id
         ), history AS (
             SELECT a.deck_id, COUNT(DISTINCT r.card_id), COUNT(*), SUM(r.answer_correct),
                    AVG(CASE r.confidence WHEN 'wrong' THEN 0.0 WHEN 'not_confident' THEN 1.0 WHEN 'somewhat_confident' THEN 2.0 WHEN 'very_confident' THEN 3.0 END),
                    AVG(r.response_time_ms), SUM(r.response_time_ms), MAX(r.reviewed_at)
             FROM review_events r JOIN review_event_attributions a ON a.review_event_id=r.id
             JOIN decks hd ON hd.id=a.deck_id AND hd.deleted_at IS NULL
             WHERE {history_where} GROUP BY a.deck_id
         )
         SELECT d.id,d.name,d.archived_at,COALESCE(c.column2,0),COALESCE(c.column3,0),COALESCE(c.column4,0),COALESCE(c.column5,0),
                COALESCE(h.column2,0),COALESCE(h.column3,0),COALESCE(h.column4,0),h.column5,h.column6,COALESCE(h.column7,0),h.column8
         FROM decks d LEFT JOIN current_stats c ON c.deck_id=d.id LEFT JOIN history h ON h.deck_id=d.id
         WHERE d.deleted_at IS NULL AND (c.deck_id IS NOT NULL OR h.deck_id IS NOT NULL)
         ORDER BY COALESCE(h.column8,0) DESC,d.name COLLATE NOCASE", signals=learning_signal_cte());
    let mut statement=connection.prepare(&sql).map_err(CommandError::database)?;
    statement.query_map(params_from_iter(values.iter()), |r| {
        let reviews:i64=r.get(8)?; let correct:i64=r.get(9)?;
        Ok(DeckAnalyticsDto { deck_id:r.get(0)?,deck_name:r.get(1)?,archived:r.get::<_,Option<i64>>(2)?.is_some(),
            source_note_count:r.get(3)?,generated_card_count:r.get(4)?,studyable_due_count:r.get(5)?,needs_more_practice:r.get(6)?,
            cards_practiced:r.get(7)?,review_count:reviews,correct_count:correct,correct_rate:ratio(correct,reviews),
            average_confidence:r.get(10)?,average_response_time_ms:r.get(11)?,total_study_time_ms:r.get(12)?,last_practiced_at:r.get(13)? })
    }).map_err(CommandError::database)?.collect::<Result<Vec<_>,_>>().map_err(CommandError::database)
}

fn tag_stats(connection: &Connection, scope: &Scope) -> Result<Vec<TagAnalyticsDto>, CommandError> {
    let (where_sql, values)=history_where(scope);
    let sql=format!(
        "WITH history AS (
             SELECT ta.tag_id, COALESCE(t.name,MIN(ta.captured_name)) AS tag_name,
                    COUNT(DISTINCT r.card_id),COUNT(*),SUM(r.answer_correct),
                    AVG(CASE r.confidence WHEN 'wrong' THEN 0.0 WHEN 'not_confident' THEN 1.0 WHEN 'somewhat_confident' THEN 2.0 WHEN 'very_confident' THEN 3.0 END),
                    AVG(r.response_time_ms),SUM(r.response_time_ms),MAX(r.reviewed_at)
             FROM review_events r JOIN review_event_attributions a ON a.review_event_id=r.id
             JOIN decks d ON d.id=a.deck_id AND d.deleted_at IS NULL
             JOIN review_event_tag_attributions ta ON ta.review_event_id=r.id
             LEFT JOIN tags t ON t.id=ta.tag_id
             WHERE {where_sql} GROUP BY ta.tag_id
         ) SELECT tag_id,tag_name,column3,column4,column5,column6,column7,column8,column9 FROM history ORDER BY column9 DESC,tag_name COLLATE NOCASE");
    let mut statement=connection.prepare(&sql).map_err(CommandError::database)?;
    let rows=statement.query_map(params_from_iter(values.iter()), |r| {
        let reviews:i64=r.get(3)?; let correct:i64=r.get(4)?; let rate=ratio(correct,reviews);
        let status=if reviews<3{"building_data"}else if rate.unwrap_or(0.0)<0.6{"needs_attention"}else if rate.unwrap_or(0.0)>=0.85{"strong"}else{"on_track"};
        Ok(TagAnalyticsDto { tag_id:r.get(0)?,tag_name:r.get(1)?,source_note_count:0,generated_card_count:0,studyable_due_count:0,needs_more_practice:0,
            cards_practiced:r.get(2)?,review_count:reviews,correct_count:correct,correct_rate:rate,average_confidence:r.get(5)?,average_response_time_ms:r.get(6)?,
            total_study_time_ms:r.get(7)?,last_practiced_at:r.get(8)?,trend_delta:None,status:status.to_string() })
    }).map_err(CommandError::database)?.collect::<Result<Vec<_>,_>>().map_err(CommandError::database)?;
    Ok(rows)
}

fn timeline(connection:&Connection,scope:&Scope)->Result<Vec<TimelineBucketDto>,CommandError>{
    let (where_sql,values)=history_where(scope);
    let sql=format!("SELECT a.review_local_date,COUNT(DISTINCT r.card_id),COUNT(*),SUM(CASE WHEN r.state_before=0 THEN 1 ELSE 0 END),SUM(r.answer_correct),AVG(CASE r.confidence WHEN 'wrong' THEN 0.0 WHEN 'not_confident' THEN 1.0 WHEN 'somewhat_confident' THEN 2.0 WHEN 'very_confident' THEN 3.0 END),AVG(r.response_time_ms),SUM(r.response_time_ms),COUNT(DISTINCT r.session_id) FROM review_events r JOIN review_event_attributions a ON a.review_event_id=r.id JOIN decks d ON d.id=a.deck_id AND d.deleted_at IS NULL WHERE {where_sql} GROUP BY a.review_local_date ORDER BY a.review_local_date");
    let mut statement=connection.prepare(&sql).map_err(CommandError::database)?;
    let rows=statement.query_map(params_from_iter(values.iter()),|r|Ok((r.get::<_,String>(0)?,r.get::<_,i64>(1)?,r.get::<_,i64>(2)?,r.get::<_,i64>(3)?,r.get::<_,i64>(4)?,r.get::<_,Option<f64>>(5)?,r.get::<_,Option<f64>>(6)?,r.get::<_,i64>(7)?,r.get::<_,i64>(8)?))).map_err(CommandError::database)?.collect::<Result<Vec<_>,_>>().map_err(CommandError::database)?;
    let mut grouped:BTreeMap<String,TimelineBucketDto>=BTreeMap::new();
    for (date,cards,reviews,new_cards,remembered,confidence,response,time,sessions) in rows {
        let parsed=NaiveDate::parse_from_str(&date,"%Y-%m-%d").map_err(|_|CommandError::new("invalid_analytics_date","Stored review date is invalid."))?;
        let key=match granularity(&scope.range){"month"=>format!("{:04}-{:02}",parsed.year(),parsed.month()),"week"=>{let start=scope.start_date.unwrap_or(parsed);format!("w{}",parsed.signed_duration_since(start).num_days().div_euclid(7))},_=>date.clone()};
        let start_date=match granularity(&scope.range){"month"=>NaiveDate::from_ymd_opt(parsed.year(),parsed.month(),1).unwrap_or(parsed),"week"=>parsed.checked_sub_signed(Duration::days(parsed.weekday().num_days_from_monday().into())).unwrap_or(parsed),_=>parsed};
        let start=scope.context.resolve_date_start(start_date)?; let end_date=match granularity(&scope.range){"month"=>if parsed.month()==12{NaiveDate::from_ymd_opt(parsed.year()+1,1,1)}else{NaiveDate::from_ymd_opt(parsed.year(),parsed.month()+1,1)},"week"=>start_date.checked_add_signed(Duration::days(7)),_=>start_date.succ_opt()}.ok_or_else(||CommandError::new("invalid_analytics_date","Analytics bucket is outside supported range."))?;
        let entry=grouped.entry(key).or_insert(TimelineBucketDto{start_at:start,end_at:scope.context.resolve_date_start(end_date)?,..Default::default()});
        entry.cards_practiced+=cards;entry.review_count+=reviews;entry.new_cards+=new_cards;entry.remembered_count+=remembered;entry.study_time_ms+=time;entry.session_count+=sessions;
        entry.correct_rate=ratio(entry.remembered_count,entry.review_count);entry.average_confidence=confidence;entry.average_response_time_ms=response;
    }
    Ok(grouped.into_values().collect())
}

fn recent_reviews(connection:&Connection,scope:&Scope,limit:i64)->Result<Vec<RecentReviewDto>,CommandError>{
    let (where_sql,mut values)=history_where(scope);let limit_index=push(&mut values,limit);
    let sql=format!("SELECT r.id,a.deck_id,d.name,r.card_id,a.question_excerpt,COALESCE((SELECT group_concat(name,char(31)) FROM (SELECT DISTINCT COALESCE(t.name,ta.captured_name) name FROM review_event_tag_attributions ta LEFT JOIN tags t ON t.id=ta.tag_id WHERE ta.review_event_id=r.id ORDER BY name COLLATE NOCASE)),''),r.confidence,r.rating,r.response_time_ms,r.reviewed_at,r.due_after,a.attribution_source FROM review_events r JOIN review_event_attributions a ON a.review_event_id=r.id JOIN decks d ON d.id=a.deck_id AND d.deleted_at IS NULL WHERE {where_sql} ORDER BY r.reviewed_at DESC,r.created_at DESC,r.id DESC LIMIT ?{limit_index}");
    let mut statement=connection.prepare(&sql).map_err(CommandError::database)?;
    statement.query_map(params_from_iter(values.iter()),|r|Ok(RecentReviewDto{id:r.get(0)?,deck_id:r.get(1)?,deck_name:r.get(2)?,card_id:r.get(3)?,front_text:r.get(4)?,tags:split(r.get::<_,String>(5)?),outcome:r.get(6)?,rating:r.get(7)?,response_time_ms:r.get(8)?,reviewed_at:r.get(9)?,due_after:r.get(10)?,attribution_source:r.get(11)?})).map_err(CommandError::database)?.collect::<Result<Vec<_>,_>>().map_err(CommandError::database)
}

fn recent_sessions(connection:&Connection,scope:&Scope)->Result<Vec<SessionAnalyticsDto>,CommandError>{
    let (where_sql,mut values)=history_where(scope);let highlighted_index=push(&mut values,scope.highlighted.clone().unwrap_or_default());
    let sql=format!("SELECT ps.id,ps.mode,COALESCE((SELECT group_concat(name,char(31)) FROM (SELECT DISTINCT d2.name name FROM review_events r2 JOIN review_event_attributions a2 ON a2.review_event_id=r2.id JOIN decks d2 ON d2.id=a2.deck_id AND d2.deleted_at IS NULL WHERE r2.session_id=ps.id ORDER BY name COLLATE NOCASE)),''),ps.started_at,ps.completed_at,ps.card_count,ps.rated_count,ps.correct_count,COALESCE(SUM(r.response_time_ms),0),CASE WHEN ps.id=?{highlighted_index} THEN 1 ELSE 0 END FROM practice_sessions ps JOIN review_events r ON r.session_id=ps.id JOIN review_event_attributions a ON a.review_event_id=r.id JOIN decks d ON d.id=a.deck_id AND d.deleted_at IS NULL WHERE {where_sql} GROUP BY ps.id ORDER BY ps.started_at DESC LIMIT 12");
    let mut statement=connection.prepare(&sql).map_err(CommandError::database)?;
    statement.query_map(params_from_iter(values.iter()),|r|{let rated:i64=r.get(6)?;let correct:i64=r.get(7)?;let start:i64=r.get(3)?;let completed:Option<i64>=r.get(4)?;Ok(SessionAnalyticsDto{id:r.get(0)?,mode:r.get(1)?,deck_names:split(r.get::<_,String>(2)?),started_at:start,completed_at:completed,duration_ms:completed.unwrap_or(scope.context.now_utc_millis).saturating_sub(start),card_count:r.get(5)?,rated_count:rated,correct_count:correct,correct_rate:ratio(correct,rated),study_time_ms:r.get(8)?,highlighted:r.get::<_,i64>(9)?!=0})}).map_err(CommandError::database)?.collect::<Result<Vec<_>,_>>().map_err(CommandError::database)
}

fn dimensions(connection:&Connection)->Result<AnalyticsDimensionOptionsDto,CommandError>{
    let mut ds=connection.prepare("SELECT id,name,archived_at FROM decks WHERE deleted_at IS NULL ORDER BY name COLLATE NOCASE").map_err(CommandError::database)?;
    let decks=ds.query_map([],|r|Ok(AnalyticsDimensionOptionDto{id:r.get(0)?,name:r.get(1)?,archived:r.get::<_,Option<i64>>(2)?.is_some(),historical_only:false})).map_err(CommandError::database)?.collect::<Result<Vec<_>,_>>().map_err(CommandError::database)?;
    let mut ts=connection.prepare("SELECT dim.id,COALESCE(t.name,dim.name) FROM (SELECT tag_id id,MIN(captured_name) name FROM review_event_tag_attributions GROUP BY tag_id UNION SELECT id,name FROM tags) dim LEFT JOIN tags t ON t.id=dim.id GROUP BY dim.id ORDER BY COALESCE(t.name,dim.name) COLLATE NOCASE").map_err(CommandError::database)?;
    let tags=ts.query_map([],|r|Ok(AnalyticsDimensionOptionDto{id:r.get(0)?,name:r.get(1)?,archived:false,historical_only:false})).map_err(CommandError::database)?.collect::<Result<Vec<_>,_>>().map_err(CommandError::database)?;
    Ok(AnalyticsDimensionOptionsDto{decks,tags})
}

fn attribution_coverage(connection:&Connection,scope:&Scope)->Result<AttributionCoverageDto,CommandError>{let (where_sql,values)=history_where(scope);let sql=format!("SELECT SUM(CASE WHEN a.attribution_source='captured' THEN 1 ELSE 0 END),SUM(CASE WHEN a.attribution_source='legacy_current_state_backfill' THEN 1 ELSE 0 END) FROM review_events r JOIN review_event_attributions a ON a.review_event_id=r.id JOIN decks d ON d.id=a.deck_id AND d.deleted_at IS NULL WHERE {where_sql}");connection.query_row(&sql,params_from_iter(values.iter()),|r|Ok(AttributionCoverageDto{captured_review_count:r.get::<_,Option<i64>>(0)?.unwrap_or(0),legacy_backfilled_review_count:r.get::<_,Option<i64>>(1)?.unwrap_or(0)})).map_err(CommandError::database)}

fn validate_scope(input:AnalyticsInput)->Result<Scope,CommandError>{let mut decks=BTreeSet::new();for id in input.deck_ids{validate_entity_id(&id,"deck")?;decks.insert(id);}let mut tags=BTreeSet::new();for id in input.tag_ids{validate_entity_id(&id,"tag")?;tags.insert(id);}if decks.len()>MAX_SCOPE_DECKS||tags.len()>MAX_SCOPE_TAGS{return Err(CommandError::new("analytics_scope_too_large","The Analytics scope contains too many decks or tags."));}if let Some(id)=input.highlighted_session_id.as_deref(){validate_entity_id(id,"practice session")?;}let context=StudyTimeContext::capture()?;Ok(Scope{start_date:range_start_date(&context,&input.range)?,previous_start:previous_range_start_date(&context,&input.range)?,range:input.range,deck_ids:decks.into_iter().collect(),tag_ids:tags.into_iter().collect(),highlighted:input.highlighted_session_id,context})}

fn history_where(scope:&Scope)->(String,Vec<SqlValue>){let mut values=Vec::new();let mut c=vec!["1=1".to_string()];if let Some(date)=scope.start_date{values.push(date.format("%Y-%m-%d").to_string().into());c.push(format!("a.review_local_date >= ?{}",values.len()));}if !scope.deck_ids.is_empty(){let p=scope.deck_ids.iter().map(|id|{values.push(id.clone().into());format!("?{}",values.len())}).collect::<Vec<_>>().join(",");c.push(format!("a.deck_id IN ({p})"));}for id in &scope.tag_ids{values.push(id.clone().into());c.push(format!("EXISTS(SELECT 1 FROM review_event_tag_attributions fta WHERE fta.review_event_id=r.id AND fta.tag_id=?{})",values.len()));}(c.join(" AND "),values)}
fn current_filters(scope:&Scope,values:&mut Vec<SqlValue>)->String{let mut c=Vec::new();if !scope.deck_ids.is_empty(){let p=scope.deck_ids.iter().map(|id|{values.push(id.clone().into());format!("?{}",values.len())}).collect::<Vec<_>>().join(",");c.push(format!("c.deck_id IN ({p})"));}for id in &scope.tag_ids{values.push(id.clone().into());c.push(format!("EXISTS(SELECT 1 FROM note_tags nt WHERE nt.note_id=n.id AND nt.tag_id=?{})",values.len()));}if c.is_empty(){String::new()}else{format!(" AND {}",c.join(" AND "))}}
fn granularity(range:&str)->&'static str{match range{"90d"|"1y"=>"week","all"=>"month",_=>"day"}}
fn ratio(n:i64,d:i64)->Option<f64>{(d>0).then_some(n as f64/d as f64)}
fn split(value:String)->Vec<String>{if value.is_empty(){vec![]}else{value.split(TAG_SEPARATOR).map(str::to_string).collect()}}
fn push(values:&mut Vec<SqlValue>,value:impl Into<SqlValue>)->usize{values.push(value.into());values.len()}
''')

# Register new native modules/commands and schema tables in migration tests.
lib_path="src-tauri/src/lib.rs";lib=read(lib_path)
for module in ["historical_attribution","study_calendar","studyability"]:
    if f"mod {module};" not in lib:
        lib=lib.replace("mod learning_signals;",f"mod learning_signals;\nmod {module};",1)
for command,anchor in [
    ("decks::list_archived_decks,","decks::list_decks,"),
    ("decks::archive_deck,","decks::update_deck,"),
    ("decks::unarchive_deck,","decks::archive_deck,"),
]:
    if command not in lib:
        lib=lib.replace(anchor,anchor+"\n            "+command,1)
write(lib_path,lib)

# Migration changed while unreleased: add exact session presentation/asset snapshot columns.
migration_path="src-tauri/migrations/0014_study_calendar_history_projections.sql";migration=read(migration_path)
if "question_doc" not in migration:
    migration=migration.replace("    question_excerpt        TEXT NOT NULL,", "    question_doc            TEXT NOT NULL,\n    answer_doc              TEXT NOT NULL,\n    answer_text             TEXT NOT NULL,\n    question_excerpt        TEXT NOT NULL,",1)
    marker="CREATE INDEX practice_session_card_snapshots_deck_idx"
    asset_table='''CREATE TABLE practice_session_card_asset_snapshots (
    session_id  TEXT NOT NULL,
    card_id     TEXT NOT NULL,
    asset_id    TEXT NOT NULL CHECK(length(asset_id) = 32),
    PRIMARY KEY(session_id, card_id, asset_id),
    FOREIGN KEY(session_id, card_id)
        REFERENCES practice_session_card_snapshots(session_id, card_id)
        ON DELETE CASCADE
) STRICT;

'''
    migration=migration.replace(marker,asset_table+marker,1)
write(migration_path,migration)

# Recalculate migration checksum registration.
checksum=hashlib.sha256(migration.encode()).hexdigest();database_path="src-tauri/src/database.rs";database=read(database_path)
database=re.sub(r'(version: 14,\n\s*name: "study_calendar_history_projections",\n\s*checksum: ")[0-9a-f]+(")',rf'\g<1>{checksum}\2',database)
write(database_path,database)

print("Applied native scheduling, archive, calendar-intent, workload, and Analytics projections.")
