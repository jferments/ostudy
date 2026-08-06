use rusqlite::{params_from_iter, types::Value as SqlValue, Connection};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    database::Database,
    error::CommandError,
    projections::scope::push_value,
    util::validate_entity_id,
};

const MAX_DECK_CARD_QUERY_CHARS: usize = 500;
const MAX_DECK_CARD_PAGE: i64 = 100;
const SUMMARY_EXCERPT_CHARS: i64 = 500;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeckCardSummaryQuery {
    pub deck_id: String,
    #[serde(default)]
    pub query: String,
    #[serde(default = "default_sort")]
    pub sort: String,
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeckCardSummaryDto {
    pub note_id: String,
    pub primary_card_id: String,
    pub question_excerpt: String,
    pub answer_excerpt: String,
    pub generated_card_count: i64,
    pub image_count: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeckCardSummaryPageDto {
    pub cards: Vec<DeckCardSummaryDto>,
    pub total_count: i64,
    pub offset: i64,
    pub has_more: bool,
}

#[tauri::command]
pub fn list_deck_card_summaries(
    database: State<'_, Database>,
    input: DeckCardSummaryQuery,
) -> Result<DeckCardSummaryPageDto, CommandError> {
    validate_input(&input)?;
    database.with_connection(|connection| list_from_connection(connection, input))
}

fn list_from_connection(
    connection: &Connection,
    input: DeckCardSummaryQuery,
) -> Result<DeckCardSummaryPageDto, CommandError> {
    let mut values = vec![SqlValue::Text(input.deck_id.clone())];
    let mut conditions = vec![
        "n.deck_id = ?1".to_string(),
        "n.deleted_at IS NULL".to_string(),
        "d.deleted_at IS NULL".to_string(),
        "d.archived_at IS NULL".to_string(),
    ];
    let normalized_query = input.query.trim();
    if !normalized_query.is_empty() {
        let pattern = format!("%{}%", escape_like(normalized_query));
        let index = push_value(&mut values, pattern);
        conditions.push(format!(
            "(n.front_text LIKE ?{index} ESCAPE '\\' COLLATE NOCASE
               OR n.back_text LIKE ?{index} ESCAPE '\\' COLLATE NOCASE)"
        ));
    }
    let where_sql = conditions.join(" AND ");
    let total_count: i64 = connection
        .query_row(
            &format!(
                "SELECT COUNT(*)
                 FROM notes n
                 JOIN decks d ON d.id = n.deck_id
                 WHERE {where_sql}"
            ),
            params_from_iter(values.iter()),
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;

    let order_sql = match input.sort.as_str() {
        "question_asc" => "n.front_text COLLATE NOCASE ASC, n.updated_at DESC, n.id ASC",
        _ => "n.updated_at DESC, n.created_at DESC, n.id ASC",
    };
    let excerpt_index = push_value(&mut values, SUMMARY_EXCERPT_CHARS);
    let limit_index = push_value(&mut values, input.limit);
    let offset_index = push_value(&mut values, input.offset);
    let sql = format!(
        "SELECT
             n.id,
             MIN(CASE WHEN c.direction = 'forward' THEN c.id END) AS primary_card_id,
             substr(n.front_text, 1, ?{excerpt_index}),
             substr(n.back_text, 1, ?{excerpt_index}),
             COUNT(DISTINCT CASE WHEN c.deleted_at IS NULL THEN c.id END),
             COUNT(DISTINCT CASE
                 WHEN a.deleted_at IS NULL AND a.lifecycle_state = 'ready' THEN a.id
             END),
             n.updated_at
         FROM notes n
         JOIN decks d ON d.id = n.deck_id
         LEFT JOIN cards c ON c.note_id = n.id
         LEFT JOIN note_assets na ON na.note_id = n.id
         LEFT JOIN assets a ON a.id = na.asset_id
         WHERE {where_sql}
         GROUP BY n.id, n.front_text, n.back_text, n.updated_at, n.created_at
         HAVING MIN(CASE WHEN c.direction = 'forward' THEN c.id END) IS NOT NULL
         ORDER BY {order_sql}
         LIMIT ?{limit_index} OFFSET ?{offset_index}"
    );
    let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
    let cards = statement
        .query_map(params_from_iter(values.iter()), |row| {
            Ok(DeckCardSummaryDto {
                note_id: row.get(0)?,
                primary_card_id: row.get(1)?,
                question_excerpt: row.get(2)?,
                answer_excerpt: row.get(3)?,
                generated_card_count: row.get(4)?,
                image_count: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    let returned = i64::try_from(cards.len()).unwrap_or(i64::MAX);
    Ok(DeckCardSummaryPageDto {
        cards,
        total_count,
        offset: input.offset,
        has_more: input.offset.saturating_add(returned) < total_count,
    })
}

fn escape_like(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

fn validate_input(input: &DeckCardSummaryQuery) -> Result<(), CommandError> {
    validate_entity_id(&input.deck_id, "deck")?;
    if input.query.chars().count() > MAX_DECK_CARD_QUERY_CHARS {
        return Err(CommandError::new(
            "deck_card_search_too_long",
            format!("Deck search text cannot exceed {MAX_DECK_CARD_QUERY_CHARS} characters."),
        ));
    }
    if !matches!(input.sort.as_str(), "updated_desc" | "question_asc") {
        return Err(CommandError::new(
            "invalid_deck_card_sort",
            "Sort deck cards by recent update or question.",
        ));
    }
    if !(1..=MAX_DECK_CARD_PAGE).contains(&input.limit) || input.offset < 0 {
        return Err(CommandError::new(
            "invalid_deck_card_page",
            "The requested deck-card page is invalid.",
        ));
    }
    Ok(())
}

fn default_sort() -> String {
    "updated_desc".to_string()
}

const fn default_limit() -> i64 {
    50
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_limits_are_bounded() {
        let error = validate_input(&DeckCardSummaryQuery {
            deck_id: "a".repeat(32),
            query: String::new(),
            sort: default_sort(),
            limit: 101,
            offset: 0,
        })
        .expect_err("oversized pages should fail");
        assert_eq!(error.code, "invalid_deck_card_page");
    }
}
