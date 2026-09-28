use super::*;
use crate::{
    document_domain::SourceUnitKind,
    document_learning::{
        ensure_document_learning_plan, DocumentLearningPlanInput, LearningEvidenceChunk,
        LearningTopicInput,
    },
};
use editorial::{chapter_id, StudyGuideChapter, StudyGuideTopicHeading};

fn fixture() -> (Database, String, DocumentLearningPlan) {
    let database = Database::open_in_memory().unwrap();
    let document_id = "1".repeat(32);
    database
        .with_connection(|connection| {
            connection
                .execute(
                    "INSERT INTO documents (
                        id, title, original_filename, mime_type, byte_size, sha256,
                        source_relative_path, lifecycle_state, created_at, updated_at
                    ) VALUES (?1, 'Systems', 'systems.pdf', 'application/pdf', 1,
                              ?2, 'documents/systems/source.pdf', 'ready', 1, 1)",
                    params![document_id, "2".repeat(64)],
                )
                .map_err(CommandError::database)?;
            Ok(())
        })
        .unwrap();
    let locators = [1, 2]
        .map(|page| SourceLocator::new(SourceUnitKind::Page, vec![page], None).unwrap());
    let plan = ensure_document_learning_plan(
        &database,
        &document_id,
        DocumentLearningPlanInput {
            summary: "Retained source plan summary.".into(),
            learning_objectives: vec!["An original source objective".into()],
            key_concepts: vec!["Feedback".into()],
            topics: locators
                .iter()
                .enumerate()
                .map(|(index, locator)| LearningTopicInput {
                    parent_index: None,
                    title: format!("Original topic {index}"),
                    summary: format!("The source describes a qualified relationship {index}."),
                    learning_objectives: vec![],
                    key_concepts: vec![],
                    source_locator: locator.clone(),
                })
                .collect(),
        },
        &locators
            .iter()
            .enumerate()
            .map(|(index, locator)| LearningEvidenceChunk {
                chunk_index: index,
                source_locator: locator.clone(),
                content_hash: format!("{index:064x}"),
            })
            .collect::<Vec<_>>(),
    )
    .unwrap();
    (database, document_id, plan)
}

fn drafts(plan: &DocumentLearningPlan) -> Vec<StudyGuideSectionDraft> {
    plan.topics
        .iter()
        .map(|topic| StudyGuideSectionDraft {
            topic_id: topic.id.clone(),
            content: StudyGuideSectionContent {
                overview: topic.summary.clone(),
                ..StudyGuideSectionContent::default()
            },
            knowledge_support: vec![],
        })
        .collect()
}

fn organization(plan: &DocumentLearningPlan) -> StudyGuideEditorial {
    let topics = plan
        .topics
        .iter()
        .rev()
        .enumerate()
        .map(|(index, topic)| StudyGuideTopicHeading {
            topic_id: topic.id.clone(),
            title: format!("Readable subsection {index}"),
        })
        .collect::<Vec<_>>();
    StudyGuideEditorial {
        title: "Systems and feedback".into(),
        chapters: vec![StudyGuideChapter {
            id: chapter_id(&topics),
            title: "Understanding the relationships".into(),
            introduction: "The following sections explain the relationships and their restrictions."
                .into(),
            topics,
        }],
    }
}

fn overview() -> StudyGuideOverview {
    StudyGuideOverview {
        summary: format!(
            "{}\n\n{}",
            "A readable introduction retains the source's scope and qualifications. ".repeat(8),
            "The second paragraph introduces the organization rather than a list of fact labels."
        ),
        learning_objectives: vec![],
        key_concepts: vec![],
    }
}

#[test]
fn editorial_publication_preserves_paragraphs_topics_locators_and_legacy_revision() {
    let (database, document_id, plan) = fixture();
    let before = serde_json::to_string(&plan).unwrap();
    let legacy = publish_document_study_guide(
        &database,
        &document_id,
        &plan.id,
        None,
        overview(),
        drafts(&plan),
    )
    .unwrap();
    assert_eq!(legacy.schema_version, 2);
    assert!(legacy.editorial.is_none());
    let narrative = overview().summary;
    let guide = publish_document_study_guide_with_editorial(
        &database,
        &document_id,
        &plan.id,
        None,
        overview(),
        drafts(&plan),
        Some(organization(&plan)),
    )
    .unwrap();
    assert_eq!(guide.schema_version, 3);
    assert_eq!(guide.overview.summary, narrative);
    assert_eq!(guide.overview_text, narrative);
    assert!(guide.overview.learning_objectives.is_empty());
    assert_eq!(guide.sections[0].topic_id, plan.topics[1].id);
    assert_eq!(guide.sections[0].title, "Readable subsection 0");
    assert_eq!(
        guide.sections[0].source_locator,
        plan.topics[1].source_locator
    );
    assert_eq!(guide.editorial.unwrap().chapters.len(), 1);
    assert_eq!(
        serde_json::to_string(&load_document_learning_plan(&database, &plan.id).unwrap()).unwrap(),
        before
    );
    let reloaded_legacy = load_document_study_guide(&database, &legacy.id).unwrap();
    assert!(reloaded_legacy.editorial.is_none());
    assert_eq!(reloaded_legacy.sections[0].title, plan.topics[0].title);
}

#[test]
fn incomplete_organization_cannot_replace_a_published_guide() {
    let (database, document_id, plan) = fixture();
    let first = publish_document_study_guide(
        &database,
        &document_id,
        &plan.id,
        None,
        overview(),
        drafts(&plan),
    )
    .unwrap();
    let mut incomplete = organization(&plan);
    incomplete.chapters[0].topics.pop();
    incomplete.chapters[0].id = chapter_id(&incomplete.chapters[0].topics);
    let error = publish_document_study_guide_with_editorial(
        &database,
        &document_id,
        &plan.id,
        None,
        overview(),
        drafts(&plan),
        Some(incomplete),
    )
    .unwrap_err();
    assert_eq!(error.code, "study_guide_editorial_invalid");
    assert_eq!(
        load_active_document_study_guide(&database, &document_id)
            .unwrap()
            .unwrap()
            .id,
        first.id
    );
}

#[test]
fn section_insert_failure_rolls_back_the_editorial_and_prior_active_transition() {
    let (database, document_id, plan) = fixture();
    let first = publish_document_study_guide(
        &database,
        &document_id,
        &plan.id,
        None,
        overview(),
        drafts(&plan),
    )
    .unwrap();
    database
        .with_connection(|connection| {
            connection
                .execute_batch(
                    "CREATE TRIGGER fail_guide_section BEFORE INSERT ON document_study_guide_sections
                     BEGIN SELECT RAISE(ABORT, 'injected section failure'); END;",
                )
                .map_err(CommandError::database)
        })
        .unwrap();
    assert!(publish_document_study_guide_with_editorial(
        &database,
        &document_id,
        &plan.id,
        None,
        overview(),
        drafts(&plan),
        Some(organization(&plan)),
    )
    .is_err());
    assert_eq!(
        load_active_document_study_guide(&database, &document_id)
            .unwrap()
            .unwrap()
            .id,
        first.id
    );
    database
        .with_connection(|connection| {
            let count: i64 = connection
                .query_row("SELECT COUNT(*) FROM document_study_guides", [], |row| row.get(0))
                .map_err(CommandError::database)?;
            assert_eq!(count, 1);
            Ok(())
        })
        .unwrap();
}
