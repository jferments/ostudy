use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{ai::AiRuntimeState, error::CommandError};

use super::{
    context::DocumentTaskContext,
    contracts::DocumentOutline,
    support::{bounded_diagnostic, output_error},
    transport::{request_json, JsonTaskRequest},
};

pub(crate) const ARTIFACT_METADATA_GENERATOR_VERSION: i64 = 1;
pub(crate) const ARTIFACT_METADATA_PROMPT_VERSION: i64 = 1;
pub(crate) const ARTIFACT_METADATA_SCHEMA_VERSION: i64 = 1;
pub(crate) const ARTIFACT_TITLE_MAX_CHARS: usize = 120;
pub(crate) const ARTIFACT_DESCRIPTION_MAX_CHARS: usize = 800;

const ARTIFACT_METADATA_VISIBLE_COMPLETION_TOKENS: usize = 600;
const ARTIFACT_METADATA_CONTEXT_MAX_CHARS: usize = 16_000;

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct DocumentArtifactMetadataCandidate {
    pub document_title: String,
    pub deck_title: String,
    pub deck_description: String,
}

/// Request user-facing artifact metadata from the same validated planning
/// context consumed by every document learning artifact.
///
/// This is a versioned structured domain task, not a UI follow-up prompt. The
/// request deliberately excludes the original filename so a provider cannot
/// turn source identity into the display title by imitation.
pub(super) async fn request_document_artifact_metadata(
    runtime: &AiRuntimeState,
    task: &DocumentTaskContext,
    outline: &DocumentOutline,
) -> Result<DocumentArtifactMetadataCandidate, CommandError> {
    let schema = artifact_metadata_schema();
    let system = artifact_metadata_system_prompt();
    let user = format!(
        "<ostudy-validated-learning-plan>\n{}\n</ostudy-validated-learning-plan>\n\nReturn concise subject-centered display metadata for this learning material.",
        bounded_learning_plan_context(outline)?,
    );
    request_json(
        runtime,
        task,
        JsonTaskRequest {
            system_prompt: &system,
            user_text: &user,
            schema: &schema,
            max_tokens: task.completion_token_ceiling(ARTIFACT_METADATA_VISIBLE_COMPLETION_TOKENS),
            images: &[],
        },
    )
    .await
    .and_then(decode_artifact_metadata)
}

fn artifact_metadata_system_prompt() -> String {
    format!(
        "You are ostudy's learning-artifact metadata planner. Use only the supplied validated whole-document learning plan. Return one concise subject-centered document title, one concise subject-centered flashcard-deck title, and one educational deck description. Do not mention AI, generation, prompts, source files, filenames, file formats, uploads, or phrases such as 'generated from' or 'AI-generated'. Do not invent facts or copy a long summary into a title. Titles must contain 1–{ARTIFACT_TITLE_MAX_CHARS} characters and the description 1–{ARTIFACT_DESCRIPTION_MAX_CHARS} characters. Return only the required structured result. Metadata schema version: {ARTIFACT_METADATA_SCHEMA_VERSION}. Metadata prompt version: {ARTIFACT_METADATA_PROMPT_VERSION}."
    )
}

fn artifact_metadata_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["documentTitle", "deckTitle", "deckDescription"],
        "properties": {
            "documentTitle": {
                "type": "string",
                "minLength": 1,
                "maxLength": ARTIFACT_TITLE_MAX_CHARS
            },
            "deckTitle": {
                "type": "string",
                "minLength": 1,
                "maxLength": ARTIFACT_TITLE_MAX_CHARS
            },
            "deckDescription": {
                "type": "string",
                "minLength": 1,
                "maxLength": ARTIFACT_DESCRIPTION_MAX_CHARS
            }
        }
    })
}

fn bounded_learning_plan_context(outline: &DocumentOutline) -> Result<String, CommandError> {
    let sections = outline
        .sections
        .iter()
        .take(24)
        .map(|section| {
            json!({
                "title": truncate_chars(&section.title, 180),
                "summary": truncate_chars(&section.summary, 420),
                "learningObjectives": section.learning_objectives.iter().take(3).collect::<Vec<_>>(),
                "keyConcepts": section.key_concepts.iter().take(5).collect::<Vec<_>>(),
            })
        })
        .collect::<Vec<_>>();
    let encoded = serde_json::to_string(&json!({
        "summary": truncate_chars(&outline.summary, 3_000),
        "learningObjectives": outline.learning_objectives.iter().take(20).collect::<Vec<_>>(),
        "keyConcepts": outline.key_concepts.iter().take(32).collect::<Vec<_>>(),
        "sections": sections,
    }))
    .map_err(CommandError::serialization)?;
    Ok(truncate_chars(
        &encoded,
        ARTIFACT_METADATA_CONTEXT_MAX_CHARS,
    ))
}

fn decode_artifact_metadata(
    value: Value,
) -> Result<DocumentArtifactMetadataCandidate, CommandError> {
    serde_json::from_value(value).map_err(|error| {
        output_error(format!(
            "The provider returned artifact metadata that did not match the required contract: {}",
            bounded_diagnostic(&error.to_string(), 320),
        ))
    })
}

fn truncate_chars(value: &str, maximum: usize) -> String {
    value.chars().take(maximum).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_schema_is_closed_and_explicitly_bounded() {
        let schema = artifact_metadata_schema();
        assert_eq!(schema["additionalProperties"], json!(false));
        assert_eq!(
            schema["properties"]["documentTitle"]["maxLength"],
            json!(ARTIFACT_TITLE_MAX_CHARS),
        );
        assert_eq!(
            schema["properties"]["deckDescription"]["maxLength"],
            json!(ARTIFACT_DESCRIPTION_MAX_CHARS),
        );
    }

    #[test]
    fn prompt_forbids_generator_and_source_boilerplate() {
        let prompt = artifact_metadata_system_prompt().to_ascii_lowercase();
        assert!(prompt.contains("subject-centered"));
        assert!(prompt.contains("ai-generated"));
        assert!(prompt.contains("generated from"));
        assert!(prompt.contains("filenames"));
    }

    #[test]
    fn decoder_rejects_unknown_metadata_fields() {
        let error = decode_artifact_metadata(json!({
            "documentTitle": "Shock physiology",
            "deckTitle": "Shock response",
            "deckDescription": "Connect hemodynamics to bedside findings.",
            "filename": "shock.pdf"
        }))
        .expect_err("unknown fields must fail closed");
        assert_eq!(error.code, "ai_document_output_invalid");
    }
}
