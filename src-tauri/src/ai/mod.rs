//! AI provider configuration, credential isolation, and provider gateway.
//!
//! This subsystem is deliberately separate from authoring. Provider networking
//! is available only through narrow profile/model/probe commands here; future
//! learning tasks call the internal gateway from task-specific services.

mod catalog;
mod endpoint_policy;
mod gateway;
pub(crate) mod probes;
pub(crate) mod profiles;
mod providers;
mod secrets;

#[cfg(test)]
mod backup_tests;
#[cfg(test)]
mod provider_contract_tests;

use std::sync::Mutex;

use gateway::AiGateway;
use secrets::SecretManager;

pub struct AiRuntimeState {
    pub(crate) secrets: SecretManager,
    pub(crate) gateway: AiGateway,
    pub(crate) profile_mutations: Mutex<()>,
}

impl AiRuntimeState {
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            secrets: SecretManager::default(),
            gateway: AiGateway::new()?,
            profile_mutations: Mutex::new(()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_state_builds_without_contacting_a_provider() {
        AiRuntimeState::new().expect("AI runtime should initialize");
    }
}
