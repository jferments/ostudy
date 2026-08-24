//! Provider verification, capability evidence, and model discovery.

mod capabilities;
mod commands;
mod contracts;
mod phases;
mod repository;
mod runner;

pub(crate) use commands::{
    get_ai_provider_probe_result, list_ai_provider_models, test_ai_provider_profile,
};
pub(crate) use contracts::{
    AiProviderProbeResultDto, CapabilityStatus, DocumentProbeEvidence, PersistedCapability,
};
pub(crate) use repository::{load_capability_records, load_document_probe_evidence};

#[cfg(test)]
mod tests;
