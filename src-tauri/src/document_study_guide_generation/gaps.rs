//! A topic the model could not write becomes a marked gap, not a failed guide.
//!
//! The gap section is native: it lists the topic's extracted knowledge units,
//! which already passed evidence validation, and says plainly that the AI did
//! not write this part. It is never checkpointed, so the next generation of
//! the guide asks the model for this topic again.

use crate::{
    document_learning::LearningTopic,
    document_study_guides::{
        validate_study_guide_section_content, GroundedStudyGuideSection, StudyGuideCallout,
        StudyGuideCalloutKind, StudyGuideExplanation, StudyGuideSectionContent,
    },
    error::CommandError,
    knowledge::authoring::KnowledgeCatalog,
};

const MAX_GAP_EXPLANATIONS: usize = 48;
/// A guide section's source links at most this many knowledge units
/// (`repository::link_artifact_units` and `linked_units` share the bound).
pub(super) const MAX_SECTION_SUPPORT: usize = 64;

/// Keep at most `MAX_SECTION_SUPPORT` items, preferring the document's core
/// units, in their original order. A topic split into halves, or listed as a
/// gap, can otherwise cite more units than one section may link.
pub(super) fn bounded<T>(
    items: Vec<T>,
    catalog: &KnowledgeCatalog,
    id: impl Fn(&T) -> &str,
) -> Vec<T> {
    if items.len() <= MAX_SECTION_SUPPORT {
        return items;
    }
    let core = items
        .iter()
        .filter(|item| catalog.document_core_units.contains(id(item)))
        .count()
        .min(MAX_SECTION_SUPPORT);
    let mut other_room = MAX_SECTION_SUPPORT - core;
    let mut core_room = core;
    let total = items.len();
    let kept = items
        .into_iter()
        .filter(|item| {
            if catalog.document_core_units.contains(id(item)) {
                core_room
                    .checked_sub(1)
                    .map(|left| core_room = left)
                    .is_some()
            } else {
                other_room
                    .checked_sub(1)
                    .map(|left| other_room = left)
                    .is_some()
            }
        })
        .collect::<Vec<_>>();
    crate::logging::debug_target(
        "ai.document.study_guide",
        "study_guide_section_support_bounded",
        "Kept the section's knowledge links within the per-section bound.",
        serde_json::json!({"selected": total, "kept": kept.len()}),
    );
    kept
}

/// A topic-local failure: a model output problem, or a topic the knowledge
/// pass left empty. Anything else still stops the guide.
pub(super) fn recoverable(error: &CommandError) -> bool {
    crate::ai::document_intelligence::is_output_failure(error)
        || matches!(
            error.code.as_str(),
            "document_study_guide_invalid"
                | "document_study_guide_incomplete"
                | "study_guide_topic_has_no_knowledge"
                | "study_guide_topic_has_no_evidence"
        )
}

pub(super) fn section(
    topic: &LearningTopic,
    catalog: &KnowledgeCatalog,
) -> Result<GroundedStudyGuideSection, CommandError> {
    let summary = topic.summary.trim();
    let overview = if summary.split_whitespace().count() >= 5 {
        summary.to_string()
    } else {
        format!("This part of the source covers {}.", topic.title.trim())
    };
    let units = bounded(catalog.units.iter().collect(), catalog, |unit| {
        unit.id.as_str()
    });
    let mut explanations = Vec::new();
    let mut key_points = Vec::new();
    for unit in &units {
        let mut body = unit.content.statement.trim().to_string();
        for qualification in &unit.content.qualifications {
            let qualification = qualification.trim();
            if !qualification.is_empty() {
                body.push(' ');
                body.push_str(qualification);
            }
        }
        if body.is_empty() {
            continue;
        }
        if explanations.len() < MAX_GAP_EXPLANATIONS {
            explanations.push(StudyGuideExplanation {
                heading: unit.content.title.trim().to_string(),
                body,
            });
        } else {
            key_points.push(body);
        }
    }
    let note = if explanations.is_empty() {
        "ostudy could not write this part of the guide and found no study facts in this part of the source. Generating the guide again tries it once more."
    } else {
        "ostudy could not write this part of the guide, so it lists the facts it extracted from this part of the source instead. Generating the guide again tries it once more."
    };
    let content = validate_study_guide_section_content(StudyGuideSectionContent {
        overview,
        key_points,
        explanations,
        callouts: vec![StudyGuideCallout {
            kind: StudyGuideCalloutKind::Note,
            title: "Not written yet".to_string(),
            body: note.to_string(),
        }],
        ..StudyGuideSectionContent::default()
    })?;
    let ids = units.iter().map(|unit| unit.id.clone()).collect::<Vec<_>>();
    let knowledge_support = if ids.is_empty() {
        Vec::new()
    } else {
        catalog.select(&ids, ids.len())?
    };
    Ok(GroundedStudyGuideSection {
        content,
        knowledge_support,
        catalog_fingerprint: catalog.fingerprint()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn topic() -> LearningTopic {
        LearningTopic {
            id: "b".repeat(32),
            parent_topic_id: None,
            sort_index: 0,
            title: "Triangles".into(),
            summary: "Triangles are polygons with three sides and three vertices.".into(),
            learning_objectives: Vec::new(),
            key_concepts: Vec::new(),
            source_locator: crate::document_domain::SourceLocator::new(
                crate::document_domain::SourceUnitKind::Page,
                vec![1],
                None,
            )
            .unwrap(),
            evidence_chunk_indices: vec![0],
            evidence_hashes: vec!["c".repeat(64)],
            evidence_hash: "c".repeat(64),
            anchor_hash: "d".repeat(64),
        }
    }

    #[test]
    fn gap_section_lists_extracted_units_and_says_it_was_not_written() {
        let catalog = crate::knowledge::authoring::tests::catalog_fixture();
        let section = section(&topic(), &catalog).unwrap();
        assert_eq!(section.content.explanations.len(), catalog.units.len());
        assert_eq!(section.knowledge_support.len(), catalog.units.len());
        assert_eq!(section.content.callouts.len(), 1);
        assert!(catalog.validates(&section.knowledge_support, catalog.units.len()));
    }

    #[test]
    fn section_support_stays_within_the_link_bound_and_keeps_core_units() {
        let mut catalog = KnowledgeCatalog::default();
        catalog.document_core_units.insert("u99".to_string());
        let ids = (0..100)
            .map(|index| format!("u{index}"))
            .collect::<Vec<_>>();
        let kept = bounded(ids, &catalog, String::as_str);
        assert_eq!(kept.len(), MAX_SECTION_SUPPORT);
        assert_eq!(kept.last().map(String::as_str), Some("u99"));
        assert_eq!(kept[0], "u0");
    }

    #[test]
    fn gap_section_without_units_still_publishes_a_note() {
        let catalog = KnowledgeCatalog::default();
        let section = section(&topic(), &catalog).unwrap();
        assert!(section.knowledge_support.is_empty());
        assert!(section.content.explanations.is_empty());
        assert!(section.content.callouts[0].body.contains("no study facts"));
    }

    #[test]
    fn only_topic_local_failures_become_gaps() {
        for code in ["ai_document_output_invalid", "ai_document_output_truncated"] {
            assert!(recoverable(&CommandError::new(code, "x")));
        }
        for code in [
            "ai_request_canceled",
            "ai_profile_changed",
            "ai_http_unauthorized",
        ] {
            assert!(!recoverable(&CommandError::new(code, "x")));
        }
    }
}
