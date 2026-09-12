use std::sync::{Arc, Mutex};

use serde_json::json;
use sha2::{Digest, Sha256};

use crate::{
    ai::document_provider::{load_document_provider_capabilities, DocumentProviderCapabilities},
    database::Database,
    error::CommandError,
};

use super::super::{
    catalog::{AiTaskKind, ProviderKind},
    endpoint_policy::{validate_endpoint, NetworkScope},
    probes::load_document_probe_evidence,
    profiles::{
        load_default_profile_request, load_profile_request, load_task_profile_request,
        ProfileRequestSnapshot, ProfileSnapshot,
    },
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
    pub(crate) fn for_tutor_profile(
        database: &Database,
        profile_id: &str,
    ) -> Result<Self, CommandError> {
        context_from_request(database, load_profile_request(database, profile_id)?)?
            .with_saved_capabilities(database)
    }

    fn with_saved_capabilities(mut self, database: &Database) -> Result<Self, CommandError> {
        let capabilities = load_document_provider_capabilities(
            database,
            &self.profile_id,
            &self.provider_kind,
            &self.model_id,
        )?;
        self.apply_provider_capabilities(&capabilities);
        Ok(self)
    }

    pub(crate) fn apply_provider_capabilities(
        &mut self,
        capabilities: &DocumentProviderCapabilities,
    ) {
        self.capabilities = capabilities.clone();
        self.reasoning_effort =
            capabilities.resolve_reasoning_effort(&self.requested_reasoning_effort);
        crate::task_cancellation::set_current_reasoning_effort(&self.reasoning_effort);
        let recorded_reasoning_effort = crate::task_cancellation::current_reasoning_effort();
        crate::logging::debug_target(
            "ai.document.reasoning",
            "document_reasoning_resolved",
            "Resolved the profile's reasoning preference against current model metadata.",
            json!({"provider": self.provider_kind.as_str(), "model": self.model_id.as_str(),
                "profileId": self.profile_id.as_str(), "requestedEffort": self.requested_reasoning_effort.as_str(),
                "effectiveEffort": self.reasoning_effort.as_str(), "recordedEffort": recorded_reasoning_effort.as_deref()}),
        );
    }

    pub(crate) fn completion_token_ceiling(&self, visible_tokens: usize) -> usize {
        self.capabilities
            .completion_token_ceiling(&self.reasoning_effort, visible_tokens)
    }

    pub(crate) fn context_window_tokens(&self) -> Option<usize> {
        self.capabilities.context_window_tokens
    }

    pub(crate) fn contract_identity(&self) -> Result<String, CommandError> {
        let contract = serde_json::to_string(&(
            &self.provider_kind,
            &self.profile.base_url,
            &self.model_id,
            &self.reasoning_effort,
            self.capabilities.planning_fingerprint(),
        ))
        .map_err(CommandError::serialization)?;
        Ok(format!("{:x}", Sha256::digest(contract.as_bytes())))
    }

    pub(crate) fn ensure_current(&self, database: &Database) -> Result<(), CommandError> {
        let current = load_profile_request(database, &self.profile_id)?;
        if current.profile.revision != self.profile_revision
            || current.profile.credential_epoch != self.credential_epoch
            || current.profile.base_url != self.profile.base_url
            || current.credential_storage != self.credential_storage
        {
            return Err(CommandError::new(
                "ai_profile_changed",
                "The selected AI provider connection changed during this task. Review the provider and retry.",
            ));
        }
        // The active model/reasoning pair belongs to the task assignment, not
        // the provider record. Validate the snapshot captured for this task;
        // the provider's legacy model field is intentionally irrelevant here.
        self.profile.validate_for_provider_request()
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
    context_from_request(database, load_default_profile_request(database)?)
}

pub(crate) fn load_tutor_task_context(
    database: &Database,
) -> Result<DocumentTaskContext, CommandError> {
    context_from_request(
        database,
        load_task_profile_request(database, AiTaskKind::Tutor)?,
    )?
    .with_saved_capabilities(database)
}

fn context_from_request(
    database: &Database,
    request: ProfileRequestSnapshot,
) -> Result<DocumentTaskContext, CommandError> {
    let profile = request.profile;
    profile.validate_for_provider_request()?;
    let endpoint = validate_endpoint(
        profile.provider_kind,
        &profile.base_url,
        profile.allow_insecure_http,
    )?;
    let evidence = load_document_probe_evidence(database, &profile)?;
    if profile.provider_kind == ProviderKind::Openrouter && !evidence.probe_matches {
        return Err(CommandError::new("ai_document_model_not_verified",
            "Test this OpenRouter profile again before using learning tools. The selected model must pass ostudy's representative structured-output contract."));
    }
    if evidence.probe_matches && !evidence.document_generation_schema_supported {
        return Err(CommandError::new("ai_document_model_not_verified",
            "The selected AI model did not pass ostudy's representative document structured-output contract. Test the profile again or choose another model."));
    }
    let include_page_images = evidence.probe_matches && evidence.image_input_supported;
    let requested_reasoning_effort = profile.reasoning_effort.clone();
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
