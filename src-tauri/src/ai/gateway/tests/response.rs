use super::super::errors::{classify_http_status, classify_provider_error_response};

#[test]
fn classifies_rate_limits_without_provider_body_text() {
    let failure = classify_http_status(429, Some(17));
    assert_eq!(failure.safe.code, "ai_http_rate_limited");
    assert_eq!(failure.safe.retry_after_seconds, Some(17));
}

#[test]
fn classifies_bad_requests_as_compatibility_failures() {
    let failure = classify_http_status(400, None);
    assert_eq!(failure.safe.code, "ai_http_bad_request");
    assert!(failure.safe.message.contains("server version"));
}

#[test]
fn openrouter_envelopes_are_typed_and_redacted() {
    let failure = classify_provider_error_response(
        503,
        Some(2),
        br#"{"error":{"code":503,"message":"private capacity detail","metadata":{"error_type":"provider_overloaded","provider_code":"capacity"}},"openrouter_metadata":{"strategy":"fallback","attempt":3,"summary":"available=2"}}"#,
        true,
        Some("gen-http-test"),
        false,
    );
    assert_eq!(failure.safe.code, "ai_provider_overloaded");
    assert_eq!(failure.safe.retry_after_seconds, Some(2));
    assert!(!failure.safe.message.contains("private capacity detail"));
}
