//! Format-neutral document lifecycle commands layered around the document core.
//!
//! Platform staging and managed import live in `document_import`. This module
//! owns cancellable generation, operating-system opening, permanent managed
//! document deletion, and deletion-trash reconciliation. Document intelligence
//! remains downstream of the canonical adapter boundary.

use std::{
    fs::{self, File},
    path::Path,
};

use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use crate::{
    ai::AiRuntimeState,
    database::Database,
    document_citations,
    document_domain::DocumentFormat,
    documents::{self, DocumentGenerationResultDto, RetryPdfGenerationInput},
    error::CommandError,
    paths::AppPaths,
    pdf_render_backend::PdfRenderRuntime,
    task_cancellation::{cancel_document_task, is_document_task_active, run_document_task},
    util::{unix_time_millis, validate_entity_id},
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeleteDocumentInput {
    pub document_id: String,
    pub revision: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteDocumentResultDto {
    pub document_id: String,
    pub preserved_deck_citation_count: i64,
}

/// Run the format-neutral document pipeline inside a task-local cancellation
/// scope. Provider POSTs made by the pipeline are therefore observable and can
/// be stopped through the matching document task identity.
#[tauri::command]
pub async fn run_document_flashcard_generation(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    runtime: State<'_, AiRuntimeState>,
    render_runtime: State<'_, PdfRenderRuntime>,
    input: RetryPdfGenerationInput,
) -> Result<DocumentGenerationResultDto, CommandError> {
    let document_id = input.document_id.clone();
    let result = run_document_task(
        &document_id,
        documents::retry_pdf_flashcard_generation(
            database.inner(),
            paths.inner(),
            runtime.inner(),
            render_runtime.inner().clone(),
            input,
        ),
    )
    .await;
    match result {
        Err(error) if error.code == "ai_request_canceled" => {
            Err(documents::flashcard_generation_canceled_error())
        }
        other => other,
    }
}

#[tauri::command]
pub fn cancel_document_flashcard_generation(document_id: String) -> Result<(), CommandError> {
    validate_entity_id(&document_id, "document")?;
    cancel_document_task(&document_id)
}

/// Open only the validated managed source belonging to one document. React does
/// not receive a filesystem path and therefore does not need opener path scope.
#[tauri::command]
pub fn open_document_source(
    app: AppHandle,
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    document_id: String,
) -> Result<(), CommandError> {
    let source = documents::resolve_document_source(database.inner(), paths.inner(), &document_id)?;
    app.opener()
        .open_path(source.to_string_lossy().into_owned(), None::<&str>)
        .map_err(|error| {
            CommandError::new(
                "document_open_failed",
                format!("The operating system could not open the managed document: {error}"),
            )
        })
}

/// Permanently delete one managed source and all document-owned derivatives.
///
/// Generated decks and notes remain. Before the document ledger is removed,
/// structured source snapshots and exact primary note locators are materialized
/// independently of the `documents` row and marked unavailable.
#[tauri::command]
pub fn delete_document(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: DeleteDocumentInput,
) -> Result<DeleteDocumentResultDto, CommandError> {
    validate_entity_id(&input.document_id, "document")?;
    if input.revision < 1 {
        return Err(CommandError::new(
            "invalid_revision",
            "The document revision is invalid.",
        ));
    }
    if is_document_task_active(&input.document_id) {
        return Err(CommandError::new(
            "document_generation_running",
            "Stop flashcard generation before deleting this document.",
        ));
    }

    let stored = database.with_connection(|connection| {
        connection
            .query_row(
                "SELECT source_format, source_relative_path, lifecycle_state, revision
                 FROM documents WHERE id = ?1",
                [input.document_id.as_str()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                },
            )
            .optional()
            .map_err(CommandError::database)?
            .ok_or_else(|| {
                CommandError::new(
                    "document_not_found",
                    "That document is no longer in the document library.",
                )
            })
    })?;
    let (source_format, source_relative_path, lifecycle_state, revision) = stored;
    if revision != input.revision {
        return Err(CommandError::new(
            "document_changed",
            "This document changed before deletion began. Reload it and try again.",
        ));
    }
    if lifecycle_state == "processing" {
        return Err(CommandError::new(
            "document_generation_running",
            "Stop flashcard generation before deleting this document.",
        ));
    }

    let format = DocumentFormat::from_db(&source_format)?;
    let expected_source = AppPaths::document_source_relative_path_with_extension(
        &input.document_id,
        format.default_extension(),
    );
    if source_relative_path != expected_source {
        return Err(CommandError::new(
            "invalid_document_path",
            "The managed source path does not match its document identity.",
        ));
    }
    let source_path = paths.resolve_relative(&source_relative_path);
    let document_directory = source_path.parent().ok_or_else(|| {
        CommandError::new(
            "invalid_document_path",
            "The managed document directory is invalid.",
        )
    })?;
    let trash_root = paths.app_data_dir().join("documents").join("trash");
    fs::create_dir_all(&trash_root).map_err(CommandError::io)?;
    let now = unix_time_millis()?;
    let trash_directory = trash_root.join(format!("{}-{now}.delete", input.document_id));
    let moved_to_trash = if document_directory.exists() {
        fs::rename(document_directory, &trash_directory).map_err(CommandError::io)?;
        sync_directory(&trash_root)?;
        true
    } else {
        false
    };

    let transaction_result = database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        let current: Option<(String, i64)> = transaction
            .query_row(
                "SELECT lifecycle_state, revision FROM documents WHERE id = ?1",
                [input.document_id.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(CommandError::database)?;
        let Some((current_state, current_revision)) = current else {
            return Err(CommandError::new(
                "document_not_found",
                "That document is no longer in the document library.",
            ));
        };
        if current_revision != input.revision || current_state == "processing" {
            return Err(CommandError::new(
                "document_changed",
                "This document changed before deletion completed. Reload it and try again.",
            ));
        }

        let preserved = document_citations::preserve_document_citations_before_delete(
            &transaction,
            &input.document_id,
            now,
        )?;

        let deleted = transaction
            .execute(
                "DELETE FROM documents
                 WHERE id = ?1 AND revision = ?2 AND lifecycle_state <> 'processing'",
                params![input.document_id, input.revision],
            )
            .map_err(CommandError::database)?;
        if deleted != 1 {
            return Err(CommandError::new(
                "document_changed",
                "This document changed before deletion completed. Reload it and try again.",
            ));
        }
        transaction.commit().map_err(CommandError::database)?;
        Ok(preserved.active_deck_count)
    });

    let citation_count = match transaction_result {
        Ok(count) => count,
        Err(error) => {
            if moved_to_trash {
                let _ = fs::rename(&trash_directory, document_directory);
                let _ = sync_directory(
                    document_directory
                        .parent()
                        .unwrap_or_else(|| paths.app_data_dir()),
                );
            }
            return Err(error);
        }
    };

    if moved_to_trash {
        // Database deletion is authoritative. A cleanup failure leaves only an
        // unreachable trash directory, which startup reconciliation removes.
        let _ = fs::remove_dir_all(&trash_directory);
        let _ = sync_directory(&trash_root);
    }
    Ok(DeleteDocumentResultDto {
        document_id: input.document_id,
        preserved_deck_citation_count: citation_count,
    })
}

pub(crate) fn reconcile_deleted_document_trash(paths: &AppPaths) -> Result<(), CommandError> {
    let trash_root = paths.app_data_dir().join("documents").join("trash");
    if !trash_root.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(&trash_root).map_err(CommandError::io)? {
        let entry = entry.map_err(CommandError::io)?;
        let path = entry.path();
        if path.is_dir() {
            fs::remove_dir_all(path).map_err(CommandError::io)?;
        } else {
            fs::remove_file(path).map_err(CommandError::io)?;
        }
    }
    sync_directory(&trash_root)
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<(), CommandError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(CommandError::io)
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> Result<(), CommandError> {
    Ok(())
}
