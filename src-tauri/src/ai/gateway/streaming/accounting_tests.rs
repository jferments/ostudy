use super::*;

#[test]
fn document_usage_snapshots_merge_known_fields_without_summing_or_erasing_them() {
    let mut stream = SseChatAccumulator::new(1_024, 8_192);
    for value in [
        json!({"usage":{"prompt_tokens":100,"completion_tokens":20,"prompt_tokens_details":{"cached_tokens":50}}}),
        json!({"usage":{"completion_tokens":40}}),
        json!({"usage":{"completion_tokens":40}}),
        json!({"usage":{"cost":0.1,"prompt_tokens_details":{"cached_tokens":0}}}),
    ] {
        stream.push_bytes(format!("data: {value}\n\n").as_bytes()).unwrap();
    }
    let usage = stream.usage.as_ref().unwrap();
    assert_eq!(usage["prompt_tokens"], 100);
    assert_eq!(usage["completion_tokens"], 40);
    assert_eq!(usage["total_tokens"], 140);
    assert_eq!(usage["prompt_tokens_details"]["cached_tokens"], 0);
    assert!(usage["completion_tokens_details"]["reasoning_tokens"].is_null());
    assert!(!stream.done);
    stream.push_bytes(b"data: [DONE]\n\n").unwrap();
    assert!(stream.done);
}
