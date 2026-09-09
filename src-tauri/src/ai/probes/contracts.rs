use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::CommandError;

use super::super::{catalog::ProviderKind, gateway::SafeAiError, providers::ProviderModel};

pub(super) const ADAPTER_REVISION: i64 = 8;
pub(super) const RESULT_SCHEMA_VERSION: i64 = 1;
pub(super) const MAX_PERSISTED_RESULT_BYTES: usize = 32 * 1024;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AiProfileReferenceInput {
    pub profile_id: String,
    pub revision: i64,
    #[serde(default)]
    pub model_id: Option<String>,
    #[serde(default)]
    pub reasoning_effort: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProbeOverallStatus {
    Verified,
    Partial,
    Failed,
}

impl ProbeOverallStatus {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Verified => "verified",
            Self::Partial => "partial",
            Self::Failed => "failed",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProbePhaseStatus {
    Passed,
    Failed,
    Skipped,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbePhaseDto {
    pub phase: String,
    pub label: String,
    pub status: ProbePhaseStatus,
    pub message: String,
    pub latency_ms: Option<i64>,
    pub error: Option<SafeAiError>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityStatus {
    Supported,
    Unsupported,
    Unknown,
}

impl CapabilityStatus {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Supported => "supported",
            Self::Unsupported => "unsupported",
            Self::Unknown => "unknown",
        }
    }

    pub(super) fn from_db(value: &str) -> Result<Self, CommandError> {
        match value {
            "supported" => Ok(Self::Supported),
            "unsupported" => Ok(Self::Unsupported),
            "unknown" => Ok(Self::Unknown),
            _ => Err(CommandError::new(
                "invalid_ai_capability_status",
                "Saved AI capability evidence is invalid.",
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CapabilitySource {
    Declared,
    Probed,
}

impl CapabilitySource {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Declared => "declared",
            Self::Probed => "probed",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiCapabilityDto {
    pub key: String,
    pub status: CapabilityStatus,
    pub source: CapabilitySource,
    pub value: Option<Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiProviderProbeResultDto {
    pub profile_id: String,
    pub profile_revision: i64,
    pub credential_epoch: i64,
    pub tested_model_id: String,
    pub adapter_revision: i64,
    pub result_schema_version: i64,
    pub overall_status: ProbeOverallStatus,
    pub phases: Vec<ProbePhaseDto>,
    pub capabilities: Vec<AiCapabilityDto>,
    pub tested_at: i64,
    pub persisted: bool,
    pub stale: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiProviderModelsDto {
    pub profile_id: String,
    pub profile_revision: i64,
    pub provider_kind: ProviderKind,
    pub models: Vec<ProviderModel>,
    pub retrieved_at: i64,
}

#[derive(Clone, Debug)]
pub(crate) struct PersistedCapability {
    pub key: String,
    pub status: CapabilityStatus,
    pub value: Option<Value>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct DocumentProbeEvidence {
    pub probe_matches: bool,
    pub image_input_supported: bool,
    pub document_generation_schema_supported: bool,
}
