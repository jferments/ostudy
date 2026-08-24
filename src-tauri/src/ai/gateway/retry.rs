use std::time::Duration;

use super::contracts::AiGatewayError;

const DOCUMENT_RETRY_DELAY_MILLIS: u64 = 750;

pub(super) fn should_retry_document_stream_error(error_value: &AiGatewayError) -> bool {
    matches!(
        error_value.safe.code.as_str(),
        "ai_provider_unavailable"
            | "ai_provider_overloaded"
            | "ai_provider_timeout"
            | "ai_stream_interrupted"
            | "ai_server_unavailable"
            | "ai_network_error"
            | "ai_connection_failed"
            | "ai_request_timeout"
            | "ai_initial_response_timeout"
            | "ai_stream_inactive"
            | "ai_rate_limited"
            | "ai_http_rate_limited"
    )
}

pub(super) fn document_retry_delay(error_value: &AiGatewayError) -> Duration {
    match error_value.safe.retry_after_seconds {
        Some(seconds @ 1..=5) => Duration::from_secs(seconds),
        _ => Duration::from_millis(DOCUMENT_RETRY_DELAY_MILLIS),
    }
}
