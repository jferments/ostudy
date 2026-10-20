//! Flashcards made from the marks a learner leaves on a document.
//!
//! The annotator's "Make a card" saves an ordinary card note first, then links
//! it here to the highlight or note it came from (release schema 10,
//! `note_annotation_sources`). The link keeps the page and a copy of the marked
//! words, and gives the note a page citation when it has none, so the card's
//! Source shows where it came from. "Study what I marked" practises every card
//! linked to a document's marks.
//!
//! The links stay on the device that made them, like the annotations: device
//! sync leaves them out, `.studydeck` files never carry them, and whole-library
//! backups keep them.

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    ai::{
        document_intelligence, document_provider::load_document_provider_capabilities,
        AiRuntimeState,
    },
    database::Database,
    document_citations::upsert_document_snapshot,
    document_domain::{SourceLocator, SourceUnitKind},
    error::CommandError,
    knowledge::runtime::TaskGuard,
    util::{unix_time_millis, validate_entity_id},
};

/// The longest marked text a link keeps; a sticky note holds up to this much.
const MAX_QUOTE_CHARS: usize = 20_000;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LinkCardNoteToAnnotationInput {
    pub note_id: String,
    pub annotation_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GetDocumentMarkedCardsInput {
    pub document_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GetCardMarkSourceInput {
    pub note_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DraftMarkQuestionInput {
    pub operation_id: String,
    pub annotation_id: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkQuestionDraftDto {
    pub question: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardMarkSourceDto {
    pub note_id: String,
    pub document_id: Option<String>,
    pub annotation_id: Option<String>,
    pub page_number: i64,
    pub quote: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentMarkedCardsDto {
    pub document_id: String,
    /// Cards made from this document's marks, oldest first.
    pub card_ids: Vec<String>,
    /// Where a new card from this document goes unless the learner picks
    /// another deck: the deck its cards were generated into, else the deck of
    /// the last card made from one of its marks.
    pub suggested_deck_id: Option<String>,
}

#[tauri::command]
pub fn link_card_note_to_annotation(
    database: State<'_, Database>,
    input: LinkCardNoteToAnnotationInput,
) -> Result<CardMarkSourceDto, CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| link_note(connection, input, now))
}

#[tauri::command]
pub fn get_document_marked_cards(
    database: State<'_, Database>,
    input: GetDocumentMarkedCardsInput,
) -> Result<DocumentMarkedCardsDto, CommandError> {
    database.with_connection(|connection| marked_cards(connection, input.document_id))
}

/// The mark a card was made from, so its source can open at that mark.
#[tauri::command]
pub fn get_card_mark_source(
    database: State<'_, Database>,
    input: GetCardMarkSourceInput,
) -> Result<Option<CardMarkSourceDto>, CommandError> {
    database.with_connection(|connection| mark_source(connection, &input.note_id))
}

/// A question the learner's flashcard model writes for a mark's words. The
/// learner edits it in the new card before anything is saved.
pub async fn draft_mark_question(
    database: &Database,
    runtime: &AiRuntimeState,
    input: DraftMarkQuestionInput,
) -> Result<MarkQuestionDraftDto, CommandError> {
    validate_entity_id(&input.operation_id, "question operation")?;
    let mark =
        database.with_connection(|connection| mark_words(connection, &input.annotation_id))?;
    let guard = TaskGuard::begin(
        &input.operation_id,
        &format!("mark-question:{}", input.annotation_id),
        false,
    )?;
    // The flashcard model writes it without reasoning, like the cards made
    // from a whole document.
    let mut task = document_intelligence::load_document_task_context(database)?;
    let capabilities = load_document_provider_capabilities(
        database,
        &task.profile_id,
        &task.provider_kind,
        &task.model_id,
    )?;
    task.apply_provider_capabilities(&capabilities);
    let question = document_intelligence::request_mark_question(
        runtime,
        &task,
        &document_intelligence::MarkQuestionSource {
            document_title: &mark.document_title,
            page_number: mark.page_number,
            words: &mark.words,
        },
        &guard.signal,
    )
    .await?;
    guard.check()?;
    Ok(MarkQuestionDraftDto { question })
}

/// What a question for a mark is asked about.
#[derive(Debug, PartialEq)]
pub(crate) struct MarkWords {
    pub document_title: String,
    pub page_number: i64,
    pub words: String,
}

/// A mark's words and where it is; strokes and shapes hold no words to ask about.
pub(crate) fn mark_words(
    connection: &Connection,
    annotation_id: &str,
) -> Result<MarkWords, CommandError> {
    validate_entity_id(annotation_id, "annotation")?;
    let (document_title, page_number, payload): (String, i64, String) = connection
        .query_row(
            "SELECT document.title, annotation.page_number, annotation.payload_json
             FROM document_annotations AS annotation
             JOIN documents AS document ON document.id = annotation.document_id
             WHERE annotation.id = ?1",
            [annotation_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(CommandError::database)?
        .ok_or_else(|| {
            CommandError::new(
                "document_annotation_not_found",
                "That mark is no longer on the document.",
            )
        })?;
    let words = quote_from_payload(&payload);
    if words.is_empty() {
        return Err(CommandError::new(
            "mark_has_no_words",
            "Only a highlight of the PDF's text or a note can suggest a question.",
        ));
    }
    Ok(MarkWords {
        document_title,
        page_number,
        words,
    })
}

/// A note's link to its mark, or none for a card not made from one. An
/// erased mark leaves the link without an annotation id.
pub(crate) fn mark_source(
    connection: &Connection,
    note_id: &str,
) -> Result<Option<CardMarkSourceDto>, CommandError> {
    validate_entity_id(note_id, "card")?;
    connection
        .query_row(
            "SELECT link.note_id, link.document_id, annotation.id,
                    link.page_number, link.quote
             FROM note_annotation_sources AS link
             LEFT JOIN document_annotations AS annotation
               ON annotation.id = link.annotation_id
              AND annotation.document_id = link.document_id
             WHERE link.note_id = ?1",
            [note_id],
            |row| {
                Ok(CardMarkSourceDto {
                    note_id: row.get(0)?,
                    document_id: row.get(1)?,
                    annotation_id: row.get(2)?,
                    page_number: row.get(3)?,
                    quote: row.get(4)?,
                })
            },
        )
        .optional()
        .map_err(CommandError::database)
}

/// Link a saved note to the mark it was made from. Linking the same note
/// again moves the link to the new mark, so a repeated request is harmless.
pub(crate) fn link_note(
    connection: &mut Connection,
    input: LinkCardNoteToAnnotationInput,
    now: i64,
) -> Result<CardMarkSourceDto, CommandError> {
    validate_entity_id(&input.note_id, "card")?;
    validate_entity_id(&input.annotation_id, "annotation")?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    let note_exists: bool = transaction
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM notes WHERE id = ?1 AND deleted_at IS NULL)",
            [&input.note_id],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    if !note_exists {
        return Err(CommandError::new(
            "card_not_found",
            "That card is no longer in the library.",
        ));
    }
    let (document_id, page_number, payload): (String, i64, String) = transaction
        .query_row(
            "SELECT document_id, page_number, payload_json
             FROM document_annotations WHERE id = ?1",
            [&input.annotation_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(CommandError::database)?
        .ok_or_else(|| {
            CommandError::new(
                "document_annotation_not_found",
                "That mark is no longer on the document.",
            )
        })?;
    let quote = quote_from_payload(&payload);
    transaction
        .execute(
            "INSERT INTO note_annotation_sources(
                 note_id, document_id, annotation_id, page_number, quote,
                 created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)
             ON CONFLICT(note_id) DO UPDATE SET
                 document_id = excluded.document_id,
                 annotation_id = excluded.annotation_id,
                 page_number = excluded.page_number,
                 quote = excluded.quote,
                 updated_at = MAX(note_annotation_sources.created_at, excluded.updated_at)",
            params![
                input.note_id,
                document_id,
                input.annotation_id,
                page_number,
                quote,
                now
            ],
        )
        .map_err(CommandError::database)?;

    // A card generated from the document already cites it; keep that citation.
    let citation_id = upsert_document_snapshot(&transaction, &document_id, now)?;
    let locator = SourceLocator::with_evidence(
        SourceUnitKind::Page,
        vec![page_number],
        None,
        Vec::new(),
        None,
    )?;
    let locator_json = serde_json::to_string(&locator)
        .map_err(|error| CommandError::new("document_citation_invalid", error.to_string()))?;
    transaction
        .execute(
            "INSERT INTO note_source_citations(
                 note_id, citation_id, source_locator_json, created_at, updated_at
             )
             SELECT ?1, ?2, ?3, ?4, ?4
             WHERE NOT EXISTS (
                 SELECT 1 FROM document_note_topics
                 WHERE note_id = ?1 AND relationship_kind = 'primary'
             )
             ON CONFLICT(note_id) DO NOTHING",
            params![input.note_id, citation_id, locator_json, now],
        )
        .map_err(CommandError::database)?;
    transaction.commit().map_err(CommandError::database)?;
    Ok(CardMarkSourceDto {
        note_id: input.note_id,
        document_id: Some(document_id),
        annotation_id: Some(input.annotation_id),
        page_number,
        quote,
    })
}

pub(crate) fn marked_cards(
    connection: &Connection,
    document_id: String,
) -> Result<DocumentMarkedCardsDto, CommandError> {
    validate_entity_id(&document_id, "document")?;
    let generated_deck: Option<Option<String>> = connection
        .query_row(
            "SELECT deck.id
             FROM documents AS document
             LEFT JOIN decks AS deck
               ON deck.id = document.generated_deck_id
              AND deck.deleted_at IS NULL
              AND deck.archived_at IS NULL
             WHERE document.id = ?1",
            [&document_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(CommandError::database)?;
    let Some(generated_deck) = generated_deck else {
        return Err(CommandError::new(
            "document_not_found",
            "That document is no longer in the library.",
        ));
    };
    let mut statement = connection
        .prepare(
            "SELECT card.id
             FROM note_annotation_sources AS link
             JOIN notes AS note ON note.id = link.note_id AND note.deleted_at IS NULL
             JOIN cards AS card ON card.note_id = note.id AND card.deleted_at IS NULL
             WHERE link.document_id = ?1
             ORDER BY card.created_at, card.id",
        )
        .map_err(CommandError::database)?;
    let card_ids = statement
        .query_map([&document_id], |row| row.get(0))
        .map_err(CommandError::database)?
        .collect::<Result<Vec<String>, _>>()
        .map_err(CommandError::database)?;
    let suggested_deck_id = match generated_deck {
        Some(deck) => Some(deck),
        None => connection
            .query_row(
                "SELECT note.deck_id
                 FROM note_annotation_sources AS link
                 JOIN notes AS note ON note.id = link.note_id AND note.deleted_at IS NULL
                 JOIN decks AS deck
                   ON deck.id = note.deck_id
                  AND deck.deleted_at IS NULL
                  AND deck.archived_at IS NULL
                 WHERE link.document_id = ?1
                 ORDER BY link.updated_at DESC, link.note_id
                 LIMIT 1",
                [&document_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(CommandError::database)?,
    };
    Ok(DocumentMarkedCardsDto {
        document_id,
        card_ids,
        suggested_deck_id,
    })
}

/// The words a mark holds: a text highlight's covered text or a note's text.
/// Strokes and shapes hold none.
fn quote_from_payload(payload: &str) -> String {
    let text = serde_json::from_str::<serde_json::Value>(payload)
        .ok()
        .and_then(|value| value.get("text")?.as_str().map(str::to_owned))
        .unwrap_or_default();
    text.trim().chars().take(MAX_QUOTE_CHARS).collect()
}

#[cfg(test)]
#[path = "annotation_cards/tests.rs"]
mod tests;
