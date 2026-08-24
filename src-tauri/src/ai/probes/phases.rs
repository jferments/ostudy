use std::time::Instant;

use super::{
    contracts::{ProbePhaseDto, ProbePhaseStatus},
    super::gateway::SafeAiError,
};

pub(super) fn elapsed_ms(started: Instant) -> i64 {
    i64::try_from(started.elapsed().as_millis()).unwrap_or(i64::MAX)
}

pub(super) fn passed_phase(
    phase: &str,
    label: &str,
    message: &str,
    latency_ms: Option<i64>,
) -> ProbePhaseDto {
    ProbePhaseDto {
        phase: phase.to_string(),
        label: label.to_string(),
        status: ProbePhaseStatus::Passed,
        message: message.to_string(),
        latency_ms,
        error: None,
    }
}

pub(super) fn failed_phase(
    phase: &str,
    label: &str,
    message: &str,
    latency_ms: Option<i64>,
    error: SafeAiError,
) -> ProbePhaseDto {
    ProbePhaseDto {
        phase: phase.to_string(),
        label: label.to_string(),
        status: ProbePhaseStatus::Failed,
        message: message.to_string(),
        latency_ms,
        error: Some(error),
    }
}

pub(super) fn skipped_phase(phase: &str, label: &str, message: &str) -> ProbePhaseDto {
    ProbePhaseDto {
        phase: phase.to_string(),
        label: label.to_string(),
        status: ProbePhaseStatus::Skipped,
        message: message.to_string(),
        latency_ms: None,
        error: None,
    }
}
