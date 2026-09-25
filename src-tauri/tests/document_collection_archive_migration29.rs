use rusqlite::Connection;

fn schema_28() -> Connection {
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
        include_str!("../migrations/0026_single_level_document_collections.sql"),
        include_str!("../migrations/0027_generation_reuse_and_usage.sql"),
        include_str!("../migrations/0028_collection_reduction_request_identity.sql"),
    ] {
        connection
            .execute_batch(migration)
            .expect("public migration should apply");
    }
    connection
}

#[test]
fn migration_29_restores_archived_collections_as_ordinary_collections_and_removes_archive_state() {
    let connection = schema_28();
    let collection = "a".repeat(32);
    connection
        .execute(
            "INSERT INTO document_collections(
                 id,parent_id,name,sort_order,archived_at,created_at,updated_at,revision
             ) VALUES(?1,NULL,'Archived course',1000,123,1,123,2)",
            [&collection],
        )
        .unwrap();

    connection
        .execute_batch(include_str!(
            "../migrations/0029_remove_document_collection_archiving.sql"
        ))
        .unwrap();

    let (name, revision): (String, i64) = connection
        .query_row(
            "SELECT name,revision FROM document_collections WHERE id=?1",
            [&collection],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(name, "Archived course");
    assert_eq!(revision, 2);

    let archive_column: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('document_collections')
             WHERE name='archived_at'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(archive_column, 0);
}
