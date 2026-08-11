use std::{
    fs,
    sync::atomic::{AtomicUsize, Ordering},
};

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    ai::{document_intelligence, document_provider, AiRuntimeState},
    authoring::card_types_and_templates::generated_import::import_generated_flashcards_bytes_with_commit,
    database::Database,
    document_checkpoints::{self, CheckpointIdentity},
    document_cost,
    document_domain::{DocumentFormat, DocumentSourceDescriptor, SourceUnitKind},
    document_generation::{self, GenerationExecution},
    document_ingestion,
    document_scale::{self, DocumentScalePlan, MAX_SUPPORTED_SOURCE_UNITS},
    document_semantics::{build_partitioned_semantic_windows, validate_semantic_windows},
    document_store::{self, DocumentAiProgressPlan, GenerationJobIdentity},
    error::CommandError,
    paths::AppPaths,
    pdf_render_backend::PdfRenderRuntime,
    util::{unix_time_millis, validate_entity_id},
};

const MAX_GENERATED_CARDS: usize = 10_000;
const MAX_DOCUMENTS_RETURNED: i64 = 500;
const INTERRUPTED_ERROR_CODE: &str = "document_generation_interrupted";
const INTERRUPTED_ERROR_MESSAGE: &str =
    "Document processing stopped when ostudy closed. Retry generation to continue.";

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
    pub extraction_mode: String,
    pub ocr_page_count: i64,
    pub ocr_completed_page_count: i64,
    pub ocr_mean_confidence: Option<f64>,
    pub ocr_engine: String,
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
    pub processing_duration_ms: Option<i64>,
    pub api_token_count: Option<i64>,
    pub api_token_usage_complete: bool,
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
    source_format: String,
    source_unit_kind: String,
}

impl StoredDocument {
    fn source_descriptor(&self) -> Result<DocumentSourceDescriptor, CommandError> {
        let unit_count = usize::try_from(self.dto.page_count).map_err(|_| {
            CommandError::new(
                "invalid_document_source",
                "The saved document source-unit count is invalid.",
            )
        })?;
        Ok(DocumentSourceDescriptor {
            format: DocumentFormat::from_db(&self.source_format)?,
            unit_kind: SourceUnitKind::from_db(&self.source_unit_kind)?,
            unit_count,
        })
    }
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
            "The managed source document is missing. Restore the library from backup or import it again.",
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
            "This document already has a generated deck. Delete that deck before generating it again.",
        ));
    }

    let result = process_and_generate(
        database.clone(),
        paths.clone(),
        runtime.clone(),
        render_runtime.inner().clone(),
        &input.document_id,
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
        Ok(load_document(connection, &input.document_id)?.dto)
    })
}

pub(crate) fn reconcile_startup(database: &Database, paths: &AppPaths) -> Result<(), CommandError> {
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
                 SET lifecycle_state = 'failed',
                     outline_state = CASE WHEN outline_state = 'building' THEN 'failed' ELSE outline_state END,
                     safe_error_code = ?1, safe_error_message = ?2,
                     processing_duration_ms = CASE
                         WHEN processing_started_at IS NULL THEN processing_duration_ms
                         ELSE MAX(0, ?3 - processing_started_at)
                     END,
                     updated_at = ?3, revision = revision + 1
                 WHERE lifecycle_state = 'processing'",
                params![INTERRUPTED_ERROR_CODE, INTERRUPTED_ERROR_MESSAGE, now],
            )
            .map_err(|error| {
                CommandError::database_at("document_reconcile_documents_failed", "scope=startup", error)
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
            .map_err(|error| {
                CommandError::database_at(
                    "document_reconcile_source_query_failed",
                    "scope=startup",
                    error,
                )
            })?;
        let managed_sources = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        Ok(managed_sources)
    })?;
    for (document_id, relative_path) in managed_sources {
        if !paths.resolve_relative(&relative_path).is_file() {
            record_document_failure(
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

async fn process_and_generate(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    runtime: State<'_, AiRuntimeState>,
    render_runtime: PdfRenderRuntime,
    document_id: &str,
) -> Result<DocumentGenerationResultDto, CommandError> {
    document_store::mark_document_processing(database.inner(), document_id)?;
    let mut stored =
        database.with_connection(|connection| load_document(connection, document_id))?;
    let source_path = paths.resolve_relative(&stored.source_relative_path);
    if !source_path.is_file() {
        return Err(CommandError::new(
            "document_source_missing",
            "The managed source document is missing. Restore the library from backup or import it again.",
        ));
    }

    let format = DocumentFormat::from_db(&stored.source_format)?;
    if format != DocumentFormat::Pdf {
        return Err(CommandError::new(
            "document_ingestion_adapter_unavailable",
            "This ostudy build does not yet include an ingestion adapter for the saved document format.",
        ));
    }
    let backend_changed = stored.extraction_backend
        != document_ingestion::active_pdf_extraction_backend()
        || stored.render_backend != document_ingestion::active_pdf_render_backend();
    let incomplete_derivatives = stored.dto.page_count == 0
        || stored.dto.extracted_page_count != stored.dto.page_count
        || stored.dto.rendered_page_count != stored.dto.page_count
        || stored.semantic_chunk_count == 0;
    if backend_changed || incomplete_derivatives {
        let paths_owned = paths.inner().clone();
        let source_owned = source_path.clone();
        let id_owned = document_id.to_string();
        let (progress_sender, mut progress_receiver) =
            tokio::sync::mpsc::unbounded_channel::<document_ingestion::IngestionProgress>();
        let processing_task = tauri::async_runtime::spawn_blocking(move || {
            document_ingestion::ingest_pdf(
                &paths_owned,
                &render_runtime,
                &id_owned,
                &source_owned,
                MAX_SUPPORTED_SOURCE_UNITS,
                |progress| {
                    progress_sender.send(progress).map_err(|_| {
                        CommandError::new(
                            "document_processing_stopped",
                            "The document-processing progress channel closed unexpectedly.",
                        )
                    })
                },
            )
        });
        while let Some(progress) = progress_receiver.recv().await {
            document_ingestion::update_ingestion_progress(database.inner(), document_id, progress)?;
        }
        let processed = processing_task.await.map_err(|error| {
            CommandError::new(
                "document_processing_failed",
                format!("The document-processing task stopped unexpectedly: {error}"),
            )
        })??;
        document_ingestion::persist_ingested_document(database.inner(), document_id, &processed)?;
        stored = database.with_connection(|connection| load_document(connection, document_id))?;
    }

    let source = stored.source_descriptor()?;
    source.validate()?;
    let renders = document_store::load_source_renders(database.inner(), document_id)?;
    if renders.len() != source.unit_count
        || renders
            .iter()
            .any(|render| !paths.resolve_relative(&render.relative_path).is_file())
    {
        return Err(CommandError::new(
            "document_render_missing",
            "One or more rendered source units are missing. Retry generation to rebuild the document derivatives.",
        ));
    }
    let chunks = document_store::load_semantic_chunks(database.inner(), document_id)?;
    if chunks.is_empty() {
        return Err(CommandError::new(
            "document_has_no_semantic_content",
            "The document did not contain structured source content that ostudy could analyze.",
        ));
    }

    let task = document_intelligence::load_document_task_context(database.inner())?;
    let capabilities = document_provider::load_document_provider_capabilities(
        database.inner(),
        &task.profile_id,
        &task.provider_kind,
        &task.model_id,
    )?;
    let calibration = document_cost::load_cost_calibration(
        database.inner(),
        &task.provider_kind,
        &task.model_id,
    )?;
    let plan = DocumentScalePlan::for_capabilities(source.unit_count, &capabilities, calibration)?;
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
        plan.outline_window_max_units,
        plan.outline_window_overlap_tokens,
    );
    let generation_windows = build_partitioned_semantic_windows(
        &chunks,
        plan,
        plan.generation_window_max_tokens,
        plan.generation_window_max_units,
        plan.generation_window_overlap_tokens,
    );
    validate_semantic_windows(&outline_windows, &source)?;
    validate_semantic_windows(&generation_windows, &source)?;

    let budget = document_scale::validate_automatic_job_calibrated(
        source.unit_count,
        estimated_source_tokens,
        plan,
        outline_windows.len(),
        generation_windows.len(),
        &capabilities,
        calibration,
    )?;
    crate::logging::info(
        "document_ai_scale_plan",
        format!(
            "document={} format={} provider={} model={} scale={} units={} source_tokens={} outline_windows={} generation_windows={} card_capacity={} estimated_job_tokens={} ceiling={} context={} calibration_samples={}",
            document_id,
            source.format.as_str(),
            task.provider_kind,
            task.model_id,
            plan.label(),
            source.unit_count,
            estimated_source_tokens,
            outline_windows.len(),
            generation_windows.len(),
            budget.generation_card_capacity,
            budget.estimated_tokens,
            budget.ceiling_tokens,
            budget.context_window_tokens.map_or_else(|| "unknown".to_string(), |value| value.to_string()),
            budget.calibration_sample_count,
        ),
    );

    let capability_fingerprint = capabilities.planning_fingerprint();
    let checkpoint_identity =
        CheckpointIdentity::from_task(&task, &stored.extraction_backend, &capability_fingerprint);
    let outline_contract = document_checkpoints::outline_contract_key(&checkpoint_identity, plan);
    let reusable_final_outline = document_checkpoints::load_final_outline_best_effort(
        database.inner(),
        document_id,
        &checkpoint_identity,
    );
    let outline = if let Some(outline) = reusable_final_outline {
        document_store::initialize_ai_plan(
            database.inner(),
            document_id,
            plan.label(),
            DocumentAiProgressPlan {
                estimated_source_tokens,
                outline_window_count: outline_windows.len(),
                outline_completed_window_count: outline_windows.len(),
                generation_window_count: generation_windows.len(),
                generation_completed_window_count: 0,
            },
        )?;
        crate::logging::info(
            "document_final_outline_reused",
            format!(
                "document={} provider={} model={} sections={}",
                document_id,
                task.provider_kind,
                task.model_id,
                outline.sections.len(),
            ),
        );
        outline
    } else {
        let cached_outline = document_checkpoints::load_outline_fragments_best_effort(
            database.inner(),
            document_id,
            &outline_contract,
            &outline_windows,
        );
        let cached_count = cached_outline
            .iter()
            .filter(|value| value.is_some())
            .count();
        document_store::initialize_ai_plan(
            database.inner(),
            document_id,
            plan.label(),
            DocumentAiProgressPlan {
                estimated_source_tokens,
                outline_window_count: outline_windows.len(),
                outline_completed_window_count: cached_count,
                generation_window_count: generation_windows.len(),
                generation_completed_window_count: 0,
            },
        )?;
        document_store::mark_outline_building(database.inner(), document_id)?;
        let generated_count = AtomicUsize::new(0);
        let outline = document_intelligence::build_document_outline(
            runtime.inner(),
            &task,
            &stored.dto.title,
            i64::try_from(source.unit_count).unwrap_or(i64::MAX),
            &outline_windows,
            &cached_outline,
            |window, fragment| {
                document_checkpoints::persist_outline_fragment_best_effort(
                    database.inner(),
                    document_id,
                    &outline_contract,
                    window,
                    fragment,
                );
                let generated_count = generated_count
                    .fetch_add(1, Ordering::Relaxed)
                    .saturating_add(1);
                document_store::update_outline_progress(
                    database.inner(),
                    document_id,
                    cached_count.saturating_add(generated_count),
                )
            },
        )
        .await?;
        document_store::update_outline_progress(
            database.inner(),
            document_id,
            outline_windows.len(),
        )?;
        persist_outline(
            database.inner(),
            document_id,
            &task,
            &stored.extraction_backend,
            &outline,
        )?;
        outline
    };

    let generation_contract = document_checkpoints::generation_contract_key(
        &checkpoint_identity,
        plan,
        &outline,
        plan.max_cards_per_window,
    );
    let cached_generation = generation_windows
        .iter()
        .map(|window| {
            generation_contract.as_deref().and_then(|contract| {
                document_checkpoints::load_generation_cards_best_effort(
                    database.inner(),
                    document_id,
                    contract,
                    window,
                )
            })
        })
        .collect::<Vec<_>>();
    let generation_completed = cached_generation
        .iter()
        .filter(|value| value.is_some())
        .count();
    document_store::update_generation_progress(
        database.inner(),
        document_id,
        generation_completed,
    )?;

    let job_id = document_store::create_generation_job(
        database.inner(),
        document_id,
        &GenerationJobIdentity::from(&task),
        &source,
        &generation_windows,
    )?;
    let generated_cards = document_generation::execute_generation_windows(GenerationExecution {
        database: database.inner(),
        paths: paths.inner(),
        runtime: runtime.inner(),
        task: &task,
        document_id,
        document_title: &stored.dto.title,
        outline: &outline,
        plan,
        windows: &generation_windows,
        job_id: &job_id,
        cached_cards: cached_generation,
        checkpoint_contract: generation_contract.as_deref(),
        diagnostic_prefix: "initial",
    })
    .await?;

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
    if generated_cards.len() > MAX_GENERATED_CARDS {
        return Err(CommandError::new(
            "too_many_generated_cards",
            "The generated document exceeded ostudy's defensive card limit.",
        ));
    }

    document_store::mark_job_committing(
        database.inner(),
        document_id,
        &job_id,
        generated_cards.len(),
    )?;
    let payload = build_generated_import_json(&stored, generated_cards)?;
    let (api_token_count, api_token_usage_complete) = task.usage_summary();
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
                .map_err(|error| {
                    CommandError::database_at(
                        "document_deck_provenance_insert_failed",
                        format!("document={document_id} job={job_id}"),
                        error,
                    )
                })?;
            let document_changed = connection
                .execute(
                    "UPDATE documents
                     SET lifecycle_state = 'ready', generated_note_count = ?1,
                         generated_card_count = ?2, generated_deck_id = ?3,
                         outline_completed_window_count = outline_window_count,
                         generation_completed_window_count = generation_window_count,
                         safe_error_code = '', safe_error_message = '',
                         processing_duration_ms = CASE
                             WHEN processing_started_at IS NULL THEN NULL
                             ELSE MAX(0, ?4 - processing_started_at)
                         END,
                         api_token_count = ?5, api_token_usage_complete = ?6,
                         updated_at = ?4, completed_at = ?4, revision = revision + 1
                     WHERE id = ?7 AND lifecycle_state = 'processing'",
                    params![
                        import_result.created_note_count,
                        import_result.created_card_count,
                        import_result.deck_id,
                        now,
                        api_token_count,
                        if api_token_usage_complete {
                            1_i64
                        } else {
                            0_i64
                        },
                        document_id,
                    ],
                )
                .map_err(|error| {
                    CommandError::database_at(
                        "document_generation_finalize_failed",
                        format!("document={document_id} job={job_id}"),
                        error,
                    )
                })?;
            if document_changed != 1 {
                return Err(CommandError::new(
                    "document_changed",
                    "The document changed before generated cards could be committed.",
                ));
            }
            let job_changed = connection
                .execute(
                    "UPDATE document_generation_jobs
                     SET state = 'succeeded', generated_candidate_count = ?1,
                         generated_deck_id = ?2, safe_error_code = '',
                         safe_error_message = '', updated_at = ?3, completed_at = ?3
                     WHERE id = ?4 AND document_id = ?5 AND state = 'committing'",
                    params![
                        import_result.created_card_count,
                        import_result.deck_id,
                        now,
                        job_id,
                        document_id,
                    ],
                )
                .map_err(|error| {
                    CommandError::database_at(
                        "document_generation_job_finalize_failed",
                        format!("document={document_id} job={job_id}"),
                        error,
                    )
                })?;
            if job_changed != 1 {
                return Err(CommandError::new(
                    "document_job_changed",
                    "The generation job changed before its commit completed.",
                ));
            }
            Ok(())
        },
    )?;
    document_checkpoints::clear_generation_checkpoints_best_effort(database.inner(), document_id);
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

fn persist_outline(
    database: &Database,
    document_id: &str,
    task: &document_intelligence::DocumentTaskContext,
    extraction_backend: &str,
    outline: &document_intelligence::DocumentOutline,
) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    let outline_json = serde_json::to_string(outline).map_err(CommandError::serialization)?;
    database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| {
                CommandError::database_at(
                    "document_outline_persist_begin_failed",
                    format!("document={document_id}"),
                    error,
                )
            })?;
        transaction
            .execute(
                "INSERT INTO document_outlines (
                     document_id, schema_version, prompt_version, profile_id,
                     provider_kind, model_id, profile_revision, credential_epoch,
                     extraction_backend, outline_json, summary_text, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12)
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
                    document_intelligence::OUTLINE_SCHEMA_VERSION,
                    document_intelligence::OUTLINE_PROMPT_VERSION,
                    task.profile_id,
                    task.provider_kind,
                    task.model_id,
                    task.profile_revision,
                    task.credential_epoch,
                    extraction_backend,
                    outline_json,
                    outline.summary,
                    now,
                ],
            )
            .map_err(|error| {
                CommandError::database_at(
                    "document_outline_persist_failed",
                    format!("document={document_id}"),
                    error,
                )
            })?;
        let changed = transaction
            .execute(
                "UPDATE documents SET outline_state = 'ready', updated_at = ?1
                 WHERE id = ?2 AND lifecycle_state = 'processing'",
                params![now, document_id],
            )
            .map_err(|error| {
                CommandError::database_at(
                    "document_outline_state_ready_failed",
                    format!("document={document_id}"),
                    error,
                )
            })?;
        if changed != 1 {
            return Err(CommandError::new(
                "document_changed",
                "The document changed before its outline could be committed.",
            ));
        }
        transaction.commit().map_err(|error| {
            CommandError::database_at(
                "document_outline_commit_failed",
                format!("document={document_id}"),
                error,
            )
        })
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
            .map_err(|db_error| {
                CommandError::database_at(
                    "document_failure_state_write_failed",
                    format!("document={document_id} original_code={}", error.code),
                    db_error,
                )
            })?;
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

fn build_generated_import_json(
    document: &StoredDocument,
    cards: Vec<serde_json::Value>,
) -> Result<String, CommandError> {
    serde_json::to_string(&serde_json::json!({
        "format": "ostudy-flashcards",
        "formatVersion": 1,
        "deck": {
            "mode": "create",
            "name": document.dto.title,
            "description": format!(
                "AI-generated flashcards from {}. Edit or delete these cards through the normal deck tools.",
                document.dto.original_filename
            )
        },
        "defaultTags": [],
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
                    extraction_mode, ocr_page_count, ocr_completed_page_count,
                    ocr_mean_confidence, ocr_engine,
                    scale_mode, estimated_source_tokens,
                    outline_window_count, outline_completed_window_count,
                    generation_window_count, generation_completed_window_count,
                    generated_note_count, generated_card_count,
                    CASE WHEN EXISTS (
                        SELECT 1 FROM decks deck
                        WHERE deck.id = documents.generated_deck_id AND deck.deleted_at IS NULL
                    ) THEN documents.generated_deck_id ELSE NULL END,
                    safe_error_code, safe_error_message, created_at, updated_at,
                    completed_at, archived_at, revision,
                    processing_duration_ms, api_token_count, api_token_usage_complete
             FROM documents
             WHERE (?1 = 1 OR archived_at IS NULL)
             ORDER BY updated_at DESC, id
             LIMIT ?2",
        )
        .map_err(CommandError::database)?;
    let documents = statement
        .query_map(
            params![
                if include_archived { 1_i64 } else { 0_i64 },
                MAX_DOCUMENTS_RETURNED
            ],
            document_row,
        )
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    Ok(documents)
}

fn load_document(
    connection: &Connection,
    document_id: &str,
) -> Result<StoredDocument, CommandError> {
    connection
        .query_row(
            "SELECT id, title, original_filename, byte_size, sha256, lifecycle_state,
                    page_count, extracted_page_count, rendered_page_count,
                    extraction_mode, ocr_page_count, ocr_completed_page_count,
                    ocr_mean_confidence, ocr_engine,
                    scale_mode, estimated_source_tokens,
                    outline_window_count, outline_completed_window_count,
                    generation_window_count, generation_completed_window_count,
                    generated_note_count, generated_card_count,
                    CASE WHEN EXISTS (
                        SELECT 1 FROM decks deck
                        WHERE deck.id = documents.generated_deck_id AND deck.deleted_at IS NULL
                    ) THEN documents.generated_deck_id ELSE NULL END,
                    safe_error_code, safe_error_message, created_at, updated_at,
                    completed_at, archived_at, revision,
                    processing_duration_ms, api_token_count, api_token_usage_complete,
                    source_relative_path, extraction_backend, render_backend, semantic_chunk_count,
                    source_format, source_unit_kind
             FROM documents WHERE id = ?1",
            [document_id],
            |row| {
                Ok(StoredDocument {
                    dto: document_row(row)?,
                    source_relative_path: row.get(33)?,
                    extraction_backend: row.get(34)?,
                    render_backend: row.get(35)?,
                    semantic_chunk_count: row.get(36)?,
                    source_format: row.get(37)?,
                    source_unit_kind: row.get(38)?,
                })
            },
        )
        .optional()
        .map_err(CommandError::database)?
        .ok_or_else(|| {
            CommandError::new(
                "document_not_found",
                "That document is no longer in the library.",
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
        extraction_mode: row.get(9)?,
        ocr_page_count: row.get(10)?,
        ocr_completed_page_count: row.get(11)?,
        ocr_mean_confidence: row.get(12)?,
        ocr_engine: row.get(13)?,
        scale_mode: row.get(14)?,
        estimated_source_tokens: row.get(15)?,
        outline_window_count: row.get(16)?,
        outline_completed_window_count: row.get(17)?,
        generation_window_count: row.get(18)?,
        generation_completed_window_count: row.get(19)?,
        generated_note_count: row.get(20)?,
        generated_card_count: row.get(21)?,
        generated_deck_id: row.get(22)?,
        safe_error_code: row.get(23)?,
        safe_error_message: row.get(24)?,
        created_at: row.get(25)?,
        updated_at: row.get(26)?,
        completed_at: row.get(27)?,
        archived_at: row.get(28)?,
        revision: row.get(29)?,
        processing_duration_ms: row.get(30)?,
        api_token_count: row.get(31)?,
        api_token_usage_complete: row.get::<_, i64>(32)? != 0,
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

fn bounded_error(value: &str, maximum: usize) -> String {
    value.chars().take(maximum).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_import_envelope_uses_source_format_tag() {
        let document = StoredDocument {
            dto: DocumentDto {
                id: "0123456789abcdef0123456789abcdef".to_string(),
                title: "Physiology".to_string(),
                original_filename: "physiology.pdf".to_string(),
                byte_size: 1,
                sha256: "0".repeat(64),
                lifecycle_state: "processing".to_string(),
                page_count: 1,
                extracted_page_count: 1,
                rendered_page_count: 1,
                extraction_mode: "native".to_string(),
                ocr_page_count: 0,
                ocr_completed_page_count: 0,
                ocr_mean_confidence: None,
                ocr_engine: String::new(),
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
                processing_duration_ms: None,
                api_token_count: None,
                api_token_usage_complete: false,
            },
            source_relative_path: "documents/id/source.pdf".to_string(),
            extraction_backend: "extractor".to_string(),
            render_backend: "renderer".to_string(),
            semantic_chunk_count: 1,
            source_format: "pdf".to_string(),
            source_unit_kind: "page".to_string(),
        };
        let payload = build_generated_import_json(
            &document,
            vec![serde_json::json!({"type":"basic","front":"Q","back":"A","tags":[]})],
        )
        .unwrap();
        assert!(payload.contains("\"PDF\""));
        assert!(payload.contains("ostudy-flashcards"));
    }
}
