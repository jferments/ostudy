//! Durable progress and replay boundaries for explicit semantic preparation.
//! Every operation retains its receipt, including superseded scope attempts.

use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::Serialize;

use crate::{database::Database, error::CommandError, util::unix_time_millis};

use super::contracts::{invalid, StudyScope};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparationStatus {
    pub scope_key: String,
    pub operation_id: String,
    pub consent_key: String,
    pub embedding_space: String,
    pub state: String,
    pub total_sources: usize,
    pub completed_sources: usize,
    pub error_code: String,
    pub error_message: String,
    pub updated_at: i64,
}

pub(crate) struct PreparationStart<'a> {
    pub scope_key: &'a str,
    pub scope: &'a StudyScope,
    pub operation_id: &'a str,
    pub consent_key: &'a str,
    pub embedding_space: &'a str,
    pub extraction_contract: &'a str,
    pub total_sources: usize,
}

pub(crate) fn find_operation(
    database: &Database,
    operation_id: &str,
) -> Result<Option<PreparationStatus>, CommandError> {
    load(database, "operation_id", operation_id)
}

pub(crate) fn get_status(
    database: &Database,
    scope_key: &str,
) -> Result<Option<PreparationStatus>, CommandError> {
    load(database, "scope_key", scope_key)
}

fn load(
    database: &Database,
    column: &str,
    value: &str,
) -> Result<Option<PreparationStatus>, CommandError> {
    let predicate = match column {
        "operation_id" => "operation_id = ?1",
        "scope_key" => "scope_key = ?1",
        _ => return Err(invalid("Invalid preparation lookup.")),
    };
    database.with_connection(|connection| {
        connection
            .query_row(
                &format!(
                    "SELECT scope_key, operation_id, consent_hash, embedding_space, state,
                            total_sources, completed_sources, safe_error_code, safe_error_message,
                            updated_at FROM knowledge_preparations WHERE {predicate}
                     ORDER BY created_at DESC, updated_at DESC, operation_id DESC LIMIT 1"
                ),
                [value],
                |row| {
                    Ok(PreparationStatus {
                        scope_key: row.get(0)?,
                        operation_id: row.get(1)?,
                        consent_key: row.get(2)?,
                        embedding_space: row.get(3)?,
                        state: row.get(4)?,
                        total_sources: row.get(5)?,
                        completed_sources: row.get(6)?,
                        error_code: row.get(7)?,
                        error_message: row.get(8)?,
                        updated_at: row.get(9)?,
                    })
                },
            )
            .optional()
            .map_err(CommandError::database)
    })
}

pub(crate) fn begin(database: &Database, input: PreparationStart<'_>) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    let scope = serde_json::to_string(input.scope).map_err(CommandError::serialization)?;
    database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        let running: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM knowledge_preparations WHERE state = 'running')",
                [],
                |row| row.get(0),
            )
            .map_err(CommandError::database)?;
        if running {
            return Err(CommandError::new(
                "learning_task_running",
                "Semantic preparation is already running. Resume its progress view or cancel it.",
            ));
        }
        transaction
            .execute(
                "INSERT INTO knowledge_preparations (
                     scope_key, scope_json, operation_id, consent_hash, embedding_space,
                     extraction_contract, state, total_sources, completed_sources,
                     safe_error_code, safe_error_message, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'running', ?7, 0, '', '', ?8, ?8)",
                params![
                    input.scope_key,
                    scope,
                    input.operation_id,
                    input.consent_key,
                    input.embedding_space,
                    input.extraction_contract,
                    input.total_sources,
                    now,
                ],
            )
            .map_err(CommandError::database)?;
        transaction.commit().map_err(CommandError::database)
    })
}

pub(crate) fn checkpoint(
    database: &Database,
    operation_id: &str,
    completed: usize,
) -> Result<(), CommandError> {
    database.with_connection(|connection| {
        let changed = connection
            .execute(
                "UPDATE knowledge_preparations SET completed_sources = ?1, updated_at = ?2
                 WHERE operation_id = ?3 AND state = 'running' AND total_sources >= ?1",
                params![completed, unix_time_millis()?, operation_id],
            )
            .map_err(CommandError::database)?;
        if changed != 1 {
            return Err(invalid(
                "Semantic preparation no longer matches this operation.",
            ));
        }
        Ok(())
    })
}

pub(crate) fn finish(
    database: &Database,
    operation_id: &str,
    result: &Result<(), CommandError>,
) -> Result<(), CommandError> {
    let (state, code, message) = match result {
        Ok(()) => ("complete", String::new(), String::new()),
        Err(error) => (
            if error.code == "ai_request_canceled" {
                "canceled"
            } else {
                "failed"
            },
            error.code.chars().take(80).collect(),
            error.message.chars().take(1000).collect(),
        ),
    };
    database.with_connection(|connection| {
        connection
            .execute(
                "UPDATE knowledge_preparations SET state = ?1, safe_error_code = ?2,
                     safe_error_message = ?3, updated_at = ?4
                 WHERE operation_id = ?5 AND state = 'running'",
                params![state, code, message, unix_time_millis()?, operation_id],
            )
            .map_err(CommandError::database)?;
        Ok(())
    })
}

pub(crate) fn reconcile_startup(database: &Database) -> Result<(), CommandError> {
    database.with_connection(|connection| {
        connection
            .execute(
                "UPDATE knowledge_preparations SET state = 'interrupted',
                     safe_error_code = 'learning_task_interrupted',
                     safe_error_message = 'Preparation was interrupted. Completed source interpretations remain available; retry explicitly to continue.',
                     updated_at = ?1 WHERE state = 'running'",
                [unix_time_millis()?],
            )
            .map_err(CommandError::database)?;
        Ok(())
    })
}

pub(crate) fn index_cleared(database: &Database) -> Result<(), CommandError> {
    database.with_connection(|connection| {
        connection
            .execute(
                "UPDATE knowledge_preparations SET state = 'interrupted',
                     safe_error_code = 'semantic_index_cleared',
                     safe_error_message = 'The local vector cache was cleared. Prepare this selection to rebuild its embeddings.',
                     updated_at = ?1 WHERE state = 'complete'",
                [unix_time_millis()?],
            )
            .map_err(CommandError::database)?;
        Ok(())
    })
}
