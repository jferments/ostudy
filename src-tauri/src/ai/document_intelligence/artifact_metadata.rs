use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{ai::AiRuntimeState, error::CommandError, task_cancellation::CancellationSignal};

use super::{
    context::DocumentTaskContext,
    contracts::DocumentOutline,
    support::{bounded_diagnostic, output_error},
    transport::{request_json_cancellable, JsonTaskRequest},
};

pub(crate) const ARTIFACT_METADATA_GENERATOR_VERSION: i64 = 4;
pub(crate) const ARTIFACT_METADATA_PROMPT_VERSION: i64 = 4;
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
    cancellation: &CancellationSignal,
) -> Result<DocumentArtifactMetadataCandidate, CommandError> {
    let schema = artifact_metadata_schema();
    let system = artifact_metadata_system_prompt();
    let user = format!(
        "<ostudy-validated-learning-plan>\n{}\n</ostudy-validated-learning-plan>\n\nReturn concise subject-centered display metadata for this learning material. Inspect the full represented source range before choosing a subject. A section title describes that section, not the whole source. When multiple subjects are present, do not promote the first, most detailed or most frequently repeated narrow label into an umbrella title. The sections are [firstSourceUnit, lastSourceUnit, title] tuples. A sampled outline is explicitly marked; omitted detail is not evidence that the source only covers the visible subtopic.",
        bounded_learning_plan_context(outline)?,
    );
    request_json_cancellable(
        runtime,
        task,
        JsonTaskRequest {
            system_prompt: &system,
            user_text: &user,
            schema: &schema,
            max_tokens: task.completion_token_ceiling(ARTIFACT_METADATA_VISIBLE_COMPLETION_TOKENS),
            images: &[],
        },
        cancellation,
    )
    .await
    .and_then(decode_artifact_metadata)
}

fn artifact_metadata_system_prompt() -> String {
    format!(
        "You are ostudy's learning-artifact metadata planner. Use only the supplied validated whole-document learning plan. Return one concise human-readable umbrella title for the document, one concise human-readable umbrella title for its flashcard deck, and one educational deck description. Titles describe the BROADEST DOCUMENT-LEVEL SUBJECT, not one interesting subsection, example, disease, nutrient, mechanism, or late/early source window. When the plan spans several major areas, choose the common course/chapter subject that contains all of them. A title should normally be a 2–10 word noun phrase in sentence case, such as 'Gastrointestinal physiology' or 'Cardiovascular pressure and flow'. NEVER return a slash-separated, semicolon-separated, comma-list, keyword inventory, section inventory, or filename-like title. Do not mention AI, generation, prompts, source files, filenames, file formats, uploads, or phrases such as 'generated from' or 'AI-generated'. Do not invent facts or copy a long summary into a title. Titles must contain 1–{ARTIFACT_TITLE_MAX_CHARS} characters and the description 1–{ARTIFACT_DESCRIPTION_MAX_CHARS} characters. Return only the required structured result. Metadata schema version: {ARTIFACT_METADATA_SCHEMA_VERSION}. Metadata prompt version: {ARTIFACT_METADATA_PROMPT_VERSION}."
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
    let indices = representative_indices(outline.sections.len(), 96);
    // Compact fields BEFORE serialization. Cutting the encoded JSON used to
    // produce malformed context and silently omit the later document sections.
    for (title_chars, summary_chars, objectives) in
        [(96, 1_200, 8), (64, 600, 4), (40, 240, 2), (24, 80, 0)]
    {
        let sections = indices
            .iter()
            .map(|index| {
                let section = &outline.sections[*index];
                json!([
                    section.page_start,
                    section.page_end,
                    truncate_chars(&section.title, title_chars)
                ])
            })
            .collect::<Vec<_>>();
        let encoded = serde_json::to_string(&json!({
            "summary": truncate_chars(&outline.summary, summary_chars),
            "totalSections": outline.sections.len(),
            "representedSections": indices.len(),
            "sectionSelection": if indices.len() == outline.sections.len() { "all" } else { "evenly distributed including first and last" },
            "learningObjectives": representative_indices(outline.learning_objectives.len(), objectives)
                .iter().map(|index| truncate_chars(&outline.learning_objectives[*index], title_chars)).collect::<Vec<_>>(),
            "keyConcepts": representative_indices(outline.key_concepts.len(), 12)
                .iter().map(|index| truncate_chars(&outline.key_concepts[*index], title_chars)).collect::<Vec<_>>(),
            "sections": sections,
        })).map_err(CommandError::serialization)?;
        if encoded.chars().count() <= ARTIFACT_METADATA_CONTEXT_MAX_CHARS {
            return Ok(encoded);
        }
    }
    Err(CommandError::new(
        "artifact_metadata_context_too_large",
        "Display metadata context could not be bounded safely; source-level fallback metadata remains available.",
    ))
}

fn representative_indices(total: usize, maximum: usize) -> Vec<usize> {
    let count = total.min(maximum);
    if count < 2 {
        return (0..count).collect();
    }
    (0..count)
        .map(|index| index * (total - 1) / (count - 1))
        .collect()
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
    fn prompt_requires_an_umbrella_title_instead_of_a_keyword_inventory() {
        let prompt = artifact_metadata_system_prompt();
        assert!(prompt.contains("Gastrointestinal physiology"));
        assert!(prompt.contains("BROADEST DOCUMENT-LEVEL SUBJECT"));
        assert!(prompt.contains("NEVER return a slash-separated"));
        assert!(prompt.contains("2–10 word noun phrase"));
    }

    #[test]
    fn prompt_forbids_generator_and_source_boilerplate() {
        let prompt = artifact_metadata_system_prompt().to_ascii_lowercase();
        assert!(prompt.contains("human-readable umbrella title"));
        assert!(prompt.contains("ai-generated"));
        assert!(prompt.contains("generated from"));
        assert!(prompt.contains("filenames"));
    }

    #[test]
    fn document_metadata_context_is_valid_json_and_represents_late_sections() {
        for count in [0, 1, 14, 96, 160, 256] {
            let outline = DocumentOutline {
                summary: "\"\\\nα".repeat(2_000),
                learning_objectives: vec!["A broad learning objective".into(); 32],
                key_concepts: vec!["A source-supported concept".into(); 50],
                sections: (0..count)
                    .map(|index| super::super::contracts::OutlineSection {
                        title: format!("Subject {index} {}", "quoted \"material\" ".repeat(40)),
                        page_start: index as i64 + 1,
                        page_end: index as i64 + 1,
                        summary: "long detail".repeat(500),
                        learning_objectives: vec![],
                        key_concepts: vec![],
                    })
                    .collect(),
            };
            let encoded = bounded_learning_plan_context(&outline).unwrap();
            assert!(encoded.chars().count() <= ARTIFACT_METADATA_CONTEXT_MAX_CHARS);
            let decoded: Value = serde_json::from_str(&encoded).unwrap();
            assert_eq!(decoded["totalSections"], count);
            let sections = decoded["sections"].as_array().unwrap();
            assert_eq!(sections.len(), count.min(96));
            if count > 0 {
                assert_eq!(sections.first().unwrap()[0], 1);
                assert_eq!(sections.last().unwrap()[1], count);
            }
            assert_eq!(
                decoded["sectionSelection"],
                if count <= 96 {
                    "all"
                } else {
                    "evenly distributed including first and last"
                }
            );
        }
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
