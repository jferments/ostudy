//! Conversation ownership and bounded read models. No model or transport calls
//! occur while a database connection is held.

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

use crate::{
    database::Database,
    error::CommandError,
    knowledge::contracts::invalid,
    util::{generate_id, unix_time_millis, validate_entity_id},
};

use super::contracts::{
    ConversationDetail, ConversationPageInput, ConversationSummary, CreateConversationInput,
    DeleteConversationInput, MAX_HISTORY_TURNS,
};

pub(crate) fn create(
    database: &Database,
    input: CreateConversationInput,
) -> Result<ConversationSummary, CommandError> {
    input.validate()?;
    database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        let existing = transaction
            .query_row(
                "SELECT id FROM tutor_conversations WHERE operation_id = ?1",
                [&input.operation_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(CommandError::database)?;
        if let Some(id) = existing {
            let saved = load_summary(&transaction, &id)?;
            if saved.scope != input.scope || saved.title != input.title.trim() {
                return Err(invalid(
                    "This conversation operation was already used for different input.",
                ));
            }
            return Ok(saved);
        }
        // Existing material identities are validated without contacting AI.
        crate::knowledge::adapters::scope_documents(&transaction, &input.scope)?;
        let id = generate_id(&transaction)?;
        let now = unix_time_millis()?;
        let scope = serde_json::to_string(&input.scope).map_err(CommandError::serialization)?;
        transaction
            .execute(
                "INSERT INTO tutor_conversations (
                     id, operation_id, title, scope_json, revision, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, 1, ?5, ?5)",
                params![id, input.operation_id, input.title.trim(), scope, now],
            )
            .map_err(CommandError::database)?;
        let result = load_summary(&transaction, &id)?;
        transaction.commit().map_err(CommandError::database)?;
        Ok(result)
    })
}

pub(crate) fn list(database: &Database) -> Result<Vec<ConversationSummary>, CommandError> {
    database.with_connection(|connection| {
        let mut statement = connection
            .prepare(
                "SELECT id, title, scope_json, revision, created_at, updated_at
                 FROM tutor_conversations ORDER BY updated_at DESC, id LIMIT 200",
            )
            .map_err(CommandError::database)?;
        let rows = statement
            .query_map([], raw_summary)
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        rows.into_iter().map(decode_summary).collect()
    })
}

pub(crate) fn get(
    database: &Database,
    input: ConversationPageInput,
) -> Result<ConversationDetail, CommandError> {
    validate_entity_id(&input.conversation_id, "conversation")?;
    if input.before_sequence.is_some_and(|value| value < 1) {
        return Err(invalid("The conversation page cursor is invalid."));
    }
    database.with_connection(|connection| {
        let conversation = load_summary(connection, &input.conversation_id)?;
        let mut statement = connection
            .prepare(
                "SELECT id FROM tutor_turns
                 WHERE conversation_id = ?1 AND (?2 IS NULL OR sequence < ?2)
                 ORDER BY sequence DESC LIMIT ?3",
            )
            .map_err(CommandError::database)?;
        let mut ids = statement
            .query_map(
                params![
                    input.conversation_id,
                    input.before_sequence,
                    (MAX_HISTORY_TURNS + 1) as i64
                ],
                |row| row.get::<_, String>(0),
            )
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        let has_earlier_turns = ids.len() > MAX_HISTORY_TURNS;
        ids.truncate(MAX_HISTORY_TURNS);
        ids.reverse();
        let turns = ids
            .iter()
            .map(|id| super::turns::load_public(connection, id))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ConversationDetail {
            conversation,
            turns,
            has_earlier_turns,
        })
    })
}

pub(crate) fn delete(
    database: &Database,
    input: DeleteConversationInput,
) -> Result<(), CommandError> {
    validate_entity_id(&input.conversation_id, "conversation")?;
    database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        let revision: Option<i64> = transaction
            .query_row(
                "SELECT revision FROM tutor_conversations WHERE id = ?1",
                [&input.conversation_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(CommandError::database)?;
        let Some(revision) = revision else { return Ok(()) };
        if revision != input.expected_revision {
            return Err(changed());
        }
        let running: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM tutor_turns WHERE conversation_id = ?1 AND state = 'running')",
                [&input.conversation_id],
                |row| row.get(0),
            )
            .map_err(CommandError::database)?;
        if running {
            return Err(CommandError::new(
                "tutor_turn_running",
                "Cancel the active tutor response before deleting this conversation.",
            ));
        }
        transaction
            .execute("DELETE FROM tutor_conversations WHERE id = ?1", [&input.conversation_id])
            .map_err(CommandError::database)?;
        transaction.commit().map_err(CommandError::database)
    })
}

pub(crate) fn load_summary(
    connection: &Connection,
    id: &str,
) -> Result<ConversationSummary, CommandError> {
    let row = connection
        .query_row(
            "SELECT id, title, scope_json, revision, created_at, updated_at
             FROM tutor_conversations WHERE id = ?1",
            [id],
            raw_summary,
        )
        .optional()
        .map_err(CommandError::database)?
        .ok_or_else(|| {
            CommandError::new(
                "tutor_conversation_missing",
                "This conversation is no longer in the library.",
            )
        })?;
    decode_summary(row)
}

type RawSummary = (String, String, String, i64, i64, i64);

fn raw_summary(row: &rusqlite::Row<'_>) -> rusqlite::Result<RawSummary> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
    ))
}

fn decode_summary(row: RawSummary) -> Result<ConversationSummary, CommandError> {
    let scope = serde_json::from_str(&row.2).map_err(CommandError::serialization)?;
    Ok(ConversationSummary {
        id: row.0,
        title: row.1,
        scope,
        revision: row.3,
        created_at: row.4,
        updated_at: row.5,
    })
}

pub(crate) fn changed() -> CommandError {
    CommandError::new(
        "tutor_conversation_changed",
        "This conversation changed. Reload its current messages before submitting another turn.",
    )
}
