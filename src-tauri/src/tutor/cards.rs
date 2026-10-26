//! Flashcards made from a Tutor answer. The flashcard model drafts them from
//! the finished answer's saved text; the learner chooses and edits the ones to
//! keep, and they are saved as ordinary cards, so this writes nothing.

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::{
    ai::{
        document_intelligence::{self, AnswerCardDraft},
        document_provider::load_document_provider_capabilities,
        AiRuntimeState,
    },
    database::Database,
    error::CommandError,
    knowledge::runtime::TaskGuard,
    util::validate_entity_id,
};

use super::figures;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DraftTutorAnswerCardsInput {
    pub operation_id: String,
    pub turn_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TutorAnswerCardsDto {
    pub cards: Vec<AnswerCardDraft>,
}

pub(crate) async fn draft_tutor_answer_cards(
    database: &Database,
    runtime: &AiRuntimeState,
    input: DraftTutorAnswerCardsInput,
) -> Result<TutorAnswerCardsDto, CommandError> {
    validate_entity_id(&input.operation_id, "card operation")?;
    let answer = database.with_connection(|connection| answer_text(connection, &input.turn_id))?;
    let guard = TaskGuard::begin(
        &input.operation_id,
        &format!("tutor-cards:{}", input.turn_id),
        false,
    )?;
    // The flashcard model writes them without reasoning, like the cards made
    // from a document.
    let mut task = document_intelligence::load_document_task_context(database)?;
    let capabilities = load_document_provider_capabilities(
        database,
        &task.profile_id,
        &task.provider_kind,
        &task.model_id,
    )?;
    task.apply_provider_capabilities(&capabilities);
    let cards =
        document_intelligence::request_answer_cards(runtime, &task, &answer, &guard.signal).await?;
    guard.check()?;
    Ok(TutorAnswerCardsDto { cards })
}

/// A finished explanation's text without the figures it shows.
pub(crate) fn answer_text(
    connection: &rusqlite::Connection,
    turn_id: &str,
) -> Result<String, CommandError> {
    validate_entity_id(turn_id, "Tutor answer")?;
    let text: Option<String> = connection
        .query_row(
            "SELECT assistant_text FROM tutor_turns
             WHERE id = ?1 AND state = 'complete' AND mode = 'explain'",
            params![turn_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(CommandError::database)?;
    let text = text
        .map(|text| figures::keep_offered(&text, &[]))
        .unwrap_or_default();
    if text.trim().is_empty() {
        return Err(CommandError::new(
            "tutor_answer_has_no_cards",
            "Only a finished answer can be made into flashcards.",
        ));
    }
    Ok(text)
}
