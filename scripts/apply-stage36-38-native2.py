#!/usr/bin/env python3
"""Apply bounded practice, card, template, and asset projections for Stages 3.7–3.8."""
from __future__ import annotations

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def write(path: str, content: str) -> None:
    target = ROOT / path
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(content, encoding="utf-8")


# A schema-14 repair must run after migration 14 creates its target tables.
database_path = "src-tauri/src/database.rs"
database = read(database_path)
database = database.replace(
    "    apply_migrations_after(connection, 12)?;\n    verify_repair_ledger(connection)?;",
    "    apply_migrations_after(connection, 12)?;\n    apply_required_repairs(connection)?;\n    verify_repair_ledger(connection)?;",
    1,
)
write(database_path, database)

# Batch asset hydration is shared by one-card detail and bounded practice windows.
assets_path = "src-tauri/src/assets.rs"
assets = read(assets_path)
assets = assets.replace(
    "use std::{fs::File, io::Read, path::Path};",
    "use std::{collections::BTreeMap, fs::File, io::Read, path::Path};",
    1,
)
assets = assets.replace(
    "use rusqlite::Connection;",
    "use rusqlite::{params_from_iter, types::Value as SqlValue, Connection};",
    1,
)
start = assets.index("pub(crate) fn get_assets_for_note(")
end = assets.index("\n/// Compatibility validation", start)
replacement = r'''pub(crate) fn get_assets_for_note(
    connection: &Connection,
    paths: &AppPaths,
    note_id: &str,
) -> Result<Vec<AssetDto>, CommandError> {
    Ok(get_assets_for_notes(connection, paths, &[note_id.to_string()])?
        .remove(note_id)
        .unwrap_or_default())
}

pub(crate) fn get_assets_for_notes(
    connection: &Connection,
    paths: &AppPaths,
    note_ids: &[String],
) -> Result<BTreeMap<String, Vec<AssetDto>>, CommandError> {
    let mut result = BTreeMap::<String, Vec<AssetDto>>::new();
    for chunk in note_ids.chunks(250) {
        let values = chunk.iter().cloned().map(SqlValue::Text).collect::<Vec<_>>();
        let placeholders = (1..=values.len()).map(|index| format!("?{index}"))
            .collect::<Vec<_>>().join(", ");
        let sql = format!(
            "SELECT DISTINCT na.note_id, a.id, a.original_filename, a.mime_type,
                    a.byte_size, a.relative_path, a.created_at
             FROM note_assets na JOIN assets a ON a.id=na.asset_id
             WHERE na.note_id IN ({placeholders})
               AND a.deleted_at IS NULL AND a.lifecycle_state='ready'
             ORDER BY na.note_id, a.created_at, a.id"
        );
        let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
        let rows = statement.query_map(params_from_iter(values.iter()), |row| {
            let relative: String = row.get(5)?;
            Ok((row.get::<_, String>(0)?, AssetDto {
                id: row.get(1)?, original_filename: row.get(2)?, mime_type: row.get(3)?,
                byte_size: row.get(4)?,
                file_path: paths.resolve_relative(&relative).to_string_lossy().into_owned(),
                created_at: row.get(6)?,
            }))
        }).map_err(CommandError::database)?;
        for row in rows {
            let (note_id, asset) = row.map_err(CommandError::database)?;
            result.entry(note_id).or_default().push(asset);
        }
    }
    Ok(result)
}

pub(crate) fn get_assets_by_ids(
    connection: &Connection,
    paths: &AppPaths,
    asset_ids: &[String],
) -> Result<BTreeMap<String, AssetDto>, CommandError> {
    let mut result = BTreeMap::new();
    for chunk in asset_ids.chunks(250) {
        let values = chunk.iter().cloned().map(SqlValue::Text).collect::<Vec<_>>();
        let placeholders = (1..=values.len()).map(|index| format!("?{index}"))
            .collect::<Vec<_>>().join(", ");
        let sql = format!(
            "SELECT id, original_filename, mime_type, byte_size, relative_path, created_at
             FROM assets WHERE id IN ({placeholders})
               AND deleted_at IS NULL AND lifecycle_state='ready'"
        );
        let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
        let rows = statement.query_map(params_from_iter(values.iter()), |row| {
            let relative: String = row.get(4)?;
            Ok(AssetDto { id: row.get(0)?, original_filename: row.get(1)?,
                mime_type: row.get(2)?, byte_size: row.get(3)?,
                file_path: paths.resolve_relative(&relative).to_string_lossy().into_owned(),
                created_at: row.get(5)? })
        }).map_err(CommandError::database)?;
        for row in rows { let asset=row.map_err(CommandError::database)?; result.insert(asset.id.clone(),asset); }
    }
    Ok(result)
}
'''
assets = assets[:start] + replacement + assets[end:]
write(assets_path, assets)

# Extend immutable session snapshots with exact rendered documents and asset IDs.
history_path = "src-tauri/src/historical_attribution.rs"
history = read(history_path)
history = history.replace(
    "    pub question_excerpt: String,",
    "    pub question_doc_json: String,\n    pub answer_doc_json: String,\n    pub answer_text: String,\n    pub asset_ids: Vec<String>,\n    pub question_excerpt: String,",
    1,
)
history = history.replace(
    "    let mut tags = load_tags_for_notes(",
    "    let mut asset_ids = load_asset_ids_for_notes(connection, &rows.values().map(|row| row.note_id.clone()).collect::<Vec<_>>())?;\n    let mut tags = load_tags_for_notes(",
    1,
)
history = history.replace(
    "                presentation_variant: variant,\n                question_excerpt,",
    "                presentation_variant: variant,\n                question_doc_json: serde_json::to_string(&rendered.front_doc).map_err(CommandError::serialization)?,\n                answer_doc_json: serde_json::to_string(&rendered.back_doc).map_err(CommandError::serialization)?,\n                answer_text: rendered.back_text,\n                asset_ids: asset_ids.remove(&row.note_id).unwrap_or_default(),\n                question_excerpt,",
    1,
)
history = history.replace(
    '''                     card_type, card_label, presentation_variant,
                     question_excerpt, question_text_hash, question_was_truncated,
                     captured_at
                 ) VALUES (
                     ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12
                 )",''',
    '''                     card_type, card_label, presentation_variant,
                     question_doc, answer_doc, answer_text,
                     question_excerpt, question_text_hash, question_was_truncated,
                     captured_at
                 ) VALUES (
                     ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15
                 )",''',
    1,
)
history = history.replace(
    '''                    attribution.presentation_variant,
                    attribution.question_excerpt,
                    attribution.question_text_hash,
                    bool_to_integer(attribution.question_was_truncated),
                    captured_at,
                ],''',
    '''                    attribution.presentation_variant,
                    attribution.question_doc_json,
                    attribution.answer_doc_json,
                    attribution.answer_text,
                    attribution.question_excerpt,
                    attribution.question_text_hash,
                    bool_to_integer(attribution.question_was_truncated),
                    captured_at,
                ],''',
    1,
)
history = history.replace(
    "        for tag in attribution.tags {",
    '''        for asset_id in &attribution.asset_ids {
            connection.execute(
                "INSERT INTO practice_session_card_asset_snapshots
                 (session_id, card_id, asset_id) VALUES (?1, ?2, ?3)",
                params![session_id, attribution.card_id, asset_id],
            ).map_err(CommandError::database)?;
        }
        for tag in attribution.tags {''',
    1,
)
marker = "fn load_tags_for_notes("
if "fn load_asset_ids_for_notes(" not in history:
    helper = r'''fn load_asset_ids_for_notes(
    connection: &Connection,
    note_ids: &[String],
) -> Result<BTreeMap<String, Vec<String>>, CommandError> {
    let mut result = BTreeMap::<String, Vec<String>>::new();
    for chunk in note_ids.chunks(ID_CHUNK_SIZE) {
        let values = chunk.iter().cloned().map(SqlValue::Text).collect::<Vec<_>>();
        let placeholders = (1..=values.len()).map(|index| format!("?{index}"))
            .collect::<Vec<_>>().join(", ");
        let sql = format!(
            "SELECT note_id, asset_id FROM note_assets
             WHERE note_id IN ({placeholders}) ORDER BY note_id, asset_id"
        );
        let mut statement=connection.prepare(&sql).map_err(CommandError::database)?;
        let rows=statement.query_map(params_from_iter(values.iter()),|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?)))
            .map_err(CommandError::database)?;
        for row in rows { let (note,asset)=row.map_err(CommandError::database)?; result.entry(note).or_default().push(asset); }
    }
    Ok(result)
}

'''
    history = history.replace(marker, helper + marker, 1)
write(history_path, history)

# Windowed practice DTOs.
practice_path = "src-tauri/src/practice.rs"
practice = read(practice_path)
old = '''#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PracticeSessionDto {
    pub id: String,
    pub cards: Vec<PracticeCardDto>,
    pub shuffle: bool,
    pub mode: String,
    pub started_at: i64,
}
'''
new = '''#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PracticeSessionDto {
    pub id: String,
    pub cards: Vec<PracticeCardDto>,
    pub card_count: i64,
    pub has_more: bool,
    pub shuffle: bool,
    pub mode: String,
    pub started_at: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PracticeCardWindowDto {
    pub start_position: i64,
    pub cards: Vec<PracticeCardDto>,
    pub has_more: bool,
}
'''
if old not in practice:
    raise SystemExit("practice DTO anchor changed")
practice = practice.replace(old, new, 1)
write(practice_path, practice)

# Replace practice selection with bounded validation and 50-card windows.
write("src-tauri/src/practice_selection.rs", r'''//! Bounded scheduled, extra, custom, and tag-scoped practice-session selection.

use std::{collections::{BTreeMap, BTreeSet, HashMap, HashSet}, hash::{Hash, Hasher}};

use rusqlite::{params, params_from_iter, types::Value as SqlValue, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::State;

use crate::{
    assets::{get_assets_by_ids, AssetDto},
    authoring::presentation_variant_for_projected,
    cards::CardDto,
    database::Database,
    error::CommandError,
    historical_attribution::{insert_session_snapshots, prepare_attributions},
    paths::AppPaths,
    practice::{PracticeCardDto, PracticeCardWindowDto, PracticeSessionDto},
    settings::load_settings,
    study_calendar::StudyTimeContext,
    studyability::learning_signal_cte,
    util::{bool_to_integer, generate_id, validate_entity_id},
    workload::{planned_extra_cards_scoped, planned_scheduled_cards_scoped, workload_overview_scoped, PlannedCard, WorkloadOverviewDto},
};

const MAX_SESSION_DECKS: usize = 100;
const MAX_SESSION_TAGS: usize = 32;
const MAX_SESSION_CARDS: usize = 10_000;
const PRACTICE_WINDOW_SIZE: i64 = 50;
const MAX_WINDOW_SIZE: i64 = 100;

type PendingCard = PlannedCard;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct PracticeWorkloadInput { pub deck_ids: Vec<String>, #[serde(default)] pub tag_ids: Vec<String> }
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct StartPracticeInput { pub operation_id: String, pub deck_ids: Vec<String>, #[serde(default)] pub tag_ids: Vec<String>, pub shuffle: bool, pub mode: String }
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct StartCardPracticeInput { pub operation_id: String, pub card_ids: Vec<String>, pub shuffle: bool }
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct PracticeWindowInput { pub session_id: String, pub start_position: i64, #[serde(default = "default_window_size")] pub limit: i64 }
fn default_window_size() -> i64 { PRACTICE_WINDOW_SIZE }

#[derive(Serialize)]
#[serde(rename_all="camelCase")]
struct SelectedIdentity<'a>{deck_ids:&'a[String],tag_ids:&'a[String],shuffle:bool,mode:&'a str}
#[derive(Serialize)]
#[serde(rename_all="camelCase")]
struct CardIdentity<'a>{card_ids:&'a[String],shuffle:bool}

#[tauri::command]
pub fn get_practice_workload(database:State<'_,Database>,input:PracticeWorkloadInput)->Result<WorkloadOverviewDto,CommandError>{
    let decks=validate_ids(input.deck_ids,MAX_SESSION_DECKS,"deck")?;let tags=validate_ids(input.tag_ids,MAX_SESSION_TAGS,"tag")?;
    database.with_connection(|c|{let settings=load_settings(c)?;let context=StudyTimeContext::capture()?;workload_overview_scoped(c,&decks,&tags,&settings,&context)})
}
#[tauri::command]
pub fn start_selected_practice(database:State<'_,Database>,paths:State<'_,AppPaths>,input:StartPracticeInput)->Result<PracticeSessionDto,CommandError>{
    validate_entity_id(&input.operation_id,"practice start operation")?;let decks=validate_ids(input.deck_ids,MAX_SESSION_DECKS,"deck")?;let tags=validate_ids(input.tag_ids,MAX_SESSION_TAGS,"tag")?;
    if decks.is_empty()&&tags.is_empty(){return Err(CommandError::new("no_practice_scope","Choose at least one deck or tag."));}
    if !matches!(input.mode.as_str(),"scheduled"|"extra"){return Err(CommandError::new("invalid_practice_mode","Choose scheduled practice or an explicit extra session."));}
    database.with_connection(|c|start_selected_practice_in_connection(c,&paths,input.operation_id,decks,tags,input.shuffle,&input.mode))
}
#[tauri::command]
pub fn start_card_practice(database:State<'_,Database>,paths:State<'_,AppPaths>,input:StartCardPracticeInput)->Result<PracticeSessionDto,CommandError>{
    validate_entity_id(&input.operation_id,"practice start operation")?;let cards=validate_ids(input.card_ids,MAX_SESSION_CARDS,"card")?;
    if cards.is_empty(){return Err(CommandError::new("no_cards_selected","Select at least one card."));}
    database.with_connection(|c|start_card_practice_in_connection(c,&paths,input.operation_id,cards,input.shuffle))
}
#[tauri::command]
pub fn get_practice_card_window(database:State<'_,Database>,paths:State<'_,AppPaths>,input:PracticeWindowInput)->Result<PracticeCardWindowDto,CommandError>{
    validate_entity_id(&input.session_id,"practice session")?;if input.start_position<0||!(1..=MAX_WINDOW_SIZE).contains(&input.limit){return Err(CommandError::new("invalid_practice_window","The requested practice-card window is invalid."));}
    database.with_connection(|c|load_practice_card_window(c,&paths,&input.session_id,input.start_position,input.limit))
}

pub(crate) fn start_selected_practice_in_connection(connection:&mut Connection,paths:&AppPaths,operation_id:String,deck_ids:Vec<String>,tag_ids:Vec<String>,shuffle:bool,mode:&str)->Result<PracticeSessionDto,CommandError>{
    let fingerprint=selected_fingerprint(&deck_ids,&tag_ids,shuffle,mode)?;if let Some(existing)=session_for_operation(connection,paths,&operation_id,&fingerprint)?{return Ok(existing);}
    let context=StudyTimeContext::capture()?;let settings=load_settings(connection)?;
    let planned=if mode=="scheduled"{planned_scheduled_cards_scoped(connection,&deck_ids,&tag_ids,&settings,&context)?.0}else{planned_extra_cards_scoped(connection,&deck_ids,&tag_ids,&context)?};
    if planned.is_empty(){return Err(CommandError::new("no_cards_available",if mode=="scheduled"{"No cards fit this scope's remaining scheduled workload."}else{"This scope has no studyable cards available for extra practice."}));}
    create_session(connection,paths,operation_id,fingerprint,planned,shuffle,mode,&context)
}

pub(crate) fn start_card_practice_in_connection(connection:&mut Connection,paths:&AppPaths,operation_id:String,card_ids:Vec<String>,shuffle:bool)->Result<PracticeSessionDto,CommandError>{
    let fingerprint=card_fingerprint(&card_ids,shuffle)?;if let Some(existing)=session_for_operation(connection,paths,&operation_id,&fingerprint)?{return Ok(existing);}
    let context=StudyTimeContext::capture()?;let planned=load_custom_cards(connection,&card_ids,&context)?;
    create_session(connection,paths,operation_id,fingerprint,planned,shuffle,"custom",&context)
}

fn create_session(connection:&mut Connection,paths:&AppPaths,operation_id:String,fingerprint:String,mut cards:Vec<PendingCard>,shuffle:bool,mode:&str,context:&StudyTimeContext)->Result<PracticeSessionDto,CommandError>{
    if cards.is_empty()||cards.len()>MAX_SESSION_CARDS{return Err(CommandError::new("too_many_practice_cards",format!("A practice session must contain 1 to {MAX_SESSION_CARDS} cards.")));}
    let session_id=generate_id(connection)?;
    if shuffle{cards.sort_by_key(|card|(context.local_day_sort_key(card.due_at).unwrap_or(i32::MAX),!card.needs_more_practice,card.state==0,shuffle_key(&session_id,&card.card_id)));}
    let attribution_input=cards.iter().map(|c|(c.card_id.clone(),c.presentation_variant.clone())).collect::<Vec<_>>();
    let attributions=prepare_attributions(connection,&attribution_input)?;
    let selected_decks=cards.iter().map(|c|c.deck_id.clone()).collect::<BTreeSet<_>>().into_iter().collect::<Vec<_>>();
    let selected_json=serde_json::to_string(&selected_decks).map_err(CommandError::serialization)?;
    let transaction=connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(CommandError::database)?;
    transaction.execute("INSERT INTO practice_sessions(id,selected_deck_ids_json,shuffle,card_count,mode,started_at,start_operation_id,start_request_fingerprint) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",params![session_id,selected_json,bool_to_integer(shuffle),i64::try_from(cards.len()).unwrap_or(i64::MAX),mode,context.now_utc_millis,operation_id,fingerprint]).map_err(CommandError::database)?;
    for (position,card) in cards.iter().enumerate(){transaction.execute("INSERT INTO practice_session_cards(session_id,card_id,position,presentation_variant) VALUES(?1,?2,?3,?4)",params![session_id,card.card_id,i64::try_from(position).unwrap_or(i64::MAX),card.presentation_variant]).map_err(CommandError::database)?;}
    insert_session_snapshots(&transaction,&session_id,attributions.into_values(),context.now_utc_millis)?;
    transaction.commit().map_err(CommandError::database)?;
    load_session(connection,paths,&session_id)
}

fn load_session(connection:&Connection,paths:&AppPaths,session_id:&str)->Result<PracticeSessionDto,CommandError>{
    let metadata:(i64,String,i64,i64)=connection.query_row("SELECT shuffle,mode,started_at,card_count FROM practice_sessions WHERE id=?1",[session_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional().map_err(CommandError::database)?.ok_or_else(||CommandError::new("practice_session_not_found","That practice session no longer exists."))?;
    let window=load_practice_card_window(connection,paths,session_id,0,PRACTICE_WINDOW_SIZE)?;
    Ok(PracticeSessionDto{id:session_id.to_string(),cards:window.cards,card_count:metadata.3,has_more:window.has_more,shuffle:metadata.0!=0,mode:metadata.1,started_at:metadata.2})
}

fn load_practice_card_window(connection:&Connection,paths:&AppPaths,session_id:&str,start:i64,limit:i64)->Result<PracticeCardWindowDto,CommandError>{
    let total:i64=connection.query_row("SELECT card_count FROM practice_sessions WHERE id=?1",[session_id],|r|r.get(0)).optional().map_err(CommandError::database)?.ok_or_else(||CommandError::new("practice_session_not_found","That practice session no longer exists."))?;
    let sql=format!("WITH {signals} SELECT psc.card_id,c.note_id,snap.deck_id,d.name,s.due_at,s.state,COALESCE(ls.needs_more_practice,0),c.direction,n.created_at,n.updated_at,snap.note_revision,snap.question_doc,snap.answer_doc,snap.question_excerpt,snap.answer_text FROM practice_session_cards psc JOIN practice_session_card_snapshots snap ON snap.session_id=psc.session_id AND snap.card_id=psc.card_id JOIN cards c ON c.id=psc.card_id JOIN notes n ON n.id=c.note_id JOIN decks d ON d.id=snap.deck_id AND d.deleted_at IS NULL JOIN card_schedules s ON s.card_id=c.id LEFT JOIN learning_signals ls ON ls.card_id=c.id WHERE psc.session_id=?1 ORDER BY psc.position LIMIT ?2 OFFSET ?3",signals=learning_signal_cte());
    let mut statement=connection.prepare(&sql).map_err(CommandError::database)?;
    let rows=statement.query_map(params![session_id,limit,start],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,i64>(4)?,r.get::<_,i64>(5)?,r.get::<_,i64>(6)?,r.get::<_,String>(7)?,r.get::<_,i64>(8)?,r.get::<_,i64>(9)?,r.get::<_,i64>(10)?,r.get::<_,String>(11)?,r.get::<_,String>(12)?,r.get::<_,String>(13)?,r.get::<_,String>(14)?))).map_err(CommandError::database)?.collect::<Result<Vec<_>,_>>().map_err(CommandError::database)?;
    let card_ids=rows.iter().map(|r|r.0.clone()).collect::<Vec<_>>();let asset_ids_by_card=load_window_asset_ids(connection,session_id,&card_ids)?;let all_asset_ids=asset_ids_by_card.values().flatten().cloned().collect::<BTreeSet<_>>().into_iter().collect::<Vec<_>>();let assets=get_assets_by_ids(connection,paths,&all_asset_ids)?;
    let cards=rows.into_iter().map(|(card_id,note_id,deck_id,deck_name,due,state,weak,direction,created,updated,revision,qdoc,adoc,qtext,atext)|{
        let card_assets=asset_ids_by_card.get(&card_id).into_iter().flatten().filter_map(|id|assets.get(id).cloned()).collect::<Vec<AssetDto>>();
        let front_doc:Value=serde_json::from_str(&qdoc).map_err(CommandError::serialization)?;let back_doc:Value=serde_json::from_str(&adoc).map_err(CommandError::serialization)?;
        Ok(PracticeCardDto{card:CardDto{id:card_id,note_id,deck_id,direction,front_doc,back_doc,front_text:qtext,back_text:atext,assets:card_assets,created_at:created,updated_at:updated,revision},deck_name,due_at:due,schedule_state:state,needs_more_practice:weak!=0})
    }).collect::<Result<Vec<_>,CommandError>>()?;
    Ok(PracticeCardWindowDto{start_position:start,has_more:start.saturating_add(i64::try_from(cards.len()).unwrap_or(i64::MAX))<total,cards})
}

fn load_window_asset_ids(connection:&Connection,session_id:&str,card_ids:&[String])->Result<BTreeMap<String,Vec<String>>,CommandError>{let mut map=BTreeMap::new();if card_ids.is_empty(){return Ok(map);}let values=card_ids.iter().cloned().map(SqlValue::Text).collect::<Vec<_>>();let p=(2..=values.len()+1).map(|i|format!("?{i}")).collect::<Vec<_>>().join(",");let sql=format!("SELECT card_id,asset_id FROM practice_session_card_asset_snapshots WHERE session_id=?1 AND card_id IN ({p}) ORDER BY card_id,asset_id");let mut params=vec![SqlValue::Text(session_id.to_string())];params.extend(values);let mut st=connection.prepare(&sql).map_err(CommandError::database)?;let rows=st.query_map(params_from_iter(params.iter()),|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).map_err(CommandError::database)?;for row in rows{let(c,a)=row.map_err(CommandError::database)?;map.entry(c).or_default().push(a);}Ok(map)}

fn load_custom_cards(connection:&Connection,ids:&[String],context:&StudyTimeContext)->Result<Vec<PlannedCard>,CommandError>{let mut result=BTreeMap::new();for chunk in ids.chunks(250){let mut values=vec![SqlValue::Integer(context.now_utc_millis)];let p=chunk.iter().map(|id|{values.push(SqlValue::Text(id.clone()));format!("?{}",values.len())}).collect::<Vec<_>>().join(",");let sql=format!("WITH {signals} SELECT c.deck_id,c.id,c.note_id,s.due_at,s.state,COALESCE(ls.needs_more_practice,0),n.note_type,n.cloze_source,n.cloze_mode,s.reps FROM cards c JOIN notes n ON n.id=c.note_id JOIN decks d ON d.id=c.deck_id JOIN card_schedules s ON s.card_id=c.id LEFT JOIN learning_signals ls ON ls.card_id=c.id WHERE c.id IN ({p}) AND c.deleted_at IS NULL AND n.deleted_at IS NULL AND d.deleted_at IS NULL AND d.archived_at IS NULL AND c.suspended_at IS NULL AND (c.buried_until IS NULL OR c.buried_until<=?1)",signals=learning_signal_cte());let mut st=connection.prepare(&sql).map_err(CommandError::database)?;let rows=st.query_map(params_from_iter(values.iter()),|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,i64>(3)?,r.get::<_,i64>(4)?,r.get::<_,i64>(5)?,r.get::<_,String>(6)?,r.get::<_,String>(7)?,r.get::<_,String>(8)?,r.get::<_,i64>(9)?))).map_err(CommandError::database)?;for row in rows{let(d,c,n,due,state,weak,typ,source,mode,reps)=row.map_err(CommandError::database)?;let variant=presentation_variant_for_projected(&typ,&source,&mode,reps)?;result.insert(c.clone(),PlannedCard{deck_id:d,card_id:c,note_id:n,due_at:due,state,needs_more_practice:weak!=0,presentation_variant:variant});}}
    if result.len()!=ids.len(){return Err(CommandError::new("practice_card_not_found","A selected card is suspended, buried, archived, deleted, or otherwise unavailable."));}Ok(ids.iter().filter_map(|id|result.remove(id)).collect())}

fn session_for_operation(connection:&Connection,paths:&AppPaths,operation:&str,fingerprint:&str)->Result<Option<PracticeSessionDto>,CommandError>{let existing:Option<(String,Option<String>)>=connection.query_row("SELECT id,start_request_fingerprint FROM practice_sessions WHERE start_operation_id=?1",[operation],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(CommandError::database)?;let Some((id,stored))=existing else{return Ok(None)};if stored.as_deref()!=Some(fingerprint){return Err(CommandError::new("idempotency_conflict","That practice retry identifier was already used for a different request."));}load_session(connection,paths,&id).map(Some)}
fn selected_fingerprint(decks:&[String],tags:&[String],shuffle:bool,mode:&str)->Result<String,CommandError>{let json=serde_json::to_string(&SelectedIdentity{deck_ids:decks,tag_ids:tags,shuffle,mode}).map_err(CommandError::serialization)?;Ok(format!("practice-selected:v2:{json}"))}
fn card_fingerprint(cards:&[String],shuffle:bool)->Result<String,CommandError>{let json=serde_json::to_string(&CardIdentity{card_ids:cards,shuffle}).map_err(CommandError::serialization)?;Ok(format!("practice-cards:v2:{json}"))}
fn validate_ids(values:Vec<String>,max:usize,label:&str)->Result<Vec<String>,CommandError>{let mut unique=BTreeSet::new();for value in values{validate_entity_id(&value,label)?;unique.insert(value);}if unique.len()>max{return Err(CommandError::new("practice_scope_too_large",format!("Select at most {max} {label} values.")));}Ok(unique.into_iter().collect())}
fn shuffle_key(session_id:&str,card_id:&str)->u64{let mut h=std::collections::hash_map::DefaultHasher::new();session_id.hash(&mut h);card_id.hash(&mut h);h.finish()}

#[cfg(test)]
mod tests { use super::*; #[test] fn window_is_bounded(){assert_eq!(PRACTICE_WINDOW_SIZE,50);assert!(MAX_WINDOW_SIZE<MAX_SESSION_CARDS as i64);} }
''')

# Paged deck-card summaries; full rich documents stay in the one-card detail command.
write("src-tauri/src/card_projections.rs", r'''//! Bounded deck-card list projections.
use rusqlite::{params_from_iter, types::Value as SqlValue};
use serde::{Deserialize,Serialize};
use tauri::State;
use crate::{database::Database,error::CommandError,util::validate_entity_id};
const MAX_PAGE:i64=100;
#[derive(Debug,Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]pub struct DeckCardPageInput{pub deck_id:String,#[serde(default)]pub query:String,#[serde(default="default_limit")]pub limit:i64,#[serde(default)]pub offset:i64}
fn default_limit()->i64{50}
#[derive(Clone,Debug,Serialize)]#[serde(rename_all="camelCase")]pub struct DeckCardSummaryDto{pub note_id:String,pub primary_card_id:String,pub question_excerpt:String,pub answer_excerpt:String,pub generated_card_count:i64,pub image_count:i64,pub updated_at:i64}
#[derive(Clone,Debug,Serialize)]#[serde(rename_all="camelCase")]pub struct DeckCardPageDto{pub cards:Vec<DeckCardSummaryDto>,pub total_count:i64}
#[tauri::command]pub fn list_deck_cards(database:State<'_,Database>,input:DeckCardPageInput)->Result<DeckCardPageDto,CommandError>{validate_entity_id(&input.deck_id,"deck")?;if !(1..=MAX_PAGE).contains(&input.limit)||input.offset<0||input.query.chars().count()>500{return Err(CommandError::new("invalid_card_page","The requested deck-card page is invalid."));}database.with_connection(|c|{let mut values=vec![SqlValue::Text(input.deck_id.clone())];let mut filter="".to_string();if !input.query.trim().is_empty(){values.push(SqlValue::Text(format!("%{}%",input.query.trim().replace('%',"\\%").replace('_',"\\_"))));filter=format!(" AND (n.front_text LIKE ?{} ESCAPE '\\' OR n.back_text LIKE ?{} ESCAPE '\\')",values.len(),values.len());}let count_sql=format!("SELECT COUNT(*) FROM notes n JOIN decks d ON d.id=n.deck_id WHERE n.deck_id=?1 AND n.deleted_at IS NULL AND d.deleted_at IS NULL AND d.archived_at IS NULL{filter}");let total=c.query_row(&count_sql,params_from_iter(values.iter()),|r|r.get(0)).map_err(CommandError::database)?;values.push(SqlValue::Integer(input.limit));let li=values.len();values.push(SqlValue::Integer(input.offset));let oi=values.len();let sql=format!("SELECT n.id,(SELECT id FROM cards pc WHERE pc.note_id=n.id AND pc.deleted_at IS NULL ORDER BY CASE pc.direction WHEN 'forward' THEN 0 ELSE 1 END,pc.template_ordinal,pc.id LIMIT 1),substr(n.front_text,1,300),substr(n.back_text,1,300),(SELECT COUNT(*) FROM cards gc WHERE gc.note_id=n.id AND gc.deleted_at IS NULL),(SELECT COUNT(DISTINCT na.asset_id) FROM note_assets na WHERE na.note_id=n.id),n.updated_at FROM notes n JOIN decks d ON d.id=n.deck_id WHERE n.deck_id=?1 AND n.deleted_at IS NULL AND d.deleted_at IS NULL AND d.archived_at IS NULL{filter} ORDER BY n.updated_at DESC,n.created_at DESC,n.id LIMIT ?{li} OFFSET ?{oi}");let mut st=c.prepare(&sql).map_err(CommandError::database)?;let cards=st.query_map(params_from_iter(values.iter()),|r|Ok(DeckCardSummaryDto{note_id:r.get(0)?,primary_card_id:r.get(1)?,question_excerpt:r.get(2)?,answer_excerpt:r.get(3)?,generated_card_count:r.get(4)?,image_count:r.get(5)?,updated_at:r.get(6)?})).map_err(CommandError::database)?.collect::<Result<Vec<_>,_>>().map_err(CommandError::database)?;Ok(DeckCardPageDto{cards,total_count:total})})}
''')

write("src-tauri/src/template_projections.rs", r'''//! Bounded reusable-template library summaries.
use serde::Serialize;use tauri::State;use crate::{database::Database,error::CommandError};
#[derive(Clone,Debug,Serialize)]#[serde(rename_all="camelCase")]pub struct CardTemplateSummaryDto{pub id:String,pub name:String,pub description:String,pub field_count:i64,pub generated_card_rule_count:i64,pub image_count:i64,pub updated_at:i64,pub revision:i64}
#[tauri::command]pub fn list_card_template_summaries(database:State<'_,Database>)->Result<Vec<CardTemplateSummaryDto>,CommandError>{database.with_connection(|c|{let mut st=c.prepare("SELECT ct.id,ct.name,ct.description,(SELECT COUNT(*) FROM card_template_fields f WHERE f.template_id=ct.id),(SELECT COUNT(*) FROM card_template_rules r WHERE r.template_id=ct.id),(SELECT COUNT(*) FROM card_template_assets a WHERE a.template_id=ct.id),ct.updated_at,ct.revision FROM card_templates ct WHERE ct.deleted_at IS NULL ORDER BY ct.name COLLATE NOCASE,ct.created_at,ct.id").map_err(CommandError::database)?;st.query_map([],|r|Ok(CardTemplateSummaryDto{id:r.get(0)?,name:r.get(1)?,description:r.get(2)?,field_count:r.get(3)?,generated_card_rule_count:r.get(4)?,image_count:r.get(5)?,updated_at:r.get(6)?,revision:r.get(7)?})).map_err(CommandError::database)?.collect::<Result<Vec<_>,_>>().map_err(CommandError::database)})}
''')

# Register bounded commands and stop exposing unbounded full-list commands.
lib_path="src-tauri/src/lib.rs";lib=read(lib_path)
for module in ["card_projections","template_projections"]:
    if f"mod {module};" not in lib: lib=lib.replace("mod cards;",f"mod cards;\nmod {module};",1)
lib=lib.replace("            cards::list_cards,\n","            card_projections::list_deck_cards,\n")
if "practice_selection::get_practice_card_window," not in lib:lib=lib.replace("            practice_selection::get_practice_workload,","            practice_selection::get_practice_workload,\n            practice_selection::get_practice_card_window,",1)
lib=lib.replace("            authoring::list_card_templates,\n","            template_projections::list_card_template_summaries,\n")
write(lib_path,lib)

# Stage 3.4 verifier accepts later forward schemas while retaining schema-13 cutover checks.
verify_path="scripts/verify-stage34-35.mjs";verify=read(verify_path)
verify=verify.replace('  "LATEST_SCHEMA_VERSION: i64 = 13",','  "LATEST_SCHEMA_VERSION: i64 = 14",')
write(verify_path,verify)

print("Applied bounded practice windows, asset batches, deck-card pages, and template summaries.")
