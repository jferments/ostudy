//! AI provider configuration, credential isolation, provider gateway, and
//! task-specific generation services.
//!
//! Provider networking remains native-only. Feature services call narrow Rust
//! tasks that own prompts, source grounding, output schemas, and validation;
//! React never receives credentials or arbitrary provider completions.

mod catalog;
pub(crate) mod document_intelligence;
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
