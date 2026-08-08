use crate::{document_semantics::SemanticWindow, error::CommandError};

use super::{catalog, endpoint_policy, gateway, profiles, secrets, AiRuntimeState};

#[path = "document_intelligence.rs"]
mod implementation;

pub(crate) use implementation::{
    curate_document_cards, generate_window_flashcards, load_document_task_context,
    DocumentOutline, DocumentTaskContext, SourcePageImage,
};

/// Compatibility entry point for callers that only have structured text.
///
/// The implementation can also accept per-window rendered page evidence; the
/// document domain can opt into that richer path without changing provider or
/// persistence contracts.
pub(crate) async fn build_document_outline(
    runtime: &AiRuntimeState,
    task: &DocumentTaskContext,
    document_title: &str,
    page_count: i64,
    windows: &[SemanticWindow],
) -> Result<DocumentOutline, CommandError> {
    let images = vec![Vec::<SourcePageImage>::new(); windows.len()];
    implementation::build_document_outline(
        runtime,
        task,
        document_title,
        page_count,
        windows,
        &images,
    )
    .await
}

/// Rich outline entry point for image-only or visually important windows.
pub(crate) async fn build_document_outline_with_images(
    runtime: &AiRuntimeState,
    task: &DocumentTaskContext,
    document_title: &str,
    page_count: i64,
    windows: &[SemanticWindow],
    window_images: &[Vec<SourcePageImage>],
) -> Result<DocumentOutline, CommandError> {
    implementation::build_document_outline(
        runtime,
        task,
        document_title,
        page_count,
        windows,
        window_images,
    )
    .await
}
