use rusqlite::{params, Connection};

use super::{invalid, AssessmentRecord};
use crate::{
    error::CommandError,
    knowledge::{authoring::validate_current_support, contracts::hash},
    util::validate_entity_id,
};

/// Called inside generated_import's transaction after duplicate filtering. Any
/// stale source, changed key, or failed write aborts the entire deck publication.
pub(crate) fn publish(
    connection: &Connection,
    note_id: &str,
    document_id: &str,
    record: &AssessmentRecord,
    now: i64,
) -> Result<(), CommandError> {
    validate_entity_id(note_id, "generated question")?;
    validate_entity_id(document_id, "source document")?;
    validate_current_support(connection, document_id, &record.support()?)?;
    let (revision, definition): (i64, String) = connection
        .query_row(
            "SELECT revision, question_definition FROM notes
         WHERE id = ?1 AND note_type = 'interactive' AND deleted_at IS NULL",
            [note_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(CommandError::database)?;
    if hash(&definition) != record.question_hash {
        return Err(invalid(
            "Published question content differs from its assessment evidence.",
        ));
    }
    let encoded = serde_json::to_string(record).map_err(CommandError::serialization)?;
    if encoded.len() > 262_144 {
        return Err(invalid(
            "Generated assessment evidence is too large to publish.",
        ));
    }
    connection.execute(
        "INSERT INTO question_generation_evidence
         (note_id, note_revision, document_id, question_hash, opportunity_id, policy_hash, record_json, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![note_id, revision, document_id, record.question_hash, record.opportunity_id,
            record.policy_hash, encoded, now],
    ).map_err(CommandError::database)?;
    Ok(())
}
