use std::collections::BTreeSet;

use rusqlite::{params_from_iter, types::Value, Connection};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    cards::{get_card_from_connection, CardDto},
    database::Database,
    error::CommandError,
    paths::AppPaths,
    util::{unix_time_millis, validate_entity_id},
};

use super::{
    ensure_active_deck,
    tags::{list_tag_summaries, load_tags_for_note, TagDto, TagSummaryDto},
};

const MAX_SEARCH_CHARACTERS: usize = 200;
const MAX_SEARCH_TERMS: usize = 12;
const MAX_TERM_CHARACTERS: usize = 64;
const MAX_PAGE_SIZE: i64 = 100;
const MAX_OFFSET: i64 = 1_000_000;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowseCardsInput {
    pub deck_id: String,
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub tag_ids: Vec<String>,
    #[serde(default = "default_status")]
    pub status: String,
    #[serde(default = "default_sort")]
    pub sort: String,
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

fn default_status() -> String {
    "all".to_string()
}

fn default_sort() -> String {
    "updated_desc".to_string()
}

const fn default_limit() -> i64 {
    100
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardBrowserItemDto {
    pub card: CardDto,
    pub tags: Vec<TagDto>,
    pub due_at: i64,
    pub schedule_state: i64,
    pub review_count: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardBrowserResultDto {
    pub items: Vec<CardBrowserItemDto>,
    pub total: i64,
    pub tags: Vec<TagSummaryDto>,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Clone, Copy)]
pub(crate) enum BrowseStatus {
    All,
    Due,
    New,
    Reviewed,
}

impl BrowseStatus {
    fn parse(value: &str) -> Result<Self, CommandError> {
        match value {
            "all" => Ok(Self::All),
            "due" => Ok(Self::Due),
            "new" => Ok(Self::New),
            "reviewed" => Ok(Self::Reviewed),
            _ => Err(CommandError::new(
                "invalid_card_filter",
                "Choose a valid card status filter.",
            )),
        }
    }

    const fn sql(self) -> &'static str {
        match self {
            Self::All => "",
            Self::Due => " AND s.due_at <= ?",
            Self::New => " AND s.reps = 0",
            Self::Reviewed => " AND s.reps > 0",
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum BrowseSort {
    Relevance,
    UpdatedDescending,
    CreatedDescending,
    QuestionAscending,
    DueAscending,
}

impl BrowseSort {
    fn parse(value: &str) -> Result<Self, CommandError> {
        match value {
            "relevance" => Ok(Self::Relevance),
            "updated_desc" => Ok(Self::UpdatedDescending),
            "created_desc" => Ok(Self::CreatedDescending),
            "question_asc" => Ok(Self::QuestionAscending),
            "due_asc" => Ok(Self::DueAscending),
            _ => Err(CommandError::new(
                "invalid_card_sort",
                "Choose a valid card sort order.",
            )),
        }
    }

    const fn sql(self, has_query: bool) -> &'static str {
        match self {
            Self::Relevance if has_query => {
                "bm25(card_search), n.updated_at DESC, n.id ASC"
            }
            Self::UpdatedDescending | Self::Relevance => {
                "n.updated_at DESC, n.created_at DESC, n.id ASC"
            }
            Self::CreatedDescending => "n.created_at DESC, n.id ASC",
            Self::QuestionAscending => {
                "n.front_text COLLATE NOCASE ASC, n.created_at ASC, n.id ASC"
            }
            Self::DueAscending => "s.due_at ASC, n.created_at ASC, n.id ASC",
        }
    }
}

#[tauri::command]
pub fn browse_cards(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: BrowseCardsInput,
) -> Result<CardBrowserResultDto, CommandError> {
    validate_entity_id(&input.deck_id, "deck")?;
    if input.query.chars().count() > MAX_SEARCH_CHARACTERS {
        return Err(CommandError::new(
            "search_too_long",
            format!("Search text cannot exceed {MAX_SEARCH_CHARACTERS} characters."),
        ));
    }
    if !(1..=MAX_PAGE_SIZE).contains(&input.limit) {
        return Err(CommandError::new(
            "invalid_page_size",
            format!("Card pages can contain between 1 and {MAX_PAGE_SIZE} cards."),
        ));
    }
    if !(0..=MAX_OFFSET).contains(&input.offset) {
        return Err(CommandError::new(
            "invalid_page_offset",
            "The requested card page is outside the supported range.",
        ));
    }

    let mut unique_tag_ids = BTreeSet::new();
    for tag_id in input.tag_ids {
        validate_entity_id(&tag_id, "tag")?;
        unique_tag_ids.insert(tag_id);
    }
    let status = BrowseStatus::parse(&input.status)?;
    let sort = BrowseSort::parse(&input.sort)?;
    let fts_query = build_fts_query(&input.query);

    database.with_connection(|connection| {
        browse_cards_in_connection(
            connection,
            &paths,
            &input.deck_id,
            fts_query.as_deref(),
            unique_tag_ids.into_iter().collect(),
            status,
            sort,
            input.limit,
            input.offset,
        )
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn browse_cards_in_connection(
    connection: &mut Connection,
    paths: &AppPaths,
    deck_id: &str,
    fts_query: Option<&str>,
    tag_ids: Vec<String>,
    status: BrowseStatus,
    sort: BrowseSort,
    limit: i64,
    offset: i64,
) -> Result<CardBrowserResultDto, CommandError> {
    ensure_active_deck(connection, deck_id)?;
    let now = unix_time_millis()?;
    let mut values = vec![Value::Text(deck_id.to_string())];
    let mut where_sql = String::from(
        " WHERE card_search.deck_id = ?\n\
            AND n.deleted_at IS NULL\n\
            AND c.deleted_at IS NULL\n\
            AND c.direction = 'forward'\n\
            AND d.deleted_at IS NULL\n\
            AND d.archived_at IS NULL",
    );

    if let Some(query) = fts_query {
        where_sql.push_str(" AND card_search MATCH ?");
        values.push(Value::Text(query.to_string()));
    }
    if matches!(status, BrowseStatus::Due) {
        values.push(Value::Integer(now));
    }
    where_sql.push_str(status.sql());

    if !tag_ids.is_empty() {
        where_sql.push_str(
            " AND n.id IN (\n\
                 SELECT nt.note_id FROM note_tags nt\n\
                 WHERE nt.tag_id IN (",
        );
        for (index, tag_id) in tag_ids.iter().enumerate() {
            if index > 0 {
                where_sql.push_str(", ");
            }
            where_sql.push('?');
            values.push(Value::Text(tag_id.clone()));
        }
        where_sql.push_str(
            ") GROUP BY nt.note_id HAVING COUNT(DISTINCT nt.tag_id) = ?)",
        );
        values.push(Value::Integer(
            i64::try_from(tag_ids.len()).unwrap_or(i64::MAX),
        ));
    }

    let from_sql = " FROM card_search\n\
                    JOIN notes n ON n.id = card_search.note_id\n\
                    JOIN cards c ON c.note_id = n.id\n\
                    JOIN decks d ON d.id = n.deck_id\n\
                    JOIN card_schedules s ON s.card_id = c.id";
    let count_sql = format!("SELECT COUNT(*){from_sql}{where_sql}");
    let total: i64 = connection
        .query_row(&count_sql, params_from_iter(values.iter()), |row| row.get(0))
        .map_err(CommandError::database)?;

    let select_sql = format!(
        "SELECT n.id, s.due_at, s.state, s.reps\n\
         {from_sql}{where_sql}\n\
         ORDER BY {}\n\
         LIMIT ? OFFSET ?",
        sort.sql(fts_query.is_some())
    );
    let mut select_values = values;
    select_values.push(Value::Integer(limit));
    select_values.push(Value::Integer(offset));
    let mut statement = connection
        .prepare(&select_sql)
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map(params_from_iter(select_values.iter()), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })
        .map_err(CommandError::database)?;
    let result_rows = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    drop(statement);

    let mut items = Vec::with_capacity(result_rows.len());
    for (note_id, due_at, schedule_state, review_count) in result_rows {
        items.push(CardBrowserItemDto {
            card: get_card_from_connection(connection, paths, &note_id)?,
            tags: load_tags_for_note(connection, &note_id)?,
            due_at,
            schedule_state,
            review_count,
        });
    }

    Ok(CardBrowserResultDto {
        items,
        total,
        tags: list_tag_summaries(connection, deck_id)?,
        limit,
        offset,
    })
}

pub(crate) fn build_fts_query(value: &str) -> Option<String> {
    if value.trim().is_empty() {
        return None;
    }
    let terms = value
        .split_whitespace()
        .filter_map(|token| {
            let normalized = token
                .chars()
                .filter(|character| character.is_alphanumeric())
                .take(MAX_TERM_CHARACTERS)
                .collect::<String>();
            (!normalized.is_empty()).then_some(normalized)
        })
        .take(MAX_SEARCH_TERMS)
        .map(|term| format!("\"{term}\"*"))
        .collect::<Vec<_>>();

    if terms.is_empty() {
        Some("\"__ostudy_no_match__\"".to_string())
    } else {
        Some(terms.join(" AND "))
    }
}
