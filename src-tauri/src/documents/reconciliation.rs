use std::fs;

use rusqlite::{params, Connection};

use crate::{
    database::Database, document_delete_reconciliation, document_operations, error::CommandError,
    paths::AppPaths, util::unix_time_millis,
};

use super::lifecycle;

pub(crate) fn reconcile_startup(database: &Database, paths: &AppPaths) -> Result<(), CommandError> {
    document_operations::reconcile_deleted_document_trash(paths)?;
    document_delete_reconciliation::reconcile_document_trash(database, paths)?;
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        let normalized = normalize_legacy_archived_documents(connection, now)?;
        let (documents, jobs) = lifecycle::reconcile_interrupted_flashcard_operations(
            connection,
            now,
            "document_generation_interrupted",
            "Document processing was interrupted when ostudy closed. Retry generation to continue.",
        )?;
        reconcile_missing_document_sources(connection, paths, now)?;
        if normalized > 0 {
            crate::logging::info_target(
                "document.lifecycle",
                "legacy_document_archives_normalized",
                "Normalized legacy archived documents into active terminal states.",
                serde_json::json!({ "documentCount": normalized }),
            );
        }
        if documents > 0 || jobs > 0 {
            crate::logging::warn_target(
                "document.lifecycle",
                "document_startup_reconciled",
                "Recovered interrupted document operations during startup.",
                serde_json::json!({ "documentCount": documents, "jobCount": jobs }),
            );
        }
        Ok(())
    })
}

fn normalize_legacy_archived_documents(
    connection: &Connection,
    now: i64,
) -> Result<usize, CommandError> {
    connection
        .execute(
            "UPDATE documents
             SET lifecycle_state = CASE
                     WHEN EXISTS (
                         SELECT 1
                         FROM decks AS deck
                         WHERE deck.id = documents.generated_deck_id
                           AND deck.deleted_at IS NULL
                     )
                     OR EXISTS (
                         SELECT 1
                         FROM document_decks AS relation
                         JOIN decks AS deck ON deck.id = relation.deck_id
                         WHERE relation.document_id = documents.id
                           AND deck.deleted_at IS NULL
                     )
                     OR EXISTS (
                         SELECT 1
                         FROM document_study_guides AS guide
                         WHERE guide.document_id = documents.id
                           AND guide.state = 'active'
                     )
                     THEN 'ready'
                     ELSE 'failed'
                 END,
                 archived_at = NULL,
                 updated_at = MAX(updated_at, COALESCE(archived_at, ?1)),
                 revision = revision + 1
             WHERE lifecycle_state = 'archived'
                OR archived_at IS NOT NULL",
            [now],
        )
        .map_err(|error| {
            CommandError::database_at(
                "legacy_document_archive_normalization_failed",
                "scope=startup",
                error,
            )
        })
}

fn reconcile_missing_document_sources(
    connection: &Connection,
    paths: &AppPaths,
    now: i64,
) -> Result<(), CommandError> {
    for (document_id, relative_path) in managed_document_sources(connection)? {
        if paths.resolve_relative(&relative_path).is_file() {
            continue;
        }
        let changed = connection
            .execute(
                "UPDATE documents
                 SET lifecycle_state = 'failed', safe_error_code = 'document_source_missing',
                     safe_error_message = 'The managed source document is missing. Restore the library from backup or import it again.',
                     updated_at = ?1, revision = revision + 1
                 WHERE id = ?2",
                params![now, document_id],
            )
            .map_err(CommandError::database)?;
        if changed != 1 {
            continue;
        }
        crate::logging::warn_target(
            "document.lifecycle",
            "document_source_missing",
            "A managed document source was missing during startup reconciliation.",
            serde_json::json!({ "document": document_id }),
        );
    }
    Ok(())
}

fn managed_document_sources(
    connection: &Connection,
) -> Result<Vec<(String, String)>, CommandError> {
    let mut statement = connection
        .prepare("SELECT id, source_relative_path FROM documents ORDER BY id")
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(CommandError::database)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)
}

pub(crate) fn remove_empty_document_staging(paths: &AppPaths) -> Result<(), CommandError> {
    let staging = paths.app_data_dir().join("documents").join("staging");
    if !staging.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(&staging).map_err(CommandError::io)? {
        let entry = entry.map_err(CommandError::io)?;
        let path = entry.path();
        if path.is_file() {
            fs::remove_file(path).map_err(CommandError::io)?;
        }
    }
    Ok(())
}
