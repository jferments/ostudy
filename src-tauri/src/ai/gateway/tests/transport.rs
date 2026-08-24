use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    thread,
    time::Duration,
};

use serde_json::{json, Value};
use tokio::sync::oneshot;
use url::Url;

use crate::{error::CommandError, task_cancellation};

use super::super::{client::AiGateway, contracts::AiGatewayError};

#[test]
fn gateway_clients_initialize_without_contacting_a_provider() {
    AiGateway::new().expect("AI gateway clients should initialize without network access");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ordinary_json_remains_non_streaming_inside_a_document_task() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("mock listener should bind");
    let url = Url::parse(&format!(
        "http://{}/ordinary",
        listener.local_addr().expect("mock address should resolve")
    ))
    .expect("mock URL should parse");
    let (request_sender, request_receiver) = oneshot::channel();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("ordinary request should arrive");
        let request = read_http_request(&mut stream);
        let _ = request_sender.send(request);
        write_json_response(&mut stream, 200, r#"{"ok":true}"#);
    });

    let gateway = AiGateway::new().expect("gateway should initialize");
    let result = task_cancellation::run_document_task("gateway-ordinary-scope", async move {
        gateway
            .post_json(
                url,
                None,
                &json!({ "model": "test", "stream": false }),
                5_000,
                1_024,
            )
            .await
            .map_err(gateway_to_command)
    })
    .await
    .expect("ordinary request should succeed");
    assert_eq!(result["ok"], true);

    let request = request_receiver
        .await
        .expect("mock server should capture the request");
    assert!(request.contains("\"stream\":false"));
    assert!(!request.contains("stream_options"));
    server.join().expect("mock server should finish");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn document_cancellation_interrupts_initial_header_wait() {
    const DOCUMENT_ID: &str = "gateway-cancel-before-headers";
    let listener = TcpListener::bind("127.0.0.1:0").expect("mock listener should bind");
    let url = Url::parse(&format!(
        "http://{}/headers",
        listener.local_addr().expect("mock address should resolve")
    ))
    .expect("mock URL should parse");
    let (accepted_sender, accepted_receiver) = oneshot::channel();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("streaming request should arrive");
        let _ = read_http_request(&mut stream);
        let _ = accepted_sender.send(());
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("server read timeout should set");
        let mut buffer = [0_u8; 1];
        let _ = stream.read(&mut buffer);
    });

    let gateway = AiGateway::new().expect("gateway should initialize");
    let task = tokio::spawn(async move {
        task_cancellation::run_document_task(DOCUMENT_ID, async move {
            gateway
                .post_document_json(url, None, &json!({ "model": "test" }), 30_000, 1_024)
                .await
                .map_err(gateway_to_command)
        })
        .await
    });
    accepted_receiver
        .await
        .expect("mock server should accept the request");
    task_cancellation::cancel_document_task(DOCUMENT_ID)
        .expect("document task should be cancellable");
    let failure = task
        .await
        .expect("document task join should succeed")
        .expect_err("header wait should be canceled");
    assert_eq!(failure.code, "ai_request_canceled");
    server.join().expect("mock server should finish");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn document_cancellation_discards_an_active_partial_stream() {
    const DOCUMENT_ID: &str = "gateway-cancel-active-stream";
    let listener = TcpListener::bind("127.0.0.1:0").expect("mock listener should bind");
    let url = Url::parse(&format!(
        "http://{}/stream",
        listener.local_addr().expect("mock address should resolve")
    ))
    .expect("mock URL should parse");
    let (chunk_sender, chunk_receiver) = oneshot::channel();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("streaming request should arrive");
        let _ = read_http_request(&mut stream);
        write_sse_headers(&mut stream);
        write_chunk(
            &mut stream,
            br#"data: {"choices":[{"delta":{"content":"partial"}}]}

"#,
        );
        stream.flush().expect("partial stream should flush");
        let _ = chunk_sender.send(());
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("server read timeout should set");
        let mut buffer = [0_u8; 1];
        let _ = stream.read(&mut buffer);
    });

    let gateway = AiGateway::new().expect("gateway should initialize");
    let task = tokio::spawn(async move {
        task_cancellation::run_document_task(DOCUMENT_ID, async move {
            gateway
                .post_document_json(url, None, &json!({ "model": "test" }), 30_000, 1_024)
                .await
                .map_err(gateway_to_command)
        })
        .await
    });
    chunk_receiver
        .await
        .expect("mock server should send one partial chunk");
    task_cancellation::cancel_document_task(DOCUMENT_ID)
        .expect("document task should be cancellable");
    let failure = task
        .await
        .expect("document task join should succeed")
        .expect_err("active stream should be canceled");
    assert_eq!(failure.code, "ai_request_canceled");
    server.join().expect("mock server should finish");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancellation_during_retry_backoff_prevents_a_second_request() {
    const DOCUMENT_ID: &str = "gateway-cancel-retry-backoff";
    let listener = TcpListener::bind("127.0.0.1:0").expect("mock listener should bind");
    let url = Url::parse(&format!(
        "http://{}/retry",
        listener.local_addr().expect("mock address should resolve")
    ))
    .expect("mock URL should parse");
    let (response_sender, response_receiver) = oneshot::channel();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("first request should arrive");
        let _ = read_http_request(&mut stream);
        write_json_response(&mut stream, 503, "");
        let _ = response_sender.send(());

        listener
            .set_nonblocking(true)
            .expect("mock listener should become nonblocking");
        thread::sleep(Duration::from_millis(1_050));
        listener.accept().is_ok()
    });

    let gateway = AiGateway::new().expect("gateway should initialize");
    let task = tokio::spawn(async move {
        task_cancellation::run_document_task(DOCUMENT_ID, async move {
            gateway
                .post_document_json(url, None, &json!({ "model": "test" }), 30_000, 1_024)
                .await
                .map_err(gateway_to_command)
        })
        .await
    });
    response_receiver
        .await
        .expect("mock server should return the transient response");
    wait_for_retry_count(DOCUMENT_ID, 1).await;
    task_cancellation::cancel_document_task(DOCUMENT_ID)
        .expect("document task should be cancellable during backoff");
    let failure = task
        .await
        .expect("document task join should succeed")
        .expect_err("retry backoff should be canceled");
    assert_eq!(failure.code, "ai_request_canceled");
    assert!(!server.join().expect("mock server should finish"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_transient_document_failure_retries_once_from_scratch() {
    const DOCUMENT_ID: &str = "gateway-one-retry-success";
    let listener = TcpListener::bind("127.0.0.1:0").expect("mock listener should bind");
    let url = Url::parse(&format!(
        "http://{}/retry-success",
        listener.local_addr().expect("mock address should resolve")
    ))
    .expect("mock URL should parse");
    let server = thread::spawn(move || {
        let (mut first, _) = listener.accept().expect("first request should arrive");
        let first_request = read_http_request(&mut first);
        write_json_response(&mut first, 503, "");

        let (mut second, _) = listener.accept().expect("retry request should arrive");
        let second_request = read_http_request(&mut second);
        write_sse_headers(&mut second);
        write_chunk(
            &mut second,
            br#"data: {"choices":[{"delta":{"content":"{}"},"finish_reason":"stop"}]}

data: [DONE]

"#,
        );
        second
            .write_all(b"0\r\n\r\n")
            .expect("terminal chunk should write");
        second.flush().expect("successful stream should flush");
        (first_request, second_request)
    });

    let gateway = AiGateway::new().expect("gateway should initialize");
    let result: Value = task_cancellation::run_document_task(DOCUMENT_ID, async move {
        gateway
            .post_document_json(url, None, &json!({ "model": "test" }), 5_000, 1_024)
            .await
            .map_err(gateway_to_command)
    })
    .await
    .expect("the bounded retry should succeed");
    assert_eq!(result.pointer("/choices/0/message/content"), Some(&json!("{}")));

    let (first, second) = server.join().expect("mock server should finish");
    assert!(first.contains("\"stream\":true"));
    assert!(second.contains("\"stream\":true"));
    assert!(first.contains("stream_options"));
    assert!(second.contains("stream_options"));
}

async fn wait_for_retry_count(document_id: &str, expected: u64) {
    for _ in 0..200 {
        if task_cancellation::document_task_progress(document_id)
            .is_some_and(|progress| progress.retry_count >= expected)
        {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("document task did not enter retry backoff");
}

fn gateway_to_command(error: AiGatewayError) -> CommandError {
    CommandError::new(error.safe.code, error.safe.message)
}

fn read_http_request(stream: &mut TcpStream) -> String {
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .expect("mock read timeout should set");
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 4_096];
    loop {
        let count = stream.read(&mut buffer).expect("request bytes should read");
        if count == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..count]);
        let Some(header_end) = find_subsequence(&bytes, b"\r\n\r\n").map(|index| index + 4)
        else {
            continue;
        };
        let headers = String::from_utf8_lossy(&bytes[..header_end]);
        let content_length = headers
            .lines()
            .find_map(|line| {
                line.split_once(':').and_then(|(name, value)| {
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())
                        .flatten()
                })
            })
            .unwrap_or(0);
        if bytes.len() >= header_end.saturating_add(content_length) {
            break;
        }
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn write_json_response(stream: &mut TcpStream, status: u16, body: &str) {
    let reason = match status {
        200 => "OK",
        503 => "Service Unavailable",
        _ => "Response",
    };
    write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .expect("JSON response should write");
    stream.flush().expect("JSON response should flush");
}

fn write_sse_headers(stream: &mut TcpStream) {
    stream
        .write_all(
            b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\nConnection: keep-alive\r\n\r\n",
        )
        .expect("SSE headers should write");
}

fn write_chunk(stream: &mut TcpStream, body: &[u8]) {
    write!(stream, "{:X}\r\n", body.len()).expect("chunk length should write");
    stream.write_all(body).expect("chunk body should write");
    stream
        .write_all(b"\r\n")
        .expect("chunk terminator should write");
}