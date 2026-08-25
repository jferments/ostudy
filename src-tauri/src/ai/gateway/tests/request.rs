use serde_json::json;
use url::Url;

use super::super::{metadata::is_openrouter_url, request::prepare_streaming_body};

#[test]
fn recognizes_only_the_fixed_openrouter_https_host_for_attribution() {
    assert!(is_openrouter_url(
        &Url::parse("https://openrouter.ai/api/v1/models").unwrap()
    ));
    assert!(!is_openrouter_url(
        &Url::parse("https://openrouter.ai.evil.example/api/v1/models").unwrap()
    ));
    assert!(!is_openrouter_url(
        &Url::parse("http://openrouter.ai/api/v1/models").unwrap()
    ));
}

#[test]
fn every_document_stream_requests_a_final_usage_chunk() {
    let prepared = prepare_streaming_body(&json!({
        "model": "example/model",
        "stream": false
    }))
    .unwrap();
    assert_eq!(prepared["stream"], true);
    assert_eq!(prepared["stream_options"]["include_usage"], true);
}

#[test]
fn streaming_request_body_must_be_an_object() {
    let failure = prepare_streaming_body(&json!(["not", "an", "object"]))
        .expect_err("non-object chat request should fail");
    assert_eq!(failure.safe.code, "ai_provider_protocol_error");
}
