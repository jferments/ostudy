use std::fs;

use rusqlite::params;
use serde_json::json;
use sha2::{Digest, Sha256};

use super::current_format_questions::{id, rich};
use crate::{
    database::Database,
    document_domain::{SourceLocator, SourceUnitKind},
    document_learning::{
        ensure_document_learning_plan, DocumentLearningPlanInput, LearningEvidenceChunk,
        LearningTopicInput,
    },
    document_study_guides::{
        publish_document_study_guide, save_study_guide_overlay, SaveStudyGuideOverlayInput,
        StudyGuideOverlayKind, StudyGuideOverview, StudyGuideSectionContent,
        StudyGuideSectionDraft,
    },
    error::CommandError,
    paths::AppPaths,
};

pub(super) fn seed(database: &Database, paths: &AppPaths, deck: &str) -> String {
    let document = id(500);
    let bytes = b"topic,rule\nOrdering,One precedes two\n";
    let hash = format!("{:x}", Sha256::digest(bytes));
    let relative = AppPaths::document_source_relative_path_with_extension(&document, "csv");
    let path = paths.resolve_relative(&relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
    let locator = SourceLocator::new(
        SourceUnitKind::Section,
        vec![1],
        Some("Ordering".to_string()),
    )
    .unwrap();
    let locator_json = serde_json::to_string(&locator).unwrap();
    database
        .with_connection(|connection| {
            connection
                .execute(
                    "INSERT INTO documents (id, title, original_filename, mime_type,
                byte_size, sha256, source_relative_path, lifecycle_state,
                created_at, updated_at, source_format, source_unit_kind,
                source_unit_count, extracted_source_unit_count, source_media_type,
                adapter_backend, extraction_backend)
             VALUES (?1, 'Ordering', 'ordering.csv', 'application/pdf', ?2, ?3, ?4,
                'failed', 10, 10, 'csv', 'section', 1, 1, 'text/csv',
                'anydoc-canonical-adapter-v1', 'anydoc-0.2.3')",
                    params![document, bytes.len() as i64, hash, relative],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "INSERT INTO document_source_files (document_id, file_ordinal,
                original_filename, media_type, relative_path, byte_size, sha256,
                created_at, updated_at)
             VALUES (?1, 1, 'ordering.csv', 'text/csv', ?2, ?3, ?4, 10, 10)",
                    params![document, relative, bytes.len() as i64, hash],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "INSERT INTO document_source_units (document_id, unit_number, unit_kind,
                label, title, plain_text, text_sha256, source_file_ordinal,
                locator_json, extraction_confidence, created_at, updated_at)
             VALUES (?1, 1, 'section', 'Section 1', 'Ordering', ?2, ?3, 1, ?4, 1.0, 10, 10)",
                    params![
                        document,
                        String::from_utf8_lossy(bytes).to_string(),
                        hash,
                        locator_json
                    ],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "INSERT INTO document_semantic_chunk_sources (document_id, chunk_index,
                text, full_text, source_locator_json, visual_asset_ids_json,
                element_types_json, heading_context, token_estimate, is_oversized,
                content_sha256, canonical_schema_version, created_at, updated_at)
             VALUES (?1, 0, ?2, ?2, ?3, '[]', '[\"paragraph\"]', 'Ordering',
                12, 0, ?4, 1, 10, 10)",
                    params![
                        document,
                        String::from_utf8_lossy(bytes).to_string(),
                        locator_json,
                        hash
                    ],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "INSERT INTO source_citation_snapshots (id, source_document_id,
                source_identity, source_fingerprint_sha256, source_title,
                original_filename, source_format, source_media_type, source_unit_kind,
                source_unit_count, created_at, updated_at)
             VALUES (?1, ?1, ?2, ?3, 'Ordering', 'ordering.csv', 'csv',
                'text/csv', 'section', 1, 10, 10)",
                    params![document, format!("sha256:{hash}"), hash],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "INSERT INTO deck_source_citations VALUES (?1, ?2, 10)",
                    params![deck, document],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "INSERT INTO note_source_citations
             VALUES (?1, ?2, ?3, 10, 10)",
                    params![id(105), document, locator_json],
                )
                .map_err(CommandError::database)?;
            let conversation = id(600);
            connection
                .execute(
                    "INSERT INTO tutor_conversations (id, operation_id, title, scope_json,
                summary_text, history_enabled, retrieval_enabled, created_at, updated_at)
             VALUES (?1, ?1, 'Retained source chat', ?2, 'Original summary', 1, 0, 10, 20)",
                    params![
                        conversation,
                        json!({"kind":"document","id":document}).to_string()
                    ],
                )
                .map_err(CommandError::database)?;
            let request = json!({
                "operationId":id(601), "conversationId":conversation, "expectedRevision":1,
                "mode":"explain", "text":"Explain the ordering rule.", "focus":null,
                "profileId":null, "includeConversations":false, "questionId":null
            })
            .to_string();
            connection
                .execute(
                    "INSERT INTO tutor_turns (id, conversation_id, operation_id, sequence,
                request_hash, request_json, mode, user_text, assistant_text, state,
                citations_json, profile_revision, provider_kind, model_id, created_at, updated_at)
             VALUES (?1, ?2, ?1, 1, ?3, ?4, 'explain', 'Explain the ordering rule.',
                'One precedes two in the selected source.', 'complete', '[]',
                1, 'llama_cpp', 'synthetic-fixture', 10, 20)",
                    params![
                        id(601),
                        conversation,
                        format!("{:x}", Sha256::digest(request.as_bytes())),
                        request
                    ],
                )
                .map_err(CommandError::database)?;
            Ok(())
        })
        .unwrap();
    let plan = ensure_document_learning_plan(
        database,
        &document,
        DocumentLearningPlanInput {
            summary: "An explicit order.".to_string(),
            learning_objectives: vec!["Recall the stated order.".to_string()],
            key_concepts: vec!["Ordering".to_string()],
            topics: vec![LearningTopicInput {
                parent_index: None,
                title: "Ordering".to_string(),
                summary: "One precedes two.".to_string(),
                learning_objectives: vec!["Identify the first item.".to_string()],
                key_concepts: vec!["Precedence".to_string()],
                source_locator: locator.clone(),
            }],
        },
        &[LearningEvidenceChunk {
            chunk_index: 0,
            source_locator: locator,
            content_hash: hash,
        }],
    )
    .unwrap();
    publish_document_study_guide(
        database,
        &document,
        &plan.id,
        None,
        StudyGuideOverview {
            summary: plan.summary.clone(),
            learning_objectives: plan.learning_objectives.clone(),
            key_concepts: plan.key_concepts.clone(),
        },
        vec![StudyGuideSectionDraft {
            knowledge_support: vec![],
            topic_id: plan.topics[0].id.clone(),
            content: StudyGuideSectionContent {
                overview: "One precedes two.".to_string(),
                key_points: vec!["Keep the direction of the relationship.".to_string()],
                ..StudyGuideSectionContent::default()
            },
        }],
    )
    .unwrap();
    save_study_guide_overlay(
        database,
        SaveStudyGuideOverlayInput {
            overlay_id: None,
            document_id: document,
            guide_id: None,
            topic_id: Some(plan.topics[0].id.clone()),
            overlay_kind: StudyGuideOverlayKind::TopicNote,
            base_content_hash: None,
            rich_doc: rich("My edited guide note: do not reverse the relation."),
            plain_text: "My edited guide note: do not reverse the relation.".to_string(),
            expected_revision: None,
        },
    )
    .unwrap();
    relative
}
