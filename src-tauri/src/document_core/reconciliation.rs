use std::fs;

use rusqlite::{params, TransactionBehavior};

use crate::{
    database::Database,
    error::CommandError,
    paths::AppPaths,
    util::unix_time_millis,
};

use super::lifecycle;

const INTERRUPTED_ERROR_CODE: &str = "document_generation_interrupted";
const INTERRUPTED_ERROR_MESSAGE: &str =
    "Document processing stopped when ostudy closed. Retry generation to continue.";

pub(crate) fn reconcile_startup(
    database: &Database,
    paths: &AppPaths,
) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| {
                CommandError::database_at("document_reconcile_begin_failed", "scope=startup", error)
            })?;
        transaction
            .execute(
                "UPDATE documents
                 SET lifecycle_state = CASE
                         WHEN EXISTS (
                             SELECT 1 FROM decks deck
                             WHERE deck.id = documents.generated_deck_id
                               AND deck.deleted_at IS NULL
                         ) OR EXISTS (
                             SELECT 1 FROM document_study_guides guide
                             WHERE guide.document_id = documents.id
                               AND guide.state = 'active'
                         ) THEN 'ready'
                         ELSE 'failed'
                     END,
                     outline_state = CASE WHEN outline_state = 'building' THEN 'failed' ELSE outline_state END,
                     safe_error_code = ?1, safe_error_message = ?2,
                     processing_duration_ms = CASE
                         WHEN processing_started_at IS NULL THEN processing_duration_ms
                         ELSE MAX(0, ?3 - processing_started_at)
                     END,
                     updated_at = ?3, revision = revision + 1
                 WHERE lifecycle_state = 'processing' AND archived_at IS NULL",
                params![INTERRUPTED_ERROR_CODE, INTERRUPTED_ERROR_MESSAGE, now],
            )
            .map_err(|error| {
                CommandError::database_at(
                    "document_reconcile_documents_failed",
                    "scope=startup",
                    error,
                )
            })?;
        transaction
            .execute(
                "UPDATE document_generation_jobs
                 SET state = 'interrupted', safe_error_code = ?1,
                     safe_error_message = ?2, updated_at = ?3
                 WHERE state IN ('queued', 'extracting', 'generating', 'committing')",
                params![INTERRUPTED_ERROR_CODE, INTERRUPTED_ERROR_MESSAGE, now],
            )
            .map_err(|error| {
                CommandError::database_at("document_reconcile_jobs_failed", "scope=startup", error)
            })?;
        transaction.commit().map_err(|error| {
            CommandError::database_at("document_reconcile_commit_failed", "scope=startup", error)
        })
    })?;

    clear_transient_staging(paths)?;
    for (document_id, relative_path) in load_managed_sources(database)? {
        if !paths.resolve_relative(&relative_path).is_file() {
            lifecycle::record_document_failure(
                database,
                &document_id,
                &CommandError::new(
                    "document_source_missing",
                    "The managed source document is missing. Restore the library from backup or import it again.",
                ),
            );
        }
    }
    Ok(())
}

fn clear_transient_staging(paths: &AppPaths) -> Result<(), CommandError> {
    if !paths.document_staging_dir().is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(paths.document_staging_dir()).map_err(CommandError::io)? {
        let entry = entry.map_err(CommandError::io)?;
        let path = entry.path();
        if path.is_dir() {
            let _ = fs::remove_dir_all(path);
        } else {
            let _ = fs::remove_file(path);
        }
    }
    Ok(())
}

fn load_managed_sources(database: &Database) -> Result<Vec<(String, String)>, CommandError> {
    database.with_connection(|connection| {
        let mut statement = connection
            .prepare("SELECT id, source_relative_path FROM documents WHERE archived_at IS NULL")
            .map_err(|error| {
                CommandError::database_at(
                    "document_reconcile_source_query_failed",
                    "scope=startup",
                    error,
                )
            })?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(CommandError::database)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)
    })
}
