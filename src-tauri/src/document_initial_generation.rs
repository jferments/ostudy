//! Initial flashcard generation for one managed canonical document.
//!
//! Source ingestion remains format-specific below `document_ingestion`, while
//! every learning artifact shares the same validated outline and durable
//! `DocumentLearningPlan`. Generated card provenance stays attached to canonical
//! source windows through selection-only curation and the authoring transaction.

use rusqlite::{params, OptionalExtension};

use crate::{
    ai::{document_intelligence, document_provider, AiRuntimeState},
    authoring::card_types_and_templates::generated_import::import_generated_flashcards_bytes_with_commit_context,
    database::Database,
    document_card_topics::{
        attach_generated_card_source_windows, link_imported_notes_to_topics,
        retain_curated_card_provenance,
    },
    document_checkpoints::{self, CheckpointIdentity},
    document_cost,
    document_domain::{DocumentFormat, DocumentSourceDescriptor, SourceUnitKind},
    document_generation::{self, GenerationExecution},
    document_ingestion,
    document_learning_service::ensure_learning_plan_for_outline,
    document_outline_service::{ensure_document_outline, EnsureDocumentOutlineInput},
    document_scale::{self, DocumentScalePlan, MAX_SUPPORTED_SOURCE_UNITS},
    document_semantics::{build_partitioned_semantic_windows, validate_semantic_windows},
    document_store::{self, DocumentAiProgressPlan, GenerationJobIdentity},
    error::CommandError,
    paths::AppPaths,
    pdf_render_backend::PdfRenderRuntime,
};

const MAX_GENERATED_CARDS: usize = 10_000;

#[derive(Clone, Debug)]
pub(crate) struct InitialGenerationResult {
    pub deck_id: String,
    pub created_note_count: i64,
    pub created_card_count: i64,
    pub skipped_duplicate_count: i64,
}

#[derive(Clone, Debug)]
struct InitialDocumentContext {
    title: String,
    original_filename: String,
    source_relative_path: String,
    adapter_backend: String,
    extraction_backend: String,
    render_backend: String,
    semantic_chunk_count: i64,
    source_format: String,
    source_unit_kind: String,
    source_unit_count: i64,
    extracted_source_unit_count: i64,
    rendered_source_unit_count: i64,
}

impl InitialDocumentContext {
    fn source_descriptor(&self) -> Result<DocumentSourceDescriptor, CommandError> {
        let unit_count = usize::try_from(self.source_unit_count).map_err(|_| {
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

pub(crate) async fn process_and_generate_initial_flashcards(
    database: &Database,
    paths: &AppPaths,
    runtime: &AiRuntimeState,
    render_runtime: PdfRenderRuntime,
    document_id: &str,
) -> Result<InitialGenerationResult, CommandError> {
    document_store::mark_document_processing(database, document_id)?;
    let mut stored = load_context(database, document_id)?;
    let source_path = paths.resolve_relative(&stored.source_relative_path);
    if !source_path.is_file() {
        return Err(CommandError::new(
            "document_source_missing",
            "The managed source document is missing. Restore the library from backup or import it again.",
        ));
    }

    let format = DocumentFormat::from_db(&stored.source_format)?;
    let active_adapter_backend = document_ingestion::active_adapter_backend(format)?;
    let active_extraction_backend = document_ingestion::active_extraction_backend(format)?;
    let active_render_backend = document_ingestion::active_render_backend(format)?;
    let backend_changed = stored.adapter_backend != active_adapter_backend
        || stored.extraction_backend != active_extraction_backend
        || stored.render_backend != active_render_backend;
    let incomplete_derivatives = stored.source_unit_count == 0
        || stored.extracted_source_unit_count != stored.source_unit_count
        || (format == DocumentFormat::Pdf
            && stored.rendered_source_unit_count != stored.source_unit_count)
        || stored.semantic_chunk_count == 0;
    if backend_changed || incomplete_derivatives {
        let paths_owned = paths.clone();
        let source_owned = source_path.clone();
        let id_owned = document_id.to_string();
        let (progress_sender, mut progress_receiver) =
            tokio::sync::mpsc::unbounded_channel::<document_ingestion::IngestionProgress>();
        let processing_task = tauri::async_runtime::spawn_blocking(move || {
            let report_progress = move |progress| {
                progress_sender.send(progress).map_err(|_| {
                    CommandError::new(
                        "document_processing_stopped",
                        "The document-processing progress channel closed unexpectedly.",
                    )
                })
            };
            if format == DocumentFormat::Pdf {
                document_ingestion::ingest_pdf(
                    &paths_owned,
                    &render_runtime,
                    &id_owned,
                    &source_owned,
                    MAX_SUPPORTED_SOURCE_UNITS,
                    report_progress,
                )
            } else {
                document_ingestion::ingest_document(
                    format,
                    &paths_owned,
                    &render_runtime,
                    &id_owned,
                    &source_owned,
                    MAX_SUPPORTED_SOURCE_UNITS,
                    report_progress,
                )
            }
        });
        while let Some(progress) = progress_receiver.recv().await {
            document_ingestion::update_ingestion_progress(database, document_id, progress)?;
        }
        let processed = processing_task.await.map_err(|error| {
            CommandError::new(
                "document_processing_failed",
                format!("The document-processing task stopped unexpectedly: {error}"),
            )
        })??;
        document_ingestion::persist_ingested_document(database, document_id, &processed)?;
        stored = load_context(database, document_id)?;
    }

    let source = stored.source_descriptor()?;
    source.validate()?;
    if source.format == DocumentFormat::Pdf {
        let renders = document_store::load_source_renders(database, document_id)?;
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
    }
    let chunks = document_store::load_semantic_chunks(database, document_id)?;
    if chunks.is_empty() {
        return Err(CommandError::new(
            "document_has_no_semantic_content",
            "The document did not contain structured source content that ostudy could analyze.",
        ));
    }

    let mut task = document_intelligence::load_document_task_context(database)?;
    let capabilities = document_provider::load_document_provider_capabilities(
        database,
        &task.profile_id,
        &task.provider_kind,
        &task.model_id,
    )?;
    task.apply_provider_capabilities(&capabilities);
    let calibration =
        document_cost::load_cost_calibration(database, &task.provider_kind, &task.model_id)?;
    let scale_plan = DocumentScalePlan::for_capabilities(
        source.unit_count,
        &capabilities,
        &task.reasoning_effort,
        calibration,
    )?;
    let estimated_source_tokens = chunks.iter().fold(0_usize, |total, chunk| {
        total.saturating_add(
            chunk
                .token_estimate
                .max(chunk.full_text.chars().count().div_ceil(4)),
        )
    });
    let outline_windows = build_partitioned_semantic_windows(
        &chunks,
        scale_plan,
        scale_plan.outline_window_max_tokens,
        scale_plan.outline_window_max_units,
        scale_plan.outline_window_overlap_tokens,
    );
    let generation_windows = build_partitioned_semantic_windows(
        &chunks,
        scale_plan,
        scale_plan.generation_window_max_tokens,
        scale_plan.generation_window_max_units,
        scale_plan.generation_window_overlap_tokens,
    );
    validate_semantic_windows(&outline_windows, &source)?;
    validate_semantic_windows(&generation_windows, &source)?;
    let budget = document_scale::validate_automatic_job_calibrated(
        source.unit_count,
        estimated_source_tokens,
        scale_plan,
        outline_windows.len(),
        generation_windows.len(),
        document_scale::DocumentBudgetContext {
            capabilities: &capabilities,
            reasoning_effort: &task.reasoning_effort,
            calibration,
        },
    )?;
    crate::logging::debug_target(
        "ai.document.plan",
        "document_ai_scale_plan",
        "Planned bounded document-AI work without logging source content.",
        serde_json::json!({
            "document": document_id,
            "format": source.format.as_str(),
            "provider": task.provider_kind.as_str(),
            "model": task.model_id.as_str(),
            "scale": scale_plan.label(),
            "sourceUnits": source.unit_count,
            "sourceTokens": estimated_source_tokens,
            "outlineWindows": outline_windows.len(),
            "generationWindows": generation_windows.len(),
            "cardCapacity": budget.generation_card_capacity,
            "estimatedJobTokens": budget.estimated_tokens,
            "contextWindowTokens": budget.context_window_tokens,
            "calibrationSamples": budget.calibration_sample_count,
        }),
    );

    document_store::initialize_ai_plan(
        database,
        document_id,
        scale_plan.label(),
        DocumentAiProgressPlan {
            estimated_source_tokens,
            outline_window_count: outline_windows.len(),
            outline_completed_window_count: 0,
            generation_window_count: generation_windows.len(),
            generation_completed_window_count: 0,
        },
    )?;

    let capability_fingerprint = capabilities.planning_fingerprint();
    let outline = ensure_document_outline(EnsureDocumentOutlineInput {
        database,
        runtime,
        task: &task,
        document_id,
        document_title: &stored.title,
        extraction_backend: &stored.extraction_backend,
        source: &source,
        chunks: &chunks,
        plan: scale_plan,
        provider_capability_fingerprint: &capability_fingerprint,
    })
    .await?;
    let learning_plan =
        ensure_learning_plan_for_outline(database, document_id, &source, &outline, &chunks)?;

    let checkpoint_identity =
        CheckpointIdentity::from_task(&task, &stored.extraction_backend, &capability_fingerprint);
    let generation_contract = document_checkpoints::generation_contract_key(
        &checkpoint_identity,
        scale_plan,
        &outline,
        scale_plan.max_cards_per_window,
    );
    let cached_generation = generation_windows
        .iter()
        .map(|window| {
            generation_contract.as_deref().and_then(|contract| {
                document_checkpoints::load_generation_cards_best_effort(
                    database,
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
    document_store::update_generation_progress(database, document_id, generation_completed)?;

    let job_id = document_store::create_generation_job(
        database,
        document_id,
        &GenerationJobIdentity::from(&task),
        &source,
        &generation_windows,
    )?;
    let generated_values = document_generation::execute_generation_windows(GenerationExecution {
        database,
        paths,
        runtime,
        task: &task,
        document_id,
        document_title: &stored.title,
        outline: &outline,
        plan: scale_plan,
        windows: &generation_windows,
        job_id: &job_id,
        cached_cards: cached_generation,
        checkpoint_contract: generation_contract.as_deref(),
        diagnostic_prefix: "initial",
    })
    .await?;
    if generated_values.is_empty() {
        return Err(CommandError::new(
            "document_generated_no_cards",
            "The document analysis found no material important enough to generate flashcards.",
        ));
    }
    let grounded_candidates = attach_generated_card_source_windows(
        database,
        &job_id,
        &generation_windows,
        generated_values.clone(),
    )?;
    let curated_values =
        document_intelligence::curate_document_cards(runtime, &task, &outline, generated_values)
            .await?;
    if curated_values.is_empty() {
        return Err(CommandError::new(
            "document_generated_no_cards",
            "The global relevance pass removed all generated cards as low-value or redundant.",
        ));
    }
    let grounded_cards = retain_curated_card_provenance(grounded_candidates, curated_values)?;
    if grounded_cards.len() > MAX_GENERATED_CARDS {
        return Err(CommandError::new(
            "too_many_generated_cards",
            "The generated document exceeded ostudy's defensive card limit.",
        ));
    }

    document_store::mark_job_committing(database, document_id, &job_id, grounded_cards.len())?;
    let payload = build_generated_import_json(&stored, &grounded_cards)?;
    let (api_token_count, api_token_usage_complete) = task.usage_summary();
    let result = import_generated_flashcards_bytes_with_commit_context(
        database,
        payload.as_bytes(),
        |connection, import_result, commit_context, now| {
            link_imported_notes_to_topics(
                connection,
                document_id,
                &job_id,
                &learning_plan,
                &grounded_cards,
                commit_context,
                now,
            )?;
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
    document_checkpoints::clear_generation_checkpoints_best_effort(database, document_id);
    Ok(InitialGenerationResult {
        deck_id: result.deck_id,
        created_note_count: result.created_note_count,
        created_card_count: result.created_card_count,
        skipped_duplicate_count: result.skipped_duplicate_count,
    })
}

fn load_context(
    database: &Database,
    document_id: &str,
) -> Result<InitialDocumentContext, CommandError> {
    database.with_connection(|connection| {
        connection
            .query_row(
                "SELECT title, original_filename, source_relative_path,
                        adapter_backend, extraction_backend, render_backend,
                        semantic_chunk_count, source_format, source_unit_kind,
                        source_unit_count, extracted_page_count, rendered_page_count
                 FROM documents WHERE id = ?1",
                [document_id],
                |row| {
                    Ok(InitialDocumentContext {
                        title: row.get(0)?,
                        original_filename: row.get(1)?,
                        source_relative_path: row.get(2)?,
                        adapter_backend: row.get(3)?,
                        extraction_backend: row.get(4)?,
                        render_backend: row.get(5)?,
                        semantic_chunk_count: row.get(6)?,
                        source_format: row.get(7)?,
                        source_unit_kind: row.get(8)?,
                        source_unit_count: row.get(9)?,
                        extracted_source_unit_count: row.get(10)?,
                        rendered_source_unit_count: row.get(11)?,
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
    })
}

fn build_generated_import_json(
    document: &InitialDocumentContext,
    cards: &[crate::document_card_topics::SourceGroundedGeneratedCard],
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
        "defaultTags": [],
        "cards": cards.iter().map(|card| &card.card).collect::<Vec<_>>(),
    }))
    .map_err(CommandError::serialization)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document_domain::{SourceLocator, SourceUnitKind};

    #[test]
    fn generated_import_payload_contains_card_content_but_not_internal_provenance() {
        let document = InitialDocumentContext {
            title: "Physiology".to_string(),
            original_filename: "physiology.pdf".to_string(),
            source_relative_path: String::new(),
            adapter_backend: String::new(),
            extraction_backend: String::new(),
            render_backend: String::new(),
            semantic_chunk_count: 1,
            source_format: "pdf".to_string(),
            source_unit_kind: "page".to_string(),
            source_unit_count: 1,
            extracted_source_unit_count: 1,
            rendered_source_unit_count: 1,
        };
        let cards = vec![crate::document_card_topics::SourceGroundedGeneratedCard {
            card: serde_json::json!({"type":"basic","front":"Q","back":"A","tags":[]}),
            source_locator: SourceLocator::new(SourceUnitKind::Page, vec![1], None).unwrap(),
        }];
        let payload = build_generated_import_json(&document, &cards).unwrap();
        assert!(payload.contains("ostudy-flashcards"));
        assert!(!payload.contains("source_locator"));
        assert!(!payload.contains("topic"));
    }
}
