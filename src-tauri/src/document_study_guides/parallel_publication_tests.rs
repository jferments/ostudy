//! Exercise the real publication transaction, not only concurrent future polling.
use super::*;
use crate::{
    document_domain::SourceUnitKind,
    document_learning::{
        ensure_document_learning_plan, DocumentLearningPlanInput, LearningEvidenceChunk,
        LearningTopicInput,
    },
};

fn fixture(lifecycle: &str) -> (Database, String, DocumentLearningPlan) {
    let database = Database::open_in_memory().unwrap();
    let document_id = "1".repeat(32);
    database
        .with_connection(|connection| {
            connection
                .execute(
                    "INSERT INTO documents (
                         id, title, original_filename, mime_type, byte_size, sha256,
                         source_relative_path, lifecycle_state, archived_at,
                         generation_window_count, generation_completed_window_count,
                         created_at, updated_at
                     ) VALUES (?1, 'Parallel guide', 'guide.pdf', 'application/pdf', 1,
                               ?2, 'documents/guide/source.pdf', ?3, ?4, 15, 12, 1, 1)",
                    params![
                        document_id,
                        "2".repeat(64),
                        lifecycle,
                        (lifecycle == "archived").then_some(3_i64),
                    ],
                )
                .map_err(CommandError::database)?;
            Ok(())
        })
        .unwrap();
    let locator = SourceLocator::new(SourceUnitKind::Page, vec![1], None).unwrap();
    let plan = ensure_document_learning_plan(
        &database,
        &document_id,
        DocumentLearningPlanInput {
            summary: "Ventricular filling before systole".to_string(),
            learning_objectives: vec!["Explain preload".to_string()],
            key_concepts: vec!["Preload".to_string()],
            topics: vec![LearningTopicInput {
                parent_index: None,
                title: "Preload".to_string(),
                summary: "Ventricular filling before systole".to_string(),
                learning_objectives: Vec::new(),
                key_concepts: vec!["Preload".to_string()],
                source_locator: locator.clone(),
            }],
        },
        &[LearningEvidenceChunk {
            chunk_index: 0,
            source_locator: locator,
            content_hash: "3".repeat(64),
        }],
    )
    .unwrap();
    (database, document_id, plan)
}

fn publish(database: &Database, document_id: &str, plan: &DocumentLearningPlan) {
    let guide = publish_document_study_guide(
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
            topic_id: plan.topics[0].id.clone(),
            knowledge_support: Vec::new(),
            content: StudyGuideSectionContent {
                overview: "Preload describes ventricular filling before contraction.".to_string(),
                ..StudyGuideSectionContent::default()
            },
        }],
    )
    .unwrap();
    let active = load_active_document_study_guide(database, document_id)
        .unwrap()
        .unwrap();
    assert_eq!(active.id, guide.id);
}

#[test]
fn guide_first_keeps_flashcards_processing_and_eligible_for_guarded_commit() {
    let (database, document_id, plan) = fixture("processing");
    publish(&database, &document_id, &plan);
    database
        .with_connection(|connection| {
            let state = connection
                .query_row(
                    "SELECT lifecycle_state, completed_at, generation_window_count,
                            generation_completed_window_count, generated_deck_id
                     FROM documents WHERE id = ?1",
                    [&document_id],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, Option<i64>>(1)?,
                            row.get::<_, i64>(2)?,
                            row.get::<_, i64>(3)?,
                            row.get::<_, Option<String>>(4)?,
                        ))
                    },
                )
                .map_err(CommandError::database)?;
            assert_eq!(state.0, "processing");
            assert_eq!(state.1, None);
            assert_eq!((state.2, state.3), (15, 12));
            assert_eq!(state.4, None);
            // This is the lifecycle guard used by initial card finalization.
            // It must still match after the real guide transaction commits.
            let changed = connection
                .execute(
                    "UPDATE documents SET lifecycle_state = 'ready'
                     WHERE id = ?1 AND lifecycle_state = 'processing'",
                    [&document_id],
                )
                .map_err(CommandError::database)?;
            assert_eq!(changed, 1);
            Ok(())
        })
        .unwrap();
    assert!(load_active_document_study_guide(&database, &document_id)
        .unwrap()
        .is_some());
}

#[test]
fn guide_first_preserves_flashcard_owned_error_and_completion_fields() {
    let (database, document_id, plan) = fixture("processing");
    database
        .with_connection(|connection| {
            connection
                .execute(
                    "UPDATE documents SET safe_error_code = 'card_error',
                         safe_error_message = 'Card diagnostic', completed_at = 4,
                         processing_duration_ms = 17 WHERE id = ?1",
                    [&document_id],
                )
                .map_err(CommandError::database)?;
            Ok(())
        })
        .unwrap();
    publish(&database, &document_id, &plan);
    database
        .with_connection(|connection| {
            let state = connection
                .query_row(
                    "SELECT safe_error_code, safe_error_message, completed_at,
                            processing_duration_ms FROM documents WHERE id = ?1",
                    [&document_id],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, i64>(2)?,
                            row.get::<_, i64>(3)?,
                        ))
                    },
                )
                .map_err(CommandError::database)?;
            assert_eq!(state, ("card_error".into(), "Card diagnostic".into(), 4, 17));
            Ok(())
        })
        .unwrap();
}

#[test]
fn guide_publication_preserves_already_completed_and_archived_lifecycles() {
    for lifecycle in ["ready", "archived"] {
        let (database, document_id, plan) = fixture(lifecycle);
        database
            .with_connection(|connection| {
                connection
                    .execute(
                        "UPDATE documents SET completed_at = 4 WHERE id = ?1",
                        [&document_id],
                    )
                    .map_err(CommandError::database)?;
                Ok(())
            })
            .unwrap();
        publish(&database, &document_id, &plan);
        database
            .with_connection(|connection| {
                let state = connection
                    .query_row(
                        "SELECT lifecycle_state, completed_at, archived_at
                         FROM documents WHERE id = ?1",
                        [&document_id],
                        |row| {
                            Ok((
                                row.get::<_, String>(0)?,
                                row.get::<_, i64>(1)?,
                                row.get::<_, Option<i64>>(2)?,
                            ))
                        },
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(state.0, lifecycle);
                assert_eq!(state.1, 4);
                assert_eq!(state.2, (lifecycle == "archived").then_some(3));
                Ok(())
            })
            .unwrap();
    }
}
