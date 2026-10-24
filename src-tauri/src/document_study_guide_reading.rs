//! How far a learner has read each document study guide (release schema 12).
//!
//! A section is read when the learner marked it read and its text is still the
//! text they read: marks are kept by topic with the section's content hash, so
//! remaking a guide keeps a mark on an unchanged section and shows a rewritten
//! one as unread, without rewriting the mark. The position is the section the
//! learner last opened in each document's guide, where the guide reopens.
//!
//! Reading is not a review: nothing here writes review evidence or changes a
//! schedule. Every save names the guide it was made in, so a reader still open
//! on a guide that was since remade cannot mark the new guide's sections.

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    database::Database,
    error::CommandError,
    util::{unix_time_millis, validate_entity_id},
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct GetDocumentStudyGuideReadingInput {
    pub document_id: String,
}

/// The section a learner opened in the guide they are reading.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct OpenDocumentStudyGuideSectionInput {
    pub document_id: String,
    pub guide_id: String,
    pub topic_id: String,
}

/// Marks one section read or unread. Saving the same state again changes
/// nothing, so a request can be repeated.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SetDocumentStudyGuideSectionReadInput {
    pub document_id: String,
    pub guide_id: String,
    pub topic_id: String,
    pub read: bool,
}

#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DocumentStudyGuideReadingDto {
    pub document_id: String,
    /// The active guide this reading describes.
    pub guide_id: String,
    pub section_count: i64,
    /// Sections read in this guide's text, in guide order.
    pub read_topic_ids: Vec<String>,
    /// The section last opened, while it is still part of this guide.
    pub last_topic_id: Option<String>,
    pub last_opened_at: Option<i64>,
}

#[tauri::command]
pub(crate) fn get_document_study_guide_reading(
    database: State<'_, Database>,
    input: GetDocumentStudyGuideReadingInput,
) -> Result<Option<DocumentStudyGuideReadingDto>, CommandError> {
    validate_entity_id(&input.document_id, "document")?;
    database.with_connection(
        |connection| match active_guide(connection, &input.document_id)? {
            Some(guide_id) => reading(connection, &input.document_id, &guide_id).map(Some),
            None => Ok(None),
        },
    )
}

#[tauri::command]
pub(crate) fn open_document_study_guide_section(
    database: State<'_, Database>,
    input: OpenDocumentStudyGuideSectionInput,
) -> Result<DocumentStudyGuideReadingDto, CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| open_section(connection, input, now))
}

#[tauri::command]
pub(crate) fn set_document_study_guide_section_read(
    database: State<'_, Database>,
    input: SetDocumentStudyGuideSectionReadInput,
) -> Result<DocumentStudyGuideReadingDto, CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| set_section_read(connection, input, now))
}

pub(crate) fn open_section(
    connection: &mut Connection,
    input: OpenDocumentStudyGuideSectionInput,
    now: i64,
) -> Result<DocumentStudyGuideReadingDto, CommandError> {
    validate_section_ids(&input.document_id, &input.guide_id, &input.topic_id)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    require_section(
        &transaction,
        &input.document_id,
        &input.guide_id,
        &input.topic_id,
    )?;
    transaction
        .execute(
            "INSERT INTO document_study_guide_positions(document_id, topic_id, opened_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(document_id) DO UPDATE SET
                 topic_id = excluded.topic_id,
                 opened_at = excluded.opened_at",
            params![input.document_id, input.topic_id, now.max(0)],
        )
        .map_err(CommandError::database)?;
    let saved = reading(&transaction, &input.document_id, &input.guide_id)?;
    transaction.commit().map_err(CommandError::database)?;
    Ok(saved)
}

pub(crate) fn set_section_read(
    connection: &mut Connection,
    input: SetDocumentStudyGuideSectionReadInput,
    now: i64,
) -> Result<DocumentStudyGuideReadingDto, CommandError> {
    validate_section_ids(&input.document_id, &input.guide_id, &input.topic_id)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    let content_hash = require_section(
        &transaction,
        &input.document_id,
        &input.guide_id,
        &input.topic_id,
    )?;
    if input.read {
        // A mark on this exact text keeps its first read_at; a mark on older
        // text of the same topic is replaced.
        transaction
            .execute(
                "INSERT INTO document_study_guide_section_reads(
                     document_id, topic_id, content_hash, read_at
                 ) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(document_id, topic_id) DO UPDATE SET
                     content_hash = excluded.content_hash,
                     read_at = excluded.read_at
                 WHERE content_hash <> excluded.content_hash",
                params![input.document_id, input.topic_id, content_hash, now.max(0)],
            )
            .map_err(CommandError::database)?;
    } else {
        transaction
            .execute(
                "DELETE FROM document_study_guide_section_reads
                 WHERE document_id = ?1 AND topic_id = ?2",
                params![input.document_id, input.topic_id],
            )
            .map_err(CommandError::database)?;
    }
    let saved = reading(&transaction, &input.document_id, &input.guide_id)?;
    transaction.commit().map_err(CommandError::database)?;
    Ok(saved)
}

/// Clears a document's reading with its guide, inside the deleting transaction.
pub(crate) fn delete_document_reading(
    connection: &Connection,
    document_id: &str,
) -> Result<(), CommandError> {
    connection
        .execute(
            "DELETE FROM document_study_guide_section_reads WHERE document_id = ?1",
            [document_id],
        )
        .map_err(CommandError::database)?;
    connection
        .execute(
            "DELETE FROM document_study_guide_positions WHERE document_id = ?1",
            [document_id],
        )
        .map_err(CommandError::database)?;
    Ok(())
}

pub(crate) fn reading(
    connection: &Connection,
    document_id: &str,
    guide_id: &str,
) -> Result<DocumentStudyGuideReadingDto, CommandError> {
    let section_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM document_study_guide_sections WHERE guide_id = ?1",
            [guide_id],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    let read_topic_ids = {
        let mut statement = connection
            .prepare(
                "SELECT section.topic_id
                 FROM document_study_guide_sections section
                 JOIN document_study_guide_section_reads mark
                   ON mark.document_id = ?1
                  AND mark.topic_id = section.topic_id
                  AND mark.content_hash = section.content_hash
                 WHERE section.guide_id = ?2
                 ORDER BY section.sort_index",
            )
            .map_err(CommandError::database)?;
        let rows = statement
            .query_map(params![document_id, guide_id], |row| {
                row.get::<_, String>(0)
            })
            .map_err(CommandError::database)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?
    };
    let position = connection
        .query_row(
            "SELECT position.topic_id, position.opened_at
             FROM document_study_guide_positions position
             JOIN document_study_guide_sections section
               ON section.guide_id = ?2 AND section.topic_id = position.topic_id
             WHERE position.document_id = ?1",
            params![document_id, guide_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()
        .map_err(CommandError::database)?;
    let (last_topic_id, last_opened_at) = position.unzip();
    Ok(DocumentStudyGuideReadingDto {
        document_id: document_id.to_string(),
        guide_id: guide_id.to_string(),
        section_count,
        read_topic_ids,
        last_topic_id,
        last_opened_at,
    })
}

fn active_guide(
    connection: &Connection,
    document_id: &str,
) -> Result<Option<String>, CommandError> {
    connection
        .query_row(
            "SELECT id FROM document_study_guides
             WHERE document_id = ?1 AND state = 'active'
             ORDER BY revision DESC LIMIT 1",
            [document_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(CommandError::database)
}

/// The section's content hash, once the guide is the document's active guide
/// and the topic is one of its sections.
fn require_section(
    connection: &Connection,
    document_id: &str,
    guide_id: &str,
    topic_id: &str,
) -> Result<String, CommandError> {
    if active_guide(connection, document_id)?.as_deref() != Some(guide_id) {
        return Err(CommandError::new(
            "study_guide_changed",
            "This study guide was remade or removed. Reopen it to keep reading.",
        ));
    }
    connection
        .query_row(
            "SELECT content_hash FROM document_study_guide_sections
             WHERE guide_id = ?1 AND topic_id = ?2",
            params![guide_id, topic_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(CommandError::database)?
        .ok_or_else(|| {
            CommandError::new(
                "study_guide_section_missing",
                "That section is not part of this study guide.",
            )
        })
}

fn validate_section_ids(
    document_id: &str,
    guide_id: &str,
    topic_id: &str,
) -> Result<(), CommandError> {
    validate_entity_id(document_id, "document")?;
    validate_entity_id(guide_id, "study guide")?;
    validate_entity_id(topic_id, "topic")
}

#[cfg(test)]
mod tests;
