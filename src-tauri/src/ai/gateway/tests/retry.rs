use std::time::Duration;

use super::super::{
    errors::error,
    retry::{document_retry_delay, should_retry_document_stream_error},
};

#[test]
fn document_stream_retry_is_limited_to_transient_transport_failures() {
    assert!(should_retry_document_stream_error(&error(
        "ai_provider_unavailable",
        "unavailable",
        true,
        None,
    )));
    assert!(should_retry_document_stream_error(&error(
        "ai_network_error",
        "network",
        true,
        None,
    )));
    assert!(!should_retry_document_stream_error(&error(
        "ai_authentication_failed",
        "credential",
        false,
        None,
    )));
    assert!(!should_retry_document_stream_error(&error(
        "ai_context_length_exceeded",
        "context",
        false,
        None,
    )));
}

#[test]
fn retry_after_is_bounded_before_falling_back_to_default_delay() {
    assert_eq!(
        document_retry_delay(&error("ai_rate_limited", "rate", true, Some(3))),
        Duration::from_secs(3)
    );
    assert_eq!(
        document_retry_delay(&error("ai_rate_limited", "rate", true, Some(30))),
        Duration::from_millis(750)
    );
}
