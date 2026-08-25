use serde_json::json;

use super::super::streaming::{sse::SseChatAccumulator, usage::safe_usage_envelope};

#[test]
fn accepts_keepalive_progress_and_json_split_across_arbitrary_chunks() {
    let mut stream = SseChatAccumulator::new(1_024, 4_096);
    stream
        .push_bytes(b": keep-alive\r\n\r\ndata: {\"prompt_progress\":{\"processed\":2}}\n\n")
        .unwrap();
    stream
        .push_bytes(b"data: {\"choices\":[{\"delta\":{\"content\":\"hel")
        .unwrap();
    stream
        .push_bytes(b"lo\"}}]}\r\n\r\ndata: [DONE]\n\n")
        .unwrap();
    stream.finish().unwrap();
    assert_eq!(stream.content, "hello");
    assert_eq!(stream.comment_events, 1);
    assert!(stream.done);
}

#[test]
fn keepalive_and_reasoning_are_progress_without_retained_reasoning() {
    let mut stream = SseChatAccumulator::new(1_024, 16_384);
    stream.push_bytes(b": OPENROUTER PROCESSING\n").unwrap();
    assert_eq!(stream.model_progress_bytes(), 0);
    stream
        .push_bytes(b"data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"working\"}}]}\n")
        .unwrap();
    assert_eq!(stream.content, "");
    assert_eq!(stream.reasoning_bytes, 7);
    assert_eq!(stream.model_progress_bytes(), 7);
}

#[test]
fn collects_fragmented_tool_arguments_and_requires_a_stable_name() {
    let mut stream = SseChatAccumulator::new(2_048, 8_192);
    stream
        .push_bytes(
            br#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"name":"submit_ostudy_document_result","arguments":"{\"cards\":["}}]}}]}
"#,
        )
        .unwrap();
    stream
        .push_bytes(
            br#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"]}"}}]}}]}

data: [DONE]
"#,
        )
        .unwrap();
    stream.finish().unwrap();
    assert_eq!(
        stream.tool_name.as_deref(),
        Some("submit_ostudy_document_result")
    );
    assert_eq!(stream.tool_arguments, "{\"cards\":[]}");

    let mut changed = SseChatAccumulator::new(2_048, 8_192);
    changed
        .push_bytes(
            br#"data: {"choices":[{"delta":{"tool_calls":[{"function":{"name":"first","arguments":"{"}}]}}]}
"#,
        )
        .unwrap();
    let failure = changed
        .push_bytes(
            br#"data: {"choices":[{"delta":{"tool_calls":[{"function":{"name":"second","arguments":"}"}}]}}]}
"#,
        )
        .expect_err("tool name changes must fail");
    assert_eq!(failure.safe.code, "ai_provider_protocol_error");
}

#[test]
fn final_full_message_snapshot_does_not_duplicate_streamed_deltas() {
    let mut stream = SseChatAccumulator::new(1_024, 4_096);
    stream
        .push_bytes(
            br#"data: {"choices":[{"delta":{"content":"hello"}}]}
data: {"choices":[{"message":{"content":"hello world"},"finish_reason":"stop"}]}
data: [DONE]
"#,
        )
        .unwrap();
    stream.finish().unwrap();
    assert_eq!(stream.content, "hello");
}

#[test]
fn in_band_error_rejects_partial_output_and_redacts_provider_message() {
    let mut stream = SseChatAccumulator::new(1_024, 16_384);
    stream
        .push_bytes(b"data: {\"choices\":[{\"delta\":{\"content\":\"partial\"}}]}\n")
        .unwrap();
    let failure = stream
        .push_bytes(b"data: {\"error\":{\"code\":\"server_error\",\"message\":\"private upstream detail\"}}\n")
        .expect_err("embedded stream error should abort");
    assert_eq!(failure.safe.code, "ai_server_unavailable");
    assert!(!failure.safe.message.contains("private upstream detail"));
}

#[test]
fn final_usage_preserves_cache_and_reasoning_counters() {
    let mut stream = SseChatAccumulator::new(2_048, 8_192);
    stream
        .push_bytes(
            br#"data: {"choices":[{"delta":{"content":"{}"},"finish_reason":"stop"}]}
data: {"usage":{"prompt_tokens":1000,"completion_tokens":250,"total_tokens":1250,"prompt_tokens_details":{"cached_tokens":700,"cache_write_tokens":120},"completion_tokens_details":{"reasoning_tokens":80},"cost":0.0003}}
data: [DONE]
"#,
        )
        .unwrap();
    stream.finish().unwrap();
    let usage = stream.usage.expect("usage should be retained");
    assert_eq!(usage["prompt_tokens"], 1_000);
    assert_eq!(usage["completion_tokens_details"]["reasoning_tokens"], 80);
    assert_eq!(usage["prompt_tokens_details"]["cached_tokens"], 700);
    assert_eq!(usage["prompt_tokens_details"]["cache_write_tokens"], 120);
}

#[test]
fn llama_timings_are_normalized_into_standard_usage() {
    let usage = safe_usage_envelope(&json!({
        "timings": {"cache_n": 180, "prompt_n": 40, "predicted_n": 45}
    }))
    .expect("llama timings should be usable");
    assert_eq!(usage["prompt_tokens"], 220);
    assert_eq!(usage["completion_tokens"], 45);
    assert_eq!(usage["total_tokens"], 265);
    assert_eq!(usage["prompt_tokens_details"]["cached_tokens"], 180);
}

#[test]
fn wire_and_content_limits_are_independent() {
    let mut wire_limited = SseChatAccumulator::new(1_024, 8);
    assert_eq!(
        wire_limited
            .push_bytes(b"data: [DONE]\n")
            .unwrap_err()
            .safe
            .code,
        "ai_response_too_large"
    );

    let mut content_limited = SseChatAccumulator::new(3, 1_024);
    assert_eq!(
        content_limited
            .push_bytes(b"data: {\"choices\":[{\"delta\":{\"content\":\"four\"}}]}\n")
            .unwrap_err()
            .safe
            .code,
        "ai_response_too_large"
    );
}
