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

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use crate::{
    ai::AiRuntimeState,
    database::Database,
    document_domain::{DocumentFormat, SourceUnitKind},
    documents::{self, DocumentGenerationResultDto, RetryPdfGenerationInput},
    error::CommandError,
    paths::AppPaths,
    pdf_render_backend::PdfRenderRuntime,
    task_cancellation::{cancel_document_task, is_document_task_active, run_document_task},
    util::{unix_time_millis, validate_entity_id},
};

const MAX_DECK_DESCRIPTION_CHARS: usize = 2_000;
const CANCELED_MESSAGE: &str =
    "Flashcard generation was stopped. The document remains in Documents and can be retried.";

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
    let database_after = database.clone();
    let result = run_document_task(
        &document_id,
        documents::retry_pdf_flashcard_generation(database, paths, runtime, render_runtime, input),
    )
    .await;
    match result {
        Err(error) if error.code == "ai_request_canceled" => {
            record_canceled_generation(database_after.inner(), &document_id);
            Err(CommandError::new(
                "document_generation_canceled",
                CANCELED_MESSAGE,
            ))
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
    let source = documents::get_document_source_path(database, paths, document_id)?;
    app.opener()
        .open_path(source, None::<&str>)
        .map_err(|error| {
            CommandError::new(
                "document_open_failed",
                format!("The operating system could not open the managed document: {error}"),
            )
        })
}

/// Permanently delete one managed source and all rebuildable document
/// derivatives. Generated cards/decks remain. Before the document ledger is
/// removed, every surviving generated deck receives (or retains) a textual
/// source citation so it never depends on the deleted filesystem path.
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
                "SELECT title, original_filename, source_format, source_unit_kind,
                        source_unit_count, source_relative_path, lifecycle_state, revision
                 FROM documents WHERE id = ?1",
                [input.document_id.as_str()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, i64>(7)?,
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
    let (
        title,
        original_filename,
        source_format,
        source_unit_kind,
        source_unit_count,
        source_relative_path,
        lifecycle_state,
        revision,
    ) = stored;
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
    let unit_kind = SourceUnitKind::from_db(&source_unit_kind)?;
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

    let citation = deleted_document_citation(
        &title,
        &original_filename,
        format,
        unit_kind,
        source_unit_count,
    );
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

        let generated_decks = load_surviving_generated_decks(&transaction, &input.document_id)?;

        let mut citation_count = 0_i64;
        for (deck_id, description) in generated_decks {
            let preserved = preserve_deleted_document_citation(
                &description,
                &citation,
                &title,
                &original_filename,
            )?;
            if preserved != description {
                transaction
                    .execute(
                        "UPDATE decks SET description = ?1, updated_at = ?2, revision = revision + 1
                         WHERE id = ?3",
                        params![preserved, now, deck_id],
                    )
                    .map_err(CommandError::database)?;
            }
            citation_count = citation_count.saturating_add(1);
        }

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
        Ok(citation_count)
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

fn load_surviving_generated_decks(
    connection: &Connection,
    document_id: &str,
) -> Result<Vec<(String, String)>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT DISTINCT deck.id, deck.description
             FROM document_decks provenance
             JOIN decks deck ON deck.id = provenance.deck_id
             WHERE provenance.document_id = ?1
               AND deck.deleted_at IS NULL
             ORDER BY deck.id",
        )
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map([document_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(CommandError::database)?;
    let decks = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    Ok(decks)
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

fn record_canceled_generation(database: &Database, document_id: &str) {
    let now = unix_time_millis().unwrap_or_default();
    let _ = database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        transaction
            .execute(
                "UPDATE documents
                 SET lifecycle_state = CASE WHEN archived_at IS NULL THEN 'failed' ELSE 'archived' END,
                     outline_state = CASE WHEN outline_state = 'building' THEN 'failed' ELSE outline_state END,
                     safe_error_code = 'document_generation_canceled',
                     safe_error_message = ?1,
                     updated_at = ?2,
                     revision = revision + 1
                 WHERE id = ?3",
                params![CANCELED_MESSAGE, now, document_id],
            )
            .map_err(CommandError::database)?;
        transaction
            .execute(
                "UPDATE document_generation_jobs
                 SET state = 'canceled',
                     safe_error_code = 'document_generation_canceled',
                     safe_error_message = ?1,
                     updated_at = ?2,
                     completed_at = ?2
                 WHERE id = (
                     SELECT id FROM document_generation_jobs
                     WHERE document_id = ?3
                       AND state = 'failed'
                       AND safe_error_code = 'ai_request_canceled'
                     ORDER BY created_at DESC
                     LIMIT 1
                 )",
                params![CANCELED_MESSAGE, now, document_id],
            )
            .map_err(CommandError::database)?;
        transaction.commit().map_err(CommandError::database)
    });
}

fn deleted_document_citation(
    title: &str,
    original_filename: &str,
    format: DocumentFormat,
    unit_kind: SourceUnitKind,
    unit_count: i64,
) -> String {
    let units = if unit_count > 0 {
        let label = if unit_count == 1 {
            unit_kind.singular_label()
        } else {
            unit_kind.plural_label()
        };
        format!(", {unit_count} {label}")
    } else {
        String::new()
    };
    format!(
        "Source document (deleted from Documents): {title} ({original_filename}; {}{units}).",
        format.display_label()
    )
}

fn preserve_deleted_document_citation(
    description: &str,
    citation: &str,
    title: &str,
    original_filename: &str,
) -> Result<String, CommandError> {
    if description.contains("Source document (deleted from Documents):") {
        return Ok(description.to_string());
    }
    let automatic = format!("AI-generated flashcards from {original_filename}.");
    if description.contains(&automatic) {
        let replacement = format!("AI-generated flashcards. {citation}");
        let updated = description.replacen(&automatic, &replacement, 1);
        if updated.chars().count() <= MAX_DECK_DESCRIPTION_CHARS {
            return Ok(updated);
        }
    }

    let separator = if description.trim().is_empty() {
        ""
    } else {
        "\n\n"
    };
    let appended = format!("{}{separator}{citation}", description.trim_end());
    if appended.chars().count() <= MAX_DECK_DESCRIPTION_CHARS {
        return Ok(appended);
    }

    // A user-edited description may already contain a serviceable textual
    // source reference. Never truncate user content merely to add our wording.
    if description.contains(original_filename) || description.contains(title) {
        return Ok(description.to_string());
    }
    Err(CommandError::new(
        "document_citation_preservation_failed",
        "The generated deck description is too long to preserve a text citation for this document. Shorten that deck description, then delete the document again.",
    ))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deletion_citation_replaces_the_generated_file_reference() {
        let description =
            "AI-generated flashcards from chapter.docx. Edit or delete these cards through the normal deck tools.";
        let citation = deleted_document_citation(
            "Chapter",
            "chapter.docx",
            DocumentFormat::Docx,
            SourceUnitKind::Section,
            10,
        );
        let preserved =
            preserve_deleted_document_citation(description, &citation, "Chapter", "chapter.docx")
                .expect("automatic citation should fit");
        assert!(preserved.contains(
            "Source document (deleted from Documents): Chapter (chapter.docx; Word, 10 sections)."
        ));
        assert!(!preserved.contains("AI-generated flashcards from chapter.docx."));
    }

    #[test]
    fn deletion_never_truncates_a_user_description_that_already_names_the_source() {
        let description = format!("{} chapter.pdf", "x".repeat(1_980));
        let citation = deleted_document_citation(
            "Chapter",
            "chapter.pdf",
            DocumentFormat::Pdf,
            SourceUnitKind::Page,
            10,
        );
        let preserved =
            preserve_deleted_document_citation(&description, &citation, "Chapter", "chapter.pdf")
                .expect("existing text source reference is sufficient");
        assert_eq!(preserved, description);
    }

    #[test]
    fn soft_deleted_generated_decks_are_not_counted_for_document_citation_preservation() {
        let connection = Connection::open_in_memory().expect("fixture database should open");
        connection
            .execute_batch(
                "CREATE TABLE decks (
                     id TEXT PRIMARY KEY,
                     description TEXT NOT NULL,
                     deleted_at INTEGER
                 );
                 CREATE TABLE document_decks (
                     document_id TEXT NOT NULL,
                     deck_id TEXT NOT NULL
                 );
                 INSERT INTO decks (id, description, deleted_at) VALUES
                     ('active', 'active deck', NULL),
                     ('deleted', 'deleted deck', 123);
                 INSERT INTO document_decks (document_id, deck_id) VALUES
                     ('document', 'active'),
                     ('document', 'deleted');",
            )
            .expect("fixture schema should initialize");

        let decks = load_surviving_generated_decks(&connection, "document")
            .expect("surviving deck lookup should succeed");
        assert_eq!(
            decks,
            vec![("active".to_string(), "active deck".to_string())]
        );
    }
}
