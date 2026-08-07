//! Card organization, generated card types, safe templates, search, and bulk actions.
//!
//! Rust owns validation, deterministic rendering, stable reverse card
//! identities, single-schedule rotating cloze presentations, and transactional
//! persistence. Saved templates are data-only bulk-entry forms; generated cards
//! are ordinary Basic cards with no template ownership link.
//!
//! Shared contracts and persistence primitives live at this module root so the
//! domain command modules can use narrow ordinary Rust module boundaries
//! without duplicating card, tag, transaction, or projection semantics.

use std::collections::{BTreeSet, HashSet};

use rusqlite::{
    params, params_from_iter, types::Value as SqlValue, Connection, OptionalExtension, Transaction,
    TransactionBehavior,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::State;

use crate::{
    asset_lifecycle::{ensure_assets_available, finalize_assets_for_owner},
    assets::{ensure_assets_exist, AssetDto},
    cards::{get_card_from_connection, validate_content, CardDto, ValidatedContent},
    database::Database,
    error::CommandError,
    paths::AppPaths,
    projections::learning_signals as signal_projection,
    scheduler::insert_new_schedule,
    study_calendar::StudyTimeContext,
    studyability,
    util::{generate_id, unix_time_millis, validate_entity_id},
};

const MAX_TAGS_PER_NOTE: usize = 32;
const MAX_TAG_CHARS: usize = 64;
const MAX_SEARCH_QUERY_CHARS: usize = 500;
const MAX_SEARCH_TAGS: usize = 32;
const MAX_SEARCH_DECKS: usize = 100;
const MAX_SEARCH_LIMIT: i64 = 200;
const MAX_SEARCH_SELECTION: usize = 10_000;
const MAX_BULK_ROWS: usize = 500;
const MAX_BULK_ACTION_NOTES: usize = 10_000;
const MAX_TRANSFER_NOTES: usize = 10_000;
const TAG_SEPARATOR: char = '\u{001f}';

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TagDto {
    pub id: String,
    pub name: String,
    pub source_note_count: i64,
    pub generated_card_count: i64,
    pub studyable_due_count: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizedCardDto {
    pub id: String,
    pub note_id: String,
    pub deck_id: String,
    pub direction: String,
    pub front_doc: Value,
    pub back_doc: Value,
    pub front_text: String,
    pub back_text: String,
    pub assets: Vec<AssetDto>,
    pub tags: Vec<String>,
    pub due_at: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub revision: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardBrowserRowDto {
    pub card_id: String,
    pub note_id: String,
    pub deck_id: String,
    pub deck_name: String,
    pub card_type: String,
    pub card_label: String,
    pub front_text: String,
    pub back_text: String,
    pub tags: Vec<String>,
    pub due_at: i64,
    pub last_reviewed_at: Option<i64>,
    pub review_count: i64,
    pub suspended_at: Option<i64>,
    pub buried_until: Option<i64>,
    pub needs_more_practice: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardSearchPageDto {
    pub cards: Vec<CardBrowserRowDto>,
    pub total_count: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionCardDto {
    pub card_id: String,
    pub note_id: String,
    pub deck_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ListTagsInput {
    #[serde(default)]
    pub deck_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchCardsInput {
    #[serde(default)]
    pub deck_ids: Vec<String>,
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub tag_ids: Vec<String>,
    #[serde(default = "default_availability_filter")]
    pub availability_filter: String,
    #[serde(default = "default_status_filter")]
    pub status_filter: String,
    #[serde(default = "default_reviewed_filter")]
    pub reviewed_filter: String,
    #[serde(default = "default_sort")]
    pub sort: String,
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateOrganizedCardInput {
    pub deck_id: String,
    pub front_doc: Value,
    pub back_doc: Value,
    pub front_text: String,
    pub back_text: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub allow_duplicate: bool,
    #[serde(default)]
    pub asset_draft_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateOrganizedCardInput {
    pub note_id: String,
    pub deck_id: String,
    pub revision: i64,
    pub front_doc: Value,
    pub back_doc: Value,
    pub front_text: String,
    pub back_text: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub allow_duplicate: bool,
    #[serde(default)]
    pub asset_draft_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DuplicateCheckInput {
    pub deck_id: String,
    pub front_text: String,
    pub back_text: String,
    pub exclude_note_id: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateCardDto {
    pub note_id: String,
    pub deck_id: String,
    pub front_text: String,
    pub back_text: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BulkCardRowInput {
    pub question: String,
    pub answer: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnalyzeBulkCardsInput {
    pub deck_id: String,
    pub rows: Vec<BulkCardRowInput>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkAnalyzeResultDto {
    pub unique_count: i64,
    pub duplicate_count: i64,
    pub duplicate_row_indices: Vec<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BulkCreateCardsInput {
    pub deck_id: String,
    pub rows: Vec<BulkCardRowInput>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkCreateResultDto {
    pub created_count: i64,
    pub skipped_duplicates: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TransferCardsInput {
    pub note_ids: Vec<String>,
    pub target_deck_id: String,
    pub mode: String,
    #[serde(default = "default_true")]
    pub skip_duplicates: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferCardsResultDto {
    pub affected_count: i64,
    pub skipped_duplicates: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BulkTagCardsInput {
    pub note_ids: Vec<String>,
    pub tags: Vec<String>,
    pub mode: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkActionResultDto {
    pub affected_count: i64,
}

#[derive(Clone)]
struct ValidatedTag {
    name: String,
    normalized: String,
}

struct SourceNote {
    id: String,
    deck_id: String,
    front_doc: String,
    back_doc: String,
    front_text: String,
    back_text: String,
    duplicate_hash: String,
}

type PreparedBulkRow = (
    ValidatedContent,
    Vec<ValidatedTag>,
    (String, String),
    String,
);

struct SearchSql {
    joins: String,
    where_sql: String,
    values: Vec<SqlValue>,
    order_sql: &'static str,
}

fn effective_deck_ids(deck_ids: &[String]) -> Result<Vec<String>, CommandError> {
    let mut unique = BTreeSet::new();
    for deck_id in deck_ids {
        validate_entity_id(deck_id, "deck")?;
        unique.insert(deck_id.clone());
    }
    if unique.len() > MAX_SEARCH_DECKS {
        return Err(CommandError::new(
            "too_many_deck_filters",
            format!("Filter by at most {MAX_SEARCH_DECKS} decks."),
        ));
    }
    Ok(unique.into_iter().collect())
}

fn list_tags_in_connection(
    connection: &Connection,
    deck_ids: &[String],
) -> Result<Vec<TagDto>, CommandError> {
    let time = StudyTimeContext::capture()?;
    let mut values = vec![SqlValue::Integer(time.now_utc_millis)];
    let studyable_due = studyability::studyable_due_sql("?1");
    let mut sql = format!(
        "SELECT
             t.id,
             t.name,
             COUNT(DISTINCT n.id),
             COUNT(DISTINCT CASE WHEN c.deleted_at IS NULL THEN c.id END),
             COALESCE(SUM(CASE WHEN {studyable_due} THEN 1 ELSE 0 END), 0)
         FROM tags t
         JOIN note_tags nt ON nt.tag_id = t.id
         JOIN notes n ON n.id = nt.note_id
         JOIN decks d ON d.id = n.deck_id
         LEFT JOIN cards c ON c.note_id = n.id
         LEFT JOIN card_schedules s ON s.card_id = c.id
         WHERE n.deleted_at IS NULL
           AND d.deleted_at IS NULL
           AND d.archived_at IS NULL"
    );
    if !deck_ids.is_empty() {
        let placeholders = deck_ids
            .iter()
            .map(|deck_id| format!("?{}", push_value(&mut values, deck_id.clone())))
            .collect::<Vec<_>>()
            .join(", ");
        sql.push_str(&format!(" AND n.deck_id IN ({placeholders})"));
    }
    sql.push_str(
        " GROUP BY t.id, t.name
          HAVING COUNT(DISTINCT n.id) > 0
          ORDER BY t.name COLLATE NOCASE ASC, t.id ASC",
    );
    let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
    let tags = statement
        .query_map(params_from_iter(values.iter()), tag_from_row)
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    Ok(tags)
}

fn tag_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TagDto> {
    Ok(TagDto {
        id: row.get(0)?,
        name: row.get(1)?,
        source_note_count: row.get(2)?,
        generated_card_count: row.get(3)?,
        studyable_due_count: row.get(4)?,
    })
}

fn validate_search_input(input: &SearchCardsInput) -> Result<(), CommandError> {
    effective_deck_ids(&input.deck_ids)?;
    if input.query.chars().count() > MAX_SEARCH_QUERY_CHARS {
        return Err(CommandError::new(
            "search_too_long",
            format!("Search text cannot exceed {MAX_SEARCH_QUERY_CHARS} characters."),
        ));
    }
    if input.tag_ids.len() > MAX_SEARCH_TAGS {
        return Err(CommandError::new(
            "too_many_tag_filters",
            format!("Filter by at most {MAX_SEARCH_TAGS} tags."),
        ));
    }
    for tag_id in &input.tag_ids {
        validate_entity_id(tag_id, "tag")?;
    }
    if !matches!(
        input.availability_filter.as_str(),
        "all" | "ready_now" | "scheduled_later"
    ) {
        return Err(CommandError::new(
            "invalid_availability_filter",
            "Choose all cards, cards ready now, or cards scheduled later.",
        ));
    }
    if !matches!(
        input.status_filter.as_str(),
        "all" | "active" | "suspended" | "buried" | "needs_more_practice"
    ) {
        return Err(CommandError::new(
            "invalid_status_filter",
            "Choose active, suspended, buried, or Needs more practice cards.",
        ));
    }
    if input.availability_filter != "all"
        && matches!(input.status_filter.as_str(), "suspended" | "buried")
    {
        return Err(CommandError::new(
            "incompatible_search_filters",
            "Ready-now and scheduled-later filters apply only to active studyable cards.",
        ));
    }
    if !matches!(
        input.reviewed_filter.as_str(),
        "all"
            | "never"
            | "today"
            | "past_7_local_dates"
            | "past_30_local_dates"
            | "before_past_30_local_dates"
    ) {
        return Err(CommandError::new(
            "invalid_reviewed_filter",
            "Choose one of the available local-date study filters.",
        ));
    }
    if !matches!(
        input.sort.as_str(),
        "relevance" | "updated_desc" | "question_asc" | "due_asc" | "last_reviewed_desc"
    ) {
        return Err(CommandError::new(
            "invalid_card_sort",
            "Choose one of the available card sort orders.",
        ));
    }
    if !(1..=MAX_SEARCH_LIMIT).contains(&input.limit) || input.offset < 0 {
        return Err(CommandError::new(
            "invalid_search_page",
            "The requested card-results page is invalid.",
        ));
    }
    Ok(())
}

fn build_search_sql(
    input: &SearchCardsInput,
    time: &StudyTimeContext,
) -> Result<SearchSql, CommandError> {
    let fts_query = build_fts_query(&input.query);
    let mut joins = String::new();
    if fts_query.is_some() {
        joins.push_str(" JOIN card_search_fts ON card_search_fts.rowid = n.rowid");
    }
    joins.push(' ');
    joins.push_str(signal_projection::join_sql());

    let mut conditions = vec![studyability::current_card_sql("c", "n", "d").to_string()];
    let mut values: Vec<SqlValue> = Vec::new();

    let deck_ids = effective_deck_ids(&input.deck_ids)?;
    if !deck_ids.is_empty() {
        let placeholders = deck_ids
            .into_iter()
            .map(|deck_id| format!("?{}", push_value(&mut values, deck_id)))
            .collect::<Vec<_>>()
            .join(", ");
        conditions.push(format!("c.deck_id IN ({placeholders})"));
    }
    if let Some(query) = fts_query {
        conditions.push(format!(
            "card_search_fts MATCH ?{}",
            push_value(&mut values, query)
        ));
    }
    for tag_id in &input.tag_ids {
        let index = push_value(&mut values, tag_id.clone());
        conditions.push(format!(
            "EXISTS (
                SELECT 1 FROM note_tags filter_nt
                WHERE filter_nt.note_id = n.id AND filter_nt.tag_id = ?{index}
             )"
        ));
    }

    match input.status_filter.as_str() {
        "active" => {
            let now = push_value(&mut values, time.now_utc_millis);
            conditions.push(studyability::studyable_card_sql(&format!("?{now}")));
        }
        "suspended" => conditions.push("c.suspended_at IS NOT NULL".to_string()),
        "buried" => {
            let now = push_value(&mut values, time.now_utc_millis);
            conditions.push("c.suspended_at IS NULL".to_string());
            conditions.push("c.archive_suspended_at IS NULL".to_string());
            conditions.push(format!("c.buried_until > ?{now}"));
        }
        "needs_more_practice" => {
            let now = push_value(&mut values, time.now_utc_millis);
            conditions.push(studyability::studyable_card_sql(&format!("?{now}")));
            conditions.push(format!("{} = 1", signal_projection::value_sql()));
        }
        _ => {}
    }

    match input.availability_filter.as_str() {
        "ready_now" => {
            let now = push_value(&mut values, time.now_utc_millis);
            conditions.push(studyability::studyable_due_sql(&format!("?{now}")));
        }
        "scheduled_later" => {
            let now = push_value(&mut values, time.now_utc_millis);
            conditions.push(studyability::studyable_card_sql(&format!("?{now}")));
            conditions.push(format!("s.due_at > ?{now}"));
        }
        _ => {}
    }

    match input.reviewed_filter.as_str() {
        "never" => conditions.push(
            "NOT EXISTS (SELECT 1 FROM review_events reviewed WHERE reviewed.card_id = c.id)"
                .to_string(),
        ),
        "today" => {
            let date = push_value(&mut values, time.local_date.to_string());
            conditions.push(review_local_date_exists_sql(&format!("= ?{date}")));
        }
        "past_7_local_dates" => {
            let start = time.trailing_local_dates(7)?.start_local_date.to_string();
            let date = push_value(&mut values, start);
            conditions.push(review_local_date_exists_sql(&format!(">= ?{date}")));
        }
        "past_30_local_dates" => {
            let start = time.trailing_local_dates(30)?.start_local_date.to_string();
            let date = push_value(&mut values, start);
            conditions.push(review_local_date_exists_sql(&format!(">= ?{date}")));
        }
        "before_past_30_local_dates" => {
            let start = time.trailing_local_dates(30)?.start_local_date.to_string();
            let date = push_value(&mut values, start);
            conditions.push(format!(
                "(
                    SELECT MAX(review_attr.review_local_date)
                    FROM review_events reviewed
                    JOIN review_event_attributions review_attr
                      ON review_attr.review_event_id = reviewed.id
                    WHERE reviewed.card_id = c.id
                 ) < ?{date}"
            ));
        }
        _ => {}
    }

    let order_sql = match input.sort.as_str() {
        "relevance" if fts_query_is_present(&input.query) => {
            "bm25(card_search_fts) ASC, n.updated_at DESC, c.template_ordinal ASC"
        }
        "question_asc" => {
            "CASE WHEN n.note_type = 'basic' AND c.direction = 'reverse' THEN n.back_text ELSE n.front_text END COLLATE NOCASE ASC, n.updated_at DESC, c.template_ordinal ASC"
        }
        "due_asc" => "s.due_at ASC, n.updated_at DESC, c.template_ordinal ASC",
        "last_reviewed_desc" => {
            "CASE WHEN s.reps > 0 THEN 0 ELSE 1 END ASC, s.last_review_at DESC, n.updated_at DESC, c.template_ordinal ASC"
        }
        _ => "n.updated_at DESC, n.created_at DESC, c.template_ordinal ASC, c.direction ASC",
    };
    Ok(SearchSql {
        joins,
        where_sql: conditions.join(" AND "),
        values,
        order_sql,
    })
}

fn review_local_date_exists_sql(predicate: &str) -> String {
    format!(
        "EXISTS (
             SELECT 1
             FROM review_events reviewed
             JOIN review_event_attributions review_attr
               ON review_attr.review_event_id = reviewed.id
             WHERE reviewed.card_id = c.id
               AND review_attr.review_local_date {predicate}
         )"
    )
}

fn search_cards_in_connection(
    connection: &Connection,
    input: SearchCardsInput,
) -> Result<CardSearchPageDto, CommandError> {
    let time = StudyTimeContext::capture()?;
    let mut search = build_search_sql(&input, &time)?;
    let count_sql = format!(
        "WITH {signals}
         SELECT COUNT(*)
         FROM notes n
         JOIN cards c ON c.note_id = n.id
         JOIN decks d ON d.id = c.deck_id
         JOIN card_schedules s ON s.card_id = c.id
         {joins}
         WHERE {where_sql}",
        signals = signal_projection::LEARNING_SIGNAL_CTES,
        joins = search.joins,
        where_sql = search.where_sql,
    );
    let total_count: i64 = connection
        .query_row(&count_sql, params_from_iter(search.values.iter()), |row| {
            row.get(0)
        })
        .map_err(CommandError::database)?;
    let limit_index = push_value(&mut search.values, input.limit);
    let offset_index = push_value(&mut search.values, input.offset);
    let query_sql = format!(
        "WITH {signals}
         SELECT
             c.id, n.id, c.deck_id, d.name,
             COALESCE((
                 SELECT group_concat(t.name, char(31))
                 FROM note_tags nt
                 JOIN tags t ON t.id = nt.tag_id
                 WHERE nt.note_id = n.id
                 ORDER BY t.name COLLATE NOCASE, t.id
             ), ''),
             s.due_at,
             CASE WHEN s.reps > 0 THEN s.last_review_at ELSE NULL END,
             s.reps, c.suspended_at, c.buried_until, n.created_at, n.updated_at,
             c.direction, c.template_ordinal, n.note_type,
             n.front_doc, n.back_doc, n.front_text, n.back_text,
             n.cloze_source, COALESCE(n.cloze_mode, 'one_at_a_time'),
             {signal_value}
         FROM notes n
         JOIN cards c ON c.note_id = n.id
         JOIN decks d ON d.id = c.deck_id
         JOIN card_schedules s ON s.card_id = c.id
         {joins}
         WHERE {where_sql}
         ORDER BY {order_sql}
         LIMIT ?{limit_index} OFFSET ?{offset_index}",
        signals = signal_projection::LEARNING_SIGNAL_CTES,
        signal_value = signal_projection::value_sql(),
        joins = search.joins,
        where_sql = search.where_sql,
        order_sql = search.order_sql,
    );
    let mut statement = connection
        .prepare(&query_sql)
        .map_err(CommandError::database)?;
    let cards = statement
        .query_map(params_from_iter(search.values.iter()), |row| {
            let direction: String = row.get(12)?;
            let template_ordinal: i64 = row.get(13)?;
            let note_type: String = row.get(14)?;
            let front_doc_json: String = row.get(15)?;
            let back_doc_json: String = row.get(16)?;
            let source_front_text: String = row.get(17)?;
            let source_back_text: String = row.get(18)?;
            let cloze_source: String = row.get(19)?;
            let cloze_mode: String = row.get(20)?;
            let reps: i64 = row.get(7)?;
            let rendered = render_projected_presentation(ProjectedPresentationInput {
                direction: &direction,
                template_ordinal,
                note_type: &note_type,
                front_doc_json: &front_doc_json,
                back_doc_json: &back_doc_json,
                front_text: &source_front_text,
                back_text: &source_back_text,
                cloze_source: &cloze_source,
                cloze_mode: &cloze_mode,
                reps,
                presentation_variant: None,
            })
            .map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    14,
                    rusqlite::types::Type::Text,
                    Box::new(std::io::Error::other(error.message)),
                )
            })?;
            let grouped_tags: String = row.get(4)?;
            Ok(CardBrowserRowDto {
                card_id: row.get(0)?,
                note_id: row.get(1)?,
                deck_id: row.get(2)?,
                deck_name: row.get(3)?,
                card_type: rendered.card_type,
                card_label: rendered.card_label,
                front_text: rendered.front_text,
                back_text: rendered.back_text,
                tags: split_tags(&grouped_tags),
                due_at: row.get(5)?,
                last_reviewed_at: row.get(6)?,
                review_count: reps,
                suspended_at: row.get(8)?,
                buried_until: row.get(9)?,
                needs_more_practice: row.get::<_, i64>(21)? != 0,
                created_at: row.get(10)?,
                updated_at: row.get(11)?,
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    Ok(CardSearchPageDto { cards, total_count })
}

fn list_matching_cards_in_connection(
    connection: &Connection,
    input: SearchCardsInput,
) -> Result<Vec<SelectionCardDto>, CommandError> {
    let time = StudyTimeContext::capture()?;
    let mut search = build_search_sql(&input, &time)?;
    let maximum = i64::try_from(MAX_SEARCH_SELECTION + 1).unwrap_or(i64::MAX);
    let limit_index = push_value(&mut search.values, maximum);
    let sql = format!(
        "WITH {signals}
         SELECT c.id, n.id, c.deck_id
         FROM notes n
         JOIN cards c ON c.note_id = n.id
         JOIN decks d ON d.id = c.deck_id
         JOIN card_schedules s ON s.card_id = c.id
         {joins}
         WHERE {where_sql}
         ORDER BY {order_sql}
         LIMIT ?{limit_index}",
        signals = signal_projection::LEARNING_SIGNAL_CTES,
        joins = search.joins,
        where_sql = search.where_sql,
        order_sql = search.order_sql,
    );
    let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
    let cards = statement
        .query_map(params_from_iter(search.values.iter()), |row| {
            Ok(SelectionCardDto {
                card_id: row.get(0)?,
                note_id: row.get(1)?,
                deck_id: row.get(2)?,
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    if cards.len() > MAX_SEARCH_SELECTION {
        return Err(CommandError::new(
            "too_many_matching_cards",
            format!("More than {MAX_SEARCH_SELECTION} cards match. Narrow the filters before selecting or practicing all results."),
        ));
    }
    Ok(cards)
}

fn fts_query_is_present(query: &str) -> bool {
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
}

fn hydrate_organized_card(
    connection: &Connection,
    paths: &AppPaths,
    note_id: &str,
) -> Result<OrganizedCardDto, CommandError> {
    let card = get_card_from_connection(connection, paths, note_id)?;
    let tags = load_tag_names(connection, note_id)?;
    let due_at = connection
        .query_row(
            "SELECT s.due_at
             FROM card_schedules s
             JOIN cards c ON c.id = s.card_id
             WHERE c.note_id = ?1
               AND c.deleted_at IS NULL
               AND c.direction = 'forward'",
            [note_id],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    Ok(organized_from_card(card, tags, due_at))
}

fn organized_from_card(card: CardDto, tags: Vec<String>, due_at: i64) -> OrganizedCardDto {
    OrganizedCardDto {
        id: card.id,
        note_id: card.note_id,
        deck_id: card.deck_id,
        direction: card.direction,
        front_doc: card.front_doc,
        back_doc: card.back_doc,
        front_text: card.front_text,
        back_text: card.back_text,
        assets: card.assets,
        tags,
        due_at,
        created_at: card.created_at,
        updated_at: card.updated_at,
        revision: card.revision,
    }
}

fn load_tag_names(connection: &Connection, note_id: &str) -> Result<Vec<String>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT t.name
             FROM tags t
             JOIN note_tags nt ON nt.tag_id = t.id
             WHERE nt.note_id = ?1
             ORDER BY t.name COLLATE NOCASE ASC",
        )
        .map_err(CommandError::database)?;
    let tags = statement
        .query_map([note_id], |row| row.get(0))
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    Ok(tags)
}

fn validate_tags(values: Vec<String>) -> Result<Vec<ValidatedTag>, CommandError> {
    if values.len() > MAX_TAGS_PER_NOTE {
        return Err(CommandError::new(
            "too_many_tags",
            format!("A card can have at most {MAX_TAGS_PER_NOTE} tags."),
        ));
    }
    let mut seen = BTreeSet::new();
    let mut tags = Vec::new();
    for value in values {
        let name = collapse_whitespace(&value);
        if name.is_empty()
            || name.chars().count() > MAX_TAG_CHARS
            || name.chars().any(char::is_control)
        {
            return Err(CommandError::new(
                "invalid_tag",
                format!("Each tag must contain 1 to {MAX_TAG_CHARS} visible characters."),
            ));
        }
        let normalized = name.to_lowercase();
        if seen.insert(normalized.clone()) {
            tags.push(ValidatedTag { name, normalized });
        }
    }
    Ok(tags)
}

fn replace_note_tags(
    transaction: &Transaction<'_>,
    note_id: &str,
    tags: &[ValidatedTag],
    now: i64,
) -> Result<(), CommandError> {
    transaction
        .execute("DELETE FROM note_tags WHERE note_id = ?1", [note_id])
        .map_err(CommandError::database)?;
    for tag in tags {
        let existing: Option<String> = transaction
            .query_row(
                "SELECT id FROM tags WHERE normalized_name = ?1",
                [tag.normalized.as_str()],
                |row| row.get(0),
            )
            .optional()
            .map_err(CommandError::database)?;
        let tag_id = match existing {
            Some(id) => id,
            None => {
                let id = generate_id(transaction)?;
                transaction
                    .execute(
                        "INSERT INTO tags (id, name, normalized_name, created_at, updated_at)
                         VALUES (?1, ?2, ?3, ?4, ?4)",
                        params![id, tag.name, tag.normalized, now],
                    )
                    .map_err(CommandError::database)?;
                id
            }
        };
        transaction
            .execute(
                "INSERT INTO note_tags (note_id, tag_id) VALUES (?1, ?2)",
                params![note_id, tag_id],
            )
            .map_err(CommandError::database)?;
    }
    cleanup_unused_tags(transaction)
}

fn cleanup_unused_tags(connection: &Connection) -> Result<(), CommandError> {
    connection
        .execute(
            "DELETE FROM tags
             WHERE NOT EXISTS (
                 SELECT 1 FROM note_tags nt WHERE nt.tag_id = tags.id
             )
               AND NOT EXISTS (
                   SELECT 1 FROM review_event_tag_attributions rta
                   WHERE rta.tag_id = tags.id
               )",
            [],
        )
        .map_err(CommandError::database)?;
    Ok(())
}

const CONTENT_FORMAT: &str = "tiptap-json";
const CONTENT_SCHEMA_VERSION: i64 = 1;

fn insert_organized_card(
    transaction: &Transaction<'_>,
    deck_id: &str,
    content: ValidatedContent,
    tags: &[ValidatedTag],
    duplicate_hash: &str,
    now: i64,
) -> Result<String, CommandError> {
    ensure_assets_exist(transaction, &content_asset_ids(&content))?;
    let note_id = generate_id(transaction)?;
    let card_id = generate_id(transaction)?;

    transaction
        .execute(
            "INSERT INTO notes (
                 id, deck_id, note_type, content_format, content_schema_version,
                 front_doc, back_doc, front_text, back_text, duplicate_hash,
                 created_at, updated_at, revision
             ) VALUES (
                 ?1, ?2, 'basic', ?3, ?4,
                 ?5, ?6, ?7, ?8, ?9, ?10, ?10, 1
             )",
            params![
                note_id,
                deck_id,
                CONTENT_FORMAT,
                CONTENT_SCHEMA_VERSION,
                content.front_doc_json,
                content.back_doc_json,
                content.front_text,
                content.back_text,
                duplicate_hash,
                now,
            ],
        )
        .map_err(CommandError::database)?;
    transaction
        .execute(
            "INSERT INTO cards (
                 id, note_id, deck_id, direction, template_ordinal, created_at, updated_at
             ) VALUES (?1, ?2, ?3, 'forward', 0, ?4, ?4)",
            params![card_id, note_id, deck_id, now],
        )
        .map_err(CommandError::database)?;
    insert_new_schedule(transaction, &card_id, now)?;
    replace_note_assets(transaction, &note_id, &content)?;
    replace_note_tags(transaction, &note_id, tags, now)?;
    Ok(note_id)
}

fn replace_note_assets(
    transaction: &Transaction<'_>,
    note_id: &str,
    content: &ValidatedContent,
) -> Result<(), CommandError> {
    transaction
        .execute("DELETE FROM note_assets WHERE note_id = ?1", [note_id])
        .map_err(CommandError::database)?;
    for asset_id in &content.front_asset_ids {
        transaction
            .execute(
                "INSERT INTO note_assets (note_id, asset_id, side)
                 VALUES (?1, ?2, 'front')",
                params![note_id, asset_id],
            )
            .map_err(CommandError::database)?;
    }
    for asset_id in &content.back_asset_ids {
        transaction
            .execute(
                "INSERT INTO note_assets (note_id, asset_id, side)
                 VALUES (?1, ?2, 'back')",
                params![note_id, asset_id],
            )
            .map_err(CommandError::database)?;
    }
    Ok(())
}

fn content_asset_ids(content: &ValidatedContent) -> Vec<String> {
    content
        .front_asset_ids
        .iter()
        .chain(&content.back_asset_ids)
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn ensure_active_deck(connection: &Connection, deck_id: &str) -> Result<(), CommandError> {
    let exists: i64 = connection
        .query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM decks
                 WHERE id = ?1 AND deleted_at IS NULL AND archived_at IS NULL
             )",
            [deck_id],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    if exists == 0 {
        return Err(CommandError::new(
            "deck_not_found",
            "The destination deck no longer exists.",
        ));
    }
    Ok(())
}

fn ensure_active_note(connection: &Connection, note_id: &str) -> Result<(), CommandError> {
    let exists: i64 = connection
        .query_row(
            "SELECT EXISTS(
                 SELECT 1
                 FROM notes n
                 JOIN decks d ON d.id = n.deck_id
                 WHERE n.id = ?1
                   AND n.deleted_at IS NULL
                   AND d.deleted_at IS NULL
                   AND d.archived_at IS NULL
             )",
            [note_id],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    if exists == 0 {
        return Err(CommandError::new(
            "card_not_found",
            "A selected flashcard no longer exists.",
        ));
    }
    Ok(())
}

fn exact_duplicates(
    connection: &Connection,
    deck_id: &str,
    hash: &str,
    normalized_front: &str,
    normalized_back: &str,
    exclude_note_id: Option<&str>,
    limit: usize,
) -> Result<Vec<DuplicateCardDto>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT id, deck_id, front_text, back_text
             FROM notes
             WHERE deck_id = ?1
               AND (duplicate_hash = ?2 OR duplicate_hash = '')
               AND deleted_at IS NULL
               AND (?3 IS NULL OR id != ?3)
             ORDER BY updated_at DESC",
        )
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map(params![deck_id, hash, exclude_note_id], |row| {
            Ok(DuplicateCardDto {
                note_id: row.get(0)?,
                deck_id: row.get(1)?,
                front_text: row.get(2)?,
                back_text: row.get(3)?,
            })
        })
        .map_err(CommandError::database)?;

    let mut matches = Vec::new();
    for row in rows {
        let candidate = row.map_err(CommandError::database)?;
        let normalized = normalized_pair(&candidate.front_text, &candidate.back_text);
        if normalized.0 == normalized_front && normalized.1 == normalized_back {
            matches.push(candidate);
            if matches.len() >= limit {
                break;
            }
        }
    }
    Ok(matches)
}

fn has_exact_duplicate(
    connection: &Connection,
    deck_id: &str,
    hash: &str,
    normalized_front: &str,
    normalized_back: &str,
    exclude_note_id: Option<&str>,
) -> Result<bool, CommandError> {
    Ok(!exact_duplicates(
        connection,
        deck_id,
        hash,
        normalized_front,
        normalized_back,
        exclude_note_id,
        20,
    )?
    .is_empty())
}

fn duplicate_error() -> CommandError {
    CommandError::new(
        "duplicate_card",
        "An identical question and answer already exist in this deck.",
    )
}

fn prepare_bulk_row(index: usize, row: BulkCardRowInput) -> Result<PreparedBulkRow, CommandError> {
    if row.question.trim().is_empty() || row.answer.trim().is_empty() {
        return Err(CommandError::new(
            "invalid_bulk_card",
            format!(
                "Bulk row {} needs both a question and an answer.",
                index + 1
            ),
        ));
    }
    let front_doc = plain_text_document(&row.question);
    let back_doc = plain_text_document(&row.answer);
    let content = validate_content(front_doc, back_doc, row.question, row.answer)?;
    let tags = validate_tags(row.tags)?;
    let normalized = normalized_pair(&content.front_text, &content.back_text);
    let hash = duplicate_hash(&normalized.0, &normalized.1);
    Ok((content, tags, normalized, hash))
}

fn plain_text_document(text: &str) -> Value {
    let paragraphs = text
        .lines()
        .map(|line| {
            if line.is_empty() {
                json!({"type": "paragraph"})
            } else {
                json!({
                    "type": "paragraph",
                    "content": [{"type": "text", "text": line}]
                })
            }
        })
        .collect::<Vec<_>>();
    json!({
        "type": "doc",
        "content": if paragraphs.is_empty() {
            vec![json!({"type": "paragraph"})]
        } else {
            paragraphs
        }
    })
}

fn validate_note_ids_for_action(
    values: Vec<String>,
    limit: usize,
    action: &str,
) -> Result<Vec<String>, CommandError> {
    let mut unique = BTreeSet::new();
    for value in values {
        validate_entity_id(&value, "note")?;
        unique.insert(value);
    }
    if unique.is_empty() {
        return Err(CommandError::new(
            "no_cards_selected",
            "Select at least one card first.",
        ));
    }
    if unique.len() > limit {
        return Err(CommandError::new(
            "too_many_cards_selected",
            format!("{action} at most {limit} cards at one time."),
        ));
    }
    Ok(unique.into_iter().collect())
}

fn resolve_tag_ids(
    transaction: &Transaction<'_>,
    tags: &[ValidatedTag],
    now: i64,
) -> Result<Vec<String>, CommandError> {
    let mut tag_ids = Vec::with_capacity(tags.len());
    for tag in tags {
        let existing: Option<String> = transaction
            .query_row(
                "SELECT id FROM tags WHERE normalized_name = ?1",
                [tag.normalized.as_str()],
                |row| row.get(0),
            )
            .optional()
            .map_err(CommandError::database)?;
        let tag_id = match existing {
            Some(id) => id,
            None => {
                let id = generate_id(transaction)?;
                transaction
                    .execute(
                        "INSERT INTO tags (
                             id, name, normalized_name, created_at, updated_at
                         ) VALUES (?1, ?2, ?3, ?4, ?4)",
                        params![id, tag.name, tag.normalized, now],
                    )
                    .map_err(CommandError::database)?;
                id
            }
        };
        tag_ids.push(tag_id);
    }
    Ok(tag_ids)
}

fn find_existing_tag_ids(
    connection: &Connection,
    tags: &[ValidatedTag],
) -> Result<Vec<String>, CommandError> {
    let mut tag_ids = Vec::new();
    for tag in tags {
        if let Some(tag_id) = connection
            .query_row(
                "SELECT id FROM tags WHERE normalized_name = ?1",
                [tag.normalized.as_str()],
                |row| row.get(0),
            )
            .optional()
            .map_err(CommandError::database)?
        {
            tag_ids.push(tag_id);
        }
    }
    Ok(tag_ids)
}

fn load_source_note(connection: &Connection, note_id: &str) -> Result<SourceNote, CommandError> {
    connection
        .query_row(
            "SELECT id, deck_id, front_doc, back_doc, front_text, back_text,
                    duplicate_hash
             FROM notes
             WHERE id = ?1 AND deleted_at IS NULL",
            [note_id],
            |row| {
                Ok(SourceNote {
                    id: row.get(0)?,
                    deck_id: row.get(1)?,
                    front_doc: row.get(2)?,
                    back_doc: row.get(3)?,
                    front_text: row.get(4)?,
                    back_text: row.get(5)?,
                    duplicate_hash: row.get(6)?,
                })
            },
        )
        .optional()
        .map_err(CommandError::database)?
        .ok_or_else(|| CommandError::new("card_not_found", "A selected card no longer exists."))
}

#[derive(Clone, Copy)]
enum TransferMode {
    Move,
    Copy,
}

impl TransferMode {
    fn parse(value: &str) -> Result<Self, CommandError> {
        match value {
            "move" => Ok(Self::Move),
            "copy" => Ok(Self::Copy),
            _ => Err(CommandError::new(
                "invalid_transfer_mode",
                "Choose whether to move or copy the selected cards.",
            )),
        }
    }
}

#[derive(Clone, Copy)]
enum TagChangeMode {
    Add,
    Remove,
}

impl TagChangeMode {
    fn parse(value: &str) -> Result<Self, CommandError> {
        match value {
            "add" => Ok(Self::Add),
            "remove" => Ok(Self::Remove),
            _ => Err(CommandError::new(
                "invalid_tag_change_mode",
                "Choose whether to add or remove the selected tags.",
            )),
        }
    }
}

fn duplicate_checkable(front: &str, back: &str) -> bool {
    !front.trim().is_empty() && !back.trim().is_empty()
}

fn normalized_pair(front: &str, back: &str) -> (String, String) {
    (
        collapse_whitespace(front).to_lowercase(),
        collapse_whitespace(back).to_lowercase(),
    )
}

fn collapse_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn duplicate_hash(front: &str, back: &str) -> String {
    // This stable FNV-1a value is only a candidate index. Duplicate checks must
    // still compare both complete normalized strings before reporting a match;
    // the hash is deliberately not treated as a collision-resistant identity.
    // The explicit separator prevents ambiguous concatenations such as
    // (`ab`, `c`) and (`a`, `bc`) from sharing the same input byte stream.
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in front
        .as_bytes()
        .iter()
        .copied()
        .chain(std::iter::once(0xff))
        .chain(back.as_bytes().iter().copied())
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn split_tags(value: &str) -> Vec<String> {
    value
        .split(TAG_SEPARATOR)
        .filter(|tag| !tag.is_empty())
        .map(str::to_string)
        .collect()
}

fn default_availability_filter() -> String {
    "all".to_string()
}

fn default_status_filter() -> String {
    "active".to_string()
}

fn default_reviewed_filter() -> String {
    "all".to_string()
}

fn default_sort() -> String {
    "updated_desc".to_string()
}

const fn default_limit() -> i64 {
    100
}

const fn default_true() -> bool {
    true
}

mod card_commands;
mod card_types_and_templates;
mod transfer;

#[cfg(test)]
mod history_tests;
#[cfg(test)]
mod tests;

pub(crate) use card_commands::{
    analyze_bulk_cards, backfill_duplicate_hashes_in_connection, bulk_create_cards, bulk_tag_cards,
    create_organized_card, find_duplicate_cards, get_organized_card, list_matching_cards,
    list_tags, search_cards, update_organized_card,
};
pub(crate) use card_types_and_templates::{
    apply_stage3_data_repairs, create_card_template, create_generated_card, delete_card_template,
    generate_cards_from_template, get_card_template, get_card_type_info,
    import_generated_flashcards, list_card_template_summaries, preview_template_cards,
    projected_presentation_variant, render_projected_presentation, update_card_template,
    update_generated_card, ProjectedPresentationInput, RenderedCardPresentation,
};
pub(crate) use transfer::transfer_cards;
