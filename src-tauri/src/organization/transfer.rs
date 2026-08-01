use std::collections::BTreeSet;

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    database::Database,
    error::CommandError,
    scheduler::insert_new_schedule,
    util::{generate_id, unix_time_millis, validate_entity_id},
};

use super::{
    duplicates::{duplicate_key, load_duplicate_keys_for_deck},
    ensure_active_deck,
};

const MAX_TRANSFER_NOTES: usize = 500;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferCardsInput {
    pub note_ids: Vec<String>,
    pub destination_deck_id: String,
    pub mode: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkippedTransferDto {
    pub note_id: String,
    pub reason: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferCardsResultDto {
    pub transferred_count: i64,
    pub skipped: Vec<SkippedTransferDto>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TransferMode {
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
                "Choose whether the selected cards should be moved or copied.",
            )),
        }
    }
}

struct SourceNote {
    id: String,
    deck_id: String,
    note_type: String,
    content_format: String,
    content_schema_version: i64,
    front_doc: String,
    back_doc: String,
    front_text: String,
    back_text: String,
}

#[tauri::command]
pub fn transfer_cards(
    database: State<'_, Database>,
    input: TransferCardsInput,
) -> Result<TransferCardsResultDto, CommandError> {
    validate_entity_id(&input.destination_deck_id, "destination deck")?;
    if input.note_ids.is_empty() {
        return Err(CommandError::new(
            "empty_card_selection",
            "Select at least one flashcard to move or copy.",
        ));
    }
    if input.note_ids.len() > MAX_TRANSFER_NOTES {
        return Err(CommandError::new(
            "too_many_selected_cards",
            format!("Move or copy at most {MAX_TRANSFER_NOTES} flashcards at a time."),
        ));
    }
    let mut note_ids = Vec::new();
    let mut seen = BTreeSet::new();
    for note_id in input.note_ids {
        validate_entity_id(&note_id, "note")?;
        if seen.insert(note_id.clone()) {
            note_ids.push(note_id);
        }
    }
    let mode = TransferMode::parse(&input.mode)?;
    database.with_connection(|connection| {
        transfer_cards_in_connection(
            connection,
            &note_ids,
            &input.destination_deck_id,
            mode,
        )
    })
}

pub(crate) fn transfer_cards_in_connection(
    connection: &mut Connection,
    note_ids: &[String],
    destination_deck_id: &str,
    mode: TransferMode,
) -> Result<TransferCardsResultDto, CommandError> {
    let now = unix_time_millis()?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    ensure_active_deck(&transaction, destination_deck_id)?;
    let mut duplicate_keys = load_duplicate_keys_for_deck(&transaction, destination_deck_id)?;
    let mut affected_source_decks = BTreeSet::new();
    let mut transferred_count = 0_i64;
    let mut skipped = Vec::new();

    for note_id in note_ids {
        let source = load_source_note(&transaction, note_id)?;
        if source.deck_id == destination_deck_id {
            return Err(CommandError::new(
                "same_destination_deck",
                "Choose a different destination deck.",
            ));
        }

        let key = duplicate_key(&source.front_text, &source.back_text);
        if duplicate_keys.contains(&key) {
            skipped.push(SkippedTransferDto {
                note_id: source.id,
                reason: "An identical question and answer already exist in the destination deck."
                    .to_string(),
            });
            continue;
        }

        match mode {
            TransferMode::Move => {
                transaction
                    .execute(
                        "UPDATE notes\n\
                         SET deck_id = ?1, updated_at = ?2, revision = revision + 1\n\
                         WHERE id = ?3 AND deleted_at IS NULL",
                        params![destination_deck_id, now, source.id],
                    )
                    .map_err(CommandError::database)?;
                transaction
                    .execute(
                        "UPDATE cards\n\
                         SET deck_id = ?1, updated_at = ?2\n\
                         WHERE note_id = ?3 AND deleted_at IS NULL",
                        params![destination_deck_id, now, source.id],
                    )
                    .map_err(CommandError::database)?;
            }
            TransferMode::Copy => {
                copy_note(&transaction, &source, destination_deck_id, now)?;
            }
        }

        affected_source_decks.insert(source.deck_id);
        duplicate_keys.insert(key);
        transferred_count += 1;
    }

    if transferred_count > 0 {
        transaction
            .execute(
                "UPDATE decks\n\
                 SET updated_at = ?1, revision = revision + 1\n\
                 WHERE id = ?2 AND deleted_at IS NULL",
                params![now, destination_deck_id],
            )
            .map_err(CommandError::database)?;
        if mode == TransferMode::Move {
            for source_deck_id in affected_source_decks {
                transaction
                    .execute(
                        "UPDATE decks\n\
                         SET updated_at = ?1, revision = revision + 1\n\
                         WHERE id = ?2 AND deleted_at IS NULL",
                        params![now, source_deck_id],
                    )
                    .map_err(CommandError::database)?;
            }
        }
    }

    transaction.commit().map_err(CommandError::database)?;
    Ok(TransferCardsResultDto {
        transferred_count,
        skipped,
    })
}

fn load_source_note(
    connection: &Connection,
    note_id: &str,
) -> Result<SourceNote, CommandError> {
    connection
        .query_row(
            "SELECT n.id, n.deck_id, n.note_type, n.content_format,\n\
                    n.content_schema_version, n.front_doc, n.back_doc,\n\
                    n.front_text, n.back_text\n\
             FROM notes n\n\
             JOIN decks d ON d.id = n.deck_id\n\
             WHERE n.id = ?1\n\
               AND n.deleted_at IS NULL\n\
               AND d.deleted_at IS NULL\n\
               AND d.archived_at IS NULL",
            [note_id],
            |row| {
                Ok(SourceNote {
                    id: row.get(0)?,
                    deck_id: row.get(1)?,
                    note_type: row.get(2)?,
                    content_format: row.get(3)?,
                    content_schema_version: row.get(4)?,
                    front_doc: row.get(5)?,
                    back_doc: row.get(6)?,
                    front_text: row.get(7)?,
                    back_text: row.get(8)?,
                })
            },
        )
        .optional()
        .map_err(CommandError::database)?
        .ok_or_else(|| CommandError::new("card_not_found", "A selected card no longer exists."))
}

fn copy_note(
    connection: &Connection,
    source: &SourceNote,
    destination_deck_id: &str,
    now: i64,
) -> Result<(), CommandError> {
    let new_note_id = generate_id(connection)?;
    connection
        .execute(
            "INSERT INTO notes (\n\
                 id, deck_id, note_type, content_format, content_schema_version,\n\
                 front_doc, back_doc, front_text, back_text, created_at, updated_at, revision\n\
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10, 1)",
            params![
                new_note_id,
                destination_deck_id,
                source.note_type,
                source.content_format,
                source.content_schema_version,
                source.front_doc,
                source.back_doc,
                source.front_text,
                source.back_text,
                now,
            ],
        )
        .map_err(CommandError::database)?;

    let mut statement = connection
        .prepare(
            "SELECT direction, template_ordinal\n\
             FROM cards\n\
             WHERE note_id = ?1 AND deleted_at IS NULL\n\
             ORDER BY template_ordinal ASC, direction ASC",
        )
        .map_err(CommandError::database)?;
    let generated_cards = statement
        .query_map([&source.id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    drop(statement);
    if generated_cards.is_empty() {
        return Err(CommandError::new(
            "card_not_found",
            "A selected note no longer has an active flashcard.",
        ));
    }

    for (direction, template_ordinal) in generated_cards {
        let new_card_id = generate_id(connection)?;
        connection
            .execute(
                "INSERT INTO cards (\n\
                     id, note_id, deck_id, direction, template_ordinal, created_at, updated_at\n\
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
                params![
                    new_card_id,
                    new_note_id,
                    destination_deck_id,
                    direction,
                    template_ordinal,
                    now,
                ],
            )
            .map_err(CommandError::database)?;
        insert_new_schedule(connection, &new_card_id, now)?;
    }

    connection
        .execute(
            "INSERT INTO note_assets (note_id, asset_id, side)\n\
             SELECT ?1, asset_id, side FROM note_assets WHERE note_id = ?2",
            params![new_note_id, source.id],
        )
        .map_err(CommandError::database)?;
    connection
        .execute(
            "INSERT INTO note_tags (note_id, tag_id)\n\
             SELECT ?1, tag_id FROM note_tags WHERE note_id = ?2",
            params![new_note_id, source.id],
        )
        .map_err(CommandError::database)?;
    Ok(())
}
