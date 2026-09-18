//! Canonical interactive-note use cases. The question domain owns admission and
//! evaluation; this service owns SQL transactions, assets, tags and stable cards.
//! A committed receipt is returned without a second fallible hydration step.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    assets::AssetDto,
    cards::questions::{QuestionDefinition, ValidatedQuestion},
    database::Database,
    editor_drafts::EditorDraftCommitRef,
    error::CommandError,
    paths::AppPaths,
    util::validate_entity_id,
};

use super::super::{ensure_active_deck, load_tag_names, replace_note_tags, validate_tags};

mod generated;
mod persistence;
mod preview;
pub(crate) use generated::insert_generated;
#[tauri::command]
pub fn preview_question_draft(
    input: preview::PreviewQuestionDraftInput,
) -> Result<preview::QuestionDraftPreviewDto, CommandError> {
    preview::preview_question_draft(input)
}

#[cfg(test)]
mod tests;

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveQuestionNoteInput {
    pub operation_id: String,
    pub note_id: Option<String>,
    pub revision: Option<i64>,
    pub deck_id: String,
    pub definition_json: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub allow_duplicate: bool,
    pub asset_draft_id: Option<String>,
    pub editor_draft: Option<EditorDraftCommitRef>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QuestionWriteReceipt {
    pub operation_id: String,
    pub note_id: String,
    pub note_revision: i64,
    pub card_ids: Vec<String>,
    pub committed_at: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestionNoteDto {
    pub note_id: String,
    pub deck_id: String,
    pub definition: QuestionDefinition,
    pub assets: Vec<AssetDto>,
    pub tags: Vec<String>,
    pub card_ids: Vec<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub revision: i64,
}

#[tauri::command]
pub fn save_question_note(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: SaveQuestionNoteInput,
) -> Result<QuestionWriteReceipt, CommandError> {
    database.with_connection(|connection| persistence::save(connection, &paths, input))
}

#[tauri::command]
pub fn get_question_note(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    note_id: String,
) -> Result<QuestionNoteDto, CommandError> {
    validate_entity_id(&note_id, "question note")?;
    database.with_connection(|connection| load(connection, &paths, &note_id))
}

pub(crate) fn load(
    connection: &Connection,
    paths: &AppPaths,
    note_id: &str,
) -> Result<QuestionNoteDto, CommandError> {
    validate_entity_id(note_id, "question note")?;
    let stored = connection
        .query_row(
            "SELECT n.deck_id, n.question_definition, n.created_at, n.updated_at, n.revision
             FROM notes n JOIN decks d ON d.id = n.deck_id
             WHERE n.id = ?1 AND n.note_type = 'interactive' AND n.deleted_at IS NULL
               AND d.deleted_at IS NULL AND d.archived_at IS NULL",
            [note_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            },
        )
        .optional()
        .map_err(CommandError::database)?
        .ok_or_else(not_found)?;
    let question = ValidatedQuestion::parse(&stored.1)?;
    let mut statement = connection
        .prepare(
            "SELECT qp.question_key, c.id FROM card_question_presentations qp
             JOIN cards c ON c.id = qp.card_id
             WHERE qp.note_id = ?1 AND c.deleted_at IS NULL",
        )
        .map_err(CommandError::database)?;
    let cards = statement
        .query_map([note_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(CommandError::database)?
        .collect::<Result<std::collections::BTreeMap<_, _>, _>>()
        .map_err(CommandError::database)?;
    let card_ids = question
        .presentation_keys()
        .iter()
        .map(|key| {
            cards.get(key).cloned().ok_or_else(|| {
                CommandError::new(
                    "question_presentation_missing",
                    "A saved question has no corresponding study presentation.",
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(QuestionNoteDto {
        note_id: note_id.to_string(),
        deck_id: stored.0,
        definition: question.definition().clone(),
        assets: crate::assets::get_assets_for_note(connection, paths, note_id)?,
        tags: load_tag_names(connection, note_id)?,
        card_ids,
        created_at: stored.2,
        updated_at: stored.3,
        revision: stored.4,
    })
}

fn not_found() -> CommandError {
    CommandError::new("card_not_found", "That question no longer exists.")
}

fn changed() -> CommandError {
    CommandError::new(
        "card_changed",
        "This question changed in another window. Reload it before saving.",
    )
}

fn operation_conflict() -> CommandError {
    CommandError::new(
        "question_operation_conflict",
        "That save identity was already used for a different question change.",
    )
}
