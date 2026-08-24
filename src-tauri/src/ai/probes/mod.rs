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
    AiProviderProbeResultDto, DocumentProbeEvidence, PersistedCapability,
};
pub(crate) use repository::{load_capabilities, load_document_probe_evidence};

#[cfg(test)]
mod tests;
