use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Seek, Write},
    path::Path,
};

use justpdf::Document;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::State;

use crate::{
    ai::{document_flashcards, AiRuntimeState},
    authoring::card_types_and_templates::generated_import::{
        import_generated_flashcards, FlashcardImportResultDto,
    },
    database::Database,
    error::CommandError,
    paths::AppPaths,
    util::{generate_id, unix_time_millis, validate_entity_id},
};

const MAX_PDF_BYTES: u64 = 512 * 1024 * 1024;
const MAX_PDF_PAGES: usize = 2_000;
const MAX_PAGE_TEXT_CHARS: usize = 200_000;
const MAX_TOTAL_TEXT_BYTES: usize = 96 * 1024 * 1024;
const MAX_RENDER_BYTES: usize = 32 * 1024 * 1024;
const MAX_GENERATED_CARDS: usize = 10_000;
const PDF_RENDER_DPI: f64 = 120.0;
const MAX_DOCUMENTS_RETURNED: i64 = 500;
const INTERRUPTED_ERROR_CODE: &str = "document_generation_interrupted";
const INTERRUPTED_ERROR_MESSAGE: &str =
    "PDF processing stopped when ostudy closed. Retry generation to continue.";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportPdfDocumentInput {
    pub source_path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RetryPdfGenerationInput {
    pub document_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetDocumentArchivedInput {
    pub document_id: String,
    pub revision: i64,
    pub archived: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentDto {
    pub id: String,
    pub title: String,
    pub original_filename: String,
    pub byte_size: i64,
    pub sha256: String,
    pub lifecycle_state: String,
    pub page_count: i64,
    pub extracted_page_count: i64,
    pub rendered_page_count: i64,
    pub generated_note_count: i64,
    pub generated_card_count: i64,
    pub generated_deck_id: Option<String>,
    pub safe_error_code: String,
    pub safe_error_message: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub completed_at: Option<i64>,
    pub archived_at: Option<i64>,
    pub revision: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentGenerationResultDto {
    pub document: DocumentDto,
    pub deck_id: String,
    pub created_note_count: i64,
    pub created_card_count: i64,
    pub skipped_duplicate_count: i64,
}

#[derive(Clone, Debug)]
struct StoredDocument {
    dto: DocumentDto,
    source_relative_path: String,
}

#[derive(Clone, Debug)]
struct ExtractedPage {
    page_number: i64,
    plain_text: String,
    text_sha256: String,
    render_relative_path: String,
    render_byte_size: i64,
    render_sha256: String,
}

#[derive(Debug)]
struct ManagedPdf {
    document_id: String,
}

#[tauri::command]
pub fn list_documents(
    database: State<'_, Database>,
    include_archived: bool,
) -> Result<Vec<DocumentDto>, CommandError> {
    database.with_connection(|connection| list_documents_from_connection(connection, include_archived))
}

#[tauri::command]
pub fn get_document_source_path(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    document_id: String,
) -> Result<String, CommandError> {
    validate_entity_id(&document_id, "document")?;
    let stored = database.with_connection(|connection| load_document(connection, &document_id))?;
    let source = paths.resolve_relative(&stored.source_relative_path);
    if !source.is_file() {
        return Err(CommandError::new(
            "document_source_missing",
            "The managed PDF is missing. Restore the library from backup or import the PDF again.",
        ));
    }
    Ok(source.to_string_lossy().into_owned())
}

#[tauri::command]
pub async fn import_pdf_and_generate_flashcards(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    runtime: State<'_, AiRuntimeState>,
    input: ImportPdfDocumentInput,
) -> Result<DocumentGenerationResultDto, CommandError> {
    let managed = import_managed_pdf(database.inner(), paths.inner(), &input.source_path)?;
    match process_and_generate(
        database.clone(),
        paths.clone(),
        runtime.clone(),
        &managed.document_id,
        false,
    )
    .await
    {
        Ok(result) => Ok(result),
        Err(error) => {
            record_document_failure(database.inner(), &managed.document_id, &error);
            Err(error)
        }
    }
}

#[tauri::command]
pub async fn retry_pdf_flashcard_generation(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    runtime: State<'_, AiRuntimeState>,
    input: RetryPdfGenerationInput,
) -> Result<DocumentGenerationResultDto, CommandError> {
    validate_entity_id(&input.document_id, "document")?;
    let document =
        database.with_connection(|connection| load_document(connection, &input.document_id))?;
    if document.dto.archived_at.is_some() {
        return Err(CommandError::new(
            "document_archived",
            "Restore this document before generating flashcards from it.",
        ));
    }
    if document.dto.generated_deck_id.is_some() {
        return Err(CommandError::new(
            "document_already_generated",
            "This PDF already has a generated deck. Delete that deck before generating it again.",
        ));
    }
    match process_and_generate(
        database.clone(),
        paths.clone(),
        runtime.clone(),
        &input.document_id,
        true,
    )
    .await
    {
        Ok(result) => Ok(result),
        Err(error) => {
            record_document_failure(database.inner(), &input.document_id, &error);
            Err(error)
        }
    }
}

#[tauri::command]
pub fn set_document_archived(
    database: State<'_, Database>,
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
        let lifecycle = if input.archived {
            "archived"
        } else {
            let generated_deck_id: Option<String> = connection
                .query_row(
                    "SELECT generated_deck_id FROM documents
                     WHERE id = ?1 AND revision = ?2",
                    params![input.document_id, input.revision],
                    |row| row.get(0),
                )
                .optional()
                .map_err(CommandError::database)?
                .flatten();
            if generated_deck_id.is_some() {
                "ready"
            } else {
                "failed"
            }
        };
        let changed = connection
            .execute(
                "UPDATE documents
                 SET lifecycle_state = ?1,
                     archived_at = ?2,
                     updated_at = ?3,
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
            .map_err(CommandError::database)?;
        if changed != 1 {
            return Err(CommandError::new(
                "document_changed",
                "This document changed before the archive action completed. Reload it and try again.",
            ));
        }
        Ok(load_document(connection, &input.document_id)?.dto)
    })
}

pub(crate) fn reconcile_startup(
    database: &Database,
    paths: &AppPaths,
) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        transaction
            .execute(
                "UPDATE documents
                 SET lifecycle_state = 'failed',
                     safe_error_code = ?1,
                     safe_error_message = ?2,
                     updated_at = ?3,
                     revision = revision + 1
                 WHERE lifecycle_state = 'processing'",
                params![INTERRUPTED_ERROR_CODE, INTERRUPTED_ERROR_MESSAGE, now],
            )
            .map_err(CommandError::database)?;
        transaction
            .execute(
                "UPDATE document_generation_jobs
                 SET state = 'interrupted',
                     safe_error_code = ?1,
                     safe_error_message = ?2,
                     updated_at = ?3
                 WHERE state IN ('queued', 'extracting', 'generating', 'committing')",
                params![INTERRUPTED_ERROR_CODE, INTERRUPTED_ERROR_MESSAGE, now],
            )
            .map_err(CommandError::database)?;
        transaction.commit().map_err(CommandError::database)
    })?;

    if paths.document_staging_dir().is_dir() {
        for entry in fs::read_dir(paths.document_staging_dir()).map_err(CommandError::io)? {
            let entry = entry.map_err(CommandError::io)?;
            let path = entry.path();
            if path.is_file() {
                let _ = fs::remove_file(path);
            } else if path.is_dir() {
                let _ = fs::remove_dir_all(path);
            }
        }
    }

    let missing = database.with_connection(|connection| {
        let mut statement = connection
            .prepare(
                "SELECT id, source_relative_path
                 FROM documents
                 WHERE archived_at IS NULL",
            )
            .map_err(CommandError::database)?;
        statement
            .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)
    })?;
    for (document_id, relative_path) in missing {
        if !paths.resolve_relative(&relative_path).is_file() {
            let error = CommandError::new(
                "document_source_missing",
                "The managed PDF is missing. Restore the library from backup or import the PDF again.",
            );
            record_document_failure(database, &document_id, &error);
        }
    }
    Ok(())
}

async fn process_and_generate(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    runtime: State<'_, AiRuntimeState>,
    document_id: &str,
    force_extract: bool,
) -> Result<DocumentGenerationResultDto, CommandError> {
    mark_document_processing(database.inner(), document_id)?;
    let stored = database.with_connection(|connection| load_document(connection, document_id))?;
    let source_path = paths.resolve_relative(&stored.source_relative_path);
    if !source_path.is_file() {
        return Err(CommandError::new(
            "document_source_missing",
            "The managed PDF is missing. Restore the library from backup or import the PDF again.",
        ));
    }

    let should_extract = force_extract
        || stored.dto.page_count == 0
        || stored.dto.extracted_page_count != stored.dto.page_count
        || stored.dto.rendered_page_count != stored.dto.page_count;
    if should_extract {
        let path_clone = paths.inner().clone();
        let document_id_owned = document_id.to_string();
        let source_clone = source_path.clone();
        let extracted = tauri::async_runtime::spawn_blocking(move || {
            extract_and_render_pdf(&path_clone, &document_id_owned, &source_clone)
        })
        .await
        .map_err(|error| {
            CommandError::new(
                "document_processing_failed",
                format!("The PDF processing task stopped unexpectedly: {error}"),
            )
        })??;
        persist_extracted_pages(database.inner(), document_id, &extracted)?;
    }

    let pages = load_generation_pages(database.inner(), paths.inner(), document_id)?;
    if pages.is_empty() {
        return Err(CommandError::new(
            "document_has_no_pages",
            "The PDF did not contain any pages that ostudy could process.",
        ));
    }
    let task = document_flashcards::load_document_task_context(database.inner())?;
    let job_id = create_generation_job(database.inner(), document_id, &task, pages.len())?;
    let mut all_cards = Vec::new();

    for page in &pages {
        mark_generation_unit_running(database.inner(), &job_id, page.page_number)?;
        let image = if task.include_page_images {
            let bytes = fs::read(paths.resolve_relative(&page.render_relative_path))
                .map_err(CommandError::io)?;
            if bytes.is_empty() || bytes.len() > MAX_RENDER_BYTES {
                return Err(CommandError::new(
                    "document_render_invalid",
                    "A rendered PDF page is empty or exceeds the provider request limit.",
                ));
            }
            Some(bytes)
        } else {
            None
        };

        if page.plain_text.trim().is_empty() && image.is_none() {
            mark_generation_unit_complete(database.inner(), &job_id, page.page_number, &[], true)?;
            continue;
        }

        let cards = document_flashcards::generate_page_flashcards(
            runtime.inner(),
            &task,
            &stored.dto.title,
            page.page_number,
            &page.plain_text,
            image.as_deref(),
        )
        .await?;
        if all_cards.len().saturating_add(cards.len()) > MAX_GENERATED_CARDS {
            return Err(CommandError::new(
                "too_many_generated_cards",
                format!(
                    "The model generated more than {MAX_GENERATED_CARDS} cards from this PDF."
                ),
            ));
        }
        mark_generation_unit_complete(
            database.inner(),
            &job_id,
            page.page_number,
            &cards,
            false,
        )?;
        all_cards.extend(cards);
    }

    if all_cards.is_empty() {
        return Err(CommandError::new(
            "document_generated_no_cards",
            "The model did not generate any valid flashcards from this PDF.",
        ));
    }

    mark_job_committing(database.inner(), &job_id, all_cards.len())?;
    let generated_json = build_generated_import_json(&stored.dto, all_cards)?;
    let import_path = paths
        .document_staging_dir()
        .join(format!("{document_id}-{job_id}.ostudy.json"));
    write_new_file(&import_path, generated_json.as_bytes())?;
    let import_result = import_generated_flashcards(
        database.clone(),
        import_path.to_string_lossy().into_owned(),
    );
    let _ = fs::remove_file(&import_path);
    let import_result = import_result?;
    finalize_generation(database.inner(), document_id, &job_id, &import_result)?;
    let document =
        database.with_connection(|connection| load_document(connection, document_id))?.dto;
    Ok(DocumentGenerationResultDto {
        document,
        deck_id: import_result.deck_id,
        created_note_count: import_result.created_note_count,
        created_card_count: import_result.created_card_count,
        skipped_duplicate_count: import_result.skipped_duplicate_count,
    })
}

fn import_managed_pdf(
    database: &Database,
    paths: &AppPaths,
    source_path: &str,
) -> Result<ManagedPdf, CommandError> {
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
    let original_filename = validate_pdf_filename(
        source
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default(),
    )?;
    let title = validate_document_title(
        Path::new(&original_filename)
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("Imported PDF"),
    )?;
    let document_id = database.with_connection(generate_id)?;
    let staged_relative = AppPaths::staged_document_relative_path(&document_id);
    let staged_path = paths.resolve_relative(&staged_relative);
    let (byte_size, sha256) = copy_pdf_with_hash(&source, &staged_path)?;

    let duplicate: Option<String> = database.with_connection(|connection| {
        connection
            .query_row(
                "SELECT id FROM documents WHERE sha256 = ?1 LIMIT 1",
                [sha256.as_str()],
                |row| row.get(0),
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
    let inserted = database.with_connection(|connection| {
        connection
            .execute(
                "INSERT INTO documents (
                     id, title, original_filename, mime_type, byte_size, sha256,
                     source_relative_path, staged_relative_path, lifecycle_state,
                     created_at, updated_at, revision
                 ) VALUES (
                     ?1, ?2, ?3, 'application/pdf', ?4, ?5,
                     ?6, NULL, 'processing', ?7, ?7, 1
                 )",
                params![
                    document_id,
                    title,
                    original_filename,
                    byte_size,
                    sha256,
                    source_relative,
                    now,
                ],
            )
            .map_err(CommandError::database)?;
        Ok(())
    });
    if let Err(error) = inserted {
        let _ = fs::remove_dir_all(directory);
        return Err(error);
    }

    Ok(ManagedPdf { document_id })
}

fn extract_and_render_pdf(
    paths: &AppPaths,
    document_id: &str,
    source_path: &Path,
) -> Result<Vec<ExtractedPage>, CommandError> {
    let document = Document::open(source_path).map_err(|error| {
        CommandError::new(
            "pdf_open_failed",
            format!("The PDF could not be opened: {error}"),
        )
    })?;
    let page_count = document.page_count();
    if page_count == 0 || page_count > MAX_PDF_PAGES {
        return Err(CommandError::new(
            "unsupported_pdf_page_count",
            format!("PDFs must contain between 1 and {MAX_PDF_PAGES} pages."),
        ));
    }

    let page_directory = paths.document_pages_dir(document_id);
    fs::create_dir_all(&page_directory).map_err(CommandError::io)?;
    let mut extracted = Vec::with_capacity(page_count);
    let mut total_text_bytes = 0_usize;

    for index in 0..page_count {
        let page = document.page(index).map_err(|error| {
            CommandError::new(
                "pdf_page_failed",
                format!("PDF page {} could not be read: {error}", index + 1),
            )
        })?;
        let plain_text = page.text().map(normalize_page_text).map_err(|error| {
            CommandError::new(
                "pdf_text_extraction_failed",
                format!(
                    "Text could not be extracted from PDF page {}: {error}",
                    index + 1
                ),
            )
        })?;
        total_text_bytes = total_text_bytes.saturating_add(plain_text.len());
        if total_text_bytes > MAX_TOTAL_TEXT_BYTES {
            return Err(CommandError::new(
                "pdf_text_too_large",
                "The extracted PDF text exceeds ostudy's 96 MB processing limit.",
            ));
        }

        let png = page.render_png(PDF_RENDER_DPI).map_err(|error| {
            CommandError::new(
                "pdf_render_failed",
                format!("PDF page {} could not be rendered: {error}", index + 1),
            )
        })?;
        if png.is_empty() || png.len() > MAX_RENDER_BYTES {
            return Err(CommandError::new(
                "pdf_render_too_large",
                format!(
                    "The rendered image for PDF page {} exceeds ostudy's page-image limit.",
                    index + 1
                ),
            ));
        }
        let page_number = i64::try_from(index + 1).unwrap_or(i64::MAX);
        let render_relative = AppPaths::document_page_relative_path(document_id, page_number);
        let render_path = paths.resolve_relative(&render_relative);
        write_replacing_file(&render_path, &png)?;

        extracted.push(ExtractedPage {
            page_number,
            text_sha256: sha256_hex(plain_text.as_bytes()),
            plain_text,
            render_relative_path: render_relative,
            render_byte_size: i64::try_from(png.len()).unwrap_or(i64::MAX),
            render_sha256: sha256_hex(&png),
        });
    }
    sync_directory(&page_directory)?;
    Ok(extracted)
}

fn persist_extracted_pages(
    database: &Database,
    document_id: &str,
    pages: &[ExtractedPage],
) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        transaction
            .execute(
                "DELETE FROM document_pages WHERE document_id = ?1",
                [document_id],
            )
            .map_err(CommandError::database)?;
        for page in pages {
            transaction
                .execute(
                    "INSERT INTO document_pages (
                         document_id, page_number, plain_text, text_sha256,
                         render_relative_path, render_mime_type, render_byte_size,
                         render_sha256, created_at, updated_at
                     ) VALUES (
                         ?1, ?2, ?3, ?4, ?5, 'image/png', ?6, ?7, ?8, ?8
                     )",
                    params![
                        document_id,
                        page.page_number,
                        page.plain_text,
                        page.text_sha256,
                        page.render_relative_path,
                        page.render_byte_size,
                        page.render_sha256,
                        now,
                    ],
                )
                .map_err(CommandError::database)?;
        }
        let count = i64::try_from(pages.len()).unwrap_or(i64::MAX);
        transaction
            .execute(
                "UPDATE documents
                 SET page_count = ?1,
                     extracted_page_count = ?1,
                     rendered_page_count = ?1,
                     safe_error_code = '',
                     safe_error_message = '',
                     updated_at = ?2,
                     revision = revision + 1
                 WHERE id = ?3",
                params![count, now, document_id],
            )
            .map_err(CommandError::database)?;
        transaction.commit().map_err(CommandError::database)
    })
}

fn load_generation_pages(
    database: &Database,
    paths: &AppPaths,
    document_id: &str,
) -> Result<Vec<ExtractedPage>, CommandError> {
    database.with_connection(|connection| {
        let mut statement = connection
            .prepare(
                "SELECT page_number, plain_text, text_sha256, render_relative_path,
                        render_byte_size, render_sha256
                 FROM document_pages
                 WHERE document_id = ?1
                 ORDER BY page_number",
            )
            .map_err(CommandError::database)?;
        let pages = statement
            .query_map([document_id], |row| {
                Ok(ExtractedPage {
                    page_number: row.get(0)?,
                    plain_text: row.get(1)?,
                    text_sha256: row.get(2)?,
                    render_relative_path: row.get(3)?,
                    render_byte_size: row.get(4)?,
                    render_sha256: row.get(5)?,
                })
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        for page in &pages {
            let path = paths.resolve_relative(&page.render_relative_path);
            if !path.is_file() {
                return Err(CommandError::new(
                    "document_render_missing",
                    "A rendered PDF page is missing. Retry generation to rebuild the document.",
                ));
            }
        }
        Ok(pages)
    })
}

fn create_generation_job(
    database: &Database,
    document_id: &str,
    task: &document_flashcards::DocumentTaskContext,
    page_count: usize,
) -> Result<String, CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        let job_id = generate_id(&transaction)?;
        transaction
            .execute(
                "INSERT INTO document_generation_jobs (
                     id, document_id, profile_id, provider_kind, model_id,
                     profile_revision, credential_epoch, network_scope, state,
                     page_count, processed_page_count, generated_candidate_count,
                     created_at, started_at, updated_at
                 ) VALUES (
                     ?1, ?2, ?3, ?4, ?5,
                     ?6, ?7, ?8, 'generating',
                     ?9, 0, 0, ?10, ?10, ?10
                 )",
                params![
                    job_id,
                    document_id,
                    task.profile_id,
                    task.provider_kind,
                    task.model_id,
                    task.profile_revision,
                    task.credential_epoch,
                    task.network_scope,
                    i64::try_from(page_count).unwrap_or(i64::MAX),
                    now,
                ],
            )
            .map_err(CommandError::database)?;
        for page_number in 1..=page_count {
            transaction
                .execute(
                    "INSERT INTO document_generation_units (
                         job_id, page_number, state, cards_json, card_count, updated_at
                     ) VALUES (?1, ?2, 'pending', '[]', 0, ?3)",
                    params![
                        job_id,
                        i64::try_from(page_number).unwrap_or(i64::MAX),
                        now
                    ],
                )
                .map_err(CommandError::database)?;
        }
        transaction.commit().map_err(CommandError::database)?;
        Ok(job_id)
    })
}

fn mark_generation_unit_running(
    database: &Database,
    job_id: &str,
    page_number: i64,
) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        connection
            .execute(
                "UPDATE document_generation_units
                 SET state = 'running', updated_at = ?1
                 WHERE job_id = ?2 AND page_number = ?3",
                params![now, job_id, page_number],
            )
            .map_err(CommandError::database)?;
        Ok(())
    })
}

fn mark_generation_unit_complete(
    database: &Database,
    job_id: &str,
    page_number: i64,
    cards: &[serde_json::Value],
    skipped: bool,
) -> Result<(), CommandError> {
    let cards_json = serde_json::to_string(cards).map_err(CommandError::serialization)?;
    if cards_json.len() > 1_048_576 {
        return Err(CommandError::new(
            "generated_page_cards_too_large",
            "The model returned too much flashcard content for one PDF page.",
        ));
    }
    let now = unix_time_millis()?;
    let card_count = i64::try_from(cards.len()).unwrap_or(i64::MAX);
    database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        transaction
            .execute(
                "UPDATE document_generation_units
                 SET state = ?1,
                     cards_json = ?2,
                     card_count = ?3,
                     safe_error_code = '',
                     safe_error_message = '',
                     updated_at = ?4
                 WHERE job_id = ?5 AND page_number = ?6",
                params![
                    if skipped { "skipped" } else { "succeeded" },
                    cards_json,
                    card_count,
                    now,
                    job_id,
                    page_number,
                ],
            )
            .map_err(CommandError::database)?;
        transaction
            .execute(
                "UPDATE document_generation_jobs
                 SET processed_page_count = (
                         SELECT COUNT(*) FROM document_generation_units
                         WHERE job_id = ?1 AND state IN ('succeeded', 'skipped')
                     ),
                     generated_candidate_count = (
                         SELECT COALESCE(SUM(card_count), 0)
                         FROM document_generation_units WHERE job_id = ?1
                     ),
                     updated_at = ?2
                 WHERE id = ?1",
                params![job_id, now],
            )
            .map_err(CommandError::database)?;
        transaction.commit().map_err(CommandError::database)
    })
}

fn mark_job_committing(
    database: &Database,
    job_id: &str,
    candidate_count: usize,
) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        connection
            .execute(
                "UPDATE document_generation_jobs
                 SET state = 'committing',
                     generated_candidate_count = ?1,
                     updated_at = ?2
                 WHERE id = ?3",
                params![
                    i64::try_from(candidate_count).unwrap_or(i64::MAX),
                    now,
                    job_id,
                ],
            )
            .map_err(CommandError::database)?;
        Ok(())
    })
}

fn finalize_generation(
    database: &Database,
    document_id: &str,
    job_id: &str,
    result: &FlashcardImportResultDto,
) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        transaction
            .execute(
                "INSERT INTO document_decks (document_id, deck_id, job_id, created_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![document_id, result.deck_id, job_id, now],
            )
            .map_err(CommandError::database)?;
        transaction
            .execute(
                "UPDATE documents
                 SET lifecycle_state = 'ready',
                     generated_note_count = ?1,
                     generated_card_count = ?2,
                     generated_deck_id = ?3,
                     safe_error_code = '',
                     safe_error_message = '',
                     updated_at = ?4,
                     completed_at = ?4,
                     revision = revision + 1
                 WHERE id = ?5",
                params![
                    result.created_note_count,
                    result.created_card_count,
                    result.deck_id,
                    now,
                    document_id,
                ],
            )
            .map_err(CommandError::database)?;
        transaction
            .execute(
                "UPDATE document_generation_jobs
                 SET state = 'succeeded',
                     generated_deck_id = ?1,
                     safe_error_code = '',
                     safe_error_message = '',
                     updated_at = ?2,
                     completed_at = ?2
                 WHERE id = ?3",
                params![result.deck_id, now, job_id],
            )
            .map_err(CommandError::database)?;
        transaction.commit().map_err(CommandError::database)
    })
}

fn mark_document_processing(database: &Database, document_id: &str) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        connection
            .execute(
                "UPDATE documents
                 SET lifecycle_state = 'processing',
                     safe_error_code = '',
                     safe_error_message = '',
                     updated_at = ?1,
                     completed_at = NULL,
                     revision = revision + 1
                 WHERE id = ?2 AND archived_at IS NULL",
                params![now, document_id],
            )
            .map_err(CommandError::database)?;
        Ok(())
    })
}

fn record_document_failure(database: &Database, document_id: &str, error: &CommandError) {
    let now = unix_time_millis().unwrap_or_default();
    let _ = database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        transaction
            .execute(
                "UPDATE documents
                 SET lifecycle_state = CASE
                         WHEN archived_at IS NULL THEN 'failed'
                         ELSE 'archived'
                     END,
                     safe_error_code = ?1,
                     safe_error_message = ?2,
                     updated_at = ?3,
                     revision = revision + 1
                 WHERE id = ?4",
                params![
                    bounded_error(&error.code, 80),
                    bounded_error(&error.message, 1000),
                    now,
                    document_id,
                ],
            )
            .map_err(CommandError::database)?;
        transaction
            .execute(
                "UPDATE document_generation_jobs
                 SET state = 'failed',
                     safe_error_code = ?1,
                     safe_error_message = ?2,
                     updated_at = ?3,
                     completed_at = ?3
                 WHERE document_id = ?4
                   AND state IN ('queued', 'extracting', 'generating', 'committing')",
                params![
                    bounded_error(&error.code, 80),
                    bounded_error(&error.message, 1000),
                    now,
                    document_id,
                ],
            )
            .map_err(CommandError::database)?;
        transaction.commit().map_err(CommandError::database)
    });
}

fn build_generated_import_json(
    document: &DocumentDto,
    cards: Vec<serde_json::Value>,
) -> Result<String, CommandError> {
    let description = format!(
        "AI-generated flashcards from {}. Edit or delete these cards through the normal deck tools.",
        document.original_filename
    );
    serde_json::to_string(&serde_json::json!({
        "format": "ostudy-flashcards",
        "formatVersion": 1,
        "deck": {
            "mode": "create",
            "name": document.title,
            "description": description,
        },
        "defaultTags": ["AI generated", "PDF"],
        "cards": cards,
    }))
    .map_err(CommandError::serialization)
}

fn list_documents_from_connection(
    connection: &Connection,
    include_archived: bool,
) -> Result<Vec<DocumentDto>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT id, title, original_filename, byte_size, sha256, lifecycle_state,
                    page_count, extracted_page_count, rendered_page_count,
                    generated_note_count, generated_card_count, generated_deck_id,
                    safe_error_code, safe_error_message, created_at, updated_at,
                    completed_at, archived_at, revision
             FROM documents
             WHERE (?1 = 1 OR archived_at IS NULL)
             ORDER BY updated_at DESC, id
             LIMIT ?2",
        )
        .map_err(CommandError::database)?;
    statement
        .query_map(
            params![
                if include_archived { 1_i64 } else { 0_i64 },
                MAX_DOCUMENTS_RETURNED
            ],
            document_row,
        )
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)
}

fn load_document(
    connection: &Connection,
    document_id: &str,
) -> Result<StoredDocument, CommandError> {
    connection
        .query_row(
            "SELECT id, title, original_filename, byte_size, sha256, lifecycle_state,
                    page_count, extracted_page_count, rendered_page_count,
                    generated_note_count, generated_card_count, generated_deck_id,
                    safe_error_code, safe_error_message, created_at, updated_at,
                    completed_at, archived_at, revision, source_relative_path
             FROM documents
             WHERE id = ?1",
            [document_id],
            |row| {
                Ok(StoredDocument {
                    dto: DocumentDto {
                        id: row.get(0)?,
                        title: row.get(1)?,
                        original_filename: row.get(2)?,
                        byte_size: row.get(3)?,
                        sha256: row.get(4)?,
                        lifecycle_state: row.get(5)?,
                        page_count: row.get(6)?,
                        extracted_page_count: row.get(7)?,
                        rendered_page_count: row.get(8)?,
                        generated_note_count: row.get(9)?,
                        generated_card_count: row.get(10)?,
                        generated_deck_id: row.get(11)?,
                        safe_error_code: row.get(12)?,
                        safe_error_message: row.get(13)?,
                        created_at: row.get(14)?,
                        updated_at: row.get(15)?,
                        completed_at: row.get(16)?,
                        archived_at: row.get(17)?,
                        revision: row.get(18)?,
                    },
                    source_relative_path: row.get(19)?,
                })
            },
        )
        .optional()
        .map_err(CommandError::database)?
        .ok_or_else(|| {
            CommandError::new("document_not_found", "That PDF is no longer in the library.")
        })
}

fn document_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<DocumentDto> {
    Ok(DocumentDto {
        id: row.get(0)?,
        title: row.get(1)?,
        original_filename: row.get(2)?,
        byte_size: row.get(3)?,
        sha256: row.get(4)?,
        lifecycle_state: row.get(5)?,
        page_count: row.get(6)?,
        extracted_page_count: row.get(7)?,
        rendered_page_count: row.get(8)?,
        generated_note_count: row.get(9)?,
        generated_card_count: row.get(10)?,
        generated_deck_id: row.get(11)?,
        safe_error_code: row.get(12)?,
        safe_error_message: row.get(13)?,
        created_at: row.get(14)?,
        updated_at: row.get(15)?,
        completed_at: row.get(16)?,
        archived_at: row.get(17)?,
        revision: row.get(18)?,
    })
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
    let parent = destination.parent().ok_or_else(|| {
        CommandError::new(
            "invalid_document_path",
            "The document staging path is invalid.",
        )
    })?;
    fs::create_dir_all(parent).map_err(CommandError::io)?;
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
        output.write_all(&buffer[..read]).map_err(CommandError::io)?;
    }
    output.sync_all().map_err(CommandError::io)?;
    Ok((
        i64::try_from(total).unwrap_or(i64::MAX),
        format!("{:x}", hasher.finalize()),
    ))
}

fn normalize_page_text(value: String) -> String {
    let normalized = value.replace("\r\n", "\n").replace('\r', "\n");
    normalized
        .chars()
        .filter(|character| {
            !character.is_control() || matches!(character, '\n' | '\t')
        })
        .take(MAX_PAGE_TEXT_CHARS)
        .collect::<String>()
        .trim()
        .to_string()
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

fn bounded_error(value: &str, maximum: usize) -> String {
    value.chars().take(maximum).collect()
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn write_replacing_file(destination: &Path, bytes: &[u8]) -> Result<(), CommandError> {
    let parent = destination.parent().ok_or_else(|| {
        CommandError::new(
            "invalid_document_path",
            "The rendered-page path is invalid.",
        )
    })?;
    fs::create_dir_all(parent).map_err(CommandError::io)?;
    let temporary = destination.with_extension(format!("png.{}.tmp", std::process::id()));
    let _ = fs::remove_file(&temporary);
    write_new_file(&temporary, bytes)?;
    if destination.exists() {
        fs::remove_file(destination).map_err(CommandError::io)?;
    }
    fs::rename(&temporary, destination).map_err(CommandError::io)?;
    sync_file(destination)?;
    Ok(())
}

fn write_new_file(destination: &Path, bytes: &[u8]) -> Result<(), CommandError> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(CommandError::io)?;
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(CommandError::io)?;
    file.write_all(bytes).map_err(CommandError::io)?;
    file.sync_all().map_err(CommandError::io)
}

fn sync_file(path: &Path) -> Result<(), CommandError> {
    File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(CommandError::io)
}

fn sync_directory(path: &Path) -> Result<(), CommandError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(CommandError::io)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_page_text_without_control_characters() {
        assert_eq!(
            normalize_page_text(" line\r\none\0two\rthree ".to_string()),
            "line\nonetwo\nthree"
        );
    }

    #[test]
    fn validates_pdf_names_and_titles() {
        assert!(validate_pdf_filename("../unsafe.pdf").is_err());
        assert!(validate_pdf_filename("notes.txt").is_err());
        assert_eq!(
            validate_pdf_filename("Lecture Notes.PDF").unwrap(),
            "Lecture Notes.PDF"
        );
        assert_eq!(
            validate_document_title(" Lecture Notes ").unwrap(),
            "Lecture Notes"
        );
    }

    #[test]
    fn generated_import_envelope_is_strict_ostudy_json() {
        let document = DocumentDto {
            id: "0123456789abcdef0123456789abcdef".to_string(),
            title: "Physiology".to_string(),
            original_filename: "physiology.pdf".to_string(),
            byte_size: 1,
            sha256: "0".repeat(64),
            lifecycle_state: "processing".to_string(),
            page_count: 1,
            extracted_page_count: 1,
            rendered_page_count: 1,
            generated_note_count: 0,
            generated_card_count: 0,
            generated_deck_id: None,
            safe_error_code: String::new(),
            safe_error_message: String::new(),
            created_at: 1,
            updated_at: 1,
            completed_at: None,
            archived_at: None,
            revision: 1,
        };
        let value: serde_json::Value = serde_json::from_str(
            &build_generated_import_json(
                &document,
                vec![serde_json::json!({
                    "type": "basic",
                    "front": "Question",
                    "back": "Answer"
                })],
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(value["format"], "ostudy-flashcards");
        assert_eq!(value["deck"]["mode"], "create");
        assert_eq!(value["cards"].as_array().unwrap().len(), 1);
    }
}
