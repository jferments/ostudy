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
    knowledge::assessment::policy::instructions::GenerationInstructions,
    paths::AppPaths,
    pdf_render_backend::PdfRenderRuntime,
};

mod parallel;

const MAX_GENERATED_CARDS: usize = 10_000;

#[derive(Clone, Debug)]
pub(crate) struct InitialGenerationResult {
    pub deck_id: String,
    pub created_note_count: i64,
    pub created_card_count: i64,
    pub skipped_duplicate_count: i64,
}

#[derive(Clone, Debug)]
pub(crate) struct InitialDocumentContext {
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

type InitialGenerationFuture<'a> = std::pin::Pin<
    Box<
        dyn std::future::Future<Output = Result<InitialGenerationResult, CommandError>> + Send + 'a,
    >,
>;

pub(crate) fn process_and_generate_initial_flashcards<'a>(
    database: &'a Database,
    paths: &'a AppPaths,
    runtime: &'a AiRuntimeState,
    render_runtime: PdfRenderRuntime,
    document_id: &'a str,
    instructions: &'a GenerationInstructions,
    destination_deck_id: Option<&'a str>,
) -> InitialGenerationFuture<'a> {
    Box::pin(process_initial_flashcards(
        database,
        paths,
        runtime,
        render_runtime,
        document_id,
        instructions,
        destination_deck_id,
        false,
    ))
}

pub(crate) fn process_reserved_initial_flashcards<'a>(
    database: &'a Database,
    paths: &'a AppPaths,
    runtime: &'a AiRuntimeState,
    render_runtime: PdfRenderRuntime,
    document_id: &'a str,
    instructions: &'a GenerationInstructions,
    destination_deck_id: Option<&'a str>,
) -> InitialGenerationFuture<'a> {
    Box::pin(process_initial_flashcards(
        database,
        paths,
        runtime,
        render_runtime,
        document_id,
        instructions,
        destination_deck_id,
        true,
    ))
}

#[allow(clippy::too_many_arguments)]
async fn process_initial_flashcards(
    database: &Database,
    paths: &AppPaths,
    runtime: &AiRuntimeState,
    render_runtime: PdfRenderRuntime,
    document_id: &str,
    instructions: &GenerationInstructions,
    destination_deck_id: Option<&str>,
    processing_reserved: bool,
) -> Result<InitialGenerationResult, CommandError> {
    // A chosen deck is checked before any work starts, and again at commit.
    if let Some(deck_id) = destination_deck_id {
        destination_deck_name(database, deck_id)?;
    }
    if !processing_reserved {
        document_store::mark_document_processing(database, document_id)?;
    }
    let (stored, figure_search) =
        ensure_document_derivatives(database, paths, render_runtime.clone(), document_id).await?;

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
    task.question_policy.generation_instructions = instructions.clone();
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
    let generation_source_token_budget =
        document_scale::generation_source_token_budget(scale_plan, &task.question_policy)?;
    let generation_windows = build_partitioned_semantic_windows(
        &chunks,
        scale_plan,
        generation_source_token_budget,
        scale_plan.generation_window_max_units,
        scale_plan.generation_window_overlap_tokens,
    );
    validate_semantic_windows(&outline_windows, &source)?;
    validate_semantic_windows(&generation_windows, &source)?;
    let semantic_work = crate::knowledge::planning::estimate(&chunks, task.include_page_images);
    let budget = document_scale::validate_automatic_job_calibrated(
        source.unit_count,
        estimated_source_tokens,
        scale_plan,
        semantic_work.requests,
        generation_windows.len(),
        document_scale::DocumentBudgetContext {
            capabilities: &capabilities,
            reasoning_effort: &task.reasoning_effort,
            calibration,
            semantic_image_count: semantic_work.images,
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
            "outlineViewWindows": outline_windows.len(),
            "semanticWindows": budget.semantic_window_count,
            "semanticTokenAllowance": budget.semantic_token_allowance,
            "semanticCharacters": semantic_work.characters,
            "semanticImages": semantic_work.images,
            "estimateBasis": "uncached semantic analysis, bounded repair, knowledge reduction, metadata, artifact authoring and curation; not a billing quote",
            "generationWindows": generation_windows.len(),
            "generationSourceTokenBudget": generation_source_token_budget,
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

    // Explicit card-only support demand is resolved before the shared outline,
    // learning plan and sibling artifact catalogs can bind to unit hashes.
    let chunk_indices = chunks
        .iter()
        .map(|chunk| chunk.chunk_index)
        .collect::<Vec<_>>();
    crate::knowledge::annotation_demand::prepare_document(
        crate::knowledge::authoring::DocumentPreparation {
            database,
            paths,
            runtime,
            task: &task,
            document_id,
            chunk_indices: &chunk_indices,
            retry_partial: false,
        },
    )
    .await?;

    let capability_fingerprint = capabilities.planning_fingerprint();
    let outline = ensure_document_outline(EnsureDocumentOutlineInput {
        database,
        paths,
        runtime,
        task: &task,
        document_id,
        extraction_backend: &stored.extraction_backend,
        source: &source,
        chunks: &chunks,
        plan: scale_plan,
    })
    .await?;
    let learning_plan =
        ensure_learning_plan_for_outline(database, document_id, &source, &outline, &chunks)?;

    let guide_task = task.fork_usage();
    let guide_future = Box::pin(
        crate::document_study_guide_commands::generate_document_study_guide_from_plan(
            crate::document_study_guide_commands::PreparedStudyGuideInput {
                database,
                paths,
                runtime,
                task: &guide_task,
                document_id,
                document_title: &stored.title,
                plan: &learning_plan,
                chunks: &chunks,
                maximum_images: scale_plan.max_images_per_window,
            },
        ),
    );
    let card_future = Box::pin(async {
        let checkpoint_identity = CheckpointIdentity::from_task(
            &task,
            &stored.extraction_backend,
            &capability_fingerprint,
        );
        let generation_contract = document_checkpoints::generation_contract_key(
            &checkpoint_identity,
            scale_plan,
            &outline,
            scale_plan.max_cards_per_window,
        );
        let generation_contract = crate::knowledge::assessment::policy::checkpoint_contract(
            generation_contract,
            &task.question_policy,
        )?;
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
        let generated_values =
            document_generation::execute_generation_windows(GenerationExecution {
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
            generated_values,
        )?;
        let generated_values = grounded_candidates
            .iter()
            .map(|card| card.generation_value())
            .collect::<Result<Vec<_>, _>>()?;
        let curated_values = document_intelligence::curate_document_cards(
            runtime,
            &task,
            &outline,
            generated_values,
        )
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
        let payload = match destination_deck_id {
            Some(deck_id) => crate::document_more::build_append_payload(
                &destination_deck_name(database, deck_id)?,
                &grounded_cards,
            )?,
            None => build_generated_import_json(&stored, &grounded_cards)?,
        };
        let (api_token_count, api_token_usage_complete) = task.usage_summary();
        let result = import_generated_flashcards_bytes_with_commit_context(
            database,
            payload.as_bytes(),
            |connection, import_result, commit_context, now| {
                if destination_deck_id.is_some_and(|deck_id| deck_id != import_result.deck_id) {
                    return Err(CommandError::new(
                        "document_destination_deck_changed",
                        "The chosen deck changed while its cards were being written. Choose it again and retry.",
                    ));
                }
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
                // A new deck joins its document's subject, so studying the
                // subject includes the document's cards. A chosen deck stays
                // where the learner keeps it.
                if destination_deck_id.is_none() {
                    crate::subjects::file_generated_deck_with_document(
                        connection,
                        document_id,
                        None,
                        now,
                    )?;
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
    });

    // Figures are read beside cards and the guide once their search is done.
    let figure_task = task.fork_usage();
    let figure_reading = crate::document_figures::read_after_search(
        crate::document_figures::ReadAfterSearch {
            database,
            paths,
            render: &render_runtime,
            runtime,
            task: &figure_task,
            document_id,
        },
        figure_search,
    );
    let ((cards, guide), ()) =
        tokio::join!(parallel::run(card_future, guide_future), figure_reading);
    match guide {
        Ok(guide) => crate::logging::info_target(
            "documents.generation",
            "parallel_study_guide_succeeded",
            "The automatic Study Guide completed alongside flashcard generation.",
            serde_json::json!({
                "document": document_id,
                "guide": guide.id,
                "revision": guide.revision,
            }),
        ),
        Err(error) => crate::logging::warn_target(
            "documents.generation",
            "parallel_study_guide_failed",
            "Flashcard generation remains independent when the automatic Study Guide fails.",
            serde_json::json!({
                "document": document_id,
                "code": error.code,
                "detail": error.message.chars().take(400).collect::<String>(),
            }),
        ),
    }
    cards
}

/// The figure search a rebuild started, which generation does not wait for.
pub(crate) type FigureSearch = tauri::async_runtime::JoinHandle<()>;

/// Extract, render and chunk the managed source when its derivatives are missing
/// or were made by an older backend. No provider is contacted. A rebuilt PDF
/// also starts its figure search, returned so a caller can read its figures.
pub(crate) async fn ensure_document_derivatives(
    database: &Database,
    paths: &AppPaths,
    render_runtime: PdfRenderRuntime,
    document_id: &str,
) -> Result<(InitialDocumentContext, Option<FigureSearch>), CommandError> {
    let mut stored = load_context(database, document_id)?;
    let mut figure_search = None;
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
        let database_owned = database.clone();
        let paths_owned = paths.clone();
        let source_owned = source_path.clone();
        let figure_runtime = render_runtime.clone();
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
            if format == DocumentFormat::ImageBundle {
                document_ingestion::ingest_image_set(
                    &database_owned,
                    &paths_owned,
                    &id_owned,
                    &source_owned,
                    MAX_SUPPORTED_SOURCE_UNITS,
                    report_progress,
                )
            } else if format == DocumentFormat::Pdf {
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
        if format == DocumentFormat::Pdf {
            // Figures are found beside generation, which does not wait for them.
            let database = database.clone();
            let document_id = document_id.to_string();
            let source_path = source_path.clone();
            figure_search = Some(tauri::async_runtime::spawn(async move {
                crate::document_figures::find_after_ingestion(
                    &database,
                    figure_runtime,
                    &document_id,
                    source_path,
                )
                .await;
            }));
        }
        stored = load_context(database, document_id)?;
    }
    Ok((stored, figure_search))
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

/// The name of the existing deck a document's first cards go into. Cards are
/// appended by name, so the name must pick out that deck alone.
pub(crate) fn destination_deck_name(
    database: &Database,
    deck_id: &str,
) -> Result<String, CommandError> {
    let name: Option<String> = database.with_connection(|connection| {
        connection
            .query_row(
                "SELECT name FROM decks
                 WHERE id = ?1 AND deleted_at IS NULL AND archived_at IS NULL",
                [deck_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(CommandError::database)
    })?;
    let Some(name) = name else {
        return Err(CommandError::new(
            "document_destination_deck_missing",
            "The chosen deck is no longer in the library. Choose another deck.",
        ));
    };
    crate::document_more::ensure_exact_append_destination(database, deck_id, &name)?;
    Ok(name)
}

fn build_generated_import_json(
    document: &InitialDocumentContext,
    cards: &[crate::document_card_topics::SourceGroundedGeneratedCard],
) -> Result<String, CommandError> {
    serde_json::to_string(&serde_json::json!({
        "format": "ostudy-flashcards",
        "formatVersion": 2,
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

    fn deck(database: &Database, name: &str) -> String {
        database
            .with_connection(|connection| {
                crate::decks::create_deck_in_connection(
                    connection,
                    crate::decks::CreateDeckInput {
                        name: name.to_string(),
                        description: None,
                    },
                )
            })
            .unwrap()
            .id
    }

    #[test]
    fn a_chosen_deck_must_be_an_active_deck_its_name_picks_out() {
        let database = Database::open_in_memory().unwrap();
        let renal = deck(&database, "Renal physiology");
        assert_eq!(
            destination_deck_name(&database, &renal).unwrap(),
            "Renal physiology"
        );

        let missing = destination_deck_name(&database, &"f".repeat(32)).unwrap_err();
        assert_eq!(missing.code, "document_destination_deck_missing");

        let archived = deck(&database, "Old cards");
        database
            .with_connection(|connection| {
                connection
                    .execute(
                        "UPDATE decks SET archived_at = 1 WHERE id = ?1",
                        [archived.as_str()],
                    )
                    .map_err(CommandError::database)
            })
            .unwrap();
        let archived = destination_deck_name(&database, &archived).unwrap_err();
        assert_eq!(archived.code, "document_destination_deck_missing");

        // Cards are appended by name, so a second active deck with that name
        // could take them instead.
        database
            .with_connection(|connection| {
                connection
                    .execute(
                        "INSERT INTO decks(id,name,description,created_at,updated_at,revision)
                         SELECT ?1,name,description,created_at,updated_at,1 FROM decks WHERE id = ?2",
                        [&"e".repeat(32), &renal],
                    )
                    .map_err(CommandError::database)
            })
            .unwrap();
        let ambiguous = destination_deck_name(&database, &renal).unwrap_err();
        assert_eq!(ambiguous.code, "document_generated_deck_name_ambiguous");
    }

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
            assessment: None,
            knowledge_support: Vec::new(),
            card: serde_json::json!({"type":"basic","front":"Q","back":"A","tags":[]}),
            source_locator: SourceLocator::new(SourceUnitKind::Page, vec![1], None).unwrap(),
        }];
        let payload = build_generated_import_json(&document, &cards).unwrap();
        assert!(payload.contains("ostudy-flashcards"));
        assert!(!payload.contains("source_locator"));
        assert!(!payload.contains("topic"));
    }
}
