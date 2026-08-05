#!/usr/bin/env python3
"""Resolve first native compile findings and replace implicit Analytics CTE columns."""
from __future__ import annotations
import re
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
def read(p):return(ROOT/p).read_text(encoding='utf-8')
def write(p,c):(ROOT/p).write_text(c,encoding='utf-8')

# Imports and warning-free source.
p='src-tauri/src/assets.rs';t=read(p)
if not t.startswith('use std::collections::BTreeMap;'):t='use std::collections::BTreeMap;\n\n'+t
write(p,t)
p='src-tauri/src/historical_attribution.rs';t=read(p).replace(', OptionalExtension','');write(p,t)
p='src-tauri/src/practice_selection.rs';t=read(p).replace('BTreeMap, BTreeSet, HashMap, HashSet','BTreeMap, BTreeSet');write(p,t)
p='src-tauri/src/schedule_controls.rs';t=read(p).replace('generate_id, unix_time_millis, validate_entity_id','generate_id, validate_entity_id');write(p,t)
p='src-tauri/src/study_calendar.rs';t=read(p).replace('DateTime, ','');write(p,t)

# Analytics owns a mutable connection only because settings loading can insert missing defaults.
p='src-tauri/src/analytics.rs';t=read(p)
t=t.replace('fn analytics_in_connection(\n    connection: &Connection,','fn analytics_in_connection(\n    connection: &mut Connection,',1)
# Fixed named deck projection and current-name history semantics.
start=t.index('fn deck_stats(');end=t.index('\nfn tag_stats(',start)
deck_fn=r'''fn deck_stats(
    connection: &Connection,
    scope: &Scope,
) -> Result<Vec<DeckAnalyticsDto>, CommandError> {
    let (history_where, mut values) = history_where(scope);
    let now_index = push(&mut values, scope.context.now_utc_millis);
    let sql = format!(
        "WITH {signals},
         current_stats(deck_id, source_note_count, generated_card_count,
                       studyable_due_count, needs_more_practice) AS (
             SELECT c.deck_id, COUNT(DISTINCT c.note_id), COUNT(c.id),
                    COALESCE(SUM(CASE WHEN c.suspended_at IS NULL
                         AND (c.buried_until IS NULL OR c.buried_until <= ?{now_index})
                         AND s.due_at <= ?{now_index} THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN c.suspended_at IS NULL
                         AND (c.buried_until IS NULL OR c.buried_until <= ?{now_index})
                         THEN COALESCE(ls.needs_more_practice,0) ELSE 0 END), 0)
             FROM cards c JOIN notes n ON n.id=c.note_id
             JOIN decks cd ON cd.id=c.deck_id
             JOIN card_schedules s ON s.card_id=c.id
             LEFT JOIN learning_signals ls ON ls.card_id=c.id
             WHERE c.deleted_at IS NULL AND n.deleted_at IS NULL
               AND cd.deleted_at IS NULL AND cd.archived_at IS NULL
             GROUP BY c.deck_id
         ),
         history(deck_id, cards_practiced, review_count, correct_count,
                 average_confidence, average_response_time_ms,
                 total_study_time_ms, last_practiced_at) AS (
             SELECT a.deck_id, COUNT(DISTINCT r.card_id), COUNT(*),
                    COALESCE(SUM(r.answer_correct),0),
                    AVG(CASE r.confidence WHEN 'wrong' THEN 0.0
                        WHEN 'not_confident' THEN 1.0
                        WHEN 'somewhat_confident' THEN 2.0
                        WHEN 'very_confident' THEN 3.0 END),
                    AVG(r.response_time_ms), COALESCE(SUM(r.response_time_ms),0),
                    MAX(r.reviewed_at)
             FROM review_events r
             JOIN review_event_attributions a ON a.review_event_id=r.id
             JOIN decks hd ON hd.id=a.deck_id AND hd.deleted_at IS NULL
             WHERE {history_where}
             GROUP BY a.deck_id
         )
         SELECT d.id,d.name,d.archived_at,
                COALESCE(c.source_note_count,0),COALESCE(c.generated_card_count,0),
                COALESCE(c.studyable_due_count,0),COALESCE(c.needs_more_practice,0),
                COALESCE(h.cards_practiced,0),COALESCE(h.review_count,0),
                COALESCE(h.correct_count,0),h.average_confidence,
                h.average_response_time_ms,COALESCE(h.total_study_time_ms,0),
                h.last_practiced_at
         FROM decks d
         LEFT JOIN current_stats c ON c.deck_id=d.id
         LEFT JOIN history h ON h.deck_id=d.id
         WHERE d.deleted_at IS NULL
           AND (c.deck_id IS NOT NULL OR h.deck_id IS NOT NULL)
         ORDER BY COALESCE(h.last_practiced_at,0) DESC,d.name COLLATE NOCASE",
        signals=learning_signal_cte());
    let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
    let rows = statement.query_map(params_from_iter(values.iter()), |r| {
        let reviews: i64 = r.get(8)?;
        let correct: i64 = r.get(9)?;
        Ok(DeckAnalyticsDto {
            deck_id:r.get(0)?, deck_name:r.get(1)?,
            archived:r.get::<_,Option<i64>>(2)?.is_some(),
            source_note_count:r.get(3)?, generated_card_count:r.get(4)?,
            studyable_due_count:r.get(5)?, needs_more_practice:r.get(6)?,
            cards_practiced:r.get(7)?, review_count:reviews, correct_count:correct,
            correct_rate:ratio(correct,reviews), average_confidence:r.get(10)?,
            average_response_time_ms:r.get(11)?, total_study_time_ms:r.get(12)?,
            last_practiced_at:r.get(13)?,
        })
    }).map_err(CommandError::database)?
      .collect::<Result<Vec<_>,_>>().map_err(CommandError::database)?;
    Ok(rows)
}
'''
t=t[:start]+deck_fn+t[end:]
# Tag projection includes current inventory, current names, and adjacent-period trend.
start=t.index('fn tag_stats(');end=t.index('\nfn timeline(',start)
tag_fn=r'''fn tag_stats(
    connection: &Connection,
    scope: &Scope,
) -> Result<Vec<TagAnalyticsDto>, CommandError> {
    let (history_where, mut values) = history_where(scope);
    let current_start_index = scope.start_date.map(|date| {
        push(&mut values, date.format("%Y-%m-%d").to_string())
    });
    let previous_start_index = scope.previous_start.map(|date| {
        push(&mut values, date.format("%Y-%m-%d").to_string())
    });
    let now_index = push(&mut values, scope.context.now_utc_millis);
    let previous_condition = match (previous_start_index, current_start_index) {
        (Some(previous), Some(current)) => format!(
            "a.review_local_date >= ?{previous} AND a.review_local_date < ?{current}"
        ),
        _ => "0=1".to_string(),
    };
    let sql = format!(
        "WITH {signals},
         current_stats(tag_id, source_note_count, generated_card_count,
                       studyable_due_count, needs_more_practice) AS (
             SELECT nt.tag_id, COUNT(DISTINCT n.id), COUNT(DISTINCT c.id),
                    COALESCE(SUM(CASE WHEN c.suspended_at IS NULL
                         AND (c.buried_until IS NULL OR c.buried_until <= ?{now_index})
                         AND s.due_at <= ?{now_index} THEN 1 ELSE 0 END),0),
                    COALESCE(SUM(CASE WHEN c.suspended_at IS NULL
                         AND (c.buried_until IS NULL OR c.buried_until <= ?{now_index})
                         THEN COALESCE(ls.needs_more_practice,0) ELSE 0 END),0)
             FROM note_tags nt JOIN notes n ON n.id=nt.note_id
             JOIN cards c ON c.note_id=n.id JOIN decks d ON d.id=c.deck_id
             JOIN card_schedules s ON s.card_id=c.id
             LEFT JOIN learning_signals ls ON ls.card_id=c.id
             WHERE n.deleted_at IS NULL AND c.deleted_at IS NULL
               AND d.deleted_at IS NULL AND d.archived_at IS NULL
             GROUP BY nt.tag_id
         ),
         history(tag_id,captured_name,cards_practiced,review_count,correct_count,
                 average_confidence,average_response_time_ms,total_study_time_ms,
                 last_practiced_at) AS (
             SELECT ta.tag_id,MIN(ta.captured_name),COUNT(DISTINCT r.card_id),COUNT(*),
                    COALESCE(SUM(r.answer_correct),0),
                    AVG(CASE r.confidence WHEN 'wrong' THEN 0.0
                        WHEN 'not_confident' THEN 1.0
                        WHEN 'somewhat_confident' THEN 2.0
                        WHEN 'very_confident' THEN 3.0 END),
                    AVG(r.response_time_ms),COALESCE(SUM(r.response_time_ms),0),
                    MAX(r.reviewed_at)
             FROM review_events r
             JOIN review_event_attributions a ON a.review_event_id=r.id
             JOIN decks d ON d.id=a.deck_id AND d.deleted_at IS NULL
             JOIN review_event_tag_attributions ta ON ta.review_event_id=r.id
             WHERE {history_where}
             GROUP BY ta.tag_id
         ),
         previous(tag_id,correct_rate) AS (
             SELECT ta.tag_id,1.0*SUM(r.answer_correct)/NULLIF(COUNT(*),0)
             FROM review_events r
             JOIN review_event_attributions a ON a.review_event_id=r.id
             JOIN decks d ON d.id=a.deck_id AND d.deleted_at IS NULL
             JOIN review_event_tag_attributions ta ON ta.review_event_id=r.id
             WHERE {previous_condition}
             GROUP BY ta.tag_id
         ),
         dimensions(tag_id) AS (
             SELECT tag_id FROM current_stats UNION SELECT tag_id FROM history
         )
         SELECT dim.tag_id,COALESCE(t.name,h.captured_name),
                COALESCE(c.source_note_count,0),COALESCE(c.generated_card_count,0),
                COALESCE(c.studyable_due_count,0),COALESCE(c.needs_more_practice,0),
                COALESCE(h.cards_practiced,0),COALESCE(h.review_count,0),
                COALESCE(h.correct_count,0),h.average_confidence,
                h.average_response_time_ms,COALESCE(h.total_study_time_ms,0),
                h.last_practiced_at,p.correct_rate
         FROM dimensions dim LEFT JOIN tags t ON t.id=dim.tag_id
         LEFT JOIN current_stats c ON c.tag_id=dim.tag_id
         LEFT JOIN history h ON h.tag_id=dim.tag_id
         LEFT JOIN previous p ON p.tag_id=dim.tag_id
         ORDER BY COALESCE(h.last_practiced_at,0) DESC,
                  COALESCE(t.name,h.captured_name) COLLATE NOCASE",
        signals=learning_signal_cte());
    let mut statement=connection.prepare(&sql).map_err(CommandError::database)?;
    let rows=statement.query_map(params_from_iter(values.iter()),|r|{
        let reviews:i64=r.get(7)?;let correct:i64=r.get(8)?;let rate=ratio(correct,reviews);
        let previous:Option<f64>=r.get(13)?;
        let status=if reviews<3{"building_data"}else if rate.unwrap_or(0.0)<0.6{"needs_attention"}else if rate.unwrap_or(0.0)>=0.85{"strong"}else{"on_track"};
        Ok(TagAnalyticsDto{tag_id:r.get(0)?,tag_name:r.get(1)?,
            source_note_count:r.get(2)?,generated_card_count:r.get(3)?,
            studyable_due_count:r.get(4)?,needs_more_practice:r.get(5)?,
            cards_practiced:r.get(6)?,review_count:reviews,correct_count:correct,
            correct_rate:rate,average_confidence:r.get(9)?,
            average_response_time_ms:r.get(10)?,total_study_time_ms:r.get(11)?,
            last_practiced_at:r.get(12)?,
            trend_delta:match(rate,previous){(Some(current),Some(old))=>Some(current-old),_=>None},
            status:status.to_string()})
    }).map_err(CommandError::database)?
      .collect::<Result<Vec<_>,_>>().map_err(CommandError::database)?;
    Ok(rows)
}
'''
t=t[:start]+tag_fn+t[end:]
# Force query iterators to drop before statements in recent lists.
for fn_name in ['recent_reviews','recent_sessions']:
    s=t.index(f'fn {fn_name}(')
    e=t.find('\nfn ',s+4)
    if e<0:e=len(t)
    block=t[s:e]
    block=block.replace('    statement\n        .query_map(', '    let rows = statement\n        .query_map(',1)
    block=block.replace('        .map_err(CommandError::database)\n}', '        .map_err(CommandError::database)?;\n    Ok(rows)\n}',1)
    t=t[:s]+block+t[e:]
write(p,t)

# Borrowed query iterator lifetimes in archive/template projections.
p='src-tauri/src/decks.rs';t=read(p)
t=t.replace('''        statement
            .query_map([deck_id], |row| row.get::<_, String>(0))
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?''','''        let rows = statement
            .query_map([deck_id], |row| row.get::<_, String>(0))
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        rows''',1)
write(p,t)
write('src-tauri/src/template_projections.rs',r'''//! Bounded reusable-template library summaries.
use serde::Serialize;
use tauri::State;
use crate::{database::Database,error::CommandError};
#[derive(Clone,Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub struct CardTemplateSummaryDto{pub id:String,pub name:String,pub description:String,pub field_count:i64,pub generated_card_rule_count:i64,pub image_count:i64,pub updated_at:i64,pub revision:i64}
#[tauri::command]
pub fn list_card_template_summaries(database:State<'_,Database>)->Result<Vec<CardTemplateSummaryDto>,CommandError>{
    database.with_connection(|connection|{
        let mut statement=connection.prepare(
            "SELECT ct.id,ct.name,ct.description,
                    (SELECT COUNT(*) FROM card_template_fields f WHERE f.template_id=ct.id),
                    (SELECT COUNT(*) FROM card_template_rules r WHERE r.template_id=ct.id),
                    (SELECT COUNT(*) FROM card_template_assets a WHERE a.template_id=ct.id),
                    ct.updated_at,ct.revision
             FROM card_templates ct WHERE ct.deleted_at IS NULL
             ORDER BY ct.name COLLATE NOCASE,ct.created_at,ct.id"
        ).map_err(CommandError::database)?;
        let rows=statement.query_map([],|row|Ok(CardTemplateSummaryDto{
            id:row.get(0)?,name:row.get(1)?,description:row.get(2)?,field_count:row.get(3)?,
            generated_card_rule_count:row.get(4)?,image_count:row.get(5)?,updated_at:row.get(6)?,revision:row.get(7)?
        })).map_err(CommandError::database)?
          .collect::<Result<Vec<_>,_>>().map_err(CommandError::database)?;
        Ok(rows)
    })
}
''')

# Intent-based scheduling tests.
p='src-tauri/src/schedule_controls/extended_tests.rs';t=read(p)
t=t.replace('due_at: None,','reschedule: None,')
t=t.replace('''                let current_due = load_schedule(connection, card_id)?.due_at;
                let mut same_due = action_input(&card_ids, 8, "reschedule");
                same_due.due_at = Some(current_due);''','''                let tomorrow = StudyTimeContext::capture()?.tomorrow_start_utc_millis;
                connection.execute(
                    "UPDATE card_schedules SET due_at = ?1 WHERE card_id = ?2",
                    params![tomorrow, card_id],
                ).map_err(CommandError::database)?;
                let mut same_due = action_input(&card_ids, 8, "reschedule");
                same_due.reschedule = Some(RescheduleIntentInput::Tomorrow);''')
t=t.replace('invalid.due_at = Some(-1);','invalid.reschedule = Some(RescheduleIntentInput::CalendarDays { days: -1 });')
t=t.replace('unexpected.due_at = Some(1);','unexpected.reschedule = Some(RescheduleIntentInput::DueNow);')
write(p,t)
# Studydeck tests use explicit generated-card units.
p='src-tauri/src/studydeck/tests.rs';t=read(p).replace('.card_count','.generated_card_count');write(p,t)

print('Resolved native compile findings and named Analytics projections.')
