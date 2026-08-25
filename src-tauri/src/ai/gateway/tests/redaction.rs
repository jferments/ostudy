use super::super::errors::classify_provider_error_response;

#[test]
fn openrouter_http_errors_use_typed_envelope_without_raw_provider_message() {
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
    assert!(failure.safe.message.contains("provider_overloaded"));
    assert!(!failure.safe.message.contains("private capacity detail"));
}

#[test]
fn provider_secret_like_text_never_enters_safe_errors() {
    let failure = classify_provider_error_response(
        502,
        None,
        br#"{"model":"deepseek/example","provider":"DeepSeek","error":{"code":502,"message":"authorization=sentinel-secret private upstream detail","metadata":{"error_type":"provider_unavailable","provider_code":"upstream_bad_gateway"}},"openrouter_metadata":{"attempts":[{"provider":"DeepSeek","model":"deepseek/example","status":502}]}}"#,
        true,
        Some("gen-test-123"),
        true,
    );
    assert_eq!(failure.safe.code, "ai_provider_unavailable");
    assert!(failure.safe.message.contains("DeepSeek"));
    assert!(failure
        .safe
        .message
        .contains("Partial output was discarded"));
    assert!(!failure.safe.message.contains("sentinel-secret"));
    assert!(!failure.safe.message.contains("authorization"));
}
