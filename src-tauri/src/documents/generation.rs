use crate::{
    ai::AiRuntimeState, database::Database,
    document_initial_generation::process_and_generate_initial_flashcards, error::CommandError,
    paths::AppPaths, pdf_render_backend::PdfRenderRuntime, util::validate_entity_id,
};

use super::{
    contracts::{DocumentGenerationResultDto, RetryPdfGenerationInput},
    lifecycle, repository,
};

pub(crate) async fn retry_pdf_flashcard_generation(
    database: &Database,
    paths: &AppPaths,
    runtime: &AiRuntimeState,
    render_runtime: PdfRenderRuntime,
    input: RetryPdfGenerationInput,
) -> Result<DocumentGenerationResultDto, CommandError> {
    validate_entity_id(&input.document_id, "document")?;
    let document = database
        .with_connection(|connection| repository::load_document(connection, &input.document_id))?;
    if document.dto.generated_deck_id.is_some() {
        return Err(CommandError::new(
            "document_already_generated",
            "This document already has a generated deck. Delete that deck before generating it again.",
        ));
    }

    let result = process_and_generate_initial_flashcards(
        database,
        paths,
        runtime,
        render_runtime,
        &input.document_id,
    )
    .await;
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
        "invalid_semantic_content" | "semantic_images_require_vision" | "semantic_source_changed" => {
            "knowledge_extraction"
        }
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
