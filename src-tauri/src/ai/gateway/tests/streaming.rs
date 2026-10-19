use serde_json::json;

use super::super::{
    contracts::ProviderWire,
    streaming::{sse::SseChatAccumulator, usage::safe_usage_envelope},
};

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
        stream.tool_calls()[0]["function"]["name"].as_str(),
        Some("submit_ostudy_document_result")
    );
    assert_eq!(
        stream.tool_calls()[0]["function"]["arguments"],
        "{\"cards\":[]}"
    );

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
    assert_eq!(stream.content, "hello world");
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

#[test]
fn interleaved_tool_indices_never_concatenate_independent_json_documents() {
    let mut stream = SseChatAccumulator::new(4096, 16384);
    for calls in [
        json!([{"index":0,"id":"a","function":{"name":"submit","arguments":"{\"units\":["}},
               {"index":1,"id":"b","function":{"name":"submit","arguments":"{\"units\":["}}]),
        json!([{"index":1,"function":{"arguments":"]}"}}]),
        json!([{"index":0,"function":{"arguments":"]}"}}]),
    ] {
        stream
            .push_bytes(
                format!(
                    "data: {}\n",
                    json!({"choices":[{"delta":{"tool_calls":calls}}]})
                )
                .as_bytes(),
            )
            .unwrap();
    }
    stream.finish().unwrap();
    let calls = stream.tool_calls();
    assert_eq!(calls.len(), 2);
    for call in calls {
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(
                call["function"]["arguments"].as_str().unwrap()
            )
            .unwrap(),
            json!({"units":[]})
        );
    }
}

#[test]
fn repeated_content_deltas_are_literal_and_tool_bytes_share_the_content_limit() {
    let mut stream = SseChatAccumulator::new(4, 4096);
    for content in ["a", "a", "ab"] {
        stream
            .push_bytes(
                format!(
                    "data: {}\n",
                    json!({"choices":[{"delta":{"content":content}}]})
                )
                .as_bytes(),
            )
            .unwrap();
    }
    assert_eq!(stream.content, "aaab");
    let failure = stream.push_bytes(b"data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"name\":\"submit\",\"arguments\":\"{}\"}}]}}]}\n").unwrap_err();
    assert_eq!(failure.safe.code, "ai_response_too_large");
}

#[test]
fn ambiguous_indices_and_changed_ids_are_rejected_without_content_in_errors() {
    let mut stream = SseChatAccumulator::new(4096, 16384);
    for id in ["first", "second"] {
        let result = stream.push_bytes(
            format!(
                "data: {}\n",
                json!({"choices":[{"delta":{"tool_calls":[{
                    "index":0,"id":id,"function":{"name":"submit","arguments":"PRIVATE"}
                }]}}]})
            )
            .as_bytes(),
        );
        if id == "first" {
            result.unwrap();
        } else {
            let failure = result.unwrap_err();
            assert_eq!(failure.safe.code, "ai_provider_protocol_error");
            assert!(!failure.safe.message.contains("PRIVATE"));
        }
    }
}

fn anthropic_stream() -> SseChatAccumulator {
    SseChatAccumulator::for_request(
        4_096,
        65_536,
        false,
        ProviderWire::AnthropicMessages,
        None,
        None,
    )
}

/// Recorded shape of a Messages stream with thinking omitted, as Claude sends
/// it by default: an empty thinking block, then the text.
const ANTHROPIC_STREAM: &str = concat!(
    "event: message_start\n",
    "data: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_01\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[],\"model\":\"claude-sonnet-5-5\",\"stop_reason\":null,\"usage\":{\"input_tokens\":12,\"cache_read_input_tokens\":2000,\"cache_creation_input_tokens\":0,\"output_tokens\":1}}}\n\n",
    "event: ping\n",
    "data: {\"type\":\"ping\"}\n\n",
    "event: content_block_start\n",
    "data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"thinking\",\"thinking\":\"\",\"signature\":\"\"}}\n\n",
    "event: content_block_delta\n",
    "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"\"}}\n\n",
    "event: content_block_delta\n",
    "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"signature_delta\",\"signature\":\"EqQB\"}}\n\n",
    "event: content_block_stop\n",
    "data: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
    "event: content_block_start\n",
    "data: {\"type\":\"content_block_start\",\"index\":1,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n",
    "event: content_block_delta\n",
    "data: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"text_delta\",\"text\":\"{\\\"ok\\\":\"}}\n\n",
    "event: content_block_delta\n",
    "data: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"text_delta\",\"text\":\"true}\"}}\n\n",
    "event: content_block_stop\n",
    "data: {\"type\":\"content_block_stop\",\"index\":1}\n\n",
    "event: message_delta\n",
    "data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\",\"stop_sequence\":null},\"usage\":{\"output_tokens\":40}}\n\n",
    "event: message_stop\n",
    "data: {\"type\":\"message_stop\"}\n\n",
);

#[test]
fn anthropic_events_reconstruct_text_finish_and_billed_usage() {
    let mut stream = anthropic_stream();
    for chunk in ANTHROPIC_STREAM.as_bytes().chunks(11) {
        stream.push_bytes(chunk).unwrap();
    }
    stream.finish().unwrap();
    assert!(stream.done);
    assert_eq!(stream.content, "{\"ok\":true}");
    assert_eq!(stream.finish_reason.as_deref(), Some("stop"));
    assert_eq!(stream.model.as_deref(), Some("claude-sonnet-5-5"));
    assert_eq!(stream.generation_id.as_deref(), Some("msg_01"));
    assert_eq!(stream.comment_events, 1);
    let usage = stream.usage.as_ref().expect("usage should be reported");
    // input_tokens excludes cache reads, which are billed separately.
    assert_eq!(usage["prompt_tokens"], 2_012);
    assert_eq!(usage["completion_tokens"], 40);
    assert_eq!(usage["total_tokens"], 2_052);
    assert_eq!(usage["prompt_tokens_details"]["cached_tokens"], 2_000);
    assert_eq!(usage["prompt_tokens_details"]["cache_write_tokens"], 0);
}

#[test]
fn anthropic_length_and_refusal_stops_map_to_shared_finish_reasons() {
    for (stop, expected) in [("max_tokens", "length"), ("refusal", "content_filter")] {
        let mut stream = anthropic_stream();
        stream
            .push_bytes(
                format!(
                    "data: {}\n\n",
                    json!({"type":"message_delta","delta":{"stop_reason":stop},"usage":{"output_tokens":3}})
                )
                .as_bytes(),
            )
            .unwrap();
        assert_eq!(stream.finish_reason.as_deref(), Some(expected));
    }
}

#[test]
fn anthropic_overload_mid_stream_is_a_retryable_failure() {
    let mut stream = anthropic_stream();
    let failure = stream
        .push_bytes(
            b"event: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"Overloaded\"}}\n\n",
        )
        .expect_err("an error event should fail the stream");
    assert_eq!(failure.safe.code, "ai_server_unavailable");
    assert!(failure.safe.retryable);
}
