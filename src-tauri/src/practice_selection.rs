//! Bounded scheduled, extra, custom, and tag-scoped practice-session selection.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    hash::{Hash, Hasher},
};

use rusqlite::{
    params, params_from_iter, types::Value as SqlValue, Connection, OptionalExtension,
    TransactionBehavior,
};
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
    workload::{
        planned_extra_cards_scoped, planned_scheduled_cards_scoped, workload_overview_scoped,
        PlannedCard, WorkloadOverviewDto,
    },
};

const MAX_SESSION_DECKS: usize = 100;
const MAX_SESSION_TAGS: usize = 32;
const MAX_SESSION_CARDS: usize = 10_000;
const PRACTICE_WINDOW_SIZE: i64 = 50;
const MAX_WINDOW_SIZE: i64 = 100;

type PendingCard = PlannedCard;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct PracticeWorkloadInput {
    pub deck_ids: Vec<String>,
    #[serde(default)]
    pub tag_ids: Vec<String>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct StartPracticeInput {
    pub operation_id: String,
    pub deck_ids: Vec<String>,
    #[serde(default)]
    pub tag_ids: Vec<String>,
    pub shuffle: bool,
    pub mode: String,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct StartCardPracticeInput {
    pub operation_id: String,
    pub card_ids: Vec<String>,
    pub shuffle: bool,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct PracticeWindowInput {
    pub session_id: String,
    pub start_position: i64,
    #[serde(default = "default_window_size")]
    pub limit: i64,
}
fn default_window_size() -> i64 {
    PRACTICE_WINDOW_SIZE
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SelectedIdentity<'a> {
    deck_ids: &'a [String],
    tag_ids: &'a [String],
    shuffle: bool,
    mode: &'a str,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CardIdentity<'a> {
    card_ids: &'a [String],
    shuffle: bool,
}

#[tauri::command]
pub fn get_practice_workload(
    database: State<'_, Database>,
    input: PracticeWorkloadInput,
) -> Result<WorkloadOverviewDto, CommandError> {
    let decks = validate_ids(input.deck_ids, MAX_SESSION_DECKS, "deck")?;
    let tags = validate_ids(input.tag_ids, MAX_SESSION_TAGS, "tag")?;
    database.with_connection(|c| {
        let settings = load_settings(c)?;
        let context = StudyTimeContext::capture()?;
        workload_overview_scoped(c, &decks, &tags, &settings, &context)
    })
}
#[tauri::command]
pub fn start_selected_practice(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: StartPracticeInput,
) -> Result<PracticeSessionDto, CommandError> {
    validate_entity_id(&input.operation_id, "practice start operation")?;
    let decks = validate_ids(input.deck_ids, MAX_SESSION_DECKS, "deck")?;
    let tags = validate_ids(input.tag_ids, MAX_SESSION_TAGS, "tag")?;
    if decks.is_empty() && tags.is_empty() {
        return Err(CommandError::new(
            "no_practice_scope",
            "Choose at least one deck or tag.",
        ));
    }
    if !matches!(input.mode.as_str(), "scheduled" | "extra") {
        return Err(CommandError::new(
            "invalid_practice_mode",
            "Choose scheduled practice or an explicit extra session.",
        ));
    }
    database.with_connection(|c| {
        start_selected_practice_in_connection(
            c,
            &paths,
            input.operation_id,
            decks,
            tags,
            input.shuffle,
            &input.mode,
        )
    })
}
#[tauri::command]
pub fn start_card_practice(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: StartCardPracticeInput,
) -> Result<PracticeSessionDto, CommandError> {
    validate_entity_id(&input.operation_id, "practice start operation")?;
    let cards = validate_ids(input.card_ids, MAX_SESSION_CARDS, "card")?;
    if cards.is_empty() {
        return Err(CommandError::new(
            "no_cards_selected",
            "Select at least one card.",
        ));
    }
    database.with_connection(|c| {
        start_card_practice_in_connection(c, &paths, input.operation_id, cards, input.shuffle)
    })
}
#[tauri::command]
pub fn get_practice_card_window(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: PracticeWindowInput,
) -> Result<PracticeCardWindowDto, CommandError> {
    validate_entity_id(&input.session_id, "practice session")?;
    if input.start_position < 0 || !(1..=MAX_WINDOW_SIZE).contains(&input.limit) {
        return Err(CommandError::new(
            "invalid_practice_window",
            "The requested practice-card window is invalid.",
        ));
    }
    database.with_connection(|c| {
        load_practice_card_window(
            c,
            &paths,
            &input.session_id,
            input.start_position,
            input.limit,
        )
    })
}

pub(crate) fn start_selected_practice_in_connection(
    connection: &mut Connection,
    paths: &AppPaths,
    operation_id: String,
    deck_ids: Vec<String>,
    tag_ids: Vec<String>,
    shuffle: bool,
    mode: &str,
) -> Result<PracticeSessionDto, CommandError> {
    let fingerprint = selected_fingerprint(&deck_ids, &tag_ids, shuffle, mode)?;
    if let Some(existing) = session_for_operation(connection, paths, &operation_id, &fingerprint)? {
        return Ok(existing);
    }
    let context = StudyTimeContext::capture()?;
    let settings = load_settings(connection)?;
    let planned = if mode == "scheduled" {
        planned_scheduled_cards_scoped(connection, &deck_ids, &tag_ids, &settings, &context)?.0
    } else {
        planned_extra_cards_scoped(connection, &deck_ids, &tag_ids, &context)?
    };
    if planned.is_empty() {
        return Err(CommandError::new(
            "no_cards_available",
            if mode == "scheduled" {
                "No cards fit this scope's remaining scheduled workload."
            } else {
                "This scope has no studyable cards available for extra practice."
            },
        ));
    }
    create_session(
        connection,
        paths,
        operation_id,
        fingerprint,
        planned,
        shuffle,
        mode,
        &context,
    )
}

pub(crate) fn start_card_practice_in_connection(
    connection: &mut Connection,
    paths: &AppPaths,
    operation_id: String,
    card_ids: Vec<String>,
    shuffle: bool,
) -> Result<PracticeSessionDto, CommandError> {
    let fingerprint = card_fingerprint(&card_ids, shuffle)?;
    if let Some(existing) = session_for_operation(connection, paths, &operation_id, &fingerprint)? {
        return Ok(existing);
    }
    let context = StudyTimeContext::capture()?;
    let planned = load_custom_cards(connection, &card_ids, &context)?;
    create_session(
        connection,
        paths,
        operation_id,
        fingerprint,
        planned,
        shuffle,
        "custom",
        &context,
    )
}

fn create_session(
    connection: &mut Connection,
    paths: &AppPaths,
    operation_id: String,
    fingerprint: String,
    mut cards: Vec<PendingCard>,
    shuffle: bool,
    mode: &str,
    context: &StudyTimeContext,
) -> Result<PracticeSessionDto, CommandError> {
    if cards.is_empty() || cards.len() > MAX_SESSION_CARDS {
        return Err(CommandError::new(
            "too_many_practice_cards",
            format!("A practice session must contain 1 to {MAX_SESSION_CARDS} cards."),
        ));
    }
    let session_id = generate_id(connection)?;
    if shuffle {
        cards.sort_by_key(|card| {
            (
                context.local_day_sort_key(card.due_at).unwrap_or(i32::MAX),
                !card.needs_more_practice,
                card.state == 0,
                shuffle_key(&session_id, &card.card_id),
            )
        });
    }
    let attribution_input = cards
        .iter()
        .map(|c| (c.card_id.clone(), c.presentation_variant.clone()))
        .collect::<Vec<_>>();
    let attributions = prepare_attributions(connection, &attribution_input)?;
    let selected_decks = cards
        .iter()
        .map(|c| c.deck_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let selected_json =
        serde_json::to_string(&selected_decks).map_err(CommandError::serialization)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    transaction.execute("INSERT INTO practice_sessions(id,selected_deck_ids_json,shuffle,card_count,mode,started_at,start_operation_id,start_request_fingerprint) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",params![session_id,selected_json,bool_to_integer(shuffle),i64::try_from(cards.len()).unwrap_or(i64::MAX),mode,context.now_utc_millis,operation_id,fingerprint]).map_err(CommandError::database)?;
    for (position, card) in cards.iter().enumerate() {
        transaction.execute("INSERT INTO practice_session_cards(session_id,card_id,position,presentation_variant) VALUES(?1,?2,?3,?4)",params![session_id,card.card_id,i64::try_from(position).unwrap_or(i64::MAX),card.presentation_variant]).map_err(CommandError::database)?;
    }
    insert_session_snapshots(
        &transaction,
        &session_id,
        attributions.into_values(),
        context.now_utc_millis,
    )?;
    transaction.commit().map_err(CommandError::database)?;
    load_session(connection, paths, &session_id)
}

fn load_session(
    connection: &Connection,
    paths: &AppPaths,
    session_id: &str,
) -> Result<PracticeSessionDto, CommandError> {
    let metadata: (i64, String, i64, i64) = connection
        .query_row(
            "SELECT shuffle,mode,started_at,card_count FROM practice_sessions WHERE id=?1",
            [session_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()
        .map_err(CommandError::database)?
        .ok_or_else(|| {
            CommandError::new(
                "practice_session_not_found",
                "That practice session no longer exists.",
            )
        })?;
    let window = load_practice_card_window(connection, paths, session_id, 0, PRACTICE_WINDOW_SIZE)?;
    Ok(PracticeSessionDto {
        id: session_id.to_string(),
        cards: window.cards,
        card_count: metadata.3,
        has_more: window.has_more,
        shuffle: metadata.0 != 0,
        mode: metadata.1,
        started_at: metadata.2,
    })
}

fn load_practice_card_window(
    connection: &Connection,
    paths: &AppPaths,
    session_id: &str,
    start: i64,
    limit: i64,
) -> Result<PracticeCardWindowDto, CommandError> {
    let total: i64 = connection
        .query_row(
            "SELECT card_count FROM practice_sessions WHERE id=?1",
            [session_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(CommandError::database)?
        .ok_or_else(|| {
            CommandError::new(
                "practice_session_not_found",
                "That practice session no longer exists.",
            )
        })?;
    let sql=format!("WITH {signals} SELECT psc.card_id,c.note_id,snap.deck_id,d.name,s.due_at,s.state,COALESCE(ls.needs_more_practice,0),c.direction,n.created_at,n.updated_at,snap.note_revision,snap.question_doc,snap.answer_doc,snap.question_excerpt,snap.answer_text FROM practice_session_cards psc JOIN practice_session_card_snapshots snap ON snap.session_id=psc.session_id AND snap.card_id=psc.card_id JOIN cards c ON c.id=psc.card_id JOIN notes n ON n.id=c.note_id JOIN decks d ON d.id=snap.deck_id AND d.deleted_at IS NULL JOIN card_schedules s ON s.card_id=c.id LEFT JOIN learning_signals ls ON ls.card_id=c.id WHERE psc.session_id=?1 ORDER BY psc.position LIMIT ?2 OFFSET ?3",signals=learning_signal_cte());
    let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
    let rows = statement
        .query_map(params![session_id, limit, start], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, i64>(4)?,
                r.get::<_, i64>(5)?,
                r.get::<_, i64>(6)?,
                r.get::<_, String>(7)?,
                r.get::<_, i64>(8)?,
                r.get::<_, i64>(9)?,
                r.get::<_, i64>(10)?,
                r.get::<_, String>(11)?,
                r.get::<_, String>(12)?,
                r.get::<_, String>(13)?,
                r.get::<_, String>(14)?,
            ))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    let card_ids = rows.iter().map(|r| r.0.clone()).collect::<Vec<_>>();
    let asset_ids_by_card = load_window_asset_ids(connection, session_id, &card_ids)?;
    let all_asset_ids = asset_ids_by_card
        .values()
        .flatten()
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let assets = get_assets_by_ids(connection, paths, &all_asset_ids)?;
    let cards = rows
        .into_iter()
        .map(
            |(
                card_id,
                note_id,
                deck_id,
                deck_name,
                due,
                state,
                weak,
                direction,
                created,
                updated,
                revision,
                qdoc,
                adoc,
                qtext,
                atext,
            )| {
                let card_assets = asset_ids_by_card
                    .get(&card_id)
                    .into_iter()
                    .flatten()
                    .filter_map(|id| assets.get(id).cloned())
                    .collect::<Vec<AssetDto>>();
                let front_doc: Value =
                    serde_json::from_str(&qdoc).map_err(CommandError::serialization)?;
                let back_doc: Value =
                    serde_json::from_str(&adoc).map_err(CommandError::serialization)?;
                Ok(PracticeCardDto {
                    card: CardDto {
                        id: card_id,
                        note_id,
                        deck_id,
                        direction,
                        front_doc,
                        back_doc,
                        front_text: qtext,
                        back_text: atext,
                        assets: card_assets,
                        created_at: created,
                        updated_at: updated,
                        revision,
                    },
                    deck_name,
                    due_at: due,
                    schedule_state: state,
                    needs_more_practice: weak != 0,
                })
            },
        )
        .collect::<Result<Vec<_>, CommandError>>()?;
    Ok(PracticeCardWindowDto {
        start_position: start,
        has_more: start.saturating_add(i64::try_from(cards.len()).unwrap_or(i64::MAX)) < total,
        cards,
    })
}

fn load_window_asset_ids(
    connection: &Connection,
    session_id: &str,
    card_ids: &[String],
) -> Result<BTreeMap<String, Vec<String>>, CommandError> {
    let mut map = BTreeMap::new();
    if card_ids.is_empty() {
        return Ok(map);
    }
    let values = card_ids
        .iter()
        .cloned()
        .map(SqlValue::Text)
        .collect::<Vec<_>>();
    let p = (2..=values.len() + 1)
        .map(|i| format!("?{i}"))
        .collect::<Vec<_>>()
        .join(",");
    let sql=format!("SELECT card_id,asset_id FROM practice_session_card_asset_snapshots WHERE session_id=?1 AND card_id IN ({p}) ORDER BY card_id,asset_id");
    let mut params = vec![SqlValue::Text(session_id.to_string())];
    params.extend(values);
    let mut st = connection.prepare(&sql).map_err(CommandError::database)?;
    let rows = st
        .query_map(params_from_iter(params.iter()), |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })
        .map_err(CommandError::database)?;
    for row in rows {
        let (c, a) = row.map_err(CommandError::database)?;
        map.entry(c).or_default().push(a);
    }
    Ok(map)
}

fn load_custom_cards(
    connection: &Connection,
    ids: &[String],
    context: &StudyTimeContext,
) -> Result<Vec<PlannedCard>, CommandError> {
    let mut result = BTreeMap::new();
    for chunk in ids.chunks(250) {
        let mut values = vec![SqlValue::Integer(context.now_utc_millis)];
        let p = chunk
            .iter()
            .map(|id| {
                values.push(SqlValue::Text(id.clone()));
                format!("?{}", values.len())
            })
            .collect::<Vec<_>>()
            .join(",");
        let sql=format!("WITH {signals} SELECT c.deck_id,c.id,c.note_id,s.due_at,s.state,COALESCE(ls.needs_more_practice,0),n.note_type,n.cloze_source,n.cloze_mode,s.reps FROM cards c JOIN notes n ON n.id=c.note_id JOIN decks d ON d.id=c.deck_id JOIN card_schedules s ON s.card_id=c.id LEFT JOIN learning_signals ls ON ls.card_id=c.id WHERE c.id IN ({p}) AND c.deleted_at IS NULL AND n.deleted_at IS NULL AND d.deleted_at IS NULL AND d.archived_at IS NULL AND c.suspended_at IS NULL AND (c.buried_until IS NULL OR c.buried_until<=?1)",signals=learning_signal_cte());
        let mut st = connection.prepare(&sql).map_err(CommandError::database)?;
        let rows = st
            .query_map(params_from_iter(values.iter()), |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, i64>(4)?,
                    r.get::<_, i64>(5)?,
                    r.get::<_, String>(6)?,
                    r.get::<_, String>(7)?,
                    r.get::<_, String>(8)?,
                    r.get::<_, i64>(9)?,
                ))
            })
            .map_err(CommandError::database)?;
        for row in rows {
            let (d, c, n, due, state, weak, typ, source, mode, reps) =
                row.map_err(CommandError::database)?;
            let variant = presentation_variant_for_projected(&typ, &source, &mode, reps)?;
            result.insert(
                c.clone(),
                PlannedCard {
                    deck_id: d,
                    card_id: c,
                    note_id: n,
                    due_at: due,
                    state,
                    needs_more_practice: weak != 0,
                    presentation_variant: variant,
                },
            );
        }
    }
    if result.len() != ids.len() {
        return Err(CommandError::new(
            "practice_card_not_found",
            "A selected card is suspended, buried, archived, deleted, or otherwise unavailable.",
        ));
    }
    Ok(ids.iter().filter_map(|id| result.remove(id)).collect())
}

fn session_for_operation(
    connection: &Connection,
    paths: &AppPaths,
    operation: &str,
    fingerprint: &str,
) -> Result<Option<PracticeSessionDto>, CommandError> {
    let existing:Option<(String,Option<String>)>=connection.query_row("SELECT id,start_request_fingerprint FROM practice_sessions WHERE start_operation_id=?1",[operation],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(CommandError::database)?;
    let Some((id, stored)) = existing else {
        return Ok(None);
    };
    if stored.as_deref() != Some(fingerprint) {
        return Err(CommandError::new(
            "idempotency_conflict",
            "That practice retry identifier was already used for a different request.",
        ));
    }
    load_session(connection, paths, &id).map(Some)
}
fn selected_fingerprint(
    decks: &[String],
    tags: &[String],
    shuffle: bool,
    mode: &str,
) -> Result<String, CommandError> {
    let json = serde_json::to_string(&SelectedIdentity {
        deck_ids: decks,
        tag_ids: tags,
        shuffle,
        mode,
    })
    .map_err(CommandError::serialization)?;
    Ok(format!("practice-selected:v2:{json}"))
}
fn card_fingerprint(cards: &[String], shuffle: bool) -> Result<String, CommandError> {
    let json = serde_json::to_string(&CardIdentity {
        card_ids: cards,
        shuffle,
    })
    .map_err(CommandError::serialization)?;
    Ok(format!("practice-cards:v2:{json}"))
}
fn validate_ids(values: Vec<String>, max: usize, label: &str) -> Result<Vec<String>, CommandError> {
    let mut unique = BTreeSet::new();
    for value in values {
        validate_entity_id(&value, label)?;
        unique.insert(value);
    }
    if unique.len() > max {
        return Err(CommandError::new(
            "practice_scope_too_large",
            format!("Select at most {max} {label} values."),
        ));
    }
    Ok(unique.into_iter().collect())
}
fn shuffle_key(session_id: &str, card_id: &str) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    session_id.hash(&mut h);
    card_id.hash(&mut h);
    h.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn window_is_bounded() {
        assert_eq!(PRACTICE_WINDOW_SIZE, 50);
        assert!(MAX_WINDOW_SIZE < MAX_SESSION_CARDS as i64);
    }
}
