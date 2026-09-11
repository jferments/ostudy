//! Provider-neutral artifact authoring over shared, native-grounded knowledge.

use crate::error::CommandError;

use super::AiRuntimeState;

mod artifact_metadata;
mod context;
mod contracts;
mod curation;
mod evidence;
mod flashcards;
mod outline;
mod schemas;
mod study_guide;
mod support;
mod transport;

pub(crate) use artifact_metadata::{
    DocumentArtifactMetadataCandidate, ARTIFACT_DESCRIPTION_MAX_CHARS,
    ARTIFACT_METADATA_GENERATOR_VERSION, ARTIFACT_METADATA_PROMPT_VERSION,
    ARTIFACT_METADATA_SCHEMA_VERSION, ARTIFACT_TITLE_MAX_CHARS,
};
pub(crate) use context::{
    load_document_task_context, load_tutor_task_context, DocumentTaskContext,
};
pub(crate) use contracts::{DocumentOutline, OutlineSection, SourcePageImage};
pub(crate) use curation::curate_document_cards;
pub(crate) use evidence::AuthoringEvidence;
pub(crate) use flashcards::{
    flashcard_completion_token_ceiling, generate_window_flashcards, FlashcardGenerationLimits,
    FLASHCARD_PROMPT_VERSION, FLASHCARD_SCHEMA_VERSION,
};
pub(crate) use outline::{OUTLINE_PROMPT_VERSION, OUTLINE_SCHEMA_VERSION};
pub(crate) use study_guide::generate_study_guide_topic_section;
pub(crate) use transport::{request_json_cancellable, JsonTaskRequest};

/// Generate optional subject-centered metadata from the validated global
/// learning outline. Callers treat failure as recoverable and apply the
/// deterministic local fallback from the same outline.
pub(crate) async fn generate_document_artifact_metadata(
    runtime: &AiRuntimeState,
    task: &DocumentTaskContext,
    outline: &DocumentOutline,
) -> Result<DocumentArtifactMetadataCandidate, CommandError> {
    artifact_metadata::request_document_artifact_metadata(runtime, task, outline).await
}
