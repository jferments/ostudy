use crate::{document_semantics::SemanticWindow, error::CommandError};

use super::{catalog, endpoint_policy, gateway, profiles, secrets, AiRuntimeState};

#[path = "document_intelligence.rs"]
mod implementation;

pub(crate) use implementation::{
    curate_document_cards, generate_window_flashcards, load_document_task_context, DocumentOutline,
    DocumentTaskContext, SourcePageImage,
};

const VISUAL_ONLY_MARKER: &str = "has no extractable structured text";

/// Build the reusable global outline used by all later generation windows.
///
/// Text-bearing PDFs use the full hierarchical outline service. Fully visual
/// PDFs deliberately avoid asking a text-only model to invent an outline from
/// placeholder text. When the configured llama.cpp profile has verified image
/// input, generation proceeds with a conservative visual-source outline and the
/// rendered page images provide the actual evidence to each generation window.
pub(crate) async fn build_document_outline(
    runtime: &AiRuntimeState,
    task: &DocumentTaskContext,
    document_title: &str,
    page_count: i64,
    windows: &[SemanticWindow],
) -> Result<DocumentOutline, CommandError> {
    let visual_only = !windows.is_empty()
        && windows
            .iter()
            .all(|window| window.source_text.contains(VISUAL_ONLY_MARKER));

    if visual_only {
        if !task.include_page_images {
            return Err(CommandError::new(
                "document_requires_image_input",
                "This PDF has no extractable text. Choose a verified image-capable llama.cpp flashcard-generation profile and retry.",
            ));
        }
        return Ok(DocumentOutline {
            summary: format!(
                "{document_title} is an image-only PDF. Rendered pages are the authoritative source for flashcard generation."
            ),
            learning_objectives: vec![
                "Identify and understand the important concepts, relationships, diagrams, tables, and equations shown in the rendered document pages."
                    .to_string(),
            ],
            key_concepts: Vec::new(),
            sections: Vec::new(),
        });
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visual_only_marker_matches_extraction_fallback_contract() {
        assert!("Rendered PDF page 2 has no extractable structured text."
            .contains(VISUAL_ONLY_MARKER));
    }
}
