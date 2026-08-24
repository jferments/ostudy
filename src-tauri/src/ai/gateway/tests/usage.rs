use serde_json::json;

use super::super::streaming::usage::usage_delta;

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
