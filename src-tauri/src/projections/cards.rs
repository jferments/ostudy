use rusqlite::{params_from_iter, types::Value as SqlValue, Connection};
use serde::{Deserialize, Serialize};

use crate::{error::CommandError, util::validate_entity_id};

pub const MAX_DECK_CARD_PAGE: i64 = 100;
const MAX_DECK_CARD_QUERY_CHARS: usize = 500;

#[derive(Clone, Debug, Deserialize)]
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
    pub cursor: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeckCardSummary {
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
pub struct DeckCardSummaryPage {
    pub cards: Vec<DeckCardSummary>,
    pub total_count: i64,
    pub next_cursor: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Cursor {
    updated_at: i64,
    note_id: String,
}

pub fn page(
    connection: &Connection,
    input: DeckCardSummaryQuery,
) -> Result<DeckCardSummaryPage, CommandError> {
    validate_entity_id(&input.deck_id, "deck")?;
    if input.query.chars().count() > MAX_DECK_CARD_QUERY_CHARS {
        return Err(CommandError::new(
            "deck_card_search_too_long",
            format!("Search text cannot exceed {MAX_DECK_CARD_QUERY_CHARS} characters."),
        ));
    }
    if !matches!(input.sort.as_str(), "updated_desc" | "question_asc") {
        return Err(CommandError::new(
            "invalid_deck_card_sort",
            "Choose newest updated or question order.",
        ));
    }
    if !(1..=MAX_DECK_CARD_PAGE).contains(&input.limit) {
        return Err(CommandError::new(
            "invalid_deck_card_page",
            format!("Load between 1 and {MAX_DECK_CARD_PAGE} source notes per page."),
        ));
    }
    let cursor = input
        .cursor
        .as_deref()
        .map(serde_json::from_str::<Cursor>)
        .transpose()
        .map_err(|_| CommandError::new("invalid_deck_card_cursor", "Reload this deck page."))?;
    if let Some(value) = &cursor {
        validate_entity_id(&value.note_id, "note")?;
    }

    let mut values: Vec<SqlValue> = vec![input.deck_id.clone().into()];
    let mut conditions = vec![
        "n.deck_id = ?1".to_string(),
        "n.deleted_at IS NULL".to_string(),
        "d.deleted_at IS NULL".to_string(),
        "d.archived_at IS NULL".to_string(),
    ];
    let query = input.query.trim();
    if !query.is_empty() {
        values.push(format!("%{}%", query.replace('%', "\\%").replace('_', "\\_" )).into());
        conditions.push(format!(
            "(n.front_text LIKE ?{} ESCAPE '\\' COLLATE NOCASE \
              OR n.back_text LIKE ?{} ESCAPE '\\' COLLATE NOCASE)",
            values.len(),
            values.len()
        ));
    }
    if let Some(cursor) = cursor {
        if input.sort == "updated_desc" {
            values.push(cursor.updated_at.into());
            let updated_index = values.len();
            values.push(cursor.note_id.into());
            let note_index = values.len();
            conditions.push(format!(
                "(n.updated_at < ?{updated_index} \
                  OR (n.updated_at = ?{updated_index} AND n.id > ?{note_index}))"
            ));
        }
    }

    let total_count: i64 = connection
        .query_row(
            "SELECT COUNT(*)
             FROM notes n JOIN decks d ON d.id = n.deck_id
             WHERE n.deck_id = ?1 AND n.deleted_at IS NULL
               AND d.deleted_at IS NULL AND d.archived_at IS NULL",
            [input.deck_id.as_str()],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;

    values.push(input.limit.saturating_add(1).into());
    let limit_index = values.len();
    let order = if input.sort == "question_asc" {
        "n.front_text COLLATE NOCASE ASC, n.id ASC"
    } else {
        "n.updated_at DESC, n.id ASC"
    };
    let sql = format!(
        "SELECT
             n.id,
             COALESCE(MIN(CASE WHEN c.direction = 'forward' THEN c.id END), MIN(c.id)),
             substr(n.front_text, 1, 500),
             substr(n.back_text, 1, 500),
             COUNT(DISTINCT c.id),
             COUNT(DISTINCT CASE
                 WHEN a.deleted_at IS NULL AND a.lifecycle_state = 'ready' THEN a.id
             END),
             n.updated_at
         FROM notes n
         JOIN decks d ON d.id = n.deck_id
         JOIN cards c ON c.note_id = n.id AND c.deleted_at IS NULL
         LEFT JOIN note_assets na ON na.note_id = n.id
         LEFT JOIN assets a ON a.id = na.asset_id
         WHERE {}
         GROUP BY n.id, n.front_text, n.back_text, n.updated_at
         ORDER BY {order}
         LIMIT ?{limit_index}",
        conditions.join(" AND ")
    );
    let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
    let mut cards = statement
        .query_map(params_from_iter(values.iter()), |row| {
            Ok(DeckCardSummary {
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
    let has_more = cards.len() > usize::try_from(input.limit).unwrap_or_default();
    if has_more {
        cards.pop();
    }
    let next_cursor = if has_more && input.sort == "updated_desc" {
        cards.last().map(|card| {
            serde_json::to_string(&Cursor {
                updated_at: card.updated_at,
                note_id: card.note_id.clone(),
            })
            .map_err(CommandError::serialization)
        }).transpose()?
    } else {
        None
    };
    Ok(DeckCardSummaryPage {
        cards,
        total_count,
        next_cursor,
    })
}

fn default_sort() -> String {
    "updated_desc".to_string()
}

fn default_limit() -> i64 {
    50
}
