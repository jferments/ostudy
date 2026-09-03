use serde::{Deserialize, Serialize};

use crate::{editor_drafts::EditorDraftCommitRef, error::CommandError};

use super::super::{
    catalog::{AiTaskKind, AuthMode, ProviderKind},
    endpoint_policy::{validate_endpoint, NetworkScope},
    reasoning::default_reasoning_effort,
    secrets::{CredentialPersistence, CredentialStatusDto, CredentialStorage},
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateAiProviderProfileInput {
    pub operation_id: String,
    pub display_name: String,
    pub provider_kind: ProviderKind,
    pub base_url: String,
    pub model_id: String,
    pub auth_mode: AuthMode,
    pub enabled: bool,
    pub allow_insecure_http: bool,
    #[serde(default)]
    pub require_zero_data_retention: bool,
    pub request_timeout_ms: i64,
    #[serde(default = "default_reasoning_effort")]
    pub reasoning_effort: String,
    #[serde(default)]
    pub editor_draft: Option<EditorDraftCommitRef>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateAiProviderProfileInput {
    pub operation_id: String,
    pub profile_id: String,
    pub revision: i64,
    pub display_name: String,
    pub base_url: String,
    pub model_id: String,
    pub auth_mode: AuthMode,
    pub enabled: bool,
    pub allow_insecure_http: bool,
    #[serde(default)]
    pub require_zero_data_retention: bool,
    pub request_timeout_ms: i64,
    #[serde(default = "default_reasoning_effort")]
    pub reasoning_effort: String,
    #[serde(default)]
    pub editor_draft: Option<EditorDraftCommitRef>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeleteAiProviderProfileInput {
    pub operation_id: String,
    pub profile_id: String,
    pub revision: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetAiProviderCredentialInput {
    pub operation_id: String,
    pub profile_id: String,
    pub revision: i64,
    pub credential: String,
    pub persistence: CredentialPersistence,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClearAiProviderCredentialInput {
    pub operation_id: String,
    pub profile_id: String,
    pub revision: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetAiTaskDefaultInput {
    pub operation_id: String,
    pub task_kind: AiTaskKind,
    pub profile_id: Option<String>,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProfileConfigurationStatus {
    Ready,
    Incomplete,
    Disabled,
    Planned,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiProviderProfileSummaryDto {
    pub id: String,
    pub display_name: String,
    pub provider_kind: ProviderKind,
    pub provider_label: String,
    pub model_id: String,
    pub reasoning_effort: String,
    pub enabled: bool,
    pub network_scope: NetworkScope,
    pub credential_status: CredentialStatusDto,
    pub configuration_status: ProfileConfigurationStatus,
    pub revision: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiProviderProfileDetailDto {
    pub id: String,
    pub display_name: String,
    pub provider_kind: ProviderKind,
    pub provider_label: String,
    pub base_url: String,
    pub base_url_editable: bool,
    pub model_id: String,
    pub reasoning_effort: String,
    pub auth_mode: AuthMode,
    pub enabled: bool,
    pub allow_insecure_http: bool,
    pub require_zero_data_retention: bool,
    pub request_timeout_ms: i64,
    pub credential_epoch: i64,
    pub network_scope: NetworkScope,
    pub credential_status: CredentialStatusDto,
    pub configuration_status: ProfileConfigurationStatus,
    pub revision: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiTaskDefaultsDto {
    pub flashcard_generation_profile_id: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteAiProviderProfileResultDto {
    pub profile_id: String,
    pub deleted: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct ProfileSnapshot {
    pub id: String,
    pub provider_kind: ProviderKind,
    pub base_url: String,
    pub model_id: String,
    pub reasoning_effort: String,
    pub auth_mode: AuthMode,
    pub enabled: bool,
    pub allow_insecure_http: bool,
    pub require_zero_data_retention: bool,
    pub request_timeout_ms: i64,
    pub credential_epoch: i64,
    pub revision: i64,
}

impl ProfileSnapshot {
    pub(crate) fn validate_for_provider_request(&self) -> Result<(), CommandError> {
        if !self.provider_kind.implemented() {
            return Err(CommandError::new(
                "ai_provider_not_implemented",
                "This AI provider is planned but is not implemented in this ostudy build.",
            ));
        }
        if !self.enabled {
            return Err(CommandError::new(
                "ai_profile_disabled",
                "Enable this AI provider profile before using it.",
            ));
        }
        if self.model_id.trim().is_empty() {
            return Err(CommandError::new(
                "ai_profile_incomplete",
                "Select or enter a model before testing this AI provider profile.",
            ));
        }
        super::super::reasoning::validate_reasoning_effort(&self.reasoning_effort)?;
        super::policy::validate_privacy_configuration(
            self.provider_kind,
            self.require_zero_data_retention,
        )?;
        validate_endpoint(
            self.provider_kind,
            &self.base_url,
            self.allow_insecure_http,
        )?;
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub(super) struct StoredProfile {
    pub id: String,
    pub display_name: String,
    pub provider_kind: ProviderKind,
    pub base_url: String,
    pub model_id: String,
    pub reasoning_effort: String,
    pub auth_mode: AuthMode,
    pub enabled: bool,
    pub allow_insecure_http: bool,
    pub require_zero_data_retention: bool,
    pub request_timeout_ms: i64,
    pub credential_storage: CredentialStorage,
    pub credential_epoch: i64,
    pub revision: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

impl From<&StoredProfile> for ProfileSnapshot {
    fn from(profile: &StoredProfile) -> Self {
        Self {
            id: profile.id.clone(),
            provider_kind: profile.provider_kind,
            base_url: profile.base_url.clone(),
            model_id: profile.model_id.clone(),
            reasoning_effort: profile.reasoning_effort.clone(),
            auth_mode: profile.auth_mode,
            enabled: profile.enabled,
            allow_insecure_http: profile.allow_insecure_http,
            require_zero_data_retention: profile.require_zero_data_retention,
            request_timeout_ms: profile.request_timeout_ms,
            credential_epoch: profile.credential_epoch,
            revision: profile.revision,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ProfileRequestSnapshot {
    pub profile: ProfileSnapshot,
    pub credential_storage: CredentialStorage,
}
