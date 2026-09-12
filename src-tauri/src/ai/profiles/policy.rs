use crate::{error::CommandError, util::validate_entity_id};

use super::{
    super::{
        catalog::{AuthMode, ProviderKind, MAX_TIMEOUT_MS, MIN_TIMEOUT_MS},
        endpoint_policy::{endpoint_origin_from_str, validate_endpoint, ValidatedEndpoint},
        reasoning::validate_reasoning_effort,
        secrets::CredentialStorage,
    },
    contracts::{CreateAiProviderProfileInput, StoredProfile, UpdateAiProviderProfileInput},
};

pub(super) const MAX_ACTIVE_PROFILES: usize = 32;
const MAX_PROFILE_NAME_CHARS: usize = 100;
const MAX_MODEL_ID_CHARS: usize = 300;

#[derive(Clone, Debug)]
pub(super) struct ValidatedProfileFields {
    pub display_name: String,
    pub normalized_name: String,
    pub model_id: String,
    pub reasoning_effort: String,
    pub endpoint: ValidatedEndpoint,
}

pub(super) fn validate_create_profile(
    input: &CreateAiProviderProfileInput,
) -> Result<ValidatedProfileFields, CommandError> {
    validate_provider_configuration(input.provider_kind, input.auth_mode)?;
    validate_privacy_configuration(input.provider_kind, input.require_zero_data_retention)?;
    validate_fields(
        input.provider_kind,
        &input.display_name,
        &input.base_url,
        &input.model_id,
        &input.reasoning_effort,
        input.allow_insecure_http,
        input.request_timeout_ms,
    )
}

pub(super) fn validate_update_profile(
    provider_kind: ProviderKind,
    input: &UpdateAiProviderProfileInput,
) -> Result<ValidatedProfileFields, CommandError> {
    validate_provider_configuration(provider_kind, input.auth_mode)?;
    validate_privacy_configuration(provider_kind, input.require_zero_data_retention)?;
    validate_fields(
        provider_kind,
        &input.display_name,
        &input.base_url,
        &input.model_id,
        &input.reasoning_effort,
        input.allow_insecure_http,
        input.request_timeout_ms,
    )
}

fn validate_fields(
    provider_kind: ProviderKind,
    display_name: &str,
    base_url: &str,
    model_id: &str,
    reasoning_effort: &str,
    allow_insecure_http: bool,
    request_timeout_ms: i64,
) -> Result<ValidatedProfileFields, CommandError> {
    let display_name = validate_display_name(display_name)?;
    let normalized_name = normalize_name(&display_name);
    let model_id = validate_model_id(model_id)?;
    let reasoning_effort = validate_reasoning_effort(reasoning_effort)?;
    validate_timeout(request_timeout_ms)?;
    let endpoint = validate_endpoint(provider_kind, base_url, allow_insecure_http)?;
    Ok(ValidatedProfileFields {
        display_name,
        normalized_name,
        model_id,
        reasoning_effort,
        endpoint,
    })
}

pub(super) fn ensure_trust_boundary_can_change(
    current: &StoredProfile,
    next_endpoint: &ValidatedEndpoint,
    next_auth_mode: AuthMode,
) -> Result<(), CommandError> {
    let current_origin = endpoint_origin_from_str(&current.base_url)?;
    let origin_changes = current_origin != next_endpoint.origin;
    let authentication_is_removed =
        current.auth_mode.requires_credential() && !next_auth_mode.requires_credential();
    if !origin_changes && !authentication_is_removed {
        return Ok(());
    }
    if current.credential_storage != CredentialStorage::None {
        return Err(CommandError::new(
            if origin_changes {
                "ai_endpoint_has_credential"
            } else {
                "ai_auth_mode_has_credential"
            },
            if origin_changes {
                "Remove this profile's credential before changing its server origin."
            } else {
                "Remove this profile's credential before disabling authentication."
            },
        ));
    }
    Ok(())
}

pub(super) fn validate_provider_configuration(
    provider_kind: ProviderKind,
    auth_mode: AuthMode,
) -> Result<(), CommandError> {
    if !provider_kind.implemented() {
        return Err(CommandError::new(
            "ai_provider_not_implemented",
            "That AI provider is planned but is not available in this ostudy build.",
        ));
    }
    if matches!(
        provider_kind,
        ProviderKind::Deepseek | ProviderKind::Openrouter
    ) && auth_mode != AuthMode::Bearer
    {
        return Err(CommandError::new(
            "invalid_ai_auth_mode",
            "Built-in cloud provider profiles require bearer-token authentication.",
        ));
    }
    Ok(())
}

pub(super) fn validate_privacy_configuration(
    provider_kind: ProviderKind,
    require_zero_data_retention: bool,
) -> Result<(), CommandError> {
    if require_zero_data_retention && provider_kind != ProviderKind::Openrouter {
        return Err(CommandError::new(
            "invalid_ai_privacy_policy",
            "Zero-data-retention routing is available only for OpenRouter profiles.",
        ));
    }
    Ok(())
}

pub(super) fn validate_display_name(value: &str) -> Result<String, CommandError> {
    let normalized_spacing = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let count = normalized_spacing.chars().count();
    if count == 0 || count > MAX_PROFILE_NAME_CHARS {
        return Err(CommandError::new(
            "invalid_ai_profile_name",
            format!(
                "AI profile names must contain between 1 and {MAX_PROFILE_NAME_CHARS} characters."
            ),
        ));
    }
    if normalized_spacing.chars().any(char::is_control) {
        return Err(CommandError::new(
            "invalid_ai_profile_name",
            "AI profile names cannot contain control characters.",
        ));
    }
    Ok(normalized_spacing)
}

pub(super) fn normalize_name(value: &str) -> String {
    value.to_lowercase()
}

pub(crate) fn validate_model_id(value: &str) -> Result<String, CommandError> {
    let trimmed = value.trim().to_string();
    if trimmed.chars().count() > MAX_MODEL_ID_CHARS || trimmed.chars().any(char::is_control) {
        return Err(CommandError::new(
            "invalid_ai_model_id",
            format!("Model IDs cannot exceed {MAX_MODEL_ID_CHARS} characters or contain controls."),
        ));
    }
    Ok(trimmed)
}

pub(super) fn validate_timeout(value: i64) -> Result<(), CommandError> {
    if !(MIN_TIMEOUT_MS..=MAX_TIMEOUT_MS).contains(&value) {
        return Err(CommandError::new(
            "invalid_ai_timeout",
            format!(
                "AI request timeouts must be between {MIN_TIMEOUT_MS} and {MAX_TIMEOUT_MS} milliseconds."
            ),
        ));
    }
    Ok(())
}

pub(super) fn validate_profile_reference(
    profile_id: &str,
    revision: i64,
) -> Result<(), CommandError> {
    validate_entity_id(profile_id, "AI provider profile")?;
    if revision < 1 {
        return Err(CommandError::new(
            "invalid_revision",
            "The AI provider profile revision is invalid.",
        ));
    }
    Ok(())
}

pub(super) fn validate_operation_id(operation_id: &str) -> Result<(), CommandError> {
    validate_entity_id(operation_id, "operation")
}

pub(super) fn profile_changed() -> CommandError {
    CommandError::new(
        "ai_profile_changed",
        "This AI provider profile changed in another operation. Reload it and try again.",
    )
}
