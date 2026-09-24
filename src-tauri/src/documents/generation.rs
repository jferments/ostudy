use rusqlite::OptionalExtension;

use crate::{
    ai::AiRuntimeState,
    database::Database,
    document_card_topics::report,
    document_initial_generation::{
        process_and_generate_initial_flashcards, process_reserved_initial_flashcards,
    },
    error::CommandError,
    paths::AppPaths,
    pdf_render_backend::PdfRenderRuntime,
    util::validate_entity_id,
};

use super::{
    contracts::{DocumentGenerationResultDto, RetryPdfGenerationInput},
    lifecycle, repository,
};

pub(crate) fn validate_retry_pdf_flashcard_generation(
    database: &Database,
    document_id: &str,
) -> Result<(), CommandError> {
    validate_entity_id(document_id, "document")?;
    let document = database
        .with_connection(|connection| repository::load_document(connection, document_id))?;
    if document.dto.generated_deck_id.is_some() {
        return Err(CommandError::new(
            "document_already_generated",
            "This document already has a generated deck. Delete that deck before generating it again.",
        ));
    }
    Ok(())
}

fn committed_initial_generation(
    database: &Database,
    document_id: &str,
) -> Result<Option<DocumentGenerationResultDto>, CommandError> {
    let document =
        database.with_connection(|connection| repository::load_document(connection, document_id))?;
    let Some(deck_id) = document.dto.generated_deck_id.clone() else {
        return Ok(None);
    };
    database.with_connection(|connection| {
        let job_id: Option<String> = connection
            .query_row(
                "SELECT provenance.job_id
                 FROM document_decks provenance
                 JOIN document_generation_jobs job ON job.id = provenance.job_id
                 WHERE provenance.document_id = ?1
                   AND provenance.deck_id = ?2
                   AND job.document_id = provenance.document_id
                   AND job.generated_deck_id = provenance.deck_id
                   AND job.state = 'succeeded'
                 ORDER BY provenance.created_at ASC, provenance.job_id ASC
                 LIMIT 1",
                rusqlite::params![document_id, deck_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(CommandError::database)?;
        let Some(job_id) = job_id else {
            return Ok(None);
        };
        let Some(summary) = report::load_for_job(connection, document_id, &job_id)? else {
            return Ok(None);
        };
        let count = |value: usize| {
            i64::try_from(value).map_err(|_| {
                CommandError::new(
                    "generation_report_invalid",
                    "The committed generation summary exceeds the supported count range.",
                )
            })
        };
        Ok(Some(DocumentGenerationResultDto {
            deck_id,
            created_note_count: count(summary.saved_note_count)?,
            created_card_count: count(summary.saved_card_count)?,
            skipped_duplicate_count: count(summary.skipped_duplicate_count)?,
            document: document.dto,
        }))
    })
}

pub(crate) async fn retry_pdf_flashcard_generation(
    database: &Database,
    paths: &AppPaths,
    runtime: &AiRuntimeState,
    render_runtime: PdfRenderRuntime,
    input: RetryPdfGenerationInput,
) -> Result<DocumentGenerationResultDto, CommandError> {
    retry_pdf_flashcard_generation_inner(database, paths, runtime, render_runtime, input, false)
        .await
}

pub(crate) async fn retry_reserved_pdf_flashcard_generation(
    database: &Database,
    paths: &AppPaths,
    runtime: &AiRuntimeState,
    render_runtime: PdfRenderRuntime,
    input: RetryPdfGenerationInput,
) -> Result<DocumentGenerationResultDto, CommandError> {
    retry_pdf_flashcard_generation_inner(database, paths, runtime, render_runtime, input, true)
        .await
}

async fn retry_pdf_flashcard_generation_inner(
    database: &Database,
    paths: &AppPaths,
    runtime: &AiRuntimeState,
    render_runtime: PdfRenderRuntime,
    input: RetryPdfGenerationInput,
    processing_reserved: bool,
) -> Result<DocumentGenerationResultDto, CommandError> {
    crate::ai::generation::outcomes::operation::bind_document(database);
    if let Some(result) = committed_initial_generation(database, &input.document_id)? {
        crate::logging::info_target(
            "documents.generation",
            "document_generation_commit_reused",
            "Recovered an already committed flashcard result without another provider request.",
            serde_json::json!({
                "document": input.document_id.as_str(),
                "deck": result.deck_id.as_str(),
            }),
        );
        return Ok(result);
    }
    validate_retry_pdf_flashcard_generation(database, &input.document_id)?;

    let result = if processing_reserved {
        process_reserved_initial_flashcards(
            database,
            paths,
            runtime,
            render_runtime,
            &input.document_id,
        )
        .await
    } else {
        process_and_generate_initial_flashcards(
            database,
            paths,
            runtime,
            render_runtime,
            &input.document_id,
        )
        .await
    };
    let result = match result {
        Ok(result) => result,
        Err(error) => {
            if error.code == "ai_request_canceled" {
                lifecycle::record_document_cancellation(database, &input.document_id);
            } else {
                let progress = crate::task_cancellation::document_task_progress(&input.document_id);
                crate::logging::error_target(
                    "documents.generation",
                    "document_generation_failed",
                    "A document learning task ended with a handled error; content was not written to diagnostics.",
                    serde_json::json!({
                        "document": input.document_id.as_str(),
                        "stage": failure_stage(&error),
                        "progressPhase": progress.as_ref().map(|value| value.phase.as_str()),
                        "code": error.code.as_str(),
                        "detail": error.message.as_str(),
                        "providerRequestCount": progress.as_ref().map(|value| value.provider_request_count),
                        "retryCount": progress.as_ref().map(|value| value.retry_count),
                        "totalTokens": progress.as_ref().map(|value| value.total_tokens),
                        "reasoningTokens": progress.as_ref().map(|value| value.reasoning_tokens),
                    }),
                );
                lifecycle::record_document_failure(database, &input.document_id, &error);
            }
            return Err(error);
        }
    };
    let document = database
        .with_connection(|connection| repository::load_document(connection, &input.document_id))?
        .dto;
    Ok(DocumentGenerationResultDto {
        document,
        deck_id: result.deck_id,
        created_note_count: result.created_note_count,
        created_card_count: result.created_card_count,
        skipped_duplicate_count: result.skipped_duplicate_count,
    })
}

fn failure_stage(error: &CommandError) -> &'static str {
    match error.code.as_str() {
        "invalid_semantic_content"
        | "semantic_images_require_vision"
        | "semantic_source_changed" => "knowledge_extraction",
        code if code.starts_with("document_outline_") => "outline",
        code if code.starts_with("document_processing_")
            || code.starts_with("document_extraction_")
            || code.starts_with("document_render_")
            || code.starts_with("document_source_") =>
        {
            "ingestion"
        }
        code if code.starts_with("ai_") => "ai_request",
        _ => "document_generation",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn insert_active_deck(
        connection: &rusqlite::Connection,
        deck_id: &str,
    ) -> Result<(), CommandError> {
        connection
            .execute(
                "INSERT INTO decks (
                     id, name, description, sort_order, created_at, updated_at, revision
                 ) VALUES (?1, 'Generated', '', 1, 1, 1, 1)",
                [deck_id],
            )
            .map_err(CommandError::database)?;
        Ok(())
    }

    #[test]
    fn committed_initial_generation_is_reused_without_another_generation_pass() {
        let database = Database::open_in_memory().unwrap();
        let document_id = "a".repeat(32);
        let deck_id = "b".repeat(32);
        let job_id = "c".repeat(32);
        let policy = crate::knowledge::assessment::policy::GenerationPolicy::default();
        let report = crate::document_card_topics::report::GenerationReport {
            schema_version: 1,
            generated_at: 2,
            requested_policy: policy.clone(),
            candidate_note_count: 3,
            selected_note_count: 2,
            curation_removed_count: 1,
            saved_note_count: 2,
            saved_card_count: 2,
            skipped_duplicate_count: 0,
            available_knowledge_units: 2,
            linked_knowledge_units: 1,
            evidence_binding_count: 2,
            item_count: 2,
            types: vec![crate::document_card_topics::report::GenerationTypeSummary {
                family: crate::knowledge::assessment::policy::QuestionFamily::Recall,
                note_count: 2,
                card_count: 2,
                item_count: 2,
            }],
        };
        database
            .with_connection(|connection| {
                insert_active_deck(connection, &deck_id)?;
                connection
                    .execute(
                        "INSERT INTO documents (
                             id, title, original_filename, mime_type, byte_size, sha256,
                             source_relative_path, lifecycle_state, generated_note_count,
                             generated_card_count, generated_deck_id, created_at, updated_at
                         ) VALUES (?1, 'Document', 'source.pdf', 'application/pdf', 1,
                                   ?2, 'documents/source/source.pdf', 'ready', 2, 2, ?3, 1, 2)",
                        rusqlite::params![document_id, "d".repeat(64), deck_id],
                    )
                    .map_err(CommandError::database)?;
                connection
                    .execute(
                        "INSERT INTO document_generation_jobs (
                             id, document_id, provider_kind, model_id, profile_revision,
                             credential_epoch, network_scope, state, page_count,
                             processed_page_count, generated_candidate_count,
                             generated_deck_id, created_at, updated_at, completed_at
                         ) VALUES (?1, ?2, 'openai_compatible', 'fixture', 1,
                                   0, 'local', 'succeeded', 1, 1, 2, ?3, 1, 2, 2)",
                        rusqlite::params![job_id, document_id, deck_id],
                    )
                    .map_err(CommandError::database)?;
                connection
                    .execute(
                        "INSERT INTO document_decks (document_id, deck_id, job_id, created_at)
                         VALUES (?1, ?2, ?3, 2)",
                        rusqlite::params![document_id, deck_id, job_id],
                    )
                    .map_err(CommandError::database)?;
                connection
                    .execute(
                        "INSERT INTO question_generation_reports (
                             job_id, document_id, policy_json, available_knowledge_units,
                             candidate_count, report_json, created_at
                         ) VALUES (?1, ?2, ?3, 2, 3, ?4, 2)",
                        rusqlite::params![
                            job_id,
                            document_id,
                            serde_json::to_string(&policy).unwrap(),
                            serde_json::to_string(&report).unwrap()
                        ],
                    )
                    .map_err(CommandError::database)?;
                Ok(())
            })
            .unwrap();

        let recovered = committed_initial_generation(&database, &document_id)
            .unwrap()
            .unwrap();
        assert_eq!(recovered.deck_id, deck_id);
        assert_eq!(recovered.created_note_count, 2);
        assert_eq!(recovered.created_card_count, 2);
        assert_eq!(recovered.skipped_duplicate_count, 0);
        assert_eq!(recovered.document.lifecycle_state, "ready");
    }

    #[test]
    fn committed_deck_without_matching_success_receipt_is_not_guessed() {
        let database = Database::open_in_memory().unwrap();
        let document_id = "a".repeat(32);
        let deck_id = "b".repeat(32);
        database
            .with_connection(|connection| {
                insert_active_deck(connection, &deck_id)?;
                connection
                    .execute(
                        "INSERT INTO documents (
                             id, title, original_filename, mime_type, byte_size, sha256,
                             source_relative_path, lifecycle_state, generated_deck_id,
                             created_at, updated_at
                         ) VALUES (?1, 'Document', 'source.pdf', 'application/pdf', 1,
                                   ?2, 'documents/source/source.pdf', 'ready', ?3, 1, 1)",
                        rusqlite::params![document_id, "d".repeat(64), deck_id],
                    )
                    .map_err(CommandError::database)?;
                Ok(())
            })
            .unwrap();
        assert!(committed_initial_generation(&database, &document_id)
            .unwrap()
            .is_none());
        assert_eq!(
            validate_retry_pdf_flashcard_generation(&database, &document_id)
                .unwrap_err()
                .code,
            "document_already_generated"
        );
    }

    #[test]
    fn handled_failures_are_classified_without_inspecting_content() {
        assert_eq!(
            failure_stage(&CommandError::new(
                "invalid_semantic_content",
                "The knowledge excerpt does not occur in its source."
            )),
            "knowledge_extraction"
        );
        assert_eq!(
            failure_stage(&CommandError::new("document_outline_invalid", "outline")),
            "outline"
        );
        assert_eq!(
            failure_stage(&CommandError::new("ai_http_bad_request", "provider")),
            "ai_request"
        );
    }
}
