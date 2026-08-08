use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    thread,
    time::Duration,
};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use rusqlite::params;
use serde_json::{json, Value};

use crate::{database::Database, error::CommandError};

use super::{document_flashcards, AiRuntimeState};

const PROFILE_ID: &str = "0123456789abcdef0123456789abcdef";
const ONE_PIXEL_PNG: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAusB9Y9ZJXkAAAAASUVORK5CYII=";

#[test]
fn llama_document_task_sends_bounded_source_and_inline_page_image() {
    let (base_url, server) = spawn_document_server();
    let database = Database::open_in_memory().expect("database should open");
    database
        .with_connection(|connection| {
            connection
                .execute(
                    "INSERT INTO ai_provider_profiles (
                         id, display_name, normalized_name, provider_kind,
                         base_url, model_id, auth_mode, enabled,
                         allow_insecure_http, request_timeout_ms,
                         credential_storage, credential_epoch, revision,
                         created_at, updated_at, deleted_at
                     ) VALUES (
                         ?1, 'Document vision', 'document vision', 'llama_cpp',
                         ?2, 'vision-model', 'none', 1,
                         0, 5000, 'none', 0, 1, 1, 1, NULL
                     )",
                    params![PROFILE_ID, base_url],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "INSERT INTO ai_task_defaults (
                         task_kind, profile_id, updated_at
                     ) VALUES ('flashcard_generation', ?1, 1)",
                    [PROFILE_ID],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "INSERT INTO ai_provider_probe_results (
                         profile_id, profile_revision, credential_epoch,
                         tested_model_id, adapter_revision,
                         result_schema_version, overall_status,
                         result_json, tested_at
                     ) VALUES (?1, 1, 0, 'vision-model', 2, 1, 'verified', '{}', 1)",
                    [PROFILE_ID],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "INSERT INTO ai_provider_capabilities (
                         profile_id, capability_key, status,
                         source_kind, value_json, checked_at
                     ) VALUES (?1, 'image_input', 'supported', 'declared', 'true', 1)",
                    [PROFILE_ID],
                )
                .map_err(CommandError::database)?;
            Ok(())
        })
        .expect("provider fixture should insert");

    let task = document_flashcards::load_document_task_context(&database)
        .expect("document task should resolve");
    assert!(task.include_page_images);
    assert_eq!(task.network_scope, "local");
    let runtime = AiRuntimeState::new().expect("AI runtime should initialize");
    let png = STANDARD
        .decode(ONE_PIXEL_PNG)
        .expect("test PNG should decode");

    let cards = tauri::async_runtime::block_on(async {
        document_flashcards::generate_page_flashcards(
            &runtime,
            &task,
            "Cardiac physiology",
            1,
            "Cardiac output equals heart rate multiplied by stroke volume.",
            Some(&png),
        )
        .await
        .expect("page flashcards should validate")
    });
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0]["type"], "basic");
    assert_eq!(cards[0]["front"], "What determines cardiac output?");
    server.join().expect("mock document server should finish");
}

fn spawn_document_server() -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("mock listener should bind");
    let address = listener.local_addr().expect("mock address should resolve");
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("document request should arrive");
        let request = read_request(&mut stream);
        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/v1/chat/completions");
        assert_document_request(&request.body);
        let content = json!({
            "cards": [{
                "type": "basic",
                "front": "What determines cardiac output?",
                "back": "Cardiac output equals heart rate multiplied by stroke volume.",
                "reverseEnabled": false,
                "tags": ["cardiac physiology"]
            }]
        })
        .to_string();
        let response = json!({
            "id": "document-completion",
            "model": "vision-model",
            "choices": [{
                "index": 0,
                "message": { "role": "assistant", "content": content },
                "finish_reason": "stop"
            }]
        })
        .to_string();
        write_response(&mut stream, &response);
    });
    (format!("http://{address}/"), handle)
}

struct MockRequest {
    method: String,
    path: String,
    body: String,
}

fn assert_document_request(body: &str) {
    let value: Value = serde_json::from_str(body).expect("request should be JSON");
    assert_eq!(
        value.get("model").and_then(Value::as_str),
        Some("vision-model")
    );
    assert_eq!(value.get("stream").and_then(Value::as_bool), Some(false));
    assert_eq!(
        value
            .pointer("/response_format/type")
            .and_then(Value::as_str),
        Some("json_schema")
    );
    assert_eq!(
        value
            .pointer("/response_format/json_schema/strict")
            .and_then(Value::as_bool),
        Some(true)
    );
    let system = value
        .pointer("/messages/0/content")
        .and_then(Value::as_str)
        .expect("system prompt should exist");
    assert!(system.contains("untrusted study material"));
    assert!(system.contains("zero to eight useful cards"));
    let parts = value
        .pointer("/messages/1/content")
        .and_then(Value::as_array)
        .expect("multimodal user content should be an array");
    assert_eq!(parts.len(), 2);
    let source = parts[0]
        .get("text")
        .and_then(Value::as_str)
        .expect("source text should exist");
    assert!(source.contains("PDF page: 1"));
    assert!(source.contains("Cardiac output equals heart rate"));
    assert!(source.contains("<ostudy-source>"));
    let data_url = parts[1]
        .pointer("/image_url/url")
        .and_then(Value::as_str)
        .expect("inline image URL should exist");
    let encoded = data_url
        .strip_prefix("data:image/png;base64,")
        .expect("image must be a local PNG data URL");
    let decoded = STANDARD
        .decode(encoded)
        .expect("inline image should decode");
    assert!(decoded.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]));
    assert!(!body.contains("/home/"));
    assert!(!body.contains("Authorization"));
}

fn read_request(stream: &mut TcpStream) -> MockRequest {
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("read timeout should set");
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 4096];
    let header_end = loop {
        let count = stream.read(&mut buffer).expect("request should read");
        assert!(count > 0, "request closed before headers completed");
        bytes.extend_from_slice(&buffer[..count]);
        if let Some(index) = find_subsequence(&bytes, b"\r\n\r\n") {
            break index + 4;
        }
        assert!(
            bytes.len() < 64 * 1024,
            "request headers exceeded test bound"
        );
    };
    let header_text =
        String::from_utf8(bytes[..header_end].to_vec()).expect("headers should be UTF-8");
    let content_length = header_text
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length").then(|| {
                value
                    .trim()
                    .parse::<usize>()
                    .expect("content length should parse")
            })
        })
        .unwrap_or(0);
    while bytes.len() < header_end + content_length {
        let count = stream.read(&mut buffer).expect("request body should read");
        assert!(count > 0, "request closed before body completed");
        bytes.extend_from_slice(&buffer[..count]);
    }
    let request_line = header_text
        .lines()
        .next()
        .expect("request line should exist");
    let mut parts = request_line.split_whitespace();
    let method = parts.next().expect("method should exist").to_string();
    let path = parts.next().expect("path should exist").to_string();
    let body = String::from_utf8(bytes[header_end..header_end + content_length].to_vec())
        .expect("request body should be UTF-8");
    MockRequest { method, path, body }
}

fn write_response(stream: &mut TcpStream, body: &str) {
    let headers = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream
        .write_all(headers.as_bytes())
        .expect("response headers should write");
    stream
        .write_all(body.as_bytes())
        .expect("response body should write");
    stream.flush().expect("response should flush");
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}
