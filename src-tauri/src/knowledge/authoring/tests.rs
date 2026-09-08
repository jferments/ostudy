use super::*;
use crate::{
    authoring::card_types_and_templates::generated_import::import_generated_flashcards_bytes_with_commit_context,
    document_card_topics::{link_imported_notes_to_topics, SourceGroundedGeneratedCard},
    document_domain::{SourceLocator, SourceUnitKind},
    document_learning::{
        ensure_document_learning_plan, DocumentLearningPlan, DocumentLearningPlanInput,
        LearningEvidenceChunk, LearningTopicInput,
    },
    document_study_guides::{
        publish_document_study_guide, StudyGuideOverview, StudyGuideSectionContent,
        StudyGuideSectionDraft,
    },
    knowledge::contracts::{KnowledgeKind, KnowledgeUnitInput, SourceRecord},
};

pub(crate) fn catalog_fixture() -> KnowledgeCatalog {
    KnowledgeCatalog {
        units: vec![KnowledgeUnit {
            id: "a".repeat(32),
            source_id: "b".repeat(32),
            source_hash: "c".repeat(64),
            origin: ContentOrigin::SourceDocument,
            content: KnowledgeUnitInput {
                title: "Triangle".to_string(),
                kind: KnowledgeKind::Definition,
                statement: "A triangle has three sides.".to_string(),
                qualifications: vec![],
                topic: "Geometry".to_string(),
                supporting_excerpt: "A triangle has three sides.".to_string(),
            },
        }],
    }
}

fn fixture() -> (
    Database,
    SourceRecord,
    KnowledgeCatalog,
    DocumentLearningPlan,
) {
    let database = Database::open_in_memory().unwrap();
    let document_id = "1".repeat(32);
    database.with_connection(|connection| {
        connection.execute("INSERT INTO documents (id,title,original_filename,mime_type,byte_size,sha256,
            source_relative_path,lifecycle_state,created_at,updated_at)
            VALUES (?1,'Geometry','geometry.pdf','application/pdf',1,?2,'documents/geometry.pdf','ready',1,1)",
            params![document_id, "2".repeat(64)]).map_err(CommandError::database)?;
        connection.execute("INSERT INTO document_generation_jobs (id,document_id,provider_kind,model_id,
            profile_revision,credential_epoch,network_scope,state,page_count,created_at,updated_at)
            VALUES (?1,?2,'llama_cpp','fixture',1,0,'local','committing',1,1,1)",
            params!["3".repeat(32), document_id]).map_err(CommandError::database)?;
        Ok(())
    }).unwrap();
    let locator = SourceLocator::new(SourceUnitKind::Page, vec![1], None).unwrap();
    let mut source = SourceRecord {
        id: String::new(),
        kind: SourceKind::Document,
        owner_id: document_id.clone(),
        segment_key: "0:0:0".to_string(),
        owner_revision: "1".to_string(),
        content_hash: String::new(),
        title: "Geometry".to_string(),
        text: "A triangle has three sides.".to_string(),
        origin: ContentOrigin::SourceDocument,
        document_id: Some(document_id.clone()),
        note_id: None,
        card_id: None,
        guide_id: None,
        conversation_id: None,
        turn_id: None,
        locator: Some(locator.clone()),
        images: vec![],
    };
    source.refresh_identity().unwrap();
    database
        .with_connection(|connection| repository::publish_source(connection, &source, "0", 1))
        .unwrap();
    let units = repository::publish_units(
        &database,
        &source,
        &hash("extraction"),
        vec![catalog_fixture().units[0].content.clone()],
    )
    .unwrap();
    let plan = ensure_document_learning_plan(
        &database,
        &document_id,
        DocumentLearningPlanInput {
            summary: "Geometry".to_string(),
            learning_objectives: vec![],
            key_concepts: vec!["Triangle".to_string()],
            topics: vec![LearningTopicInput {
                parent_index: None,
                title: "Triangle".to_string(),
                summary: "Three-sided shape".to_string(),
                learning_objectives: vec![],
                key_concepts: vec![],
                source_locator: locator.clone(),
            }],
        },
        &[LearningEvidenceChunk {
            chunk_index: 0,
            source_locator: locator,
            content_hash: source.content_hash.clone(),
        }],
    )
    .unwrap();
    (database, source, KnowledgeCatalog { units }, plan)
}

fn guide_content() -> StudyGuideSectionContent {
    StudyGuideSectionContent {
        overview: "Triangles have three sides.".to_string(),
        key_points: vec!["A triangle has three sides.".to_string()],
        explanations: vec![],
        important_terms: vec![],
        process_steps: vec![],
        comparisons: vec![],
        common_misconceptions: vec![],
        visual_explanations: vec![],
    }
}

fn publish_guide(
    database: &Database,
    source: &SourceRecord,
    catalog: &KnowledgeCatalog,
    plan: &DocumentLearningPlan,
) -> Result<crate::document_study_guides::DocumentStudyGuide, CommandError> {
    publish_document_study_guide(
        database,
        &source.owner_id,
        &plan.id,
        None,
        StudyGuideOverview {
            summary: "Geometry".to_string(),
            learning_objectives: vec![],
            key_concepts: vec![],
        },
        vec![StudyGuideSectionDraft {
            topic_id: plan.topics[0].id.clone(),
            content: guide_content(),
            knowledge_support: catalog.select(&[catalog.units[0].id.clone()], 64)?,
        }],
    )
}

#[test]
fn catalog_rejects_unknown_duplicate_empty_and_excessive_targets() {
    let catalog = catalog_fixture();
    assert!(catalog.select(&[], 2).is_err());
    assert!(catalog.select(&["f".repeat(32)], 2).is_err());
    assert!(catalog
        .select(&["a".repeat(32), "a".repeat(32)], 2)
        .is_err());
    assert!(catalog.select(&["a".repeat(32)], 0).is_err());
    let support = catalog.select(&["a".repeat(32)], 2).unwrap();
    assert!(catalog.validates(&support, 2));
    let mut changed = catalog.clone();
    changed.units[0]
        .content
        .qualifications
        .push("A qualification must survive.".to_string());
    assert_ne!(
        catalog.fingerprint().unwrap(),
        changed.fingerprint().unwrap()
    );
    assert!(!changed.validates(&support, 2));
}

#[test]
fn exact_support_rejects_other_documents_and_source_revisions() {
    let (database, mut source, catalog, _) = fixture();
    let support = catalog.select(&[catalog.units[0].id.clone()], 2).unwrap();
    database
        .with_connection(|connection| {
            validate_current_support(connection, &source.owner_id, &support)
        })
        .unwrap();
    assert!(database
        .with_connection(|connection| validate_current_support(
            connection,
            &"f".repeat(32),
            &support
        ))
        .is_err());
    source.text = "An edited source statement.".to_string();
    source.refresh_identity().unwrap();
    database
        .with_connection(|connection| repository::publish_source(connection, &source, "0", 2))
        .unwrap();
    assert!(database
        .with_connection(|connection| validate_current_support(
            connection,
            &source.owner_id,
            &support
        ))
        .is_err());
}

#[test]
fn card_import_links_actual_presentations_after_duplicate_filtering() {
    let (database, source, catalog, plan) = fixture();
    let support = catalog.select(&[catalog.units[0].id.clone()], 2).unwrap();
    let card = json!({"type":"basic","front":"How many sides does a triangle have?","back":"Three","reverseEnabled":true});
    let other = json!({"type":"basic","front":"What shape has three sides?","back":"A triangle"});
    let grounded = [card.clone(), card.clone(), other.clone()]
        .into_iter()
        .map(|card| SourceGroundedGeneratedCard {
            card,
            source_locator: source.locator.clone().unwrap(),
            knowledge_support: support.clone(),
        })
        .collect::<Vec<_>>();
    let payload = json!({"format":"ostudy-flashcards","formatVersion":1,
        "deck":{"mode":"create","name":"Geometry"},"cards":[card.clone(),card,other]})
    .to_string();
    let result = import_generated_flashcards_bytes_with_commit_context(
        &database,
        payload.as_bytes(),
        |connection, _, imported, now| {
            assert_eq!(
                imported
                    .created_notes
                    .iter()
                    .map(|note| note.source_index)
                    .collect::<Vec<_>>(),
                vec![0, 2]
            );
            link_imported_notes_to_topics(
                connection,
                &source.owner_id,
                &"3".repeat(32),
                &plan,
                &grounded,
                imported,
                now,
            )
        },
    )
    .unwrap();
    assert_eq!(result.created_note_count, 2);
    assert_eq!(result.created_card_count, 3);
    let links: i64 = database
        .with_connection(|connection| {
            connection
                .query_row(
                    "SELECT COUNT(*) FROM knowledge_artifact_links WHERE relationship = 'assesses'",
                    [],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)
        })
        .unwrap();
    assert_eq!(
        links, 3,
        "forward and reverse presentations both retain their content target"
    );
}

#[test]
fn stale_support_rolls_back_the_whole_generated_card_import() {
    let (database, source, catalog, plan) = fixture();
    let mut support = catalog.select(&[catalog.units[0].id.clone()], 2).unwrap();
    support[0].source_hash = "f".repeat(64);
    let card = json!({"type":"basic","front":"How many sides?","back":"Three"});
    let grounded = vec![SourceGroundedGeneratedCard {
        card: card.clone(),
        source_locator: source.locator.clone().unwrap(),
        knowledge_support: support,
    }];
    let payload = json!({"format":"ostudy-flashcards","formatVersion":1,
        "deck":{"mode":"create","name":"Rollback"},"cards":[card]})
    .to_string();
    assert!(import_generated_flashcards_bytes_with_commit_context(
        &database,
        payload.as_bytes(),
        |connection, _, imported, now| link_imported_notes_to_topics(
            connection,
            &source.owner_id,
            &"3".repeat(32),
            &plan,
            &grounded,
            imported,
            now
        )
    )
    .is_err());
    database
        .with_connection(|connection| {
            for table in [
                "decks",
                "notes",
                "cards",
                "card_schedules",
                "document_note_topics",
                "knowledge_artifact_links",
            ] {
                let count: i64 = connection
                    .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                        row.get(0)
                    })
                    .map_err(CommandError::database)?;
                assert_eq!(count, 0, "transaction leaked {table}");
            }
            Ok(())
        })
        .unwrap();
}

#[test]
fn guide_publication_links_shared_units_and_rolls_back_stale_replacement() {
    let (database, source, catalog, plan) = fixture();
    let guide = publish_guide(&database, &source, &catalog, &plan).unwrap();
    database.with_connection(|connection| {
        let count: i64 = connection.query_row(
            "SELECT COUNT(*) FROM knowledge_artifact_links l JOIN knowledge_sources s ON s.id = l.source_id
             WHERE s.guide_id = ?1 AND l.relationship = 'explains' AND l.unit_id = ?2",
            params![guide.id, catalog.units[0].id], |row| row.get(0),
        ).map_err(CommandError::database)?;
        assert_eq!(count, 1);
        Ok(())
    }).unwrap();
    let mut stale = catalog.clone();
    stale.units[0].source_hash = "f".repeat(64);
    assert!(publish_guide(&database, &source, &stale, &plan).is_err());
    database
        .with_connection(|connection| {
            let id: String = connection
                .query_row(
                    "SELECT id FROM document_study_guides WHERE state = 'active'",
                    [],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            assert_eq!(id, guide.id);
            let count: i64 = connection
                .query_row("SELECT COUNT(*) FROM document_study_guides", [], |row| {
                    row.get(0)
                })
                .map_err(CommandError::database)?;
            assert_eq!(
                count, 1,
                "failed replacement must not supersede or partially publish"
            );
            Ok(())
        })
        .unwrap();
    let public_json = serde_json::to_value(&guide).unwrap();
    assert!(!public_json.to_string().contains("knowledgeSupport"));
    assert!(!public_json.to_string().contains("catalogFingerprint"));
}

#[test]
fn internal_support_is_not_part_of_the_public_card_import_format() {
    let catalog = catalog_fixture();
    let mut card = json!({"type":"basic","front":"How many sides?","back":"Three"});
    let support = catalog.select(&["a".repeat(32)], 2).unwrap();
    card[CARD_SUPPORT_KEY] = serde_json::to_value(&support).unwrap();
    assert_eq!(card_support(&card).unwrap(), support);
    let plain = without_card_support(card);
    assert_eq!(
        plain,
        json!({"type":"basic","front":"How many sides?","back":"Three"})
    );
    assert!(card_support(&plain).is_err());
}
