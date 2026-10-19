//! Exercise transport and durable accounting together at the generation owner.
use std::time::Duration;

use super::*;

fn event(value: Value) -> String {
    format!("data: {value}\n\n")
}

fn serve(body: String) -> (Url, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = Url::parse(&format!("http://{}/usage", listener.local_addr().unwrap())).unwrap();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        read_http_request(&mut stream);
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        ).unwrap();
        stream.flush().unwrap();
    });
    (url, server)
}

#[derive(Debug, PartialEq)]
struct RecordedAttempt {
    status: String,
    code: Option<String>,
    input: Option<i64>,
    output: Option<i64>,
    total: Option<i64>,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn document_stream_failures_keep_reported_usage_without_inventing_totals() {
    let complete_usage = json!({"usage": {
        "prompt_tokens":100,"completion_tokens":400,"total_tokens":500,
        "completion_tokens_details":{"reasoning_tokens":390},"cost":0.2
    }});
    let length = event(json!({"choices":[{
        "delta":{"reasoning_content":"PRIVATE REASONING"},"finish_reason":"length"
    }]}));
    let success = event(json!({"choices":[{
        "delta":{"content":"{}"},"finish_reason":"stop"
    }]}));
    let cases = [
        (
            format!("{length}{}data: [DONE]\n\n", event(complete_usage.clone())),
            Some("ai_document_output_truncated"),
            Some(100),
            Some(400),
            Some(500),
        ),
        (
            format!("{}data: broken-json\n\n", event(complete_usage.clone())),
            Some("ai_provider_protocol_error"),
            Some(100),
            Some(400),
            Some(500),
        ),
        (
            format!(
                "{success}{}{}{}data: [DONE]\n\n",
                event(complete_usage.clone()),
                event(complete_usage),
                event(json!({"usage":{"cost":0.3}}))
            ),
            None,
            Some(100),
            Some(400),
            Some(500),
        ),
        (
            format!(
                "{length}{}data: [DONE]\n\n",
                event(json!({"usage":{"prompt_tokens":100}}))
            ),
            Some("ai_document_output_truncated"),
            Some(100),
            None,
            None,
        ),
        (format!("{success}data: [DONE]\n\n"), None, None, None, None),
    ];
    for (body, expected_error, input, output, total) in cases {
        let (url, server) = serve(body);
        let database = Database::open_in_memory().unwrap();
        let mut operation =
            Operation::begin(&database, None, None, None, "stream_usage_fixture").unwrap();
        let gateway = AiGateway::new().unwrap();
        let request = json!({"model":"fixture","max_tokens":400});
        let signal = task_cancellation::CancellationSignal::new();
        let result = scoped_request(
            Some(operation.context()),
            "guide_authoring",
            Some("usage-window"),
            "openrouter",
            gateway.post_task_json(url, None, &request, 5_000, 32_000, &signal),
        )
        .await;
        assert_eq!(
            result.as_ref().err().map(|error| error.safe.code.as_str()),
            expected_error
        );
        server.join().unwrap();
        operation.finish(result.is_ok());
        database
            .with_connection(|connection| {
                let count: i64 = connection
                    .query_row("SELECT COUNT(*) FROM generation_requests", [], |row| {
                        row.get(0)
                    })
                    .map_err(CommandError::database)?;
                let (recorded, payload) = connection.query_row(
                "SELECT status,error_code,input_tokens,output_tokens,total_tokens,payload_bytes_json
                 FROM generation_requests",
                [], |row| Ok((RecordedAttempt {
                    status: row.get(0)?, code: row.get(1)?, input: row.get(2)?,
                    output: row.get(3)?, total: row.get(4)?,
                }, row.get::<_, String>(5)?)),
            ).map_err(CommandError::database)?;
                assert_eq!(
                    count, 1,
                    "one physical stream, not one count per usage event"
                );
                assert_eq!(
                    recorded,
                    RecordedAttempt {
                        status: if expected_error.is_some() {
                            "failed"
                        } else {
                            "response"
                        }
                        .into(),
                        code: expected_error.map(str::to_string),
                        input,
                        output,
                        total,
                    }
                );
                assert!(!payload.contains("PRIVATE REASONING"));
                Ok(())
            })
            .unwrap();
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn anthropic_streams_record_billed_input_including_cache_reads() {
    let body = [
        json!({"type":"message_start","message":{"id":"msg_1","model":"claude-haiku-5-5",
            "usage":{"input_tokens":12,"cache_read_input_tokens":300,
                "cache_creation_input_tokens":40,"output_tokens":1}}}),
        json!({"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}),
        json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"{}"}}),
        json!({"type":"content_block_stop","index":0}),
        json!({"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":9}}),
        json!({"type":"message_stop"}),
    ]
    .into_iter()
    .map(event)
    .collect::<String>();
    let (url, server) = serve(body);
    let database = Database::open_in_memory().unwrap();
    let mut operation =
        Operation::begin(&database, None, None, None, "anthropic_usage_fixture").unwrap();
    let gateway = AiGateway::new().unwrap();
    let request = json!({"model":"claude-haiku-5-5","max_tokens":400,"messages":[]});
    let signal = task_cancellation::CancellationSignal::new();
    let access = crate::ai::gateway::ProviderAccess::new(
        crate::ai::gateway::ProviderWire::AnthropicMessages,
        Some("sentinel"),
    );
    let response = scoped_request(
        Some(operation.context()),
        "guide_authoring",
        Some("usage-window"),
        "claude",
        gateway.post_task_json(url, access, &request, 5_000, 32_000, &signal),
    )
    .await
    .unwrap();
    server.join().unwrap();
    operation.finish(true);
    assert_eq!(response["choices"][0]["message"]["content"], "{}");
    assert_eq!(response["choices"][0]["finish_reason"], "stop");
    database
        .with_connection(|connection| {
            let recorded = connection
                .query_row(
                    "SELECT input_tokens,output_tokens,total_tokens FROM generation_requests",
                    [],
                    |row| {
                        Ok((
                            row.get::<_, Option<i64>>(0)?,
                            row.get::<_, Option<i64>>(1)?,
                            row.get::<_, Option<i64>>(2)?,
                        ))
                    },
                )
                .map_err(CommandError::database)?;
            assert_eq!(recorded, (Some(352), Some(9), Some(361)));
            Ok(())
        })
        .unwrap();
}
