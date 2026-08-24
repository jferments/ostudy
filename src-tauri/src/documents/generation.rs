use tauri::State;

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
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    runtime: State<'_, AiRuntimeState>,
    render_runtime: State<'_, PdfRenderRuntime>,
    input: RetryPdfGenerationInput,
) -> Result<DocumentGenerationResultDto, CommandError> {
    validate_entity_id(&input.document_id, "document")?;
    let document = database
        .with_connection(|connection| repository::load_document(connection, &input.document_id))?;
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
        database.inner(),
        paths.inner(),
        runtime.inner(),
        render_runtime.inner().clone(),
        &input.document_id,
    )
    .await;
    let result = match result {
        Ok(result) => result,
        Err(error) => {
            lifecycle::record_document_failure(database.inner(), &input.document_id, &error);
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
