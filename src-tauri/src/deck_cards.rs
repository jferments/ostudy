use rusqlite::{params_from_iter, types::Value as SqlValue, Connection};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    database::Database, error::CommandError, projections::scope::push_value,
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
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex,
    };

    use rusqlite::{
        params,
        trace::{TraceEvent, TraceEventCodes},
        TransactionBehavior,
    };

    use super::*;

    static TRACE_LOCK: Mutex<()> = Mutex::new(());
    static SELECT_STATEMENT_COUNT: AtomicUsize = AtomicUsize::new(0);

    fn count_select_statements(event: TraceEvent<'_>) {
        if let TraceEvent::Stmt(_, sql) = event {
            if sql.trim_start().to_ascii_uppercase().starts_with("SELECT") {
                SELECT_STATEMENT_COUNT.fetch_add(1, Ordering::SeqCst);
            }
        }
    }

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

    #[test]
    fn deck_card_page_uses_two_selects_for_one_hundred_and_one_thousand_notes() {
        let _guard = TRACE_LOCK
            .lock()
            .expect("trace test lock should be available");
        for note_count in [1_usize, 100, 1_000] {
            let database = Database::open_in_memory().expect("database should open");
            database
                .with_connection(|connection| {
                    let deck_id = "1".repeat(32);
                    connection
                        .execute(
                            "INSERT INTO decks (
                                 id, name, description, sort_order,
                                 created_at, updated_at, revision
                             ) VALUES (?1, 'Large deck', '', 0, 1, 1, 1)",
                            [deck_id.as_str()],
                        )
                        .map_err(CommandError::database)?;
                    let transaction = connection
                        .transaction_with_behavior(TransactionBehavior::Immediate)
                        .map_err(CommandError::database)?;
                    for index in 0..note_count {
                        let note_id = format!("{:032x}", index + 2);
                        let card_id = format!("{:032x}", index + note_count + 2);
                        transaction
                            .execute(
                                "INSERT INTO notes (
                                     id, deck_id, note_type, content_format,
                                     content_schema_version, front_doc, back_doc,
                                     front_text, back_text, created_at, updated_at,
                                     revision, duplicate_hash, reverse_enabled,
                                     cloze_source, cloze_mode
                                 ) VALUES (
                                     ?1, ?2, 'basic', 'tiptap-json', 1,
                                     '{}', '{}', ?3, 'Answer', ?4, ?4, 1,
                                     ?5, 0, '', 'one_at_a_time'
                                 )",
                                params![
                                    note_id,
                                    deck_id,
                                    format!("Question {index}"),
                                    i64::try_from(index).unwrap_or(i64::MAX) + 2,
                                    format!("{:064x}", index + 1),
                                ],
                            )
                            .map_err(CommandError::database)?;
                        transaction
                            .execute(
                                "INSERT INTO cards (
                                     id, note_id, deck_id, direction,
                                     template_ordinal, created_at, updated_at
                                 ) VALUES (?1, ?2, ?3, 'forward', 0, 1, 1)",
                                params![card_id, note_id, deck_id],
                            )
                            .map_err(CommandError::database)?;
                    }
                    transaction.commit().map_err(CommandError::database)?;

                    SELECT_STATEMENT_COUNT.store(0, Ordering::SeqCst);
                    connection.trace_v2(
                        TraceEventCodes::SQLITE_TRACE_STMT,
                        Some(count_select_statements),
                    );
                    let page = list_from_connection(
                        connection,
                        DeckCardSummaryQuery {
                            deck_id,
                            query: String::new(),
                            sort: default_sort(),
                            limit: 50,
                            offset: 0,
                        },
                    )?;
                    connection.trace_v2(TraceEventCodes::SQLITE_TRACE_STMT, None);

                    assert_eq!(
                        SELECT_STATEMENT_COUNT.load(Ordering::SeqCst),
                        2,
                        "deck page statement count grew for {note_count} notes"
                    );
                    assert_eq!(
                        page.total_count,
                        i64::try_from(note_count).unwrap_or(i64::MAX)
                    );
                    assert_eq!(page.cards.len(), note_count.min(50));
                    Ok(())
                })
                .expect("bounded deck page should load");
        }
    }
}
