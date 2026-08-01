use rusqlite::{params, Connection, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::State;

use crate::{
    cards::{validate_content, ValidatedContent},
    database::Database,
    error::CommandError,
    scheduler::insert_new_schedule,
    util::{generate_id, unix_time_millis, validate_entity_id},
};

use super::{
    duplicates::{duplicate_key, load_duplicate_keys_for_deck},
    ensure_active_deck,
    tags::{normalize_tag_names, replace_note_tags, NormalizedTag},
};

const MAX_BULK_ROWS: usize = 500;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkCardRowInput {
    pub front_text: String,
    pub back_text: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkCreateCardsInput {
    pub deck_id: String,
    pub rows: Vec<BulkCardRowInput>,
    #[serde(default = "default_duplicate_policy")]
    pub duplicate_policy: String,
}

fn default_duplicate_policy() -> String {
    "skip".to_string()
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkSkippedRowDto {
    pub row_number: i64,
    pub reason: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkCreateResultDto {
    pub created_count: i64,
    pub skipped: Vec<BulkSkippedRowDto>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DuplicatePolicy {
    Skip,
    Allow,
    Error,
}

impl DuplicatePolicy {
    fn parse(value: &str) -> Result<Self, CommandError> {
        match value {
            "skip" => Ok(Self::Skip),
            "allow" => Ok(Self::Allow),
            "error" => Ok(Self::Error),
            _ => Err(CommandError::new(
                "invalid_duplicate_policy",
                "Choose whether duplicate bulk rows should be skipped, allowed, or stop the import.",
            )),
        }
    }
}

pub(crate) struct PreparedRow {
    row_number: i64,
    content: ValidatedContent,
    tags: Vec<NormalizedTag>,
}

#[tauri::command]
pub fn bulk_create_cards(
    database: State<'_, Database>,
    input: BulkCreateCardsInput,
) -> Result<BulkCreateResultDto, CommandError> {
    validate_entity_id(&input.deck_id, "deck")?;
    let policy = DuplicatePolicy::parse(&input.duplicate_policy)?;
    let rows = prepare_rows(input.rows)?;
    database.with_connection(|connection| {
        bulk_create_cards_in_connection(connection, &input.deck_id, rows, policy)
    })
}

pub(crate) fn prepare_rows(
    rows: Vec<BulkCardRowInput>,
) -> Result<Vec<PreparedRow>, CommandError> {
    if rows.is_empty() {
        return Err(CommandError::new(
            "empty_bulk_entry",
            "Add at least one question and answer row.",
        ));
    }
    if rows.len() > MAX_BULK_ROWS {
        return Err(CommandError::new(
            "too_many_bulk_rows",
            format!("Bulk entry accepts at most {MAX_BULK_ROWS} cards at a time."),
        ));
    }

    rows.into_iter()
        .enumerate()
        .map(|(index, row)| {
            let front_text = row.front_text.trim().to_string();
            let back_text = row.back_text.trim().to_string();
            if front_text.is_empty() || back_text.is_empty() {
                return Err(CommandError::new(
                    "invalid_bulk_row",
                    format!(
                        "Bulk row {} must contain both a question and an answer.",
                        index + 1
                    ),
                ));
            }
            let front_doc = plain_text_document(&front_text);
            let back_doc = plain_text_document(&back_text);
            Ok(PreparedRow {
                row_number: i64::try_from(index + 1).unwrap_or(i64::MAX),
                content: validate_content(front_doc, back_doc, front_text, back_text)?,
                tags: normalize_tag_names(row.tags)?,
            })
        })
        .collect()
}

pub(crate) fn bulk_create_cards_in_connection(
    connection: &mut Connection,
    deck_id: &str,
    rows: Vec<PreparedRow>,
    policy: DuplicatePolicy,
) -> Result<BulkCreateResultDto, CommandError> {
    let now = unix_time_millis()?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    ensure_active_deck(&transaction, deck_id)?;
    let mut duplicate_keys = load_duplicate_keys_for_deck(&transaction, deck_id)?;
    let mut created_count = 0_i64;
    let mut skipped = Vec::new();

    for row in rows {
        let key = duplicate_key(&row.content.front_text, &row.content.back_text);
        if duplicate_keys.contains(&key) && policy != DuplicatePolicy::Allow {
            if policy == DuplicatePolicy::Error {
                return Err(CommandError::new(
                    "duplicate_card",
                    format!(
                        "Bulk row {} duplicates a card already in this deck.",
                        row.row_number
                    ),
                ));
            }
            skipped.push(BulkSkippedRowDto {
                row_number: row.row_number,
                reason: "An identical question and answer already exist in this deck.".to_string(),
            });
            continue;
        }

        insert_prepared_row(&transaction, deck_id, &row, now)?;
        duplicate_keys.insert(key);
        created_count += 1;
    }

    if created_count > 0 {
        transaction
            .execute(
                "UPDATE decks\n\
                 SET updated_at = ?1, revision = revision + 1\n\
                 WHERE id = ?2 AND deleted_at IS NULL",
                params![now, deck_id],
            )
            .map_err(CommandError::database)?;
    }
    transaction.commit().map_err(CommandError::database)?;

    Ok(BulkCreateResultDto {
        created_count,
        skipped,
    })
}

fn insert_prepared_row(
    connection: &Connection,
    deck_id: &str,
    row: &PreparedRow,
    now: i64,
) -> Result<(), CommandError> {
    let note_id = generate_id(connection)?;
    let card_id = generate_id(connection)?;
    connection
        .execute(
            "INSERT INTO notes (\n\
                 id, deck_id, note_type, content_format, content_schema_version,\n\
                 front_doc, back_doc, front_text, back_text, created_at, updated_at, revision\n\
             ) VALUES (?1, ?2, 'basic', 'tiptap-json', 1, ?3, ?4, ?5, ?6, ?7, ?7, 1)",
            params![
                note_id,
                deck_id,
                row.content.front_doc_json,
                row.content.back_doc_json,
                row.content.front_text,
                row.content.back_text,
                now,
            ],
        )
        .map_err(CommandError::database)?;
    connection
        .execute(
            "INSERT INTO cards (\n\
                 id, note_id, deck_id, direction, template_ordinal, created_at, updated_at\n\
             ) VALUES (?1, ?2, ?3, 'forward', 0, ?4, ?4)",
            params![card_id, note_id, deck_id, now],
        )
        .map_err(CommandError::database)?;
    insert_new_schedule(connection, &card_id, now)?;
    replace_note_tags(connection, &note_id, &row.tags, now)
}

pub(crate) fn plain_text_document(value: &str) -> Value {
    let normalized = value.replace("\r\n", "\n").replace('\r', "\n");
    let content = normalized
        .split('\n')
        .map(|line| {
            if line.is_empty() {
                json!({ "type": "paragraph" })
            } else {
                json!({
                    "type": "paragraph",
                    "content": [{ "type": "text", "text": line }]
                })
            }
        })
        .collect::<Vec<_>>();
    json!({ "type": "doc", "content": content })
}
