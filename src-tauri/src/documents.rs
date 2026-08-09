use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::Path,
};

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::State;

use crate::{
    ai::{document_intelligence, AiRuntimeState},
    authoring::card_types_and_templates::generated_import::import_generated_flashcards_bytes_with_commit,
    database::Database,
    document_checkpoints,
    document_scale::{self, DocumentScalePlan, MAX_SUPPORTED_PDF_PAGES},
    document_semantics::{build_partitioned_semantic_windows, SemanticChunk, SemanticWindow},
    error::CommandError,
    paths::AppPaths,
    pdf_backend::{self, ExtractedDocument, PdfBackendError, PdfBackendErrorKind},
    pdf_render_backend::{
        self, PdfRenderError, PdfRenderErrorKind, PdfRenderRuntime, RenderProcessError,
    },
    util::{generate_id, unix_time_millis, validate_entity_id},
};

const MAX_PDF_PAGES: usize = 2_000;
const MAX_PAGE_TEXT_CHARS: usize = 200_000;
const MAX_TOTAL_TEXT_BYTES: usize = 96 * 1024 * 1024;
const MAX_SEMANTIC_TEXT_BYTES: usize = 128 * 1024 * 1024;
const MAX_RENDER_BYTES: usize = 32 * 1024 * 1024;
const MAX_GENERATED_CARDS: usize = 10_000;
const MAX_DOCUMENTS_RETURNED: i64 = 500;

// Keep each provider request comfortably below common local llama-server
// context allocations. Large documents are handled by more semantic windows,
// not by making individual prompts approach the model's advertised maximum.
const OUTLINE_WINDOW_MAX_TOKENS: usize = 3_200;
const OUTLINE_WINDOW_MAX_PAGES: usize = 8;
const OUTLINE_WINDOW_OVERLAP_TOKENS: usize = 350;
const GENERATION_WINDOW_MAX_TOKENS: usize = 5_000;
const GENERATION_WINDOW_MAX_PAGES: usize = 10;
const GENERATION_WINDOW_OVERLAP_TOKENS: usize = 700;
const MAX_IMAGES_PER_GENERATION_WINDOW: usize = 6;

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
    pub scale_mode: String,
    pub estimated_source_tokens: i64,
    pub outline_window_count: i64,
    pub outline_completed_window_count: i64,
    pub generation_window_count: i64,
    pub generation_completed_window_count: i64,
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
    extraction_backend: String,
    render_backend: String,
    semantic_chunk_count: i64,
}

#[derive(Clone, Debug)]
struct PageRecord {
    page_number: i64,
    plain_text: String,
    text_sha256: String,
    render_relative_path: String,
    render_byte_size: i64,
    render_sha256: String,
}

#[derive(Debug)]
struct ProcessedDocument {
    pages: Vec<PageRecord>,
    semantic: ExtractedDocument,
}

#[tauri::command]
pub fn list_documents(
    database: State<'_, Database>,
    include_archived: bool,
) -> Result<Vec<DocumentDto>, CommandError> {
    database.with_connection(|connection| list_document_rows(connection, include_archived))
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
pub async fn retry_pdf_flashcard_generation(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    runtime: State<'_, AiRuntimeState>,
    render_runtime: State<'_, PdfRenderRuntime>,
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

    let result = process_and_generate(
        database.clone(),
        paths.clone(),
        runtime.clone(),
        render_runtime.inner().clone(),
        &input.document_id,
        false,
    )
    .await;
    if let Err(error) = &result {
        record_document_failure(database.inner(), &input.document_id, error);
    }
    result
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
        } else if active_generated_deck_exists(connection, &input.document_id)? {
            "ready"
        } else {
            "failed"
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

pub(crate) fn reconcile_startup(database: &Database, paths: &AppPaths) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        transaction
            .execute(
                "UPDATE documents
                 SET lifecycle_state = 'failed',
                     outline_state = CASE WHEN outline_state = 'building' THEN 'failed' ELSE outline_state END,
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
            if path.is_dir() {
                let _ = fs::remove_dir_all(path);
            } else {
                let _ = fs::remove_file(path);
            }
        }
    }

    let managed_sources = database.with_connection(|connection| {
        let mut statement = connection
            .prepare("SELECT id, source_relative_path FROM documents WHERE archived_at IS NULL")
            .map_err(CommandError::database)?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        Ok(rows)
    })?;
    for (document_id, relative_path) in managed_sources {
        if !paths.resolve_relative(&relative_path).is_file() {
            record_document_failure(
                database,
                &document_id,
                &CommandError::new(
                    "document_source_missing",
                    "The managed PDF is missing. Restore the library from backup or import the PDF again.",
                ),
            );
        }
    }
    Ok(())
}

async fn process_and_generate(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    runtime: State<'_, AiRuntimeState>,
    render_runtime: PdfRenderRuntime,
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

    let backend_changed = stored.extraction_backend != pdf_backend::active_backend_id()
        || stored.render_backend != pdf_render_backend::active_backend_id();
    let incomplete_derivatives = stored.dto.page_count == 0
        || stored.dto.extracted_page_count != stored.dto.page_count
        || stored.dto.rendered_page_count != stored.dto.page_count
        || stored.semantic_chunk_count == 0;
    if force_extract || backend_changed || incomplete_derivatives {
        let paths_owned = paths.inner().clone();
        let source_owned = source_path.clone();
        let id_owned = document_id.to_string();
        let (progress_sender, mut progress_receiver) =
            tokio::sync::mpsc::unbounded_channel::<(usize, usize)>();
        let processing_task = tauri::async_runtime::spawn_blocking(move || {
            extract_and_render_pdf(
                &paths_owned,
                &render_runtime,
                &id_owned,
                &source_owned,
                |completed, total| {
                    progress_sender.send((completed, total)).map_err(|_| {
                        CommandError::new(
                            "document_processing_stopped",
                            "The PDF processing progress channel closed unexpectedly.",
                        )
                    })
                },
            )
        });
        while let Some((completed, total)) = progress_receiver.recv().await {
            update_render_progress(database.inner(), document_id, completed, total)?;
        }
        let processed = processing_task.await.map_err(|error| {
            CommandError::new(
                "document_processing_failed",
                format!("The PDF processing task stopped unexpectedly: {error}"),
            )
        })??;
        persist_processed_document(database.inner(), document_id, &processed)?;
    }

    let pages = load_pages(database.inner(), paths.inner(), document_id)?;
    if pages.is_empty() {
        return Err(CommandError::new(
            "document_has_no_pages",
            "The PDF did not contain any pages that ostudy could process.",
        ));
    }
    let chunks = load_semantic_chunks(database.inner(), document_id)?;
    if chunks.is_empty() {
        return Err(CommandError::new(
            "document_has_no_semantic_content",
            "The PDF did not contain structured text that ostudy could analyze. Image-only PDFs require an image-capable generation profile.",
        ));
    }

    let task = document_intelligence::load_document_task_context(database.inner())?;
    let plan = DocumentScalePlan::for_provider(pages.len(), &task.provider_kind);
    let estimated_source_tokens = chunks.iter().fold(0_usize, |total, chunk| {
        total.saturating_add(
            chunk
                .token_estimate
                .max(chunk.full_text.chars().count().div_ceil(4)),
        )
    });
    let outline_windows = build_partitioned_semantic_windows(
        &chunks,
        plan,
        plan.outline_window_max_tokens,
        plan.outline_window_max_pages,
        plan.outline_window_overlap_tokens,
    );
    let generation_windows = build_partitioned_semantic_windows(
        &chunks,
        plan,
        plan.generation_window_max_tokens,
        plan.generation_window_max_pages,
        plan.generation_window_overlap_tokens,
    );
    if outline_windows.is_empty() || generation_windows.is_empty() {
        return Err(CommandError::new(
            "document_has_no_semantic_content",
            "The PDF did not contain bounded semantic source windows for document analysis.",
        ));
    }

    let budget = document_scale::validate_automatic_job(
        pages.len(),
        estimated_source_tokens,
        plan,
        outline_windows.len(),
        generation_windows.len(),
        &task.provider_kind,
    )?;
    crate::logging::info(
        "document_ai_scale_plan",
        format!(
            "document={} provider={} scale={} pages={} source_tokens={} outline_windows={} generation_windows={} card_capacity={} estimated_job_tokens={} ceiling={}",
            document_id,
            task.provider_kind,
            plan.label(),
            pages.len(),
            estimated_source_tokens,
            outline_windows.len(),
            generation_windows.len(),
            budget.generation_card_capacity,
            budget.estimated_tokens,
            budget.ceiling_tokens,
        ),
    );

    let outline_contract = document_checkpoints::outline_contract_key(&task, plan);
    let cached_outline = document_checkpoints::load_outline_fragments(
        database.inner(),
        document_id,
        &outline_contract,
        &outline_windows,
    )?;
    let outline_completed = cached_outline
        .iter()
        .filter(|value| value.is_some())
        .count();
    initialize_document_ai_plan(
        database.inner(),
        document_id,
        plan,
        estimated_source_tokens,
        outline_windows.len(),
        outline_completed,
        generation_windows.len(),
        0,
    )?;
    crate::logging::info(
        "document_scale_plan_selected",
        format!(
            "document={} scale={} pages={} source_tokens={} outline_windows={} generation_windows={} max_cards_per_window={} max_images_per_window={}",
            document_id,
            plan.label(),
            pages.len(),
            estimated_source_tokens,
            outline_windows.len(),
            generation_windows.len(),
            plan.max_cards_per_window,
            plan.max_images_per_window,
        ),
    );

    mark_outline_building(database.inner(), document_id)?;
    let outline = document_intelligence::build_document_outline(
        runtime.inner(),
        &task,
        &stored.dto.title,
        i64::try_from(pages.len()).unwrap_or(i64::MAX),
        &outline_windows,
        &cached_outline,
        |window, fragment| {
            document_checkpoints::persist_outline_fragment(
                database.inner(),
                document_id,
                &outline_contract,
                window,
                fragment,
            )?;
            let completed = document_checkpoints::count_outline_checkpoints(
                database.inner(),
                document_id,
                &outline_contract,
            )?;
            update_outline_progress(database.inner(), document_id, completed)?;
            Ok(())
        },
    )
    .await?;
    update_outline_progress(
        database.inner(),
        document_id,
        i64::try_from(outline_windows.len()).unwrap_or(i64::MAX),
    )?;
    persist_outline(database.inner(), document_id, &task, &outline)?;

    let generation_contract = document_checkpoints::generation_contract_key(
        &task,
        plan,
        &outline,
        plan.max_cards_per_window,
    )?;
    let cached_generation = generation_windows
        .iter()
        .map(|window| {
            document_checkpoints::load_generation_cards(
                database.inner(),
                document_id,
                &generation_contract,
                window,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let generation_completed = cached_generation
        .iter()
        .filter(|value| value.is_some())
        .count();
    update_generation_progress(
        database.inner(),
        document_id,
        i64::try_from(generation_completed).unwrap_or(i64::MAX),
    )?;

    let job_id = create_generation_job(
        database.inner(),
        document_id,
        &task,
        pages.len(),
        &generation_windows,
    )?;
    let mut generated_cards = Vec::new();
    for (window, cached_cards) in generation_windows.iter().zip(cached_generation.into_iter()) {
        let cards = if let Some(cards) = cached_cards {
            crate::logging::info(
                "document_generation_window_reused",
                format!(
                    "window={}/{} partition={} pages={}-{} cards={}",
                    window.window_index + 1,
                    generation_windows.len(),
                    window.partition_index + 1,
                    window.page_start,
                    window.page_end,
                    cards.len(),
                ),
            );
            cards
        } else {
            mark_window_running(database.inner(), &job_id, window.window_index)?;
            let images = if task.include_page_images {
                load_window_images(paths.inner(), &pages, window, plan.max_images_per_window)?
            } else {
                Vec::new()
            };
            let cards = generate_window_with_recovery(
                runtime.inner(),
                &task,
                &stored.dto.title,
                &outline,
                window,
                &images,
                plan.max_cards_per_window,
            )
            .await?;
            document_checkpoints::persist_generation_cards(
                database.inner(),
                document_id,
                &generation_contract,
                window,
                &cards,
            )?;
            let completed = document_checkpoints::count_generation_checkpoints(
                database.inner(),
                document_id,
                &generation_contract,
            )?;
            update_generation_progress(database.inner(), document_id, completed)?;
            cards
        };
        if generated_cards.len().saturating_add(cards.len()) > MAX_GENERATED_CARDS {
            return Err(CommandError::new(
                "too_many_generated_cards",
                format!("The model generated more than {MAX_GENERATED_CARDS} cards from this PDF."),
            ));
        }
        mark_window_complete(database.inner(), &job_id, window, cards.len())?;
        generated_cards.extend(cards);
    }
    update_generation_progress(
        database.inner(),
        document_id,
        i64::try_from(generation_windows.len()).unwrap_or(i64::MAX),
    )?;

    if generated_cards.is_empty() {
        return Err(CommandError::new(
            "document_generated_no_cards",
            "The document analysis found no material important enough to generate flashcards.",
        ));
    }
    let generated_cards = document_intelligence::curate_document_cards(
        runtime.inner(),
        &task,
        &outline,
        generated_cards,
    )
    .await?;
    if generated_cards.is_empty() {
        return Err(CommandError::new(
            "document_generated_no_cards",
            "The global relevance pass removed all generated cards as low-value or redundant.",
        ));
    }

    mark_job_committing(database.inner(), &job_id, generated_cards.len())?;
    let payload = build_generated_import_json(&stored.dto, generated_cards)?;
    let result = import_generated_flashcards_bytes_with_commit(
        database.inner(),
        payload.as_bytes(),
        |connection, import_result, now| {
            connection
                .execute(
                    "INSERT INTO document_decks (document_id, deck_id, job_id, created_at)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![document_id, import_result.deck_id, job_id, now],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "UPDATE documents
                     SET lifecycle_state = 'ready', generated_note_count = ?1,
                         generated_card_count = ?2, generated_deck_id = ?3,
                         outline_completed_window_count = outline_window_count,
                         generation_completed_window_count = generation_window_count,
                         safe_error_code = '', safe_error_message = '',
                         updated_at = ?4, completed_at = ?4, revision = revision + 1
                     WHERE id = ?5",
                    params![
                        import_result.created_note_count,
                        import_result.created_card_count,
                        import_result.deck_id,
                        now,
                        document_id,
                    ],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "UPDATE document_generation_jobs
                     SET state = 'succeeded', generated_candidate_count = ?1,
                         generated_deck_id = ?2, safe_error_code = '',
                         safe_error_message = '', updated_at = ?3, completed_at = ?3
                     WHERE id = ?4",
                    params![
                        import_result.created_note_count,
                        import_result.deck_id,
                        now,
                        job_id,
                    ],
                )
                .map_err(CommandError::database)?;
            Ok(())
        },
    )?;
    // Checkpoints are required until the canonical deck transaction succeeds.
    // Their cleanup is rebuildable/best-effort and cannot roll back the deck.
    let _ = document_checkpoints::clear_generation_checkpoints(database.inner(), document_id);
    let document = database
        .with_connection(|connection| load_document(connection, document_id))?
        .dto;
    Ok(DocumentGenerationResultDto {
        document,
        deck_id: result.deck_id,
        created_note_count: result.created_note_count,
        created_card_count: result.created_card_count,
        skipped_duplicate_count: result.skipped_duplicate_count,
    })
}

async fn generate_window_with_recovery(
    runtime: &AiRuntimeState,
    task: &document_intelligence::DocumentTaskContext,
    document_title: &str,
    outline: &document_intelligence::DocumentOutline,
    window: &SemanticWindow,
    images: &[document_intelligence::SourcePageImage],
    maximum_cards: usize,
) -> Result<Vec<serde_json::Value>, CommandError> {
    let first = document_intelligence::generate_window_flashcards(
        runtime,
        task,
        document_title,
        outline,
        window,
        images,
        maximum_cards.max(1),
    )
    .await;
    match first {
        Err(error)
            if maximum_cards > 2
                && (error.code == "ai_document_output_truncated"
                    || (error.code == "ai_document_output_invalid"
                        && error.message.contains("more than"))) =>
        {
            // One reduced retry only. Never recursively keep halving and
            // spending provider tokens after a completion/context failure.
            let reduced_budget = (maximum_cards / 2).max(2);
            crate::logging::info(
                "document_generation_window_compact_retry",
                format!(
                    "window={} partition={} pages={}-{} old_budget={} new_budget={} code={}",
                    window.window_index + 1,
                    window.partition_index + 1,
                    window.page_start,
                    window.page_end,
                    maximum_cards,
                    reduced_budget,
                    error.code,
                ),
            );
            document_intelligence::generate_window_flashcards(
                runtime,
                task,
                document_title,
                outline,
                window,
                images,
                reduced_budget,
            )
            .await
        }
        other => other,
    }
}

fn extract_and_render_pdf<F>(
    paths: &AppPaths,
    render_runtime: &PdfRenderRuntime,
    document_id: &str,
    source_path: &Path,
    mut report_progress: F,
) -> Result<ProcessedDocument, CommandError>
where
    F: FnMut(usize, usize) -> Result<(), CommandError>,
{
    let semantic = pdf_backend::extract_document(source_path).map_err(map_pdf_backend_error)?;
    if semantic.page_count == 0 || semantic.page_count > MAX_SUPPORTED_PDF_PAGES {
        return Err(CommandError::new(
            "unsupported_pdf_page_count",
            format!("PDFs must contain between 1 and {MAX_SUPPORTED_PDF_PAGES} pages."),
        ));
    }

    let mut total_text_bytes = 0_usize;
    let normalized_text = semantic
        .pages
        .iter()
        .map(|page| {
            let text = normalize_page_text(&page.text);
            total_text_bytes = total_text_bytes.saturating_add(text.len());
            (page.page_number, text)
        })
        .collect::<Vec<_>>();
    if total_text_bytes > MAX_TOTAL_TEXT_BYTES {
        return Err(CommandError::new(
            "pdf_text_too_large",
            "The extracted PDF text exceeds ostudy's 96 MB processing limit.",
        ));
    }
    let semantic_text_bytes = semantic
        .chunks
        .iter()
        .map(|chunk| chunk.full_text.len())
        .fold(0_usize, usize::saturating_add);
    if semantic_text_bytes > MAX_SEMANTIC_TEXT_BYTES {
        return Err(CommandError::new(
            "pdf_semantic_content_too_large",
            "The structured PDF content exceeds ostudy's semantic-processing limit.",
        ));
    }

    report_progress(0, semantic.page_count)?;

    let page_directory = paths.document_pages_dir(document_id);
    fs::create_dir_all(&page_directory).map_err(CommandError::io)?;
    let mut rendered = Vec::<(usize, String, i64, String)>::new();
    let render_result = pdf_render_backend::render_pages(
        render_runtime,
        source_path,
        |page_number, png| -> Result<(), CommandError> {
            if png.is_empty() || png.len() > MAX_RENDER_BYTES {
                return Err(CommandError::new(
                    "pdf_render_too_large",
                    format!(
                        "The rendered image for PDF page {page_number} exceeds ostudy's page-image limit."
                    ),
                ));
            }
            let relative = AppPaths::document_page_relative_path(
                document_id,
                i64::try_from(page_number).unwrap_or(i64::MAX),
            );
            write_replacing_file(&paths.resolve_relative(&relative), &png)?;
            rendered.push((
                page_number,
                relative,
                i64::try_from(png.len()).unwrap_or(i64::MAX),
                sha256_hex(&png),
            ));
            report_progress(rendered.len(), semantic.page_count)?;
            Ok(())
        },
    );
    let rendered_count = match render_result {
        Ok(count) => count,
        Err(RenderProcessError::Visitor(error)) => return Err(error),
        Err(RenderProcessError::Backend(error)) => return Err(map_pdf_render_error(error)),
    };
    if rendered_count != semantic.page_count {
        return Err(CommandError::new(
            "pdf_backend_page_count_mismatch",
            "The extraction and rendering backends reported different PDF page counts.",
        ));
    }
    sync_directory(&page_directory)?;

    let mut pages = Vec::with_capacity(semantic.page_count);
    for page_number in 1..=semantic.page_count {
        let plain_text = normalized_text
            .iter()
            .find(|(number, _)| *number == page_number)
            .map(|(_, text)| text.clone())
            .unwrap_or_default();
        let (_, render_relative_path, render_byte_size, render_sha256) = rendered
            .iter()
            .find(|(number, _, _, _)| *number == page_number)
            .cloned()
            .ok_or_else(|| {
                CommandError::new(
                    "pdf_render_failed",
                    format!("The renderer did not return PDF page {page_number}."),
                )
            })?;
        pages.push(PageRecord {
            page_number: i64::try_from(page_number).unwrap_or(i64::MAX),
            text_sha256: sha256_hex(plain_text.as_bytes()),
            plain_text,
            render_relative_path,
            render_byte_size,
            render_sha256,
        });
    }
    Ok(ProcessedDocument { pages, semantic })
}

fn update_render_progress(
    database: &Database,
    document_id: &str,
    rendered_pages: usize,
    total_pages: usize,
) -> Result<(), CommandError> {
    let total = i64::try_from(total_pages).unwrap_or(i64::MAX);
    let rendered = i64::try_from(rendered_pages).unwrap_or(i64::MAX).min(total);
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        let changed = connection
            .execute(
                "UPDATE documents
                 SET page_count = ?1,
                     extracted_page_count = ?1,
                     rendered_page_count = ?2,
                     updated_at = ?3
                 WHERE id = ?4 AND lifecycle_state = 'processing'",
                params![total, rendered, now, document_id],
            )
            .map_err(CommandError::database)?;
        if changed != 1 {
            return Err(CommandError::new(
                "document_changed",
                "The document changed while PDF rendering progress was being recorded.",
            ));
        }
        Ok(())
    })
}

fn persist_processed_document(
    database: &Database,
    document_id: &str,
    processed: &ProcessedDocument,
) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        for table in [
            "document_pages",
            "document_semantic_elements",
            "document_semantic_chunks",
            "document_outlines",
            "document_outline_checkpoints",
            "document_generation_checkpoints",
        ] {
            transaction
                .execute(
                    &format!("DELETE FROM {table} WHERE document_id = ?1"),
                    [document_id],
                )
                .map_err(CommandError::database)?;
        }

        for page in &processed.pages {
            transaction
                .execute(
                    "INSERT INTO document_pages (
                         document_id, page_number, plain_text, text_sha256,
                         render_relative_path, render_mime_type, render_byte_size,
                         render_sha256, created_at, updated_at
                     ) VALUES (?1, ?2, ?3, ?4, ?5, 'image/png', ?6, ?7, ?8, ?8)",
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

        for element in &processed.semantic.elements {
            let text = normalize_semantic_text(&element.text, 250_000);
            transaction
                .execute(
                    "INSERT INTO document_semantic_elements (
                         document_id, element_index, page_number, element_type,
                         text, parent_heading, heading_path_json,
                         bbox_x, bbox_y, bbox_width, bbox_height, confidence,
                         font_name, font_size, is_bold, is_italic,
                         text_sha256, created_at
                     ) VALUES (
                         ?1, ?2, ?3, ?4, ?5, ?6, ?7,
                         ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18
                     )",
                    params![
                        document_id,
                        i64::try_from(element.element_index).unwrap_or(i64::MAX),
                        i64::try_from(element.page_number).unwrap_or(i64::MAX),
                        normalize_element_type(&element.element_type)?,
                        text,
                        element.parent_heading,
                        serde_json::to_string(&element.heading_path)
                            .map_err(CommandError::serialization)?,
                        element.bbox.x,
                        element.bbox.y,
                        element.bbox.width.max(0.0),
                        element.bbox.height.max(0.0),
                        element.confidence.clamp(0.0, 1.0),
                        element.font_name,
                        element.font_size,
                        i64::from(element.is_bold),
                        i64::from(element.is_italic),
                        sha256_hex(text.as_bytes()),
                        now,
                    ],
                )
                .map_err(CommandError::database)?;
        }

        for chunk in &processed.semantic.chunks {
            let text = normalize_semantic_text(&chunk.text, 1_000_000);
            let full_text = normalize_semantic_text(&chunk.full_text, 1_100_000);
            let page_numbers = chunk
                .page_numbers
                .iter()
                .map(|page| i64::try_from(*page).unwrap_or(i64::MAX))
                .collect::<Vec<_>>();
            transaction
                .execute(
                    "INSERT INTO document_semantic_chunks (
                         document_id, chunk_index, text, full_text,
                         page_numbers_json, bounding_boxes_json, element_types_json,
                         heading_context, token_estimate, is_oversized,
                         text_sha256, created_at
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                    params![
                        document_id,
                        i64::try_from(chunk.chunk_index).unwrap_or(i64::MAX),
                        text,
                        full_text,
                        serde_json::to_string(&page_numbers)
                            .map_err(CommandError::serialization)?,
                        serde_json::to_string(&chunk.bounding_boxes)
                            .map_err(CommandError::serialization)?,
                        serde_json::to_string(&chunk.element_types)
                            .map_err(CommandError::serialization)?,
                        chunk.heading_context,
                        i64::try_from(chunk.token_estimate).unwrap_or(i64::MAX),
                        i64::from(chunk.is_oversized),
                        sha256_hex(text.as_bytes()),
                        now,
                    ],
                )
                .map_err(CommandError::database)?;
        }

        let page_count = i64::try_from(processed.pages.len()).unwrap_or(i64::MAX);
        transaction
            .execute(
                "UPDATE documents
                 SET page_count = ?1, extracted_page_count = ?1, rendered_page_count = ?1,
                     extraction_backend = ?2, render_backend = ?3,
                     semantic_element_count = ?4, semantic_chunk_count = ?5,
                     scale_mode = 'standard', estimated_source_tokens = 0,
                     outline_window_count = 0, outline_completed_window_count = 0,
                     generation_window_count = 0, generation_completed_window_count = 0,
                     outline_state = 'pending', safe_error_code = '', safe_error_message = '',
                     updated_at = ?6, revision = revision + 1
                 WHERE id = ?7",
                params![
                    page_count,
                    pdf_backend::active_backend_id(),
                    pdf_render_backend::active_backend_id(),
                    i64::try_from(processed.semantic.elements.len()).unwrap_or(i64::MAX),
                    i64::try_from(processed.semantic.chunks.len()).unwrap_or(i64::MAX),
                    now,
                    document_id,
                ],
            )
            .map_err(CommandError::database)?;
        transaction.commit().map_err(CommandError::database)
    })
}

fn load_pages(
    database: &Database,
    paths: &AppPaths,
    document_id: &str,
) -> Result<Vec<PageRecord>, CommandError> {
    database.with_connection(|connection| {
        let mut statement = connection
            .prepare(
                "SELECT page_number, plain_text, text_sha256, render_relative_path,
                        render_byte_size, render_sha256
                 FROM document_pages WHERE document_id = ?1 ORDER BY page_number",
            )
            .map_err(CommandError::database)?;
        let pages = statement
            .query_map([document_id], |row| {
                Ok(PageRecord {
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
            if !paths.resolve_relative(&page.render_relative_path).is_file() {
                return Err(CommandError::new(
                    "document_render_missing",
                    "A rendered PDF page is missing. Retry generation to rebuild the document.",
                ));
            }
        }
        Ok(pages)
    })
}

fn load_semantic_chunks(
    database: &Database,
    document_id: &str,
) -> Result<Vec<SemanticChunk>, CommandError> {
    database.with_connection(|connection| {
        let mut statement = connection
            .prepare(
                "SELECT chunk_index, text, full_text, page_numbers_json,
                        element_types_json, heading_context, token_estimate
                 FROM document_semantic_chunks
                 WHERE document_id = ?1 ORDER BY chunk_index",
            )
            .map_err(CommandError::database)?;
        let rows = statement
            .query_map([document_id], |row| {
                let pages_json: String = row.get(3)?;
                let types_json: String = row.get(4)?;
                let token_estimate: i64 = row.get(6)?;
                let page_numbers = serde_json::from_str(&pages_json).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        pages_json.len(),
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?;
                let element_types = serde_json::from_str(&types_json).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        types_json.len(),
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?;
                Ok(SemanticChunk {
                    chunk_index: row.get(0)?,
                    text: row.get(1)?,
                    full_text: row.get(2)?,
                    page_numbers,
                    element_types,
                    heading_context: row.get(5)?,
                    token_estimate: usize::try_from(token_estimate).unwrap_or(usize::MAX),
                })
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        Ok(rows)
    })
}

fn persist_outline(
    database: &Database,
    document_id: &str,
    task: &document_intelligence::DocumentTaskContext,
    outline: &document_intelligence::DocumentOutline,
) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    let outline_json = serde_json::to_string(outline).map_err(CommandError::serialization)?;
    database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        transaction
            .execute(
                "INSERT INTO document_outlines (
                     document_id, schema_version, prompt_version, profile_id,
                     provider_kind, model_id, profile_revision, credential_epoch,
                     extraction_backend, outline_json, summary_text, created_at, updated_at
                 ) VALUES (?1, 1, 4, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)
                 ON CONFLICT(document_id) DO UPDATE SET
                     schema_version = excluded.schema_version,
                     prompt_version = excluded.prompt_version,
                     profile_id = excluded.profile_id,
                     provider_kind = excluded.provider_kind,
                     model_id = excluded.model_id,
                     profile_revision = excluded.profile_revision,
                     credential_epoch = excluded.credential_epoch,
                     extraction_backend = excluded.extraction_backend,
                     outline_json = excluded.outline_json,
                     summary_text = excluded.summary_text,
                     updated_at = excluded.updated_at",
                params![
                    document_id,
                    task.profile_id,
                    task.provider_kind,
                    task.model_id,
                    task.profile_revision,
                    task.credential_epoch,
                    pdf_backend::active_backend_id(),
                    outline_json,
                    outline.summary,
                    now,
                ],
            )
            .map_err(CommandError::database)?;
        transaction
            .execute(
                "UPDATE documents SET outline_state = 'ready', updated_at = ?1 WHERE id = ?2",
                params![now, document_id],
            )
            .map_err(CommandError::database)?;
        transaction.commit().map_err(CommandError::database)
    })
}

fn initialize_document_ai_plan(
    database: &Database,
    document_id: &str,
    plan: DocumentScalePlan,
    estimated_source_tokens: usize,
    outline_window_count: usize,
    outline_completed_window_count: usize,
    generation_window_count: usize,
    generation_completed_window_count: usize,
) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        connection
            .execute(
                "UPDATE documents
                 SET scale_mode = ?1, estimated_source_tokens = ?2,
                     outline_window_count = ?3, outline_completed_window_count = ?4,
                     generation_window_count = ?5, generation_completed_window_count = ?6,
                     updated_at = ?7
                 WHERE id = ?8 AND lifecycle_state = 'processing'",
                params![
                    plan.label(),
                    i64::try_from(estimated_source_tokens).unwrap_or(i64::MAX),
                    i64::try_from(outline_window_count).unwrap_or(i64::MAX),
                    i64::try_from(outline_completed_window_count).unwrap_or(i64::MAX),
                    i64::try_from(generation_window_count).unwrap_or(i64::MAX),
                    i64::try_from(generation_completed_window_count).unwrap_or(i64::MAX),
                    now,
                    document_id,
                ],
            )
            .map_err(CommandError::database)?;
        Ok(())
    })
}

fn update_outline_progress(
    database: &Database,
    document_id: &str,
    completed: i64,
) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        connection
            .execute(
                "UPDATE documents
                 SET outline_completed_window_count = MIN(outline_window_count, MAX(0, ?1)),
                     updated_at = ?2
                 WHERE id = ?3 AND lifecycle_state = 'processing'",
                params![completed, now, document_id],
            )
            .map_err(CommandError::database)?;
        Ok(())
    })
}

fn update_generation_progress(
    database: &Database,
    document_id: &str,
    completed: i64,
) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        connection
            .execute(
                "UPDATE documents
                 SET generation_completed_window_count = MIN(generation_window_count, MAX(0, ?1)),
                     updated_at = ?2
                 WHERE id = ?3 AND lifecycle_state = 'processing'",
                params![completed, now, document_id],
            )
            .map_err(CommandError::database)?;
        Ok(())
    })
}

fn create_generation_job(
    database: &Database,
    document_id: &str,
    task: &document_intelligence::DocumentTaskContext,
    page_count: usize,
    windows: &[SemanticWindow],
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
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'generating', ?9, 0, 0, ?10, ?10, ?10)",
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
        for window in windows {
            transaction
                .execute(
                    "INSERT INTO document_generation_windows (
                         job_id, window_index, first_chunk_index, last_chunk_index,
                         page_start, page_end, page_numbers_json, token_estimate,
                         state, card_count, updated_at
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'pending', 0, ?9)",
                    params![
                        job_id,
                        i64::try_from(window.window_index).unwrap_or(i64::MAX),
                        window.first_chunk_index,
                        window.last_chunk_index,
                        window.page_start,
                        window.page_end,
                        serde_json::to_string(&window.page_numbers)
                            .map_err(CommandError::serialization)?,
                        i64::try_from(window.token_estimate).unwrap_or(i64::MAX),
                        now,
                    ],
                )
                .map_err(CommandError::database)?;
        }
        transaction.commit().map_err(CommandError::database)?;
        Ok(job_id)
    })
}

fn mark_document_processing(database: &Database, document_id: &str) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        connection
            .execute(
                "UPDATE documents
                 SET lifecycle_state = 'processing', safe_error_code = '', safe_error_message = '',
                     updated_at = ?1, completed_at = NULL, revision = revision + 1
                 WHERE id = ?2 AND archived_at IS NULL",
                params![now, document_id],
            )
            .map_err(CommandError::database)?;
        Ok(())
    })
}

fn mark_outline_building(database: &Database, document_id: &str) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        connection
            .execute(
                "UPDATE documents SET outline_state = 'building', updated_at = ?1 WHERE id = ?2",
                params![now, document_id],
            )
            .map_err(CommandError::database)?;
        Ok(())
    })
}

fn mark_window_running(
    database: &Database,
    job_id: &str,
    window_index: usize,
) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        connection
            .execute(
                "UPDATE document_generation_windows
                 SET state = 'running', updated_at = ?1
                 WHERE job_id = ?2 AND window_index = ?3",
                params![now, job_id, i64::try_from(window_index).unwrap_or(i64::MAX)],
            )
            .map_err(CommandError::database)?;
        Ok(())
    })
}

fn mark_window_complete(
    database: &Database,
    job_id: &str,
    window: &SemanticWindow,
    card_count: usize,
) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        transaction
            .execute(
                "UPDATE document_generation_windows
                 SET state = ?1, card_count = ?2, safe_error_code = '',
                     safe_error_message = '', updated_at = ?3
                 WHERE job_id = ?4 AND window_index = ?5",
                params![
                    if card_count == 0 {
                        "skipped"
                    } else {
                        "succeeded"
                    },
                    i64::try_from(card_count).unwrap_or(i64::MAX),
                    now,
                    job_id,
                    i64::try_from(window.window_index).unwrap_or(i64::MAX),
                ],
            )
            .map_err(CommandError::database)?;
        transaction
            .execute(
                "UPDATE document_generation_jobs
                 SET processed_page_count = MAX(processed_page_count, ?1),
                     generated_candidate_count = (
                         SELECT COALESCE(SUM(card_count), 0)
                         FROM document_generation_windows WHERE job_id = ?2
                     ),
                     updated_at = ?3
                 WHERE id = ?2",
                params![window.page_end, job_id, now],
            )
            .map_err(CommandError::database)?;
        transaction.commit().map_err(CommandError::database)
    })
}

fn mark_job_committing(
    database: &Database,
    job_id: &str,
    card_count: usize,
) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        connection
            .execute(
                "UPDATE document_generation_jobs
                 SET state = 'committing', generated_candidate_count = ?1, updated_at = ?2
                 WHERE id = ?3",
                params![i64::try_from(card_count).unwrap_or(i64::MAX), now, job_id],
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
                 SET lifecycle_state = CASE WHEN archived_at IS NULL THEN 'failed' ELSE 'archived' END,
                     outline_state = CASE WHEN outline_state = 'building' THEN 'failed' ELSE outline_state END,
                     safe_error_code = ?1, safe_error_message = ?2,
                     updated_at = ?3, revision = revision + 1
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
                 SET state = 'failed', safe_error_code = ?1, safe_error_message = ?2,
                     updated_at = ?3, completed_at = ?3
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

fn load_window_images(
    paths: &AppPaths,
    pages: &[PageRecord],
    window: &SemanticWindow,
    maximum: usize,
) -> Result<Vec<document_intelligence::SourcePageImage>, CommandError> {
    if window.visual_page_numbers.is_empty() || maximum == 0 {
        return Ok(Vec::new());
    }
    let selected = select_image_pages(&window.visual_page_numbers, maximum);
    let mut images = Vec::with_capacity(selected.len());
    for page_number in selected {
        let page = pages
            .iter()
            .find(|page| page.page_number == page_number)
            .ok_or_else(|| {
                CommandError::new(
                    "document_render_missing",
                    "A selected PDF page render is missing.",
                )
            })?;
        let bytes = fs::read(paths.resolve_relative(&page.render_relative_path))
            .map_err(CommandError::io)?;
        if bytes.is_empty()
            || bytes.len() > MAX_RENDER_BYTES
            || i64::try_from(bytes.len()).unwrap_or(i64::MAX) != page.render_byte_size
            || sha256_hex(&bytes) != page.render_sha256
        {
            return Err(CommandError::new(
                "document_render_invalid",
                "A rendered PDF page failed its size or integrity check. Retry generation to rebuild it.",
            ));
        }
        images.push(document_intelligence::SourcePageImage { page_number, bytes });
    }
    Ok(images)
}

fn select_image_pages(page_numbers: &[i64], maximum: usize) -> Vec<i64> {
    if maximum == 0 {
        return Vec::new();
    }
    let mut unique = page_numbers.to_vec();
    unique.sort_unstable();
    unique.dedup();
    if unique.len() <= maximum {
        return unique;
    }
    if maximum == 1 {
        return vec![unique[unique.len() / 2]];
    }
    let last = unique.len() - 1;
    (0..maximum)
        .map(|index| unique[index * last / (maximum - 1)])
        .collect()
}

fn build_generated_import_json(
    document: &DocumentDto,
    cards: Vec<serde_json::Value>,
) -> Result<String, CommandError> {
    serde_json::to_string(&serde_json::json!({
        "format": "ostudy-flashcards",
        "formatVersion": 1,
        "deck": {
            "mode": "create",
            "name": document.title,
            "description": format!(
                "AI-generated flashcards from {}. Edit or delete these cards through the normal deck tools.",
                document.original_filename
            )
        },
        "defaultTags": ["AI generated", "PDF"],
        "cards": cards,
    }))
    .map_err(CommandError::serialization)
}

fn list_document_rows(
    connection: &Connection,
    include_archived: bool,
) -> Result<Vec<DocumentDto>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT id, title, original_filename, byte_size, sha256, lifecycle_state,
                    page_count, extracted_page_count, rendered_page_count,
                    scale_mode, estimated_source_tokens,
                    outline_window_count, outline_completed_window_count,
                    generation_window_count, generation_completed_window_count,
                    generated_note_count, generated_card_count,
                    CASE WHEN EXISTS (
                        SELECT 1 FROM decks deck
                        WHERE deck.id = documents.generated_deck_id AND deck.deleted_at IS NULL
                    ) THEN documents.generated_deck_id ELSE NULL END,
                    safe_error_code, safe_error_message, created_at, updated_at,
                    completed_at, archived_at, revision
             FROM documents
             WHERE (?1 = 1 OR archived_at IS NULL)
             ORDER BY updated_at DESC, id
             LIMIT ?2",
        )
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map(
            params![
                if include_archived { 1_i64 } else { 0_i64 },
                MAX_DOCUMENTS_RETURNED,
            ],
            document_row,
        )
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    Ok(rows)
}

fn load_document(
    connection: &Connection,
    document_id: &str,
) -> Result<StoredDocument, CommandError> {
    connection
        .query_row(
            "SELECT id, title, original_filename, byte_size, sha256, lifecycle_state,
                    page_count, extracted_page_count, rendered_page_count,
                    scale_mode, estimated_source_tokens,
                    outline_window_count, outline_completed_window_count,
                    generation_window_count, generation_completed_window_count,
                    generated_note_count, generated_card_count,
                    CASE WHEN EXISTS (
                        SELECT 1 FROM decks deck
                        WHERE deck.id = documents.generated_deck_id AND deck.deleted_at IS NULL
                    ) THEN documents.generated_deck_id ELSE NULL END,
                    safe_error_code, safe_error_message, created_at, updated_at,
                    completed_at, archived_at, revision, source_relative_path,
                    extraction_backend, render_backend, semantic_chunk_count
             FROM documents WHERE id = ?1",
            [document_id],
            |row| {
                Ok(StoredDocument {
                    dto: document_row(row)?,
                    source_relative_path: row.get(25)?,
                    extraction_backend: row.get(26)?,
                    render_backend: row.get(27)?,
                    semantic_chunk_count: row.get(28)?,
                })
            },
        )
        .optional()
        .map_err(CommandError::database)?
        .ok_or_else(|| {
            CommandError::new(
                "document_not_found",
                "That PDF is no longer in the library.",
            )
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
        scale_mode: row.get(9)?,
        estimated_source_tokens: row.get(10)?,
        outline_window_count: row.get(11)?,
        outline_completed_window_count: row.get(12)?,
        generation_window_count: row.get(13)?,
        generation_completed_window_count: row.get(14)?,
        generated_note_count: row.get(15)?,
        generated_card_count: row.get(16)?,
        generated_deck_id: row.get(17)?,
        safe_error_code: row.get(18)?,
        safe_error_message: row.get(19)?,
        created_at: row.get(20)?,
        updated_at: row.get(21)?,
        completed_at: row.get(22)?,
        archived_at: row.get(23)?,
        revision: row.get(24)?,
    })
}

fn active_generated_deck_exists(
    connection: &Connection,
    document_id: &str,
) -> Result<bool, CommandError> {
    connection
        .query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM documents document
                 JOIN decks deck ON deck.id = document.generated_deck_id
                 WHERE document.id = ?1 AND deck.deleted_at IS NULL
             )",
            [document_id],
            |row| row.get::<_, i64>(0),
        )
        .map(|value| value != 0)
        .map_err(CommandError::database)
}

fn map_pdf_backend_error(error: PdfBackendError) -> CommandError {
    let backend = pdf_backend::active_backend_id();
    let page = error.page_number.unwrap_or_default();
    match error.kind {
        PdfBackendErrorKind::Open | PdfBackendErrorKind::PageCount => CommandError::new(
            "pdf_open_failed",
            format!(
                "The PDF could not be opened by the structured extraction backend ({backend}): {error}"
            ),
        ),
        PdfBackendErrorKind::ExtractText => CommandError::new(
            "pdf_text_extraction_failed",
            format!("Text could not be extracted from PDF page {page} by {backend}: {error}"),
        ),
        PdfBackendErrorKind::SemanticAnalysis => CommandError::new(
            "pdf_semantic_analysis_failed",
            format!("The PDF's document structure could not be analyzed by {backend}: {error}"),
        ),
    }
}

fn map_pdf_render_error(error: PdfRenderError) -> CommandError {
    let backend = pdf_render_backend::active_backend_id();
    let page = error.page_number.unwrap_or_default();
    match error.kind {
        PdfRenderErrorKind::Bind => CommandError::new(
            "pdf_renderer_unavailable",
            format!("The PDF page renderer ({backend}) could not be loaded: {error}"),
        ),
        PdfRenderErrorKind::Open => CommandError::new(
            "pdf_render_open_failed",
            format!("The PDF could not be opened by the page renderer ({backend}): {error}"),
        ),
        PdfRenderErrorKind::RenderPage | PdfRenderErrorKind::EncodePage => CommandError::new(
            "pdf_render_failed",
            format!("PDF page {page} could not be rendered by {backend}: {error}"),
        ),
    }
}

fn normalize_element_type(value: &str) -> Result<&'static str, CommandError> {
    match value.to_ascii_lowercase().replace('-', "_").as_str() {
        "title" => Ok("title"),
        "paragraph" => Ok("paragraph"),
        "table" => Ok("table"),
        "header" => Ok("header"),
        "footer" => Ok("footer"),
        "listitem" | "list_item" => Ok("list_item"),
        "image" => Ok("image"),
        "codeblock" | "code_block" => Ok("code_block"),
        "keyvalue" | "key_value" => Ok("key_value"),
        _ => Err(CommandError::new(
            "pdf_semantic_element_unsupported",
            "The extraction backend returned an unsupported semantic element type.",
        )),
    }
}

fn normalize_page_text(value: &str) -> String {
    normalize_semantic_text(
        &value.replace("\r\n", "\n").replace('\r', "\n"),
        MAX_PAGE_TEXT_CHARS,
    )
}

fn normalize_semantic_text(value: &str, maximum: usize) -> String {
    value
        .chars()
        .filter(|character| !character.is_control() || matches!(character, '\n' | '\t'))
        .take(maximum)
        .collect::<String>()
        .trim()
        .to_string()
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
            normalize_page_text(" line\r\none\0two\rthree "),
            "line\nonetwo\nthree"
        );
    }

    #[test]
    fn samples_images_across_a_large_semantic_window() {
        let pages = (1..=40).collect::<Vec<_>>();
        let selected = select_image_pages(&pages, 6);
        assert_eq!(selected.first(), Some(&1));
        assert_eq!(selected.last(), Some(&40));
        assert_eq!(selected.len(), 6);
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
            scale_mode: "standard".to_string(),
            estimated_source_tokens: 100,
            outline_window_count: 1,
            outline_completed_window_count: 1,
            generation_window_count: 1,
            generation_completed_window_count: 1,
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
