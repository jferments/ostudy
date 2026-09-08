//! Additional flashcard generation for an already-generated managed document.
//!
//! A second pass never creates a replacement deck. It reuses retained canonical
//! semantics, the validated outline, and the same durable learning plan used by
//! study guides and initial card generation. New notes retain source-window topic
//! provenance through the canonical authoring transaction.

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    ai::{document_intelligence, document_provider, AiRuntimeState},
    authoring::card_types_and_templates::generated_import::import_generated_flashcards_bytes_with_commit_context,
    database::Database,
    document_card_topics::{
        attach_generated_card_source_windows, link_imported_notes_to_topics,
        retain_curated_card_provenance, SourceGroundedGeneratedCard,
    },
    document_cost,
    document_domain::{DocumentFormat, DocumentSourceDescriptor, SourceUnitKind},
    document_generation::{self, GenerationExecution},
    document_learning_service::ensure_learning_plan_for_outline,
    document_scale::{self, DocumentScalePlan},
    document_semantics::{build_partitioned_semantic_windows, validate_semantic_windows},
    document_store::{self, GenerationJobIdentity},
    documents,
    error::CommandError,
    paths::AppPaths,
    task_cancellation::run_document_task,
    util::validate_entity_id,
};
#[cfg(test)]
use serde_json::Value;

const MAX_EXISTING_COVERAGE_CHARS: usize = 36_000;
const CANCELED_MESSAGE: &str =
    "Additional flashcard generation was stopped. Existing cards were not changed.";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GenerateMorePdfFlashcardsInput {
    pub document_id: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateMorePdfFlashcardsResultDto {
    pub document_id: String,
    pub deck_id: String,
    pub created_note_count: i64,
    pub created_card_count: i64,
    pub skipped_duplicate_count: i64,
}

#[derive(Clone, Debug)]
struct AdditionalDocumentContext {
    title: String,
    deck_id: String,
    deck_name: String,
    source_format: String,
    source_unit_kind: String,
    source_unit_count: usize,
}

impl AdditionalDocumentContext {
    fn source_descriptor(&self) -> Result<DocumentSourceDescriptor, CommandError> {
        Ok(DocumentSourceDescriptor {
            format: DocumentFormat::from_db(&self.source_format)?,
            unit_kind: SourceUnitKind::from_db(&self.source_unit_kind)?,
            unit_count: self.source_unit_count,
        })
    }
}

#[tauri::command]
pub async fn generate_more_pdf_flashcards(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    runtime: State<'_, AiRuntimeState>,
    input: GenerateMorePdfFlashcardsInput,
) -> Result<GenerateMorePdfFlashcardsResultDto, CommandError> {
    validate_entity_id(&input.document_id, "document")?;
    let document_id = input.document_id.clone();
    let result = run_document_task(&document_id, async {
        let result =
            generate_more_impl(database.inner(), paths.inner(), runtime.inner(), input).await;
        if let Err(error) = &result {
            if error.code == "ai_request_canceled" {
                documents::record_document_cancellation_with_message(
                    database.inner(),
                    &document_id,
                    CANCELED_MESSAGE,
                );
            } else {
                documents::record_document_failure(database.inner(), &document_id, error);
            }
        }
        result
    })
    .await;

    match result {
        Err(error) if error.code == "ai_request_canceled" => Err(CommandError::new(
            "document_generation_canceled",
            CANCELED_MESSAGE,
        )),
        other => other,
    }
}

async fn generate_more_impl(
    database: &Database,
    paths: &AppPaths,
    runtime: &AiRuntimeState,
    input: GenerateMorePdfFlashcardsInput,
) -> Result<GenerateMorePdfFlashcardsResultDto, CommandError> {
    let context = load_context(database, &input.document_id)?;
    ensure_exact_append_destination(database, &context.deck_id, &context.deck_name)?;
    document_store::mark_document_processing(database, &input.document_id)?;

    let source = context.source_descriptor()?;
    source.validate()?;
    let outline = load_outline(database, &input.document_id)?;
    let chunks = document_store::load_semantic_chunks(database, &input.document_id)?;
    if chunks.is_empty() {
        return Err(CommandError::new(
            "document_has_no_semantic_content",
            "The retained document has no semantic source chunks available for another generation pass.",
        ));
    }
    let learning_plan =
        ensure_learning_plan_for_outline(database, &input.document_id, &source, &outline, &chunks)?;
    let existing_coverage =
        load_existing_coverage(database, &context.deck_id, MAX_EXISTING_COVERAGE_CHARS)?;
    let second_pass_outline = second_pass_outline(outline, &existing_coverage);

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
    let plan = DocumentScalePlan::for_capabilities(
        source.unit_count,
        &capabilities,
        &task.reasoning_effort,
        calibration,
    )?;
    let windows = build_partitioned_semantic_windows(
        &chunks,
        plan,
        plan.generation_window_max_tokens,
        plan.generation_window_max_units,
        plan.generation_window_overlap_tokens,
    );
    validate_semantic_windows(&windows, &source)?;

    let estimated_source_tokens = chunks.iter().fold(0_usize, |total, chunk| {
        total.saturating_add(
            chunk
                .token_estimate
                .max(chunk.full_text.chars().count().div_ceil(4)),
        )
    });
    let budget = document_scale::validate_automatic_job_calibrated(
        source.unit_count,
        estimated_source_tokens,
        plan,
        0,
        windows.len(),
        document_scale::DocumentBudgetContext {
            capabilities: &capabilities,
            reasoning_effort: &task.reasoning_effort,
            calibration,
        },
    )?;
    crate::logging::info(
        "document_ai_additional_scale_plan",
        format!(
            "document={} format={} provider={} model={} scale={} units={} source_tokens={} generation_windows={} card_capacity={} estimated_job_tokens={} ceiling={} context={} calibration_samples={}",
            input.document_id,
            source.format.as_str(),
            task.provider_kind,
            task.model_id,
            plan.label(),
            source.unit_count,
            estimated_source_tokens,
            windows.len(),
            budget.generation_card_capacity,
            budget.estimated_tokens,
            budget.ceiling_tokens,
            budget.context_window_tokens.map_or_else(|| "unknown".to_string(), |value| value.to_string()),
            budget.calibration_sample_count,
        ),
    );

    document_store::initialize_additional_progress(
        database,
        &input.document_id,
        plan.label(),
        windows.len(),
    )?;
    let job_id = document_store::create_generation_job(
        database,
        &input.document_id,
        &GenerationJobIdentity::from(&task),
        &source,
        &windows,
    )?;
    let generated_values = document_generation::execute_generation_windows(GenerationExecution {
        database,
        paths,
        runtime,
        task: &task,
        document_id: &input.document_id,
        document_title: &context.title,
        outline: &second_pass_outline,
        plan,
        windows: &windows,
        job_id: &job_id,
        cached_cards: Vec::new(),
        checkpoint_contract: None,
        diagnostic_prefix: "additional",
    })
    .await?;

    if generated_values.is_empty() {
        return Err(CommandError::new(
            "document_no_additional_cards",
            "The second pass did not find important material that was not already tested by the existing deck.",
        ));
    }
    let grounded_candidates = attach_generated_card_source_windows(
        database,
        &job_id,
        &windows,
        generated_values.clone(),
    )?;
    let curated_values = document_intelligence::curate_document_cards(
        runtime,
        &task,
        &second_pass_outline,
        generated_values,
    )
    .await?;
    if curated_values.is_empty() {
        return Err(CommandError::new(
            "document_no_additional_cards",
            "The second-pass relevance check found no additional cards worth adding to the existing deck.",
        ));
    }
    let grounded_cards = retain_curated_card_provenance(grounded_candidates, curated_values)?;

    document_store::mark_job_committing(
        database,
        &input.document_id,
        &job_id,
        grounded_cards.len(),
    )?;
    ensure_exact_append_destination(database, &context.deck_id, &context.deck_name)?;
    let payload = build_append_payload(&context.deck_name, &grounded_cards)?;
    let (api_token_count, api_token_usage_complete) = task.usage_summary();
    let result = import_generated_flashcards_bytes_with_commit_context(
        database,
        payload.as_bytes(),
        |connection, import_result, commit_context, now| {
            if import_result.deck_id != context.deck_id {
                return Err(CommandError::new(
                    "document_generated_deck_changed",
                    "The generated deck changed before additional cards could be committed. No cards were added.",
                ));
            }
            link_imported_notes_to_topics(
                connection,
                &input.document_id,
                &job_id,
                &learning_plan,
                &grounded_cards,
                commit_context,
                now,
            )?;
            let document_changed = connection
                .execute(
                    "UPDATE documents
                     SET lifecycle_state = 'ready',
                         generated_note_count = generated_note_count + ?1,
                         generated_card_count = generated_card_count + ?2,
                         generation_completed_window_count = generation_window_count,
                         safe_error_code = '', safe_error_message = '',
                         processing_duration_ms = CASE
                             WHEN processing_started_at IS NULL THEN NULL
                             ELSE MAX(0, ?3 - processing_started_at)
                         END,
                         api_token_count = ?4, api_token_usage_complete = ?5,
                         updated_at = ?3, completed_at = ?3, revision = revision + 1
                     WHERE id = ?6 AND generated_deck_id = ?7 AND lifecycle_state = 'processing'",
                    params![
                        import_result.created_note_count,
                        import_result.created_card_count,
                        now,
                        api_token_count,
                        if api_token_usage_complete {
                            1_i64
                        } else {
                            0_i64
                        },
                        input.document_id,
                        context.deck_id,
                    ],
                )
                .map_err(|error| {
                    CommandError::database_at(
                        "document_additional_finalize_failed",
                        format!("document={} job={job_id}", input.document_id),
                        error,
                    )
                })?;
            if document_changed != 1 {
                return Err(CommandError::new(
                    "document_changed",
                    "The document changed before additional cards could be committed.",
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
                        context.deck_id,
                        now,
                        job_id,
                        input.document_id,
                    ],
                )
                .map_err(|error| {
                    CommandError::database_at(
                        "document_additional_job_finalize_failed",
                        format!("document={} job={job_id}", input.document_id),
                        error,
                    )
                })?;
            if job_changed != 1 {
                return Err(CommandError::new(
                    "document_job_changed",
                    "The additional generation job changed before its commit completed.",
                ));
            }
            Ok(())
        },
    )?;

    Ok(GenerateMorePdfFlashcardsResultDto {
        document_id: input.document_id,
        deck_id: result.deck_id,
        created_note_count: result.created_note_count,
        created_card_count: result.created_card_count,
        skipped_duplicate_count: result.skipped_duplicate_count,
    })
}

fn load_context(
    database: &Database,
    document_id: &str,
) -> Result<AdditionalDocumentContext, CommandError> {
    database.with_connection(|connection| {
        connection
            .query_row(
                "SELECT document.title, document.source_unit_count,
                        document.source_format, document.source_unit_kind,
                        document.generated_deck_id, document.archived_at,
                        deck.name, deck.deleted_at, deck.archived_at
                 FROM documents document
                 LEFT JOIN decks deck ON deck.id = document.generated_deck_id
                 WHERE document.id = ?1",
                [document_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, Option<i64>>(5)?,
                        row.get::<_, Option<String>>(6)?,
                        row.get::<_, Option<i64>>(7)?,
                        row.get::<_, Option<i64>>(8)?,
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
            .and_then(
                |(
                    title,
                    source_unit_count,
                    source_format,
                    source_unit_kind,
                    deck_id,
                    document_archived,
                    deck_name,
                    deck_deleted,
                    deck_archived,
                )| {
                    if document_archived.is_some() {
                        return Err(CommandError::new(
                            "document_archived",
                            "Restore this document before generating more flashcards from it.",
                        ));
                    }
                    let deck_id = deck_id.ok_or_else(|| {
                        CommandError::new(
                            "document_generated_deck_missing",
                            "This document does not currently have a generated deck to extend.",
                        )
                    })?;
                    if deck_deleted.is_some() || deck_archived.is_some() {
                        return Err(CommandError::new(
                            "document_generated_deck_unavailable",
                            "Restore the generated deck before adding more cards to it.",
                        ));
                    }
                    let deck_name = deck_name.ok_or_else(|| {
                        CommandError::new(
                            "document_generated_deck_missing",
                            "The generated deck is no longer available.",
                        )
                    })?;
                    let source_unit_count = usize::try_from(source_unit_count).map_err(|_| {
                        CommandError::new(
                            "invalid_document_source",
                            "The saved document source-unit count is invalid.",
                        )
                    })?;
                    Ok(AdditionalDocumentContext {
                        title,
                        deck_id,
                        deck_name,
                        source_format,
                        source_unit_kind,
                        source_unit_count,
                    })
                },
            )
    })
}

fn load_outline(
    database: &Database,
    document_id: &str,
) -> Result<document_intelligence::DocumentOutline, CommandError> {
    let json = database.with_connection(|connection| {
        connection
            .query_row(
                "SELECT outline_json FROM document_outlines WHERE document_id = ?1",
                [document_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(CommandError::database)?
            .ok_or_else(|| {
                CommandError::new(
                    "document_outline_missing",
                    "The retained document outline is missing. Retry normal generation to rebuild the document before requesting more cards.",
                )
            })
    })?;
    serde_json::from_str(&json).map_err(|_| {
        CommandError::new(
            "document_outline_invalid",
            "The retained document outline could not be read. Rebuild the document before requesting more cards.",
        )
    })
}

fn load_existing_coverage(
    database: &Database,
    deck_id: &str,
    maximum_chars: usize,
) -> Result<String, CommandError> {
    database.with_connection(|connection| {
        let mut statement = connection
            .prepare(
                "SELECT front_text, back_text
                 FROM notes
                 WHERE deck_id = ?1 AND deleted_at IS NULL
                 ORDER BY created_at, id",
            )
            .map_err(CommandError::database)?;
        let mut summary = String::new();
        let rows = statement
            .query_map([deck_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(CommandError::database)?;
        for row in rows {
            let (front, back) = row.map_err(CommandError::database)?;
            let entry = format!(
                "- Already tested: {} => {}\n",
                compact_text(&front, 360),
                compact_text(&back, 500)
            );
            if summary
                .chars()
                .count()
                .saturating_add(entry.chars().count())
                > maximum_chars
            {
                summary.push_str(
                    "- Additional existing cards omitted from this bounded coverage summary.\n",
                );
                break;
            }
            summary.push_str(&entry);
        }
        if summary.is_empty() {
            summary.push_str("- No readable existing card coverage was found.\n");
        }
        Ok(summary)
    })
}

fn second_pass_outline(
    mut outline: document_intelligence::DocumentOutline,
    existing_coverage: &str,
) -> document_intelligence::DocumentOutline {
    let original_summary = outline.summary.clone();
    outline.summary = format!(
        "SECOND-PASS FLASHCARD CONSTRAINT: The existing generated deck already tests the knowledge listed below. Generate additional cards only for important concepts, mechanisms, distinctions, applications, or relationships that are not already meaningfully tested. Do not create paraphrases of existing questions merely to increase card count.\n\n<ostudy-existing-card-coverage>\n{existing_coverage}</ostudy-existing-card-coverage>\n\nOriginal document summary: {original_summary}"
    );
    outline.learning_objectives.insert(
        0,
        "Second-pass objective: fill important learning-objective coverage gaps that are not already tested by the existing deck; avoid paraphrased duplicates.".to_string(),
    );
    outline
}

fn ensure_exact_append_destination(
    database: &Database,
    expected_deck_id: &str,
    deck_name: &str,
) -> Result<(), CommandError> {
    database.with_connection(|connection| {
        let mut statement = connection
            .prepare(
                "SELECT id FROM decks
                 WHERE name = ?1 AND deleted_at IS NULL AND archived_at IS NULL
                 ORDER BY created_at, id",
            )
            .map_err(CommandError::database)?;
        let ids = statement
            .query_map([deck_name], |row| row.get::<_, String>(0))
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        match ids.as_slice() {
            [only] if only == expected_deck_id => Ok(()),
            [] => Err(CommandError::new(
                "document_generated_deck_missing",
                "The generated deck is no longer available for additional cards.",
            )),
            _ => Err(CommandError::new(
                "document_generated_deck_name_ambiguous",
                "More than one active deck has the generated deck's name. Rename one of those decks before generating more cards so ostudy cannot append to the wrong destination.",
            )),
        }
    })
}

fn build_append_payload(
    deck_name: &str,
    cards: &[SourceGroundedGeneratedCard],
) -> Result<String, CommandError> {
    serde_json::to_string(&serde_json::json!({
        "format": "ostudy-flashcards",
        "formatVersion": 1,
        "deck": {
            "mode": "append",
            "name": deck_name
        },
        "defaultTags": [],
        "cards": cards.iter().map(|card| &card.card).collect::<Vec<_>>()
    }))
    .map_err(CommandError::serialization)
}

fn compact_text(value: &str, maximum: usize) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(maximum)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outline() -> document_intelligence::DocumentOutline {
        document_intelligence::DocumentOutline {
            summary: "Original summary".to_string(),
            learning_objectives: vec!["Understand pulmonary perfusion".to_string()],
            key_concepts: vec!["V/Q matching".to_string()],
            sections: Vec::new(),
        }
    }

    #[test]
    fn second_pass_planning_context_names_existing_coverage_as_a_constraint() {
        let augmented = second_pass_outline(
            outline(),
            "- Already tested: What is V/Q matching? => Relationship of ventilation to perfusion.\n",
        );
        assert!(augmented
            .summary
            .contains("SECOND-PASS FLASHCARD CONSTRAINT"));
        assert!(augmented.summary.contains("Already tested"));
        assert!(augmented.learning_objectives[0].contains("coverage gaps"));
    }

    #[test]
    fn append_payload_keeps_only_card_subject_tags() {
        let payload = build_append_payload(
            "Physiology",
            &[SourceGroundedGeneratedCard {
                knowledge_support: Vec::new(),
                card: serde_json::json!({
                    "type": "basic",
                    "front": "Q",
                    "back": "A",
                    "tags": ["Cardiology"]
                }),
                source_locator: crate::document_domain::SourceLocator::new(
                    crate::document_domain::SourceUnitKind::Page,
                    vec![1],
                    None,
                )
                .unwrap(),
            }],
        )
        .unwrap();
        let value: Value = serde_json::from_str(&payload).unwrap();
        assert_eq!(value["defaultTags"], serde_json::json!([]));
        assert_eq!(value["cards"][0]["tags"], serde_json::json!(["Cardiology"]));
        assert!(!payload.contains("AI generated"));
        assert!(!payload.contains("\"PDF\""));
        assert!(!payload.contains("source_locator"));
    }
}
