use std::fs;

use rusqlite::TransactionBehavior;

use crate::{database::Database, error::CommandError, paths::AppPaths, util::unix_time_millis};

use super::lifecycle;

const INTERRUPTED_ERROR_CODE: &str = "document_generation_interrupted";
const INTERRUPTED_ERROR_MESSAGE: &str =
    "Document processing stopped when ostudy closed. Retry generation to continue.";

pub(crate) fn reconcile_startup(database: &Database, paths: &AppPaths) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| {
                CommandError::database_at("document_reconcile_begin_failed", "scope=startup", error)
            })?;
        lifecycle::reconcile_interrupted_flashcard_operations(
            &transaction,
            now,
            INTERRUPTED_ERROR_CODE,
            INTERRUPTED_ERROR_MESSAGE,
        )?;
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
