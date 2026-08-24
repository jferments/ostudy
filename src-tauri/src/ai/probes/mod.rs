//! Provider verification, capability evidence, and model discovery.
//!
//! Serialized probe responses remain owned by `contracts::AiProviderProbeResultDto`;
//! the package facade exposes only the command module and typed capability reads.

mod capabilities;
pub(crate) mod commands;
mod contracts;
mod phases;
mod repository;
mod runner;

pub(crate) use contracts::CapabilityStatus;
pub(crate) use repository::{load_capability_records, load_document_probe_evidence};

#[cfg(test)]
mod tests;
