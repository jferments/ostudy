use rusqlite::{params, Connection, TransactionBehavior};

use crate::{
    database::Database,
    error::CommandError,
    util::{unix_time_millis, validate_entity_id},
};

use super::{
    contracts::{DocumentDto, SetDocumentArchivedInput},
    repository,
};

pub(crate) const FLASHCARD_CANCELED_CODE: &str = "document_generation_canceled";
pub(crate) const FLASHCARD_CANCELED_MESSAGE: &str =
    "Flashcard generation was stopped. The document remains in Documents and can be retried.";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SettledDocumentState {
    Ready,
    Failed,
    Archived,
}

impl SettledDocumentState {
    const fn as_db(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Failed => "failed",
            Self::Archived => "archived",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FlashcardJobTerminalState {
    Failed,
    Canceled,
}

impl FlashcardJobTerminalState {
    const fn as_db(self) -> &'static str {
        match self {
            Self::Failed => "failed",
            Self::Canceled => "canceled",
        }
    }
}

pub(crate) fn set_document_archived(
    database: &Database,
    input: SetDocumentArchivedInput,
) -> Result<DocumentDto, CommandError> {
    validate_entity_id(&input.document_id, "document")?;
    if input.revision < 1 {
        return Err(CommandError::new(
            "invalid_revision",
            "The document revision is invalid.",
        ));
    }

    database.with_connection(|connection| {
        let now = unix_time_millis()?;
        let lifecycle = settled_state(connection, &input.document_id, input.archived)?.as_db();
        let changed = connection
            .execute(
                "UPDATE documents
                 SET lifecycle_state = ?1, archived_at = ?2, updated_at = ?3,
                     revision = revision + 1
                 WHERE id = ?4 AND revision = ?5",
                params![
                    lifecycle,
                    input.archived.then_some(now),
                    now,
                    input.document_id,
                    input.revision,
                ],
            )
            .map_err(|error| {
                CommandError::database_at(
                    "document_archive_write_failed",
                    format!("document={}", input.document_id),
                    error,
                )
            })?;
        if changed != 1 {
            return Err(CommandError::new(
                "document_changed",
                "This document changed before the archive action completed. Reload it and try again.",
            ));
        }
        Ok(repository::load_document(connection, &input.document_id)?.dto)
    })
}

pub(crate) fn record_document_failure(
    database: &Database,
    document_id: &str,
    error: &CommandError,
) {
    record_flashcard_terminal(
        database,
        document_id,
        FlashcardJobTerminalState::Failed,
        &error.code,
        &error.message,
    );
}

pub(crate) fn record_document_cancellation(database: &Database, document_id: &str) {
    record_flashcard_terminal(
        database,
        document_id,
        FlashcardJobTerminalState::Canceled,
        FLASHCARD_CANCELED_CODE,
        FLASHCARD_CANCELED_MESSAGE,
    );
}

pub(crate) fn flashcard_generation_canceled_error() -> CommandError {
    CommandError::new(FLASHCARD_CANCELED_CODE, FLASHCARD_CANCELED_MESSAGE)
}

pub(crate) fn reconcile_interrupted_flashcard_operations(
    connection: &Connection,
    now: i64,
    safe_error_code: &str,
    safe_error_message: &str,
) -> Result<(usize, usize), CommandError> {
    let interrupted_documents = {
        let mut statement = connection
            .prepare(
                "SELECT id, archived_at IS NOT NULL
                 FROM documents
                 WHERE lifecycle_state = 'processing'
                 ORDER BY id",
            )
            .map_err(|error| {
                CommandError::database_at(
                    "document_reconcile_documents_query_failed",
                    "scope=startup",
                    error,
                )
            })?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, bool>(1)?))
            })
            .map_err(CommandError::database)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?
    };

    let mut document_count = 0_usize;
    for (document_id, archived) in interrupted_documents {
        let lifecycle = settled_state(connection, &document_id, archived)?.as_db();
        let changed = connection
            .execute(
                "UPDATE documents
                 SET lifecycle_state = ?1,
                     outline_state = CASE WHEN outline_state = 'building' THEN 'failed' ELSE outline_state END,
                     safe_error_code = ?2, safe_error_message = ?3,
                     processing_duration_ms = CASE
                         WHEN processing_started_at IS NULL THEN processing_duration_ms
                         ELSE MAX(0, ?4 - processing_started_at)
                     END,
                     updated_at = ?4, revision = revision + 1
                 WHERE id = ?5 AND lifecycle_state = 'processing'",
                params![
                    lifecycle,
                    bounded_error(safe_error_code, 80),
                    bounded_error(safe_error_message, 1000),
                    now,
                    document_id,
                ],
            )
            .map_err(|error| {
                CommandError::database_at(
                    "document_reconcile_documents_failed",
                    format!("document={document_id}"),
                    error,
                )
            })?;
        document_count = document_count.saturating_add(changed);
    }

    let job_count = connection
        .execute(
            "UPDATE document_generation_jobs
             SET state = 'interrupted', safe_error_code = ?1,
                 safe_error_message = ?2, updated_at = ?3
             WHERE state IN ('queued', 'extracting', 'generating', 'committing')",
            params![
                bounded_error(safe_error_code, 80),
                bounded_error(safe_error_message, 1000),
                now,
            ],
        )
        .map_err(|error| {
            CommandError::database_at("document_reconcile_jobs_failed", "scope=startup", error)
        })?;
    Ok((document_count, job_count))
}

fn record_flashcard_terminal(
    database: &Database,
    document_id: &str,
    job_state: FlashcardJobTerminalState,
    safe_error_code: &str,
    safe_error_message: &str,
) {
    let now = unix_time_millis().unwrap_or_default();
    let usage = crate::task_cancellation::current_usage_summary();
    let (api_token_count, api_token_usage_complete) = usage.unwrap_or((None, false));
    let result = database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        let archived = transaction
            .query_row(
                "SELECT archived_at IS NOT NULL FROM documents WHERE id = ?1",
                [document_id],
                |row| row.get::<_, bool>(0),
            )
            .map_err(CommandError::database)?;
        let lifecycle = settled_state(&transaction, document_id, archived)?.as_db();
        let document_changed = transaction
            .execute(
                "UPDATE documents
                 SET lifecycle_state = ?1,
                     outline_state = CASE WHEN outline_state = 'building' THEN 'failed' ELSE outline_state END,
                     safe_error_code = ?2, safe_error_message = ?3,
                     processing_duration_ms = CASE
                         WHEN processing_started_at IS NULL THEN processing_duration_ms
                         ELSE MAX(0, ?4 - processing_started_at)
                     END,
                     api_token_count = CASE WHEN ?5 IS NULL THEN api_token_count ELSE ?5 END,
                     api_token_usage_complete = CASE WHEN ?5 IS NULL THEN api_token_usage_complete ELSE ?6 END,
                     updated_at = ?4, revision = revision + 1
                 WHERE id = ?7",
                params![
                    lifecycle,
                    bounded_error(safe_error_code, 80),
                    bounded_error(safe_error_message, 1000),
                    now,
                    api_token_count,
                    if api_token_usage_complete { 1_i64 } else { 0_i64 },
                    document_id,
                ],
            )
            .map_err(|db_error| {
                CommandError::database_at(
                    "document_failure_state_write_failed",
                    format!("document={document_id} original_code={safe_error_code}"),
                    db_error,
                )
            })?;
        if document_changed != 1 {
            return Err(CommandError::new(
                "document_not_found",
                "That document is no longer in the library.",
            ));
        }
        transaction
            .execute(
                "UPDATE document_generation_jobs
                 SET state = ?1, safe_error_code = ?2, safe_error_message = ?3,
                     updated_at = ?4, completed_at = ?4
                 WHERE document_id = ?5
                   AND state IN ('queued', 'extracting', 'generating', 'committing')",
                params![
                    job_state.as_db(),
                    bounded_error(safe_error_code, 80),
                    bounded_error(safe_error_message, 1000),
                    now,
                    document_id,
                ],
            )
            .map_err(CommandError::database)?;
        transaction.commit().map_err(CommandError::database)
    });
    if let Err(projection_error) = result {
        crate::logging::error_target(
            "document.lifecycle",
            "document_terminal_projection_failed",
            "The document operation stopped and its durable terminal projection could not be recorded.",
            serde_json::json!({
                "document": document_id,
                "originalCode": safe_error_code,
                "projectionCode": projection_error.code.as_str(),
                "jobState": job_state.as_db(),
            }),
        );
    }
}

fn settled_state(
    connection: &Connection,
    document_id: &str,
    archived: bool,
) -> Result<SettledDocumentState, CommandError> {
    let has_learning_artifact = repository::active_generated_deck_exists(connection, document_id)?
        || repository::active_study_guide_exists(connection, document_id)?;
    Ok(settled_state_for_presence(archived, has_learning_artifact))
}

const fn settled_state_for_presence(
    archived: bool,
    has_learning_artifact: bool,
) -> SettledDocumentState {
    if archived {
        SettledDocumentState::Archived
    } else if has_learning_artifact {
        SettledDocumentState::Ready
    } else {
        SettledDocumentState::Failed
    }
}

fn bounded_error(value: &str, maximum: usize) -> String {
    value.chars().take(maximum).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settled_state_truth_table_preserves_available_learning_artifacts() {
        assert_eq!(
            settled_state_for_presence(true, false),
            SettledDocumentState::Archived
        );
        assert_eq!(
            settled_state_for_presence(true, true),
            SettledDocumentState::Archived
        );
        assert_eq!(
            settled_state_for_presence(false, true),
            SettledDocumentState::Ready
        );
        assert_eq!(
            settled_state_for_presence(false, false),
            SettledDocumentState::Failed
        );
    }

    #[test]
    fn cancellation_preserves_ready_state_for_a_guide_only_document() {
        let database = Database::open_in_memory().expect("database should open");
        let document_id = "a".repeat(32);
        insert_guide_only_document(&database, &document_id);

        record_document_cancellation(&database, &document_id);

        database
            .with_connection(|connection| {
                let (lifecycle, code): (String, String) = connection
                    .query_row(
                        "SELECT lifecycle_state, safe_error_code FROM documents WHERE id = ?1",
                        [document_id.as_str()],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(lifecycle, "ready");
                assert_eq!(code, FLASHCARD_CANCELED_CODE);
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn startup_interruption_preserves_ready_state_for_a_guide_only_document() {
        let database = Database::open_in_memory().expect("database should open");
        let document_id = "1".repeat(32);
        insert_guide_only_document(&database, &document_id);
        database
            .with_connection(|connection| {
                connection
                    .execute(
                        "UPDATE documents SET lifecycle_state = 'processing' WHERE id = ?1",
                        [document_id.as_str()],
                    )
                    .map_err(CommandError::database)?;
                let (documents, _) = reconcile_interrupted_flashcard_operations(
                    connection,
                    10,
                    "document_generation_interrupted",
                    "interrupted",
                )?;
                assert_eq!(documents, 1);
                let lifecycle: String = connection
                    .query_row(
                        "SELECT lifecycle_state FROM documents WHERE id = ?1",
                        [document_id.as_str()],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(lifecycle, "ready");
                Ok(())
            })
            .unwrap();
    }

    fn insert_guide_only_document(database: &Database, document_id: &str) {
        database
            .with_connection(|connection| {
                connection
                    .execute(
                        "INSERT INTO documents (
                         id, title, original_filename, mime_type, byte_size, sha256,
                         source_relative_path, lifecycle_state, created_at, updated_at
                     ) VALUES (?1, 'Guide only', 'guide.pdf', 'application/pdf', 1,
                               ?2, 'documents/guide/source.pdf', 'failed', 1, 1)",
                        params![document_id, "b".repeat(64)],
                    )
                    .map_err(CommandError::database)?;
                let plan_id = "c".repeat(32);
                connection
                    .execute(
                        "INSERT INTO document_learning_plans (
                         id, document_id, revision, compiler_version, outline_fingerprint,
                         plan_fingerprint, summary_text, learning_objectives_json,
                         key_concepts_json, state, created_at, updated_at
                     ) VALUES (?1, ?2, 1, 1, ?3, ?4, 'Summary', '[]', '[]', 'active', 1, 1)",
                        params![plan_id, document_id, "d".repeat(64), "e".repeat(64)],
                    )
                    .map_err(CommandError::database)?;
                connection
                    .execute(
                        "INSERT INTO document_study_guides (
                         id, document_id, plan_id, revision, schema_version, prompt_version,
                         assembly_version, overview_json, overview_text, state,
                         created_at, updated_at, published_at
                     ) VALUES (?1, ?2, ?3, 1, 1, 1, 1, '{}', 'Summary', 'active', 1, 1, 1)",
                        params!["f".repeat(32), document_id, plan_id],
                    )
                    .map_err(CommandError::database)?;
                Ok(())
            })
            .expect("guide-only fixture should initialize");
    }
}
