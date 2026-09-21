use rusqlite::{params, Connection};

fn schema_24() -> Connection {
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
    ] {
        connection
            .execute_batch(migration)
            .expect("public migration should apply");
    }
    connection
}

#[test]
fn migration_25_freezes_preexisting_turn_scope_before_project_moves_can_change_conversation_scope() {
    let connection = schema_24();
    let conversation_id = "a".repeat(32);
    let turn_id = "b".repeat(32);
    let original_scope = r#"{"kind":"deck","id":"cccccccccccccccccccccccccccccccc"}"#;
    connection
        .execute(
            "INSERT INTO tutor_conversations(
                 id,operation_id,title,scope_json,created_at,updated_at
             ) VALUES(?1,?1,'Historical project candidate',?2,1,1)",
            params![conversation_id, original_scope],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO tutor_turns(
                 id,conversation_id,operation_id,sequence,request_hash,request_json,
                 mode,user_text,assistant_text,state,profile_revision,provider_kind,model_id,
                 created_at,updated_at
             ) VALUES(?1,?2,?1,1,?3,'{}','explain','Question','Answer','complete',
                      1,'llama_cpp','fixture',1,1)",
            params![turn_id, conversation_id, "d".repeat(64)],
        )
        .unwrap();

    connection
        .execute_batch(include_str!("../migrations/0025_tutor_projects.sql"))
        .unwrap();

    let captured: String = connection
        .query_row(
            "SELECT captured_scope_json FROM tutor_turns WHERE id=?1",
            [&turn_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(captured, original_scope);

    let later_scope = r#"{"kind":"document","id":"eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"}"#;
    connection
        .execute(
            "UPDATE tutor_conversations SET scope_json=?1 WHERE id=?2",
            params![later_scope, conversation_id],
        )
        .unwrap();
    let still_captured: String = connection
        .query_row(
            "SELECT captured_scope_json FROM tutor_turns WHERE id=?1",
            [&turn_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(still_captured, original_scope);
}
