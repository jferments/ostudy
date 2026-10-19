use serde_json::json;
use url::Url;

use super::super::{
    contracts::ProviderWire,
    metadata::is_openrouter_url,
    request::{prepare_streaming_body, streaming_url},
};

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
    let prepared = prepare_streaming_body(
        &json!({
            "model": "example/model",
            "stream": false
        }),
        ProviderWire::OpenAiCompatible,
    )
    .unwrap();
    assert_eq!(prepared["stream"], true);
    assert_eq!(prepared["stream_options"]["include_usage"], true);
}

#[test]
fn anthropic_streams_report_usage_without_unknown_request_fields() {
    let prepared = prepare_streaming_body(
        &json!({"model": "claude-haiku-5-5", "stream": false}),
        ProviderWire::AnthropicMessages,
    )
    .unwrap();
    assert_eq!(prepared["stream"], true);
    assert!(prepared.get("stream_options").is_none());
}

#[test]
fn gemini_streams_from_the_sibling_method_without_a_stream_field() {
    let prepared = prepare_streaming_body(
        &json!({"contents": [], "generationConfig": {}}),
        ProviderWire::GeminiGenerateContent,
    )
    .unwrap();
    assert!(prepared.get("stream").is_none());
    assert!(prepared.get("stream_options").is_none());
    let url = streaming_url(
        url::Url::parse(
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:generateContent",
        )
        .unwrap(),
        ProviderWire::GeminiGenerateContent,
    );
    assert_eq!(
        url.as_str(),
        "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:streamGenerateContent?alt=sse"
    );
    let unchanged = streaming_url(
        url::Url::parse("https://api.openai.com/v1/chat/completions").unwrap(),
        ProviderWire::OpenAiCompatible,
    );
    assert_eq!(
        unchanged.as_str(),
        "https://api.openai.com/v1/chat/completions"
    );
}

#[test]
fn streaming_request_body_must_be_an_object() {
    let failure = prepare_streaming_body(
        &json!(["not", "an", "object"]),
        ProviderWire::OpenAiCompatible,
    )
    .expect_err("non-object chat request should fail");
    assert_eq!(failure.safe.code, "ai_provider_protocol_error");
}
