use serde_json::json;

use super::super::streaming::usage::{safe_usage_envelope, usage_delta};

#[test]
fn normalized_usage_projects_to_the_document_task_accumulator() {
    let delta = usage_delta(&json!({
        "prompt_tokens": 100,
        "completion_tokens": 40,
        "total_tokens": 140,
        "prompt_tokens_details": {
            "cached_tokens": 60,
            "cache_write_tokens": 20
        },
        "completion_tokens_details": {
            "reasoning_tokens": 10
        }
    }));
    assert_eq!(delta.prompt_tokens, Some(100));
    assert_eq!(delta.completion_tokens, Some(40));
    assert_eq!(delta.total_tokens, Some(140));
    assert_eq!(delta.cached_prompt_tokens, Some(60));
    assert_eq!(delta.cache_write_tokens, Some(20));
    assert_eq!(delta.reasoning_tokens, Some(10));
}

#[test]
fn ordinary_and_nested_openai_usage_shapes_share_one_normalizer() {
    for response in [
        json!({"usage":{"input_tokens":10,"output_tokens":4,"total_tokens":14}}),
        json!({"response":{"usage":{"prompt_tokens":10,"completion_tokens":4,"total_tokens":14}}}),
        json!({"data":{"usage":{"prompt_tokens":10,"completion_tokens":4,"total_tokens":14}}}),
    ] {
        let usage = safe_usage_envelope(&response).expect("usage should normalize");
        assert_eq!(usage["prompt_tokens"], 10);
        assert_eq!(usage["completion_tokens"], 4);
        assert_eq!(usage["total_tokens"], 14);
    }
}
