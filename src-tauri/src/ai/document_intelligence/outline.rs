//! Bounded projections of the knowledge-derived outline for artifact authors.
//! Canonical source analysis lives in knowledge; this module makes no requests.
use super::contracts::DocumentOutline;
use crate::{document_semantics::SemanticWindow, error::CommandError};
use serde_json::json;

pub(crate) const OUTLINE_PROMPT_VERSION: i64 = 8;
pub(crate) const OUTLINE_SCHEMA_VERSION: i64 = 3;

pub(super) fn outline_context_for_window(
    outline: &DocumentOutline,
    window: &SemanticWindow,
) -> Result<String, CommandError> {
    let locator = window.source_locator();
    let mut selected = outline
        .sections
        .iter()
        .filter(|section| {
            section.page_start <= locator.last() && section.page_end >= locator.first()
        })
        .take(12)
        .cloned()
        .collect::<Vec<_>>();
    if selected.is_empty() {
        if let Some(nearest) = outline.sections.iter().min_by_key(|section| {
            if section.page_end < locator.first() {
                locator.first().saturating_sub(section.page_end)
            } else if section.page_start > locator.last() {
                section.page_start.saturating_sub(locator.last())
            } else {
                0
            }
        }) {
            selected.push(nearest.clone());
        }
    }
    serde_json::to_string(&json!({
        "summary": truncate_chars(&outline.summary, 2_000),
        "learningObjectives": outline.learning_objectives.iter().take(20).collect::<Vec<_>>(),
        "keyConcepts": outline.key_concepts.iter().take(32).collect::<Vec<_>>(),
        "sections": selected,
    }))
    .map_err(CommandError::serialization)
}

pub(super) fn outline_context_for_curation(
    outline: &DocumentOutline,
) -> Result<String, CommandError> {
    serde_json::to_string(&json!({
        "summary": truncate_chars(&outline.summary, 3_000),
        "learningObjectives": outline.learning_objectives.iter().take(32).collect::<Vec<_>>(),
        "keyConcepts": outline.key_concepts.iter().take(48).collect::<Vec<_>>(),
        "sections": outline.sections.iter().take(40).collect::<Vec<_>>(),
    }))
    .map_err(CommandError::serialization)
}

fn truncate_chars(value: &str, maximum_chars: usize) -> String {
    value.chars().take(maximum_chars).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compact_projection_is_valid_json_without_reinterpreting_source() {
        let outline = DocumentOutline {
            summary: "Geometry".into(),
            learning_objectives: vec![],
            key_concepts: vec!["Triangle".into()],
            sections: vec![],
        };
        let value: serde_json::Value =
            serde_json::from_str(&outline_context_for_curation(&outline).unwrap()).unwrap();
        assert_eq!(value["summary"], "Geometry");
        assert_eq!(value["keyConcepts"][0], "Triangle");
    }
}
