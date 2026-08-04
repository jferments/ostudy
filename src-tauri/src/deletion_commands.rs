//! Stable Tauri command names for soft deletion with undo tokens.

use serde::Deserialize;
use tauri::State;

use crate::{
    database::Database,
    deletion::{self, DeletionResultDto},
    error::CommandError,
    util::validate_entity_id,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeleteCardsInput {
    pub note_ids: Vec<String>,
}

#[tauri::command(rename = "delete_card")]
pub fn delete_card_with_undo(
    database: State<'_, Database>,
    note_id: String,
) -> Result<DeletionResultDto, CommandError> {
    validate_entity_id(&note_id, "note")?;
    database.with_connection(|connection| {
        deletion::delete_cards(connection, vec![note_id], "Flashcard".to_string())
    })
}

#[tauri::command(rename = "delete_cards")]
pub fn delete_cards_with_undo(
    database: State<'_, Database>,
    input: DeleteCardsInput,
) -> Result<DeletionResultDto, CommandError> {
    let count = input
        .note_ids
        .iter()
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    let label = if count == 1 {
        "Flashcard".to_string()
    } else {
        format!("{count} flashcards")
    };
    database.with_connection(|connection| deletion::delete_cards(connection, input.note_ids, label))
}

#[tauri::command(rename = "delete_deck")]
pub fn delete_deck_with_undo(
    database: State<'_, Database>,
    deck_id: String,
) -> Result<DeletionResultDto, CommandError> {
    validate_entity_id(&deck_id, "deck")?;
    database.with_connection(|connection| deletion::delete_deck(connection, &deck_id))
}
