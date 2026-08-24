use std::sync::{Arc, Mutex};

use serde_json::json;

use crate::{
    ai::document_provider::DocumentProviderCapabilities, database::Database, error::CommandError,
};

use super::super::{
    catalog::ProviderKind,
    endpoint_policy::{validate_endpoint, NetworkScope},
    probes::load_document_probe_evidence,
    profiles::{load_default_profile_request, ProfileSnapshot},
    secrets::CredentialStorage,
};

#[derive(Clone, Debug)]
pub(crate) struct DocumentTaskContext {
    pub profile_id: String,
    pub provider_kind: String,
    pub model_id: String,
    pub profile_revision: i64,
    pub credential_epoch: i64,
    pub network_scope: String,
    pub include_page_images: bool,
    pub requested_reasoning_effort: String,
    pub reasoning_effort: String,
    profile: ProfileSnapshot,
    credential_storage: CredentialStorage,
    capabilities: DocumentProviderCapabilities,
    usage: Arc<Mutex<DocumentUsageAccumulator>>,
}

#[derive(Clone, Copy, Debug, Default)]
struct DocumentUsageAccumulator {
    total_tokens: i64,
    reported_requests: i64,
    unreported_requests: i64,
}

impl DocumentTaskContext {
    pub(crate) fn apply_provider_capabilities(
        &mut self,
        capabilities: &DocumentProviderCapabilities,
    ) {
        self.capabilities = capabilities.clone();
        self.reasoning_effort =
            capabilities.resolve_reasoning_effort(&self.requested_reasoning_effort);
        crate::task_cancellation::set_current_reasoning_effort(&self.reasoning_effort);
        crate::logging::debug_target(
            "ai.document.reasoning",
            "document_reasoning_resolved",
            "Resolved the document reasoning preference against current model metadata.",
            json!({
                "provider": self.provider_kind.as_str(),
                "model": self.model_id.as_str(),
                "requestedEffort": self.requested_reasoning_effort.as_str(),
                "effectiveEffort": self.reasoning_effort.as_str(),
            }),
        );
    }

    pub(super) fn completion_token_ceiling(&self, visible_tokens: usize) -> usize {
        self.capabilities
            .completion_token_ceiling(&self.reasoning_effort, visible_tokens)
    }

    pub(crate) fn usage_summary(&self) -> (Option<i64>, bool) {
        let Ok(usage) = self.usage.lock() else {
            return (None, false);
        };
        let tokens = (usage.reported_requests > 0).then_some(usage.total_tokens);
        let complete = usage.reported_requests > 0 && usage.unreported_requests == 0;
        (tokens, complete)
    }

    pub(super) fn profile(&self) -> &ProfileSnapshot {
        &self.profile
    }

    pub(super) fn credential_storage(&self) -> CredentialStorage {
        self.credential_storage
    }

    pub(super) fn record_request_usage(&self, total_tokens: Option<u64>) {
        let Ok(mut usage) = self.usage.lock() else {
            return;
        };
        if let Some(total) = total_tokens {
            usage.total_tokens = usage
                .total_tokens
                .saturating_add(i64::try_from(total).unwrap_or(i64::MAX));
            usage.reported_requests = usage.reported_requests.saturating_add(1);
        } else {
            usage.unreported_requests = usage.unreported_requests.saturating_add(1);
        }
    }
}

pub(crate) fn load_document_task_context(
    database: &Database,
) -> Result<DocumentTaskContext, CommandError> {
    let request = load_default_profile_request(database)?;
    let profile = request.profile;
    profile.validate_for_provider_request()?;
    let endpoint = validate_endpoint(
        profile.provider_kind,
        &profile.base_url,
        profile.allow_insecure_http,
    )?;
    let evidence = load_document_probe_evidence(database, &profile)?;
    if profile.provider_kind == ProviderKind::Openrouter
        && (!evidence.probe_matches || !evidence.document_generation_schema_supported)
    {
        return Err(CommandError::new(
            "ai_document_model_not_verified",
            "Test this OpenRouter profile again before document generation. The selected model must pass ostudy's representative flashcard structured-output contract; choose another model if that test fails.",
        ));
    }
    let include_page_images = profile.provider_kind == ProviderKind::LlamaCpp
        && evidence.probe_matches
        && evidence.image_input_supported;
    let requested_reasoning_effort = crate::settings::current_ai_reasoning_effort();
    let capabilities = DocumentProviderCapabilities::conservative(
        profile.provider_kind.as_str(),
        &profile.model_id,
    );

    Ok(DocumentTaskContext {
        profile_id: profile.id.clone(),
        provider_kind: profile.provider_kind.as_str().to_string(),
        model_id: profile.model_id.clone(),
        profile_revision: profile.revision,
        credential_epoch: profile.credential_epoch,
        network_scope: network_scope_name(endpoint.scope).to_string(),
        include_page_images,
        requested_reasoning_effort,
        reasoning_effort: "none".to_string(),
        profile,
        credential_storage: request.credential_storage,
        capabilities,
        usage: Arc::new(Mutex::new(DocumentUsageAccumulator::default())),
    })
}

fn network_scope_name(scope: NetworkScope) -> &'static str {
    match scope {
        NetworkScope::Local => "local",
        NetworkScope::Lan => "lan",
        NetworkScope::Cloud => "cloud",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::{
        catalog::AuthMode,
        document_provider::{DocumentReasoningCapabilities, StructuredOutputStrategy},
    };

    #[test]
    fn completion_budget_delegates_to_the_negotiated_provider_capability_model() {
        let profile = ProfileSnapshot {
            id: "a".repeat(32),
            provider_kind: ProviderKind::Openrouter,
            base_url: "https://openrouter.ai/api/v1/".to_string(),
            model_id: "openai/example".to_string(),
            auth_mode: AuthMode::Bearer,
            enabled: true,
            allow_insecure_http: false,
            require_zero_data_retention: false,
            request_timeout_ms: 60_000,
            credential_epoch: 0,
            revision: 1,
        };
        let mut task = DocumentTaskContext {
            profile_id: profile.id.clone(),
            provider_kind: "openrouter".to_string(),
            model_id: profile.model_id.clone(),
            profile_revision: 1,
            credential_epoch: 0,
            network_scope: "cloud".to_string(),
            include_page_images: false,
            requested_reasoning_effort: "high".to_string(),
            reasoning_effort: "none".to_string(),
            credential_storage: CredentialStorage::None,
            capabilities: DocumentProviderCapabilities::conservative(
                "openrouter",
                &profile.model_id,
            ),
            profile,
            usage: Arc::new(Mutex::new(DocumentUsageAccumulator::default())),
        };
        task.apply_provider_capabilities(&DocumentProviderCapabilities {
            provider_kind: "openrouter".to_string(),
            model_id: task.model_id.clone(),
            context_window_tokens: Some(128_000),
            image_input: false,
            usage_reporting: true,
            structured_output: StructuredOutputStrategy::JsonSchema,
            reasoning: Some(DocumentReasoningCapabilities {
                supported_efforts: Some(vec!["high".to_string()]),
                default_effort: Some("high".to_string()),
                default_enabled: Some(true),
                supports_max_tokens: false,
                mandatory: false,
            }),
        });
        assert_eq!(task.reasoning_effort, "high");
        assert_eq!(task.completion_token_ceiling(2_000), 10_000);
    }
}
