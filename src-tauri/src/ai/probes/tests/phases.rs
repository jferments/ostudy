use crate::ai::{
    gateway::SafeAiError,
    probes::{
        contracts::ProbePhaseStatus,
        phases::{failed_phase, passed_phase, skipped_phase},
    },
};

#[test]
fn phase_builders_preserve_status_latency_and_safe_errors() {
    let passed = passed_phase("text", "Text", "Passed.", Some(4));
    assert_eq!(passed.phase, "text");
    assert_eq!(passed.label, "Text");
    assert_eq!(passed.status, ProbePhaseStatus::Passed);
    assert_eq!(passed.latency_ms, Some(4));
    assert!(passed.error.is_none());

    let failure = SafeAiError {
        code: "ai_connection_failed".to_string(),
        message: "The provider could not be reached.".to_string(),
        retryable: true,
        retry_after_seconds: None,
    };
    let failed = failed_phase(
        "connection",
        "Connection",
        "The provider request failed.",
        Some(9),
        failure,
    );
    assert_eq!(failed.status, ProbePhaseStatus::Failed);
    assert_eq!(failed.latency_ms, Some(9));
    assert_eq!(
        failed.error.as_ref().map(|error| error.code.as_str()),
        Some("ai_connection_failed")
    );

    let skipped = skipped_phase("optional", "Optional", "Not required.");
    assert_eq!(skipped.status, ProbePhaseStatus::Skipped);
    assert_eq!(skipped.latency_ms, None);
    assert!(skipped.error.is_none());
}
