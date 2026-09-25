use rusqlite::{params, Connection};

fn schema_25() -> Connection {
    let connection = Connection::open_in_memory().expect("in-memory SQLite should open");
    connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .expect("foreign keys should enable");
    for migration in [
        include_str!("../migrations/0001_public_baseline.sql"),
        include_str!("../migrations/0002_canonical_documents.sql"),
        include_str!("../migrations/0003_anydoc_document_adapters.sql"),
        include_str!("../migrations/0004_study_guides_and_learning_topics.sql"),
        include_str!("../migrations/0005_study_guide_pdf_exports.sql"),
        include_str!("../migrations/0006_document_citations.sql"),
        include_str!("../migrations/0007_document_artifact_metadata.sql"),
        include_str!("../migrations/0008_ai_profile_reasoning.sql"),
        include_str!("../migrations/0009_anki_apkg_staging.sql"),
        include_str!("../migrations/0010_semantic_tutor.sql"),
        include_str!("../migrations/0011_semantic_lifecycle.sql"),
        include_str!("../migrations/0012_semantic_eviction_queue.sql"),
        include_str!("../migrations/0013_provider_multi_model.sql"),
        include_str!("../migrations/0014_native_knowledge_evidence.sql"),
        include_str!("../migrations/0015_interactive_questions.sql"),
        include_str!("../migrations/0016_assessment_generation.sql"),
        include_str!("../migrations/0017_generation_reports.sql"),
        include_str!("../migrations/0018_canonical_evidence.sql"),
        include_str!("../migrations/0019_image_sets.sql"),
        include_str!("../migrations/0020_image_extraction.sql"),
        include_str!("../migrations/0021_image_source_edits.sql"),
        include_str!("../migrations/0022_document_collections.sql"),
        include_str!("../migrations/0023_collection_synthesis.sql"),
        include_str!("../migrations/0024_study_targets.sql"),
        include_str!("../migrations/0025_tutor_projects.sql"),
    ] {
        connection
            .execute_batch(migration)
            .expect("public migration should apply");
    }
    connection
}

#[test]
fn migration_26_flattens_existing_document_folders_and_prevents_new_nesting() {
    let connection = schema_25();
    let parent = "a".repeat(32);
    let child = "b".repeat(32);
    connection
        .execute(
            "INSERT INTO document_collections(
                 id,parent_id,name,sort_order,archived_at,created_at,updated_at,revision
             ) VALUES(?1,NULL,'Course',1000,NULL,1,1,1)",
            [&parent],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO document_collections(
                 id,parent_id,name,sort_order,archived_at,created_at,updated_at,revision
             ) VALUES(?1,?2,'Week 1',1000,NULL,1,1,1)",
            params![child, parent],
        )
        .unwrap();

    connection
        .execute_batch(include_str!(
            "../migrations/0026_single_level_document_collections.sql"
        ))
        .unwrap();

    let (parent_id, revision): (Option<String>, i64) = connection
        .query_row(
            "SELECT parent_id,revision FROM document_collections WHERE id=?1",
            [&child],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(parent_id, None);
    assert_eq!(revision, 2);
    let organization_revision: i64 = connection
        .query_row(
            "SELECT revision FROM document_collection_organization WHERE singleton_id=1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(organization_revision, 2);

    let error = connection
        .execute(
            "UPDATE document_collections SET parent_id=?1 WHERE id=?2",
            params![parent, child],
        )
        .unwrap_err();
    assert!(error
        .to_string()
        .contains("document_collections_parent_must_be_null"));
}
