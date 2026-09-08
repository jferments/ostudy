use super::*;
use crate::{
    document_domain::SourceUnitKind,
    document_learning::{
        ensure_document_learning_plan, DocumentLearningPlanInput, LearningEvidenceChunk,
        LearningTopicInput,
    },
};
use rusqlite::params;
use serde_json::json;

fn section_content(overview: &str) -> StudyGuideSectionContent {
    StudyGuideSectionContent {
        overview: overview.to_string(),
        key_points: vec!["More filling can increase stroke volume.".to_string()],
        explanations: vec![StudyGuideExplanation {
            heading: "Mechanism".to_string(),
            body: "Sarcomere stretch changes force generation.".to_string(),
        }],
        important_terms: vec![StudyGuideTerm {
            term: "Preload".to_string(),
            explanation: "Ventricular filling/stretch before systole.".to_string(),
        }],
        process_steps: Vec::new(),
        comparisons: Vec::new(),
        common_misconceptions: Vec::new(),
        visual_explanations: Vec::new(),
    }
}

fn rich_doc(text: &str) -> Value {
    json!({
        "type": "doc",
        "content": [{
            "type": "paragraph",
            "content": [{"type": "text", "text": text}]
        }]
    })
}

fn fixture() -> (Database, String, DocumentLearningPlan) {
    let database = Database::open_in_memory().expect("database should open");
    let document_id = "7".repeat(32);
    database
        .with_connection(|connection| {
            connection
                .execute(
                    "INSERT INTO documents (
                         id, title, original_filename, mime_type, byte_size, sha256,
                         source_relative_path, lifecycle_state, created_at, updated_at
                     ) VALUES (?1, 'Overlay fixture', 'overlay.pdf', 'application/pdf', 1,
                               ?2, ?3, 'ready', 1, 1)",
                    params![
                        document_id,
                        "8".repeat(64),
                        format!("documents/{document_id}/source.pdf"),
                    ],
                )
                .map_err(CommandError::database)?;
            Ok(())
        })
        .expect("document fixture should insert");

    let locator = SourceLocator::new(SourceUnitKind::Page, vec![1], None).unwrap();
    let plan = ensure_document_learning_plan(
        &database,
        &document_id,
        DocumentLearningPlanInput {
            summary: "Cardiac filling".to_string(),
            learning_objectives: vec!["Explain preload".to_string()],
            key_concepts: vec!["Preload".to_string()],
            topics: vec![LearningTopicInput {
                parent_index: None,
                title: "Preload".to_string(),
                summary: "Ventricular filling before systole".to_string(),
                learning_objectives: Vec::new(),
                key_concepts: vec!["Frank-Starling".to_string()],
                source_locator: locator.clone(),
            }],
        },
        &[LearningEvidenceChunk {
            chunk_index: 0,
            source_locator: locator,
            content_hash: "9".repeat(64),
        }],
    )
    .expect("learning plan should publish");
    (database, document_id, plan)
}

fn publish(
    database: &Database,
    document_id: &str,
    plan: &DocumentLearningPlan,
    overview: &str,
) -> DocumentStudyGuide {
    publish_document_study_guide(
        database,
        document_id,
        &plan.id,
        None,
        StudyGuideOverview {
            summary: plan.summary.clone(),
            learning_objectives: plan.learning_objectives.clone(),
            key_concepts: plan.key_concepts.clone(),
        },
        vec![StudyGuideSectionDraft {
            knowledge_support: Vec::new(),
            topic_id: plan.topics[0].id.clone(),
            content: section_content(overview),
        }],
    )
    .expect("guide should publish")
}

fn body_override_input(
    document_id: &str,
    guide: &DocumentStudyGuide,
    topic_id: &str,
    base_content_hash: String,
) -> SaveStudyGuideOverlayInput {
    SaveStudyGuideOverlayInput {
        overlay_id: None,
        document_id: document_id.to_string(),
        guide_id: Some(guide.id.clone()),
        topic_id: Some(topic_id.to_string()),
        overlay_kind: StudyGuideOverlayKind::BodyOverride,
        base_content_hash: Some(base_content_hash),
        rich_doc: rich_doc("My learner-owned explanation."),
        plain_text: "My learner-owned explanation.".to_string(),
        expected_revision: None,
    }
}

#[test]
fn content_bound_overlay_rejects_a_hash_outside_the_active_section() {
    let (database, document_id, plan) = fixture();
    let guide = publish(
        &database,
        &document_id,
        &plan,
        "Preload describes ventricular filling before contraction.",
    );
    let topic_id = plan.topics[0].id.as_str();

    let error = save_study_guide_overlay(
        &database,
        body_override_input(&document_id, &guide, topic_id, "a".repeat(64)),
    )
    .expect_err("a syntactically valid but stale base hash must be rejected");
    assert_eq!(error.code, "study_guide_overlay_base_changed");

    let overlay = save_study_guide_overlay(
        &database,
        body_override_input(
            &document_id,
            &guide,
            topic_id,
            guide.sections[0].content_hash.clone(),
        ),
    )
    .expect("the exact active section hash should be accepted");
    assert_eq!(overlay.state, StudyGuideOverlayState::Active);
}

#[test]
fn regeneration_keeps_compatible_edit_active_then_surfaces_changed_base_as_conflict() {
    let (database, document_id, plan) = fixture();
    let first = publish(
        &database,
        &document_id,
        &plan,
        "Preload describes ventricular filling before contraction.",
    );
    let topic_id = plan.topics[0].id.as_str();
    let original_hash = first.sections[0].content_hash.clone();
    let overlay = save_study_guide_overlay(
        &database,
        body_override_input(&document_id, &first, topic_id, original_hash.clone()),
    )
    .expect("override should save");

    let second = publish(
        &database,
        &document_id,
        &plan,
        "Preload describes ventricular filling before contraction.",
    );
    let carried = second
        .overlays
        .iter()
        .find(|candidate| candidate.id == overlay.id)
        .expect("compatible edit should be visible on the replacement guide");
    assert_eq!(carried.guide_id.as_deref(), Some(second.id.as_str()));
    assert_eq!(carried.state, StudyGuideOverlayState::Active);
    assert_eq!(
        carried.base_content_hash.as_deref(),
        Some(original_hash.as_str())
    );

    let third = publish(
        &database,
        &document_id,
        &plan,
        "Updated source-grounded explanation after regeneration.",
    );
    let conflict = third
        .overlays
        .iter()
        .find(|candidate| candidate.id == overlay.id)
        .expect("changed-base edit should remain visible for learner review");
    assert_eq!(conflict.guide_id.as_deref(), Some(third.id.as_str()));
    assert_eq!(conflict.state, StudyGuideOverlayState::Conflicted);
    assert_eq!(
        conflict.base_content_hash.as_deref(),
        Some(original_hash.as_str())
    );
    assert_eq!(conflict.plain_text, "My learner-owned explanation.");
    assert_ne!(third.sections[0].content_hash, original_hash);
}
