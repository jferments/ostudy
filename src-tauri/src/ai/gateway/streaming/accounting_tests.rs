use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    thread,
};

use super::*;
use crate::{ai::generation, database::Database, error::CommandError};

fn event(value: Value) -> String {
    format!("data: {value}\n\n")
}

fn serve(body: String) -> (Url, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = Url::parse(&format!("http://{}/usage", listener.local_addr().unwrap())).unwrap();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        read_request(&mut stream);
        write!(stream,
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()).unwrap();
        stream.flush().unwrap();
    });
    (url, server)
}

fn read_request(stream: &mut TcpStream) {
    stream.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    let mut bytes = Vec::new();
    let mut buffer = [0; 4096];
    loop {
        let count = stream.read(&mut buffer).unwrap();
        assert!(count > 0);
        bytes.extend_from_slice(&buffer[..count]);
        assert!(bytes.len() <= 65_536);
        let Some(end) = bytes.windows(4).position(|part| part == b"\r\n\r\n") else {
            continue;
        };
        let end = end + 4;
        let length = String::from_utf8_lossy(&bytes[..end]).lines().find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length").then(|| value.trim().parse::<usize>().unwrap())
        }).unwrap_or(0);
        if bytes.len() >= end + length { return; }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn document_stream_failures_keep_reported_usage_without_inventing_totals() {
    let complete_usage = json!({"usage": {
        "prompt_tokens":100,"completion_tokens":400,"total_tokens":500,
        "completion_tokens_details":{"reasoning_tokens":390},"cost":0.2
    }});
    let length = event(json!({"choices":[{"delta":{"reasoning_content":"PRIVATE REASONING"},"finish_reason":"length"}]}));
    let success = event(json!({"choices":[{"delta":{"content":"{}"},"finish_reason":"stop"}]}));
    let cases = [
        (format!("{length}{}data: [DONE]\n\n", event(complete_usage.clone())), Some("ai_document_output_truncated"), Some(100), Some(400), Some(500)),
        (format!("{}data: broken-json\n\n", event(complete_usage.clone())), Some("ai_provider_protocol_error"), Some(100), Some(400), Some(500)),
        (format!("{success}{}{}{}data: [DONE]\n\n", event(complete_usage.clone()), event(complete_usage), event(json!({"usage":{"cost":0.3}}))), None, Some(100), Some(400), Some(500)),
        (format!("{length}{}data: [DONE]\n\n", event(json!({"usage":{"prompt_tokens":100}}))), Some("ai_document_output_truncated"), Some(100), None, None),
        (format!("{success}data: [DONE]\n\n"), None, None, None, None),
    ];
    for (body, expected_error, input, output, total) in cases {
        let (url, server) = serve(body);
        let database = Database::open_in_memory().unwrap();
        let mut operation = generation::Operation::begin(&database, None, None, None, "stream_usage_fixture").unwrap();
        let gateway = AiGateway::new().unwrap();
        let request = json!({"model":"fixture","max_tokens":400});
        let signal = CancellationSignal::new();
        let result = generation::scoped_request(
            Some(operation.context()), "guide_authoring", Some("usage-window"), "openrouter",
            gateway.post_sse_chat_json(url, None, &request, 5_000, 32_000, &signal),
        ).await;
        assert_eq!(result.as_ref().err().map(|error| error.safe.code.as_str()), expected_error);
        server.join().unwrap();
        operation.finish(result.is_ok());
        database.with_connection(|connection| {
            let (count, status, code, actual_input, actual_output, actual_total, payload):
                (i64, String, Option<String>, Option<i64>, Option<i64>, Option<i64>, String) =
                connection.query_row(
                    "SELECT COUNT(*),status,error_code,input_tokens,output_tokens,total_tokens,payload_bytes_json FROM generation_requests",
                    [], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?,row.get(6)?)),
                ).map_err(CommandError::database)?;
            assert_eq!(count, 1, "one physical stream, not one count per usage event");
            assert_eq!(status, if expected_error.is_some() { "failed" } else { "response" });
            assert_eq!(code.as_deref(), expected_error);
            assert_eq!((actual_input, actual_output, actual_total), (input, output, total));
            assert!(!payload.contains("PRIVATE REASONING"));
            Ok(())
        }).unwrap();
    }
}

#[test]
fn document_usage_snapshots_merge_known_fields_without_summing_or_erasing_them() {
    let mut stream = SseChatAccumulator::new(1_024, 8_192);
    for value in [
        json!({"usage":{"prompt_tokens":100,"completion_tokens":20,"prompt_tokens_details":{"cached_tokens":50}}}),
        json!({"usage":{"completion_tokens":40}}),
        json!({"usage":{"completion_tokens":40}}),
        json!({"usage":{"cost":0.1,"prompt_tokens_details":{"cached_tokens":0}}}),
    ] {
        stream.push_bytes(event(value).as_bytes()).unwrap();
    }
    assert_eq!(stream.usage.as_ref().unwrap()["prompt_tokens"], 100);
    assert_eq!(stream.usage.as_ref().unwrap()["completion_tokens"], 40);
    assert_eq!(stream.usage.as_ref().unwrap()["total_tokens"], 140);
    assert_eq!(stream.usage.as_ref().unwrap()["prompt_tokens_details"]["cached_tokens"], 0);
    assert!(stream.usage.as_ref().unwrap()["completion_tokens_details"]["reasoning_tokens"].is_null());
    assert!(!stream.done);
    stream.push_bytes(b"data: [DONE]\n\n").unwrap();
    assert!(stream.done);
}
