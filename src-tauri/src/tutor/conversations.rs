//! Conversation ownership and bounded read models. No model or transport calls
//! occur while a database connection is held.

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

use crate::{
    database::Database,
    error::CommandError,
    knowledge::contracts::invalid,
    util::{generate_id, unix_time_millis, validate_entity_id},
};

use super::{
    contracts::{
        ConversationDetail, ConversationPageInput, ConversationSummary, CreateConversationInput,
        CreateProjectConversationInput, DeleteConversationInput, MoveConversationInput,
        RenameConversationInput, SetConversationArchivedInput, MAX_HISTORY_TURNS,
    },
    organization, projects,
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

pub(crate) fn create_in_project(
    database: &Database,
    input: CreateProjectConversationInput,
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
            if saved.project_id.as_deref() != Some(input.project_id.as_str())
                || saved.title != input.title.trim()
            {
                return Err(invalid(
                    "This conversation operation was already used for different input.",
                ));
            }
            return Ok(saved);
        }
        let scope = projects::active_default_scope(&transaction, &input.project_id)?;
        let id = generate_id(&transaction)?;
        let now = unix_time_millis()?;
        let scope_json = serde_json::to_string(&scope).map_err(CommandError::serialization)?;
        transaction
            .execute(
                "INSERT INTO tutor_conversations(
                     id,operation_id,title,scope_json,project_id,revision,
                     organization_revision,created_at,updated_at
                 ) VALUES(?1,?2,?3,?4,?5,1,1,?6,?6)",
                params![
                    id,
                    input.operation_id,
                    input.title.trim(),
                    scope_json,
                    input.project_id,
                    now
                ],
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
                "SELECT id,title,scope_json,project_id,revision,organization_revision,
                        archived_at,created_at,updated_at
                 FROM tutor_conversations
                 ORDER BY archived_at IS NOT NULL, updated_at DESC, id LIMIT 500",
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

pub(crate) fn rename(
    database: &Database,
    input: RenameConversationInput,
) -> Result<ConversationSummary, CommandError> {
    validate_organization_input(
        &input.operation_id,
        &input.conversation_id,
        input.expected_organization_revision,
    )?;
    crate::knowledge::contracts::bounded(&input.title, 200, "conversation title", false)?;
    let request_hash = organization::request_hash("rename_conversation", &input)?;
    database.with_connection(|connection| {
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        if let Some(saved) = organization::replay(
            &tx,
            &input.operation_id,
            "rename_conversation",
            &request_hash,
        )? {
            return Ok(saved);
        }
        let now = unix_time_millis()?;
        let changed = tx
            .execute(
                "UPDATE tutor_conversations
                 SET title=?1,organization_revision=organization_revision+1,updated_at=?2
                 WHERE id=?3 AND organization_revision=?4",
                params![
                    input.title.trim(),
                    now,
                    input.conversation_id,
                    input.expected_organization_revision
                ],
            )
            .map_err(CommandError::database)?;
        if changed != 1 {
            return Err(organization_changed());
        }
        let result = load_summary(&tx, &input.conversation_id)?;
        organization::record(
            &tx,
            &input.operation_id,
            "rename_conversation",
            "conversation",
            &input.conversation_id,
            &request_hash,
            &result,
        )?;
        tx.commit().map_err(CommandError::database)?;
        Ok(result)
    })
}

pub(crate) fn move_to_project(
    database: &Database,
    input: MoveConversationInput,
) -> Result<ConversationSummary, CommandError> {
    validate_organization_input(
        &input.operation_id,
        &input.conversation_id,
        input.expected_organization_revision,
    )?;
    if let Some(project_id) = input.project_id.as_deref() {
        validate_entity_id(project_id, "Tutor project")?;
    } else if input.apply_project_default {
        return Err(invalid(
            "A project default can only be applied while moving into a Tutor project.",
        ));
    }
    let request_hash = organization::request_hash("move_conversation", &input)?;
    database.with_connection(|connection| {
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        if let Some(saved) =
            organization::replay(&tx, &input.operation_id, "move_conversation", &request_hash)?
        {
            return Ok(saved);
        }
        let default_scope = input
            .project_id
            .as_deref()
            .map(|project_id| projects::active_default_scope(&tx, project_id))
            .transpose()?;
        let scope_json = if input.apply_project_default {
            Some(
                serde_json::to_string(
                    default_scope
                        .as_ref()
                        .ok_or_else(|| invalid("The project default scope is unavailable."))?,
                )
                .map_err(CommandError::serialization)?,
            )
        } else {
            None
        };
        let now = unix_time_millis()?;
        let changed = tx
            .execute(
                "UPDATE tutor_conversations
                 SET project_id=?1,
                     scope_json=COALESCE(?2,scope_json),
                     organization_revision=organization_revision+1,
                     updated_at=?3
                 WHERE id=?4 AND organization_revision=?5",
                params![
                    input.project_id,
                    scope_json,
                    now,
                    input.conversation_id,
                    input.expected_organization_revision
                ],
            )
            .map_err(CommandError::database)?;
        if changed != 1 {
            return Err(organization_changed());
        }
        let result = load_summary(&tx, &input.conversation_id)?;
        organization::record(
            &tx,
            &input.operation_id,
            "move_conversation",
            "conversation",
            &input.conversation_id,
            &request_hash,
            &result,
        )?;
        tx.commit().map_err(CommandError::database)?;
        Ok(result)
    })
}

pub(crate) fn set_archived(
    database: &Database,
    input: SetConversationArchivedInput,
) -> Result<ConversationSummary, CommandError> {
    validate_organization_input(
        &input.operation_id,
        &input.conversation_id,
        input.expected_organization_revision,
    )?;
    let action = if input.archived {
        "archive_conversation"
    } else {
        "restore_conversation"
    };
    let request_hash = organization::request_hash(action, &input)?;
    database.with_connection(|connection| {
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        if let Some(saved) = organization::replay(&tx, &input.operation_id, action, &request_hash)?
        {
            return Ok(saved);
        }
        let current = load_summary(&tx, &input.conversation_id)?;
        if current.organization_revision != input.expected_organization_revision {
            return Err(organization_changed());
        }
        if current.archived_at.is_some() != input.archived {
            let now = unix_time_millis()?;
            let archived_at = input.archived.then_some(now);
            tx.execute(
                "UPDATE tutor_conversations
                 SET archived_at=?1,organization_revision=organization_revision+1,updated_at=?2
                 WHERE id=?3 AND organization_revision=?4",
                params![
                    archived_at,
                    now,
                    input.conversation_id,
                    input.expected_organization_revision
                ],
            )
            .map_err(CommandError::database)?;
        }
        let result = load_summary(&tx, &input.conversation_id)?;
        organization::record(
            &tx,
            &input.operation_id,
            action,
            "conversation",
            &input.conversation_id,
            &request_hash,
            &result,
        )?;
        tx.commit().map_err(CommandError::database)?;
        Ok(result)
    })
}

pub(crate) fn delete(
    database: &Database,
    input: DeleteConversationInput,
) -> Result<(), CommandError> {
    validate_entity_id(&input.operation_id, "conversation delete operation")?;
    validate_entity_id(&input.conversation_id, "conversation")?;
    if input.expected_revision < 1 || input.expected_organization_revision < 1 {
        return Err(invalid("The conversation revision is invalid."));
    }
    let request_hash = organization::request_hash("delete_conversation", &input)?;
    database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        if organization::replay::<serde_json::Value>(
            &transaction,
            &input.operation_id,
            "delete_conversation",
            &request_hash,
        )?
        .is_some()
        {
            return Ok(());
        }
        let revisions: Option<(i64, i64)> = transaction
            .query_row(
                "SELECT revision,organization_revision FROM tutor_conversations WHERE id=?1",
                [&input.conversation_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(CommandError::database)?;
        let Some((revision, organization_revision)) = revisions else {
            organization::record(
                &transaction,
                &input.operation_id,
                "delete_conversation",
                "conversation",
                &input.conversation_id,
                &request_hash,
                &serde_json::Value::Null,
            )?;
            transaction.commit().map_err(CommandError::database)?;
            return Ok(());
        };
        if revision != input.expected_revision {
            return Err(changed());
        }
        if organization_revision != input.expected_organization_revision {
            return Err(organization_changed());
        }
        let running: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM tutor_turns WHERE conversation_id=?1 AND state='running')",
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
            .execute(
                "DELETE FROM tutor_conversations WHERE id=?1",
                [&input.conversation_id],
            )
            .map_err(CommandError::database)?;
        organization::record(
            &transaction,
            &input.operation_id,
            "delete_conversation",
            "conversation",
            &input.conversation_id,
            &request_hash,
            &serde_json::Value::Null,
        )?;
        transaction.commit().map_err(CommandError::database)
    })
}

pub(crate) fn load_summary(
    connection: &Connection,
    id: &str,
) -> Result<ConversationSummary, CommandError> {
    let row = connection
        .query_row(
            "SELECT id,title,scope_json,project_id,revision,organization_revision,
                    archived_at,created_at,updated_at
             FROM tutor_conversations WHERE id=?1",
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

type RawSummary = (
    String,
    String,
    String,
    Option<String>,
    i64,
    i64,
    Option<i64>,
    i64,
    i64,
);

fn raw_summary(row: &rusqlite::Row<'_>) -> rusqlite::Result<RawSummary> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
        row.get(6)?,
        row.get(7)?,
        row.get(8)?,
    ))
}

fn decode_summary(row: RawSummary) -> Result<ConversationSummary, CommandError> {
    let scope = serde_json::from_str(&row.2).map_err(CommandError::serialization)?;
    Ok(ConversationSummary {
        id: row.0,
        title: row.1,
        scope,
        project_id: row.3,
        revision: row.4,
        organization_revision: row.5,
        archived_at: row.6,
        created_at: row.7,
        updated_at: row.8,
    })
}

fn validate_organization_input(
    operation_id: &str,
    conversation_id: &str,
    organization_revision: i64,
) -> Result<(), CommandError> {
    validate_entity_id(operation_id, "Tutor organization operation")?;
    validate_entity_id(conversation_id, "conversation")?;
    if organization_revision < 1 {
        return Err(invalid(
            "The conversation organization revision is invalid.",
        ));
    }
    Ok(())
}

pub(crate) fn organization_changed() -> CommandError {
    CommandError::new(
        "tutor_conversation_organization_changed",
        "This conversation organization changed. Reload Tutor before applying another rename, move, or archive action.",
    )
}

pub(crate) fn changed() -> CommandError {
    CommandError::new(
        "tutor_conversation_changed",
        "This conversation changed. Reload its current messages before submitting another turn.",
    )
}
