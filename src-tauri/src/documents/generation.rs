use crate::{
    ai::AiRuntimeState,
    database::Database,
    document_initial_generation::process_and_generate_initial_flashcards,
    error::CommandError,
    paths::AppPaths,
    pdf_render_backend::PdfRenderRuntime,
    util::validate_entity_id,
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
    let document =
        database.with_connection(|connection| repository::load_document(connection, &input.document_id))?;
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
