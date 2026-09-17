//! Fresh generated objective notes join the caller's existing publication
//! transaction. No nested save transaction, author draft, imported schedule or
//! synthesized review is created here.

use rusqlite::Transaction;

use super::*;
use crate::{
    cards::questions::QuestionKind,
    scheduler::insert_new_schedule,
    util::generate_id,
};

pub(crate) fn insert_generated(
    transaction: &Transaction<'_>,
    note_id: &str,
    deck_id: &str,
    question: &ValidatedQuestion,
    now: i64,
) -> Result<usize, CommandError> {
    validate_entity_id(note_id, "generated note")?;
    validate_entity_id(deck_id, "deck")?;
    ensure_active_deck(transaction, deck_id)?;
    if question.kind() == QuestionKind::ImageOcclusion || !question.asset_ids().is_empty() {
        return Err(CommandError::new(
            "unsupported_generated_question",
            "Automatic generation accepts objective text questions, not image occlusion or media ownership.",
        ));
    }
    let text = question.search_text()?;
    transaction
        .execute(
            "INSERT INTO notes (
                 id, deck_id, note_type, content_format, content_schema_version,
                 question_definition, front_text, back_text, duplicate_hash,
                 created_at, updated_at, revision
             ) VALUES (?1, ?2, 'interactive', 'question-json', 1,
                 ?3, ?4, ?5, ?6, ?7, ?7, 1)",
            params![
                note_id,
                deck_id,
                question.to_json()?,
                text.prompt,
                text.answer,
                question.duplicate_hash()?,
                now
            ],
        )
        .map_err(CommandError::database)?;
    let keys = question.presentation_keys();
    for (ordinal, key) in keys.iter().enumerate() {
        let card_id = generate_id(transaction)?;
        transaction
            .execute(
                "INSERT INTO cards (
                     id, note_id, deck_id, direction, template_ordinal, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, 'forward', ?4, ?5, ?5)",
                params![card_id, note_id, deck_id, ordinal as i64, now],
            )
            .map_err(CommandError::database)?;
        transaction
            .execute(
                "INSERT INTO card_question_presentations (card_id, note_id, question_key)
                 VALUES (?1, ?2, ?3)",
                params![card_id, note_id, key],
            )
            .map_err(CommandError::database)?;
        insert_new_schedule(transaction, &card_id, now)?;
    }
    Ok(keys.len())
}
