//! AI provider configuration, credential isolation, provider gateway, and
//! task-specific generation services.
//!
//! Provider networking remains native-only. Feature services call narrow Rust
//! tasks that own prompts, source grounding, output schemas, and validation;
//! React never receives credentials or arbitrary provider completions.

mod catalog;
pub(crate) mod concurrency;
pub(crate) mod document_intelligence;
pub(crate) mod document_provider;
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

use std::sync::{Arc, Mutex};

use crate::platform::SecureSecretStore;
use gateway::AiGateway;
use secrets::SecretManager;

pub struct AiRuntimeState {
    pub(crate) secrets: SecretManager,
    pub(crate) gateway: AiGateway,
    pub(crate) profile_mutations: Mutex<()>,
}

impl AiRuntimeState {
    pub fn new(secret_store: Arc<dyn SecureSecretStore>) -> Result<Self, String> {
        Ok(Self {
            secrets: SecretManager::new(secret_store),
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
        AiRuntimeState::new(crate::platform::unavailable_secret_store())
            .expect("AI runtime should initialize");
    }
}
