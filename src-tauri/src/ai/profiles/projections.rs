use crate::error::CommandError;

use super::{
    super::{endpoint_policy::validate_endpoint, secrets::CredentialStatus, AiRuntimeState},
    contracts::{
        AiProviderProfileDetailDto, AiProviderProfileSummaryDto, ProfileConfigurationStatus,
        StoredProfile,
    },
};

pub(super) fn build_summary(
    profile: &StoredProfile,
    runtime: &AiRuntimeState,
) -> Result<AiProviderProfileSummaryDto, CommandError> {
    let detail = build_detail(profile, runtime)?;
    Ok(AiProviderProfileSummaryDto {
        id: detail.id,
        display_name: detail.display_name,
        provider_kind: detail.provider_kind,
        provider_label: detail.provider_label,
        model_id: detail.model_id,
        reasoning_effort: detail.reasoning_effort,
        enabled: detail.enabled,
        network_scope: detail.network_scope,
        credential_status: detail.credential_status,
        configuration_status: detail.configuration_status,
        revision: detail.revision,
        updated_at: detail.updated_at,
    })
}

pub(super) fn build_detail(
    profile: &StoredProfile,
    runtime: &AiRuntimeState,
) -> Result<AiProviderProfileDetailDto, CommandError> {
    let endpoint = validate_endpoint(
        profile.provider_kind,
        &profile.base_url,
        profile.allow_insecure_http,
    )?;
    let credential_status =
        runtime
            .secrets
            .status(&profile.id, profile.auth_mode, profile.credential_storage)?;
    let configuration_status = configuration_status(profile, credential_status.status);
    Ok(AiProviderProfileDetailDto {
        id: profile.id.clone(),
        display_name: profile.display_name.clone(),
        provider_kind: profile.provider_kind,
        provider_label: profile.provider_kind.display_name().to_string(),
        base_url: profile.base_url.clone(),
        base_url_editable: profile.provider_kind.fixed_base_url().is_none(),
        model_id: profile.model_id.clone(),
        reasoning_effort: profile.reasoning_effort.clone(),
        auth_mode: profile.auth_mode,
        enabled: profile.enabled,
        allow_insecure_http: profile.allow_insecure_http,
        require_zero_data_retention: profile.require_zero_data_retention,
        request_timeout_ms: profile.request_timeout_ms,
        credential_epoch: profile.credential_epoch,
        network_scope: endpoint.scope,
        credential_status,
        configuration_status,
        revision: profile.revision,
        created_at: profile.created_at,
        updated_at: profile.updated_at,
    })
}

fn configuration_status(
    profile: &StoredProfile,
    credential_status: CredentialStatus,
) -> ProfileConfigurationStatus {
    if !profile.provider_kind.implemented() {
        ProfileConfigurationStatus::Planned
    } else if !profile.enabled {
        ProfileConfigurationStatus::Disabled
    } else if profile.auth_mode.requires_credential()
        && !matches!(
            credential_status,
            CredentialStatus::SavedSecurely | CredentialStatus::SessionOnly
        )
    {
        ProfileConfigurationStatus::Incomplete
    } else {
        // Provider profiles are connection/credential owners. A model is chosen
        // independently for generation, Tutor, or embeddings.
        ProfileConfigurationStatus::Ready
    }
}

#[cfg(test)]
pub(super) fn configuration_status_for_test(
    profile: &StoredProfile,
    credential_status: CredentialStatus,
) -> ProfileConfigurationStatus {
    configuration_status(profile, credential_status)
}
