use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    thread,
};

use url::Url;

use super::*;
use crate::{ai::gateway::AiGateway, task_cancellation};

#[path = "stream_usage_tests.rs"]
mod stream_usage;

fn request() -> Value {
    json!({"model":"test-model","max_tokens":4096,"messages":[
        {"role":"system","content":"Application instructions"},
        {"role":"user","content":"PRIVATE SOURCE TEXT"}
    ]})
}

#[test]
fn provider_session_id_is_content_free_and_operation_stable() {
    let database = Database::open_in_memory().unwrap();
    let mut operation = Operation::begin(&database, None, None, None, "session_fixture").unwrap();
    let context = operation.context();
    let first = context.provider_session_id();
    let second = context.provider_session_id();
    assert_eq!(first, second);
    assert_eq!(first, format!("ostudy:{}", context.operation_id));
    assert!(first.len() < 256);
    operation.finish(true);
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

        let retry_payload: String = connection.query_row(
            "SELECT payload_bytes_json FROM generation_requests
             WHERE error_code='ai_timeout'",
            [],
            |row| row.get(0),
        ).map_err(CommandError::database)?;
        let retry_payload: Value = serde_json::from_str(&retry_payload).unwrap();
        assert_eq!(retry_payload["cachePrefix"]["comparable"], true);
        assert!(retry_payload["cachePrefix"]["commonPrefixBytes"].as_u64().unwrap() > 0);
        assert_eq!(
            retry_payload["cachePrefix"]["previousWireRequestHash"],
            parsed["wireRequestHash"]
        );
        assert!(retry_payload["cachePrefix"].get("prompt").is_none());
        Ok(())
    }).unwrap();
}

#[test]
fn logical_task_recovery_budget_blocks_duplicate_and_third_nested_retries() {
    let database = Database::open_in_memory().unwrap();
    let mut operation = Operation::begin(&database, None, None, None, "recovery_fixture").unwrap();
    let context = operation.context();
    let key = hash(b"stable-window");

    assert!(context.reserve_recovery(
        "flashcard_authoring",
        Some(&key),
        RecoveryReason::TransientTransport,
        "ai_provider_timeout",
    ));
    assert!(!context.reserve_recovery(
        "flashcard_authoring",
        Some(&key),
        RecoveryReason::TransientTransport,
        "ai_provider_timeout",
    ));
    assert!(context.reserve_recovery(
        "flashcard_authoring",
        Some(&key),
        RecoveryReason::ContractRepair,
        "ai_document_output_invalid",
    ));
    assert!(!context.reserve_recovery(
        "flashcard_authoring",
        Some(&key),
        RecoveryReason::OutputCompaction,
        "ai_document_output_truncated",
    ));
    assert!(context.reserve_recovery(
        "flashcard_authoring",
        Some(&hash(b"other-window")),
        RecoveryReason::OutputCompaction,
        "ai_document_output_truncated",
    ));
    operation.finish(true);
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

#[test]
fn embedding_accounting_records_non_generative_output_and_content_free_input_size() {
    let database = Database::open_in_memory().unwrap();
    let mut operation = Operation::begin(&database, None, None, None, "embedding_fixture").unwrap();
    let context = operation.context();
    let body = json!({
        "model": "embedding-model",
        "input": ["PRIVATE EMBEDDING TEXT", "SECOND PRIVATE INPUT"]
    });
    let mut attempt = context
        .start_attempt("embedding_indexing", &body, None, "openrouter")
        .unwrap();
    attempt.non_generative_response(&json!({
        "id": "embedding-response",
        "usage": {"prompt_tokens": 42, "total_tokens": 42}
    }));
    operation.finish(true);
    database
        .with_connection(|connection| {
            let values: (i64, i64, i64, String) = connection
                .query_row(
                    "SELECT input_tokens,output_tokens,reasoning_tokens,payload_bytes_json
                     FROM generation_requests WHERE response_id='embedding-response'",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .map_err(CommandError::database)?;
            assert_eq!((values.0, values.1, values.2), (42, 0, 0));
            assert!(!values.3.contains("PRIVATE EMBEDDING TEXT"));
            assert!(!values.3.contains("SECOND PRIVATE INPUT"));
            let payload: Value = serde_json::from_str(&values.3).unwrap();
            assert_eq!(payload["userParts"]["embeddingInputCount"], 2);
            assert!(payload["userParts"]["embeddingInput"].as_u64().unwrap() > 0);
            Ok(())
        })
        .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn physical_gateway_retry_records_two_attempts_for_one_logical_request() {
    const DOCUMENT_ID: &str = "generation-ledger-retry";
    let listener = TcpListener::bind("127.0.0.1:0").expect("mock listener should bind");
    let url = Url::parse(&format!(
        "http://{}/retry",
        listener.local_addr().expect("mock address should resolve")
    ))
    .expect("mock URL should parse");
    let server = thread::spawn(move || {
        let (mut first, _) = listener.accept().expect("first request should arrive");
        read_http_request(&mut first);
        first
            .write_all(
                b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            )
            .expect("first response should write");
        first.flush().expect("first response should flush");

        let (mut second, _) = listener.accept().expect("retry request should arrive");
        read_http_request(&mut second);
        let body =
            "data: {\"choices\":[{\"delta\":{\"content\":\"{}\"},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n";
        write!(
            second,
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        )
        .expect("streaming response should write");
        second.flush().expect("streaming response should flush");
    });

    let database = Database::open_in_memory().expect("database should open");
    let mut operation = Operation::begin(&database, None, None, None, "gateway_retry_fixture")
        .expect("diagnostic operation should begin");
    let diagnostic_context = operation.context();
    let request_key = hash(b"retry-window");
    let gateway = AiGateway::new().expect("gateway should initialize");
    let result = task_cancellation::run_document_task(DOCUMENT_ID, async move {
        scoped_request(
            Some(diagnostic_context),
            "guide_authoring",
            Some(&request_key),
            "openrouter",
            async move {
                gateway
                    .post_document_json(url, None, &json!({ "model": "test" }), 5_000, 1_024)
                    .await
                    .map_err(|error| CommandError::new(error.safe.code, error.safe.message))
            },
        )
        .await
    })
    .await
    .expect("the bounded retry should succeed");
    operation.finish(true);
    assert_eq!(
        result.pointer("/choices/0/message/content"),
        Some(&json!("{}"))
    );
    server.join().expect("mock server should finish");

    database
        .with_connection(|connection| {
            let mut statement = connection
                .prepare(
                    "SELECT attempt,status FROM generation_requests
                     WHERE operation_id=?1 ORDER BY attempt",
                )
                .map_err(CommandError::database)?;
            let rows = statement
                .query_map([operation.context().operation_id], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
                })
                .map_err(CommandError::database)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(CommandError::database)?;
            assert_eq!(
                rows,
                vec![(1, "failed".to_string()), (2, "response".to_string())]
            );
            Ok(())
        })
        .expect("physical retry attempts should be durable");
}

fn read_http_request(stream: &mut TcpStream) {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 4_096];
    loop {
        let count = stream.read(&mut buffer).expect("request bytes should read");
        assert!(count > 0, "request closed before body completed");
        bytes.extend_from_slice(&buffer[..count]);
        let Some(header_end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") else {
            continue;
        };
        let header_end = header_end + 4;
        let headers = String::from_utf8_lossy(&bytes[..header_end]);
        let content_length = headers
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().ok())
                    .flatten()
            })
            .unwrap_or(0);
        if bytes.len() >= header_end.saturating_add(content_length) {
            return;
        }
    }
}
