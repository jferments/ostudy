//! Explicit conversation memory policy. Resetting memory advances a durable
//! sequence boundary; it does not erase the readable transcript or call AI.

use crate::{
    database::Database,
    error::CommandError,
    knowledge::contracts::{hash, invalid},
    util::{unix_time_millis, validate_entity_id},
};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TutorMemory {
    pub conversation_id: String,
    pub revision: i64,
    pub history_enabled: bool,
    pub retrieval_enabled: bool,
    pub start_sequence: i64,
    pub summary: String,
    pub summary_through_sequence: i64,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryAction {
    EnableHistory,
    DisableHistory,
    AllowRetrieval,
    DenyRetrieval,
    ForgetEarlier,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChangeTutorMemoryInput {
    pub operation_id: String,
    pub conversation_id: String,
    pub expected_revision: i64,
    pub action: MemoryAction,
}

pub(crate) fn get(database: &Database, conversation_id: &str) -> Result<TutorMemory, CommandError> {
    validate_entity_id(conversation_id, "conversation")?;
    database.with_connection(|connection| load(connection, conversation_id))
}

fn load(connection: &Connection, conversation_id: &str) -> Result<TutorMemory, CommandError> {
    connection.query_row(
        "SELECT revision,history_enabled,retrieval_enabled,memory_start_sequence,summary_text,summary_through_sequence
         FROM tutor_conversations WHERE id = ?1",
        [conversation_id], |row| Ok(TutorMemory {
            conversation_id: conversation_id.to_string(), revision: row.get(0)?,
            history_enabled: row.get(1)?, retrieval_enabled: row.get(2)?, start_sequence: row.get(3)?,
            summary: row.get(4)?, summary_through_sequence: row.get(5)?,
        }),
    ).optional().map_err(CommandError::database)?
        .ok_or_else(|| CommandError::new("tutor_conversation_missing", "This conversation is no longer in the library."))
}

pub(crate) fn change(
    database: &Database,
    input: ChangeTutorMemoryInput,
) -> Result<TutorMemory, CommandError> {
    validate_entity_id(&input.operation_id, "memory operation")?;
    validate_entity_id(&input.conversation_id, "conversation")?;
    if input.expected_revision < 1 {
        return Err(invalid("The conversation revision is invalid."));
    }
    let request_hash = hash(&serde_json::to_string(&input).map_err(CommandError::serialization)?);
    database.with_connection(|connection| {
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(CommandError::database)?;
        let previous: Option<(String,String)> = transaction.query_row(
            "SELECT conversation_id,request_hash FROM tutor_memory_operations WHERE operation_id = ?1",
            [&input.operation_id], |row| Ok((row.get(0)?,row.get(1)?)),
        ).optional().map_err(CommandError::database)?;
        if let Some((id, old_hash)) = previous {
            if id != input.conversation_id || old_hash != request_hash {
                return Err(invalid("This memory operation was already used for a different change."));
            }
            // Return current state without replaying an old action over newer choices.
            return load(&transaction, &id);
        }
        let current = load(&transaction, &input.conversation_id)?;
        if current.revision != input.expected_revision { return Err(super::conversations::changed()); }
        let running: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM tutor_turns WHERE conversation_id = ?1 AND state = 'running')",
            [&input.conversation_id], |row| row.get(0),
        ).map_err(CommandError::database)?;
        if running { return Err(CommandError::new("tutor_turn_running", "Finish or cancel the current response before changing conversation memory.")); }
        let (mut history, mut retrieval, mut start) = (current.history_enabled, current.retrieval_enabled, current.start_sequence);
        match input.action {
            MemoryAction::EnableHistory => history = true,
            MemoryAction::DisableHistory => history = false,
            MemoryAction::AllowRetrieval => retrieval = true,
            MemoryAction::DenyRetrieval => retrieval = false,
            MemoryAction::ForgetEarlier => {
                start = transaction.query_row(
                    "SELECT MAX(?2, COALESCE(MAX(sequence), 0)) FROM tutor_turns WHERE conversation_id = ?1",
                    params![input.conversation_id, start], |row| row.get(0),
                ).map_err(CommandError::database)?;
            }
        }
        let now = unix_time_millis()?;
        transaction.execute(
            "UPDATE tutor_conversations SET history_enabled = ?1, retrieval_enabled = ?2,
                 memory_start_sequence = ?3, revision = revision + 1, updated_at = ?4
             WHERE id = ?5 AND revision = ?6",
            params![history,retrieval,start,now,input.conversation_id,input.expected_revision],
        ).map_err(CommandError::database)?;
        // Explicit reset also clears a summary when the boundary did not move.
        if matches!(input.action, MemoryAction::ForgetEarlier) {
            transaction.execute(
                "UPDATE tutor_conversations SET summary_text = '', summary_through_sequence = memory_start_sequence WHERE id = ?1",
                [&input.conversation_id],
            ).map_err(CommandError::database)?;
            transaction.execute("DELETE FROM knowledge_sources WHERE conversation_id = ?1", [&input.conversation_id])
                .map_err(CommandError::database)?;
        }
        transaction.execute(
            "INSERT INTO tutor_memory_operations (operation_id,conversation_id,request_hash,created_at) VALUES (?1,?2,?3,?4)",
            params![input.operation_id,input.conversation_id,request_hash,now],
        ).map_err(CommandError::database)?;
        let result = load(&transaction, &input.conversation_id)?;
        transaction.commit().map_err(CommandError::database)?;
        Ok(result)
    })
}

#[cfg(test)]
mod tests;
