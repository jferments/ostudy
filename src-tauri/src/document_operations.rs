//! Narrow document lifecycle commands layered around the Stage 4 document core.
//!
//! This module owns the current PDF-specific acquisition/lifecycle edge: user-
//! controlled import start, cancellable generation, operating-system opening,
//! and permanent managed-PDF deletion. Once a PDF is acquired, format-neutral
//! document services own extraction planning, AI execution, and persistence.

use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Seek, Write},
    path::Path,
};

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use crate::{
    ai::AiRuntimeState,
    database::Database,
    document_domain::{DocumentFormat, SourceUnitKind},
    documents::{
        self, DocumentGenerationResultDto, ImportPdfDocumentInput, RetryPdfGenerationInput,
    },
    error::CommandError,
    external_files::{self, ExternalFileKind, ExternalImportPurpose},
    paths::AppPaths,
    pdf_render_backend::PdfRenderRuntime,
    task_cancellation::{cancel_document_task, is_document_task_active, run_document_task},
    util::{generate_id, unix_time_millis, validate_entity_id},
};

const MAX_PDF_BYTES: u64 = 512 * 1024 * 1024;
const MAX_DECK_DESCRIPTION_CHARS: usize = 2_000;
const CANCELED_MESSAGE: &str =
    "PDF flashcard generation was stopped. The PDF remains in Documents and can be retried.";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportedPdfDocumentDto {
    pub document_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeletePdfDocumentInput {
    pub document_id: String,
    pub revision: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeletePdfDocumentResultDto {
    pub document_id: String,
    pub preserved_deck_citation_count: i64,
}

/// Copy a selected PDF into immutable managed storage without starting provider
/// traffic. The frontend immediately follows this with the cancellable retry
/// command, giving the task a stable document identity before inference.
#[tauri::command]
pub fn import_pdf_document(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: ImportPdfDocumentInput,
) -> Result<ImportedPdfDocumentDto, CommandError> {
    validate_entity_id(&input.staged_file_id, "staged PDF")?;
    if let Some(result) = database.with_connection(|connection| {
        external_files::consumed_import_result::<ImportedPdfDocumentDto>(
            connection,
            &input.staged_file_id,
        )
    })? {
        return Ok(result);
    }
    let staged = database.with_connection(|connection| {
        external_files::resolve_staged_import(
            connection,
            paths.inner(),
            &input.staged_file_id,
            ExternalImportPurpose::PdfDocument,
            &[ExternalFileKind::Pdf],
        )
    })?;
    import_managed_pdf(
        database.inner(),
        paths.inner(),
        &staged.path,
        &staged.display_name,
        &input.staged_file_id,
    )
}

/// Run the existing document pipeline inside a task-local cancellation scope.
/// Every provider POST made by the pipeline is therefore upgraded by the native
/// gateway to an observable SSE stream with initial-activity timeout semantics.
#[tauri::command]
pub async fn run_pdf_flashcard_generation(
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
pub fn cancel_pdf_flashcard_generation(document_id: String) -> Result<(), CommandError> {
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
                format!("The operating system could not open the managed PDF: {error}"),
            )
        })
}

/// Permanently delete one managed PDF and all rebuildable document derivatives.
/// Generated cards/decks remain. Before the document ledger is removed, every
/// generated deck receives (or retains) a human-readable source citation so no
/// surviving study content depends on the deleted filesystem path.
#[tauri::command]
pub fn delete_pdf_document(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: DeletePdfDocumentInput,
) -> Result<DeletePdfDocumentResultDto, CommandError> {
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
            "Stop flashcard generation before deleting this PDF.",
        ));
    }

    let stored = database.with_connection(|connection| {
        connection
            .query_row(
                "SELECT title, original_filename, page_count, source_relative_path,
                        lifecycle_state, revision
                 FROM documents WHERE id = ?1",
                [input.document_id.as_str()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, i64>(5)?,
                    ))
                },
            )
            .optional()
            .map_err(CommandError::database)?
            .ok_or_else(|| {
                CommandError::new(
                    "document_not_found",
                    "That PDF is no longer in the document library.",
                )
            })
    })?;
    let (title, original_filename, page_count, source_relative_path, lifecycle_state, revision) =
        stored;
    if revision != input.revision {
        return Err(CommandError::new(
            "document_changed",
            "This document changed before deletion began. Reload it and try again.",
        ));
    }
    if lifecycle_state == "processing" {
        return Err(CommandError::new(
            "document_generation_running",
            "Stop flashcard generation before deleting this PDF.",
        ));
    }

    let expected_source = AppPaths::document_source_relative_path(&input.document_id);
    if source_relative_path != expected_source {
        return Err(CommandError::new(
            "invalid_document_path",
            "The managed PDF path does not match its document identity.",
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

    let citation = deleted_pdf_citation(&title, &original_filename, page_count);
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
                "That PDF is no longer in the document library.",
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
            let preserved =
                preserve_deleted_pdf_citation(&description, &citation, &title, &original_filename)?;
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
    Ok(DeletePdfDocumentResultDto {
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

fn import_managed_pdf(
    database: &Database,
    paths: &AppPaths,
    source_path: &Path,
    source_display_name: &str,
    staged_file_id: &str,
) -> Result<ImportedPdfDocumentDto, CommandError> {
    let source = fs::canonicalize(source_path).map_err(CommandError::io)?;
    if !source.is_file() {
        return Err(CommandError::new("invalid_pdf", "Select a PDF file."));
    }
    let metadata = fs::metadata(&source).map_err(CommandError::io)?;
    if metadata.len() == 0 || metadata.len() > MAX_PDF_BYTES {
        return Err(CommandError::new(
            "invalid_pdf_size",
            "The PDF is empty or exceeds the 512 MB document limit.",
        ));
    }
    let original_filename = validate_pdf_filename(source_display_name)?;
    let title = validate_document_title(
        Path::new(&original_filename)
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("Imported PDF"),
    )?;
    let document_id = database.with_connection(|connection| generate_id(connection))?;
    let staged_relative = AppPaths::staged_document_relative_path(&document_id);
    let staged_path = paths.resolve_relative(&staged_relative);
    let (byte_size, sha256) = copy_pdf_with_hash(&source, &staged_path)?;

    let duplicate = database.with_connection(|connection| {
        connection
            .query_row(
                "SELECT id FROM documents WHERE sha256 = ?1 LIMIT 1",
                [sha256.as_str()],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(CommandError::database)
    })?;
    if duplicate.is_some() {
        let _ = fs::remove_file(&staged_path);
        return Err(CommandError::new(
            "document_already_imported",
            "This exact PDF is already in the document library.",
        ));
    }

    let source_relative = AppPaths::document_source_relative_path(&document_id);
    let destination = paths.resolve_relative(&source_relative);
    let directory = destination.parent().ok_or_else(|| {
        CommandError::new(
            "invalid_document_path",
            "The managed document path is invalid.",
        )
    })?;
    fs::create_dir_all(directory).map_err(CommandError::io)?;
    fs::rename(&staged_path, &destination).map_err(CommandError::io)?;
    sync_file(&destination)?;
    sync_directory(directory)?;

    let now = unix_time_millis()?;
    let result = ImportedPdfDocumentDto {
        document_id: document_id.clone(),
    };
    let insert_result = database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        transaction
            .execute(
                "INSERT INTO documents (
                     id, title, original_filename, mime_type, byte_size, sha256,
                     source_relative_path, staged_relative_path, lifecycle_state,
                     source_format, source_unit_kind,
                     extraction_backend, render_backend, outline_state,
                     created_at, updated_at, revision
                 ) VALUES (
                     ?1, ?2, ?3, 'application/pdf', ?4, ?5,
                     ?6, NULL, 'processing', ?7, ?8,
                     'pending', 'pending', 'pending', ?9, ?9, 1
                 )",
                params![
                    document_id,
                    title,
                    original_filename,
                    byte_size,
                    sha256,
                    source_relative,
                    DocumentFormat::Pdf.as_str(),
                    SourceUnitKind::Page.as_str(),
                    now,
                ],
            )
            .map_err(|error| {
                CommandError::database_at(
                    "document_import_insert_failed",
                    format!("document={document_id} format=pdf"),
                    error,
                )
            })?;
        external_files::mark_import_consumed(&transaction, staged_file_id, &result, now)?;
        transaction.commit().map_err(CommandError::database)
    });
    if let Err(error) = insert_result {
        let _ = fs::remove_dir_all(directory);
        return Err(error);
    }
    Ok(result)
}

fn deleted_pdf_citation(title: &str, original_filename: &str, page_count: i64) -> String {
    let pages = if page_count > 0 {
        format!(
            ", {page_count} PDF page{}",
            if page_count == 1 { "" } else { "s" }
        )
    } else {
        String::new()
    };
    format!("Source PDF (deleted from Documents): {title} ({original_filename}){pages}.")
}

fn preserve_deleted_pdf_citation(
    description: &str,
    citation: &str,
    title: &str,
    original_filename: &str,
) -> Result<String, CommandError> {
    if description.contains("Source PDF (deleted from Documents):") {
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
        "The generated deck description is too long to preserve a text citation for this PDF. Shorten that deck description, then delete the PDF again.",
    ))
}

fn copy_pdf_with_hash(source: &Path, destination: &Path) -> Result<(i64, String), CommandError> {
    let mut input = File::open(source).map_err(CommandError::io)?;
    let mut signature = [0_u8; 5];
    input.read_exact(&mut signature).map_err(CommandError::io)?;
    if &signature != b"%PDF-" {
        return Err(CommandError::new(
            "invalid_pdf",
            "The selected file does not have a valid PDF signature.",
        ));
    }
    input.rewind().map_err(CommandError::io)?;
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(CommandError::io)?;
    }
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(CommandError::io)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 1024 * 1024];
    let mut total = 0_u64;
    loop {
        let read = input.read(&mut buffer).map_err(CommandError::io)?;
        if read == 0 {
            break;
        }
        total = total.saturating_add(read as u64);
        if total > MAX_PDF_BYTES {
            let _ = fs::remove_file(destination);
            return Err(CommandError::new(
                "invalid_pdf_size",
                "The PDF exceeds the 512 MB document limit.",
            ));
        }
        hasher.update(&buffer[..read]);
        output
            .write_all(&buffer[..read])
            .map_err(CommandError::io)?;
    }
    output.sync_all().map_err(CommandError::io)?;
    Ok((
        i64::try_from(total).unwrap_or(i64::MAX),
        format!("{:x}", hasher.finalize()),
    ))
}

fn validate_pdf_filename(value: &str) -> Result<String, CommandError> {
    let trimmed = value.trim();
    let valid = !trimmed.is_empty()
        && trimmed.chars().count() <= 255
        && !trimmed
            .chars()
            .any(|character| character.is_control() || matches!(character, '/' | '\\' | '\0'))
        && trimmed.to_ascii_lowercase().ends_with(".pdf");
    if !valid {
        return Err(CommandError::new(
            "invalid_pdf_name",
            "Select a PDF with a valid filename.",
        ));
    }
    Ok(trimmed.to_string())
}

fn validate_document_title(value: &str) -> Result<String, CommandError> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.chars().count() > 200 {
        return Err(CommandError::new(
            "invalid_document_title",
            "The PDF title must contain 1 to 200 characters.",
        ));
    }
    Ok(trimmed.to_string())
}

fn sync_file(path: &Path) -> Result<(), CommandError> {
    File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(CommandError::io)
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
            "AI-generated flashcards from chapter.pdf. Edit or delete these cards through the normal deck tools.";
        let citation = deleted_pdf_citation("Chapter", "chapter.pdf", 10);
        let preserved =
            preserve_deleted_pdf_citation(description, &citation, "Chapter", "chapter.pdf")
                .expect("automatic citation should fit");
        assert!(preserved
            .contains("Source PDF (deleted from Documents): Chapter (chapter.pdf), 10 PDF pages."));
        assert!(!preserved.contains("AI-generated flashcards from chapter.pdf."));
    }

    #[test]
    fn deletion_never_truncates_a_user_description_that_already_names_the_source() {
        let description = format!("{} chapter.pdf", "x".repeat(1_980));
        let citation = deleted_pdf_citation("Chapter", "chapter.pdf", 10);
        let preserved =
            preserve_deleted_pdf_citation(&description, &citation, "Chapter", "chapter.pdf")
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
