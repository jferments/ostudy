use rusqlite::{params, Connection};

fn latest_schema() -> Connection {
    let connection = Connection::open_in_memory().expect("in-memory SQLite should open");
    connection
        .execute_batch(
            "PRAGMA foreign_keys = ON;\n",
        )
        .expect("foreign keys should enable");
    for migration in [
        include_str!("../migrations/0001_public_baseline.sql"),
        include_str!("../migrations/0002_canonical_documents.sql"),
        include_str!("../migrations/0003_anydoc_document_adapters.sql"),
        include_str!("../migrations/0004_study_guides_and_learning_topics.sql"),
        include_str!("../migrations/0005_study_guide_pdf_exports.sql"),
    ] {
        connection
            .execute_batch(migration)
            .expect("public migration should apply to the fixture");
    }
    connection
}

#[test]
fn study_guide_pdf_is_a_valid_prepared_export_purpose() {
    let connection = latest_schema();
    connection
        .execute(
            "INSERT INTO prepared_exports (
                 id, operation_id, purpose, suggested_filename, media_type,
                 byte_size, sha256, relative_path, state, summary_json,
                 created_at, updated_at, expires_at
             ) VALUES (?1, ?2, 'study_guide_pdf', 'guide.pdf', 'application/pdf',
                       10, ?3, 'transfers/exports/guide.pdf', 'prepared', '{}',
                       1, 1, 2)",
            params!["a".repeat(32), "b".repeat(32), "c".repeat(64)],
        )
        .expect("study-guide PDF export should satisfy the public schema");

    let purpose: String = connection
        .query_row(
            "SELECT purpose FROM prepared_exports WHERE id = ?1",
            ["a".repeat(32)],
            |row| row.get(0),
        )
        .expect("prepared export should be readable");
    assert_eq!(purpose, "study_guide_pdf");
}

#[test]
fn deleting_document_cascades_through_parent_child_learning_topics() {
    let connection = latest_schema();
    let document_id = "1".repeat(32);
    let plan_id = "2".repeat(32);
    let parent_topic_id = "3".repeat(32);
    let child_topic_id = "4".repeat(32);

    connection
        .execute(
            "INSERT INTO documents (
                 id, title, original_filename, mime_type, byte_size, sha256,
                 source_relative_path, lifecycle_state, created_at, updated_at
             ) VALUES (?1, 'Deletion fixture', 'fixture.pdf', 'application/pdf',
                       1, ?2, ?3, 'ready', 1, 1)",
            params![
                document_id,
                "5".repeat(64),
                format!("documents/{document_id}/source.pdf"),
            ],
        )
        .expect("document fixture should insert");
    connection
        .execute(
            "INSERT INTO document_learning_plans (
                 id, document_id, revision, compiler_version, outline_fingerprint,
                 plan_fingerprint, summary_text, learning_objectives_json,
                 key_concepts_json, state, created_at, updated_at
             ) VALUES (?1, ?2, 1, 1, ?3, ?4, 'Summary', '[]', '[]',
                       'active', 1, 1)",
            params![plan_id, document_id, "6".repeat(64), "7".repeat(64)],
        )
        .expect("learning plan fixture should insert");
    for topic_id in [&parent_topic_id, &child_topic_id] {
        connection
            .execute(
                "INSERT INTO document_learning_topics (
                     id, document_id, anchor_hash, created_at
                 ) VALUES (?1, ?2, ?3, 1)",
                params![topic_id, document_id, format!("{:0<64}", topic_id)],
            )
            .expect("topic fixture should insert");
    }
    connection
        .execute(
            "INSERT INTO document_learning_plan_topics (
                 plan_id, topic_id, parent_topic_id, sort_index, title,
                 summary_text, learning_objectives_json, key_concepts_json,
                 source_locator_json, evidence_chunk_indices_json,
                 evidence_hashes_json, evidence_hash, created_at
             ) VALUES (?1, ?2, NULL, 0, 'Parent', 'Parent summary', '[]', '[]',
                       ?3, '[0]', ?4, ?5, 1)",
            params![
                plan_id,
                parent_topic_id,
                r#"{"unitKind":"page","unitIndices":[1],"assetIds":[]}"#,
                format!("[\"{}\"]", "8".repeat(64)),
                "8".repeat(64),
            ],
        )
        .expect("parent plan topic should insert");
    connection
        .execute(
            "INSERT INTO document_learning_plan_topics (
                 plan_id, topic_id, parent_topic_id, sort_index, title,
                 summary_text, learning_objectives_json, key_concepts_json,
                 source_locator_json, evidence_chunk_indices_json,
                 evidence_hashes_json, evidence_hash, created_at
             ) VALUES (?1, ?2, ?3, 1, 'Child', 'Child summary', '[]', '[]',
                       ?4, '[1]', ?5, ?6, 1)",
            params![
                plan_id,
                child_topic_id,
                parent_topic_id,
                r#"{"unitKind":"page","unitIndices":[2],"assetIds":[]}"#,
                format!("[\"{}\"]", "9".repeat(64)),
                "9".repeat(64),
            ],
        )
        .expect("child plan topic should insert");

    connection
        .execute("DELETE FROM documents WHERE id = ?1", [&document_id])
        .expect("document-owned learning hierarchy should cascade cleanly");

    let topic_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM document_learning_topics WHERE document_id = ?1",
            [&document_id],
            |row| row.get(0),
        )
        .expect("topic count should be readable");
    let plan_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM document_learning_plans WHERE document_id = ?1",
            [&document_id],
            |row| row.get(0),
        )
        .expect("plan count should be readable");
    assert_eq!(topic_count, 0);
    assert_eq!(plan_count, 0);
}
