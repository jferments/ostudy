//! AI provider configuration, credential isolation, and provider gateway.
//!
//! This subsystem is deliberately separate from authoring. Provider networking
//! is available only through narrow profile/model/probe commands here; future
//! learning tasks call the internal gateway from task-specific services.

mod catalog;
mod endpoint_policy;
mod gateway;
mod probes;
mod profiles;
mod providers;
mod secrets;

pub(crate) use probes::{
    get_ai_provider_probe_result, list_ai_provider_models, test_ai_provider_profile,
};
pub(crate) use profiles::{
    clear_ai_provider_credential, create_ai_provider_profile, delete_ai_provider_profile,
    get_ai_provider_catalog, get_ai_provider_profile, get_ai_task_defaults,
    list_ai_provider_profiles, set_ai_provider_credential, set_ai_task_default,
    update_ai_provider_profile,
};

use gateway::AiGateway;
use secrets::SecretManager;

pub struct AiRuntimeState {
    pub(crate) secrets: SecretManager,
    pub(crate) gateway: AiGateway,
}

impl AiRuntimeState {
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            secrets: SecretManager::default(),
            gateway: AiGateway::new()?,
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
