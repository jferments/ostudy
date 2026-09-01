use rusqlite::{params, Connection};

fn schema_v5() -> Connection {
    let connection = Connection::open_in_memory().expect("in-memory SQLite should open");
    connection
        .execute_batch("PRAGMA foreign_keys = ON;\n")
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

fn insert_archived_document(
    connection: &Connection,
    document_id: &str,
    sha256: &str,
    generated_deck_id: Option<&str>,
) {
    connection
        .execute(
            "INSERT INTO documents (
                 id, title, original_filename, mime_type, byte_size, sha256,
                 source_relative_path, lifecycle_state, generated_deck_id,
                 created_at, updated_at, completed_at, archived_at
             ) VALUES (?1, ?2, ?3, 'application/pdf', 10, ?4, ?5, 'archived',
                       ?6, 1, 2, 2, 3)",
            params![
                document_id,
                format!("Subject {document_id}"),
                format!("{document_id}.pdf"),
                sha256,
                format!("documents/{document_id}/source.pdf"),
                generated_deck_id,
            ],
        )
        .expect("archived document fixture should insert");
}

#[test]
fn migration_restores_documents_and_backfills_durable_citations() {
    let connection = schema_v5();

    let direct_deck_id = "a".repeat(32);
    let provenance_deck_id = "b".repeat(32);
    for deck_id in [&direct_deck_id, &provenance_deck_id] {
        connection
            .execute(
                "INSERT INTO decks (id, name, created_at, updated_at)
                 VALUES (?1, ?2, 1, 1)",
                params![deck_id.as_str(), format!("Deck {deck_id}")],
            )
            .expect("deck fixture should insert");
    }

    let direct_document_id = "1".repeat(32);
    let empty_document_id = "2".repeat(32);
    let guide_document_id = "3".repeat(32);
    let provenance_document_id = "4".repeat(32);
    insert_archived_document(
        &connection,
        &direct_document_id,
        &"1".repeat(64),
        Some(direct_deck_id.as_str()),
    );
    insert_archived_document(&connection, &empty_document_id, &"2".repeat(64), None);
    insert_archived_document(&connection, &guide_document_id, &"3".repeat(64), None);
    insert_archived_document(
        &connection,
        &provenance_document_id,
        &"4".repeat(64),
        None,
    );

    let plan_id = "c".repeat(32);
    connection
        .execute(
            "INSERT INTO document_learning_plans (
                 id, document_id, revision, compiler_version, outline_fingerprint,
                 plan_fingerprint, summary_text, learning_objectives_json,
                 key_concepts_json, state, created_at, updated_at
             ) VALUES (?1, ?2, 1, 1, ?3, ?4, 'Summary', '[]', '[]',
                       'active', 1, 1)",
            params![
                plan_id,
                guide_document_id,
                "5".repeat(64),
                "6".repeat(64),
            ],
        )
        .expect("learning-plan fixture should insert");
    connection
        .execute(
            "INSERT INTO document_study_guides (
                 id, document_id, plan_id, revision, schema_version,
                 prompt_version, assembly_version, overview_json, overview_text,
                 state, created_at, updated_at, published_at
             ) VALUES (?1, ?2, ?3, 1, 1, 1, 1, '{}', 'Overview',
                       'active', 1, 1, 1)",
            params!["d".repeat(32), guide_document_id, plan_id],
        )
        .expect("guide fixture should insert");

    let job_id = "e".repeat(32);
    connection
        .execute(
            "INSERT INTO document_generation_jobs (
                 id, document_id, provider_kind, model_id, profile_revision,
                 credential_epoch, network_scope, state, page_count,
                 generated_deck_id, created_at, updated_at
             ) VALUES (?1, ?2, 'openai-compatible', 'fixture', 1, 0, 'cloud',
                       'succeeded', 1, ?3, 1, 1)",
            params![job_id, provenance_document_id, provenance_deck_id],
        )
        .expect("generation-job fixture should insert");
    connection
        .execute(
            "INSERT INTO document_decks (document_id, deck_id, job_id, created_at)
             VALUES (?1, ?2, ?3, 1)",
            params![provenance_document_id, provenance_deck_id, job_id],
        )
        .expect("document-deck fixture should insert");

    let note_id = "f".repeat(32);
    let topic_id = "7".repeat(32);
    connection
        .execute(
            "INSERT INTO notes (
                 id, deck_id, front_doc, back_doc, front_text, back_text,
                 created_at, updated_at
             ) VALUES (?1, ?2, '{}', '{}', 'Question', 'Answer', 1, 1)",
            params![note_id, provenance_deck_id],
        )
        .expect("note fixture should insert");
    connection
        .execute(
            "INSERT INTO document_learning_topics (
                 id, document_id, anchor_hash, created_at
             ) VALUES (?1, ?2, ?3, 1)",
            params![topic_id, provenance_document_id, "8".repeat(64)],
        )
        .expect("topic fixture should insert");
    let locator =
        r#"{"unitKind":"page","unitNumbers":[2],"sectionLabel":"Evidence","assetIds":[],"timeRangeMs":null}"#;
    connection
        .execute(
            "INSERT INTO document_note_topics (
                 document_id, note_id, topic_id, relationship_kind,
                 source_locator_json, evidence_hash, created_at
             ) VALUES (?1, ?2, ?3, 'primary', ?4, ?5, 1)",
            params![
                provenance_document_id,
                note_id,
                topic_id,
                locator,
                "9".repeat(64),
            ],
        )
        .expect("note-topic fixture should insert");

    connection
        .execute_batch(include_str!("../migrations/0006_document_citations.sql"))
        .expect("citation migration should apply");

    let mut statement = connection
        .prepare(
            "SELECT id, lifecycle_state, archived_at, revision
             FROM documents ORDER BY id",
        )
        .expect("state query should prepare");
    let states = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<i64>>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })
        .expect("states should query")
        .collect::<Result<Vec<_>, _>>()
        .expect("states should decode");
    assert_eq!(
        states,
        vec![
            (direct_document_id.clone(), "ready".into(), None, 2),
            (empty_document_id.clone(), "failed".into(), None, 2),
            (guide_document_id.clone(), "ready".into(), None, 2),
            (provenance_document_id.clone(), "ready".into(), None, 2),
        ]
    );
    drop(statement);

    let snapshot: (String, String, String, i64) = connection
        .query_row(
            "SELECT source_title, original_filename, source_identity, source_removed
             FROM source_citation_snapshots WHERE source_document_id = ?1",
            [&provenance_document_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("snapshot should read");
    assert_eq!(
        snapshot,
        (
            format!("Subject {provenance_document_id}"),
            format!("{provenance_document_id}.pdf"),
            format!("sha256:{}", "4".repeat(64)),
            0,
        )
    );

    let stored_locator: String = connection
        .query_row(
            "SELECT source_locator_json
             FROM note_source_citations WHERE note_id = ?1",
            [&note_id],
            |row| row.get(0),
        )
        .expect("note citation should read");
    assert_eq!(stored_locator, locator);

    connection
        .execute("DELETE FROM documents WHERE id = ?1", [&provenance_document_id])
        .expect("source document should delete");
    let surviving_links: (i64, i64, i64) = connection
        .query_row(
            "SELECT
                 (SELECT COUNT(*) FROM source_citation_snapshots
                  WHERE source_document_id = ?1),
                 (SELECT COUNT(*) FROM deck_source_citations
                  WHERE deck_id = ?2),
                 (SELECT COUNT(*) FROM note_source_citations
                  WHERE note_id = ?3)",
            params![provenance_document_id, provenance_deck_id, note_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("citation survival counts should read");
    assert_eq!(surviving_links, (1, 1, 1));

    let foreign_key_violations: i64 = connection
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })
        .expect("foreign-key check should run");
    assert_eq!(foreign_key_violations, 0);
    let integrity: String = connection
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .expect("integrity check should run");
    assert_eq!(integrity, "ok");
}
