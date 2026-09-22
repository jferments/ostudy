use super::*;

fn request() -> Value {
    json!({"model":"test-model","max_tokens":4096,"messages":[
        {"role":"system","content":"Application instructions"},
        {"role":"user","content":"PRIVATE SOURCE TEXT"}
    ]})
}

#[test]
fn document_generation_accounts_for_every_attempt_and_preserves_unknown_usage() {
    let database = Database::open_in_memory().unwrap();
    let mut operation = Operation::begin(&database, None, None, None, "fixture").unwrap();
    let context = operation.context();
    let key = hash(b"window");
    let mut first = context
        .start_attempt("guide_authoring", &request(), Some(&key), "openrouter")
        .unwrap();
    first.response(&json!({"id":"provider-first","usage":{
        "prompt_tokens":800,"completion_tokens":200,"total_tokens":1000,
        "prompt_tokens_details":{"cached_tokens":600},
        "completion_tokens_details":{"reasoning_tokens":50},"cost":0.012
    },"choices":[{"finish_reason":"length","message":{"content":"PRIVATE OUTPUT"}}]}));
    let mut second = context
        .start_attempt("guide_authoring", &request(), Some(&key), "openrouter")
        .unwrap();
    second.failed("ai_timeout");
    drop(
        context
            .start_attempt("knowledge_extraction", &request(), None, "openrouter")
            .unwrap(),
    );
    operation.finish(false);
    database.with_connection(|connection| {
        let row: (i64,i64,i64,i64,i64,String) = connection.query_row(
            "SELECT COUNT(*),SUM(input_tokens),SUM(output_tokens),SUM(reasoning_tokens),
                    SUM(cached_input_tokens),GROUP_CONCAT(status,',') FROM generation_requests",
            [], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?)),
        ).map_err(CommandError::database)?;
        assert_eq!((row.0,row.1,row.2,row.3,row.4),(3,800,200,50,600));
        assert!(row.5.contains("response"));
        assert!(row.5.contains("failed"));
        assert!(row.5.contains("interrupted"));
        let repeat: i64 = connection.query_row(
            "SELECT attempt FROM generation_requests WHERE error_code='ai_timeout'", [], |row| row.get(0),
        ).map_err(CommandError::database)?;
        assert_eq!(repeat,2);
        let missing: i64 = connection.query_row(
            "SELECT COUNT(*) FROM generation_requests WHERE input_tokens IS NULL AND output_tokens IS NULL",
            [], |row| row.get(0),
        ).map_err(CommandError::database)?;
        assert_eq!(missing,2);
        let payload: String = connection.query_row(
            "SELECT payload_bytes_json FROM generation_requests WHERE response_id='provider-first'",
            [], |row| row.get(0),
        ).map_err(CommandError::database)?;
        assert!(!payload.contains("PRIVATE SOURCE TEXT"));
        assert!(!payload.contains("PRIVATE OUTPUT"));
        let parsed: Value = serde_json::from_str(&payload).unwrap();
        assert_eq!(parsed["response"]["visibleContentBytes"],14);
        Ok(())
    }).unwrap();
}

#[test]
fn document_generation_interruption_and_startup_reconciliation_do_not_invent_zero_tokens() {
    let database = Database::open_in_memory().unwrap();
    let operation = Operation::begin(&database, None, None, None, "abandoned").unwrap();
    let context = operation.context();
    let attempt = context
        .start_attempt("flashcard_authoring", &request(), None, "openrouter")
        .unwrap();
    // Simulate process loss without running destructors. Startup owns recovery.
    std::mem::forget(attempt);
    std::mem::forget(operation);
    reconcile_startup(&database).unwrap();
    database
        .with_connection(|connection| {
            let row: (String, Option<i64>) = connection
                .query_row(
                    "SELECT status,input_tokens FROM generation_requests",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .map_err(CommandError::database)?;
            assert_eq!(row, ("interrupted".into(), None));
            let state: String = connection
                .query_row("SELECT state FROM generation_operations", [], |row| {
                    row.get(0)
                })
                .map_err(CommandError::database)?;
            assert_eq!(state, "interrupted");
            Ok(())
        })
        .unwrap();
}

#[test]
fn document_generation_payload_parts_are_content_free_and_owner_stages_are_explicit() {
    let body = json!({"model":"model","messages":[
        {"role":"system","content":"Instructions <ostudy-output-schema>{\"type\":\"object\"}</ostudy-output-schema>"},
        {"role":"user","content":"<ostudy-knowledge>{\"units\":[{\"statement\":\"PRIVATE\"}],\"evidence\":[\"SECRET\"],\"preparation\":{\"evidenceStructures\":[\"TABLE\"]}}</ostudy-knowledge>"}
    ],"response_format":{"type":"json_schema","json_schema":{"type":"object"}}});
    let report = payload::measure(&body).unwrap();
    assert!(report["embeddedSchemaBytes"].as_u64().unwrap() > 0);
    assert!(report["wireSchemaBytes"].as_u64().unwrap() > 0);
    assert!(report["userParts"]["structuralEvidence"].as_u64().unwrap() > 0);
    for secret in ["PRIVATE", "SECRET", "TABLE"] {
        assert!(!report.to_string().contains(secret));
    }
    assert_eq!(
        stage("src-tauri/src/knowledge/extraction.rs"),
        "knowledge_extraction"
    );
    assert_eq!(
        stage("src-tauri\\src\\knowledge\\collection_view.rs"),
        "collection_synthesis"
    );
    assert_eq!(
        stage("src-tauri/src/ai/document_intelligence/study_guide.rs"),
        "guide_authoring"
    );
    assert_eq!(stage("unrecognized.rs"), "unclassified");
}
