use std::time::Instant;

use crate::{database::Database, error::CommandError, util::unix_time_millis};

use super::{
    super::{
        catalog::ProviderKind,
        gateway::{AiGatewayError, SafeAiError},
        profiles::{load_profile_request, ProfileRequestSnapshot, ProfileSnapshot},
        providers::{self, ProviderModel, ProviderProperties},
        AiRuntimeState,
    },
    capabilities::{
        build_capabilities, default_unknown_capabilities, set_capability, CapabilityEvidence,
    },
    contracts::{
        AiProfileReferenceInput, AiProviderModelsDto, AiProviderProbeResultDto, CapabilitySource,
        CapabilityStatus, ProbeOverallStatus,
    },
    phases::{elapsed_ms, failed_phase, passed_phase, skipped_phase},
    repository,
};

#[derive(Clone, Debug, PartialEq, Eq)]
enum ModelMatch {
    Exact,
    SoleLlamaServerId(String),
    Missing,
}

pub(super) async fn list_models(
    database: &Database,
    runtime: &AiRuntimeState,
    input: AiProfileReferenceInput,
) -> Result<AiProviderModelsDto, CommandError> {
    validate_profile_reference(&input)?;
    let request = load_profile_request(database, &input.profile_id)?;
    let profile = &request.profile;
    if profile.revision != input.revision {
        return Err(profile_changed());
    }
    if !profile.provider_kind.implemented() {
        return Err(CommandError::new(
            "ai_provider_not_implemented",
            "Model discovery is not implemented for this provider yet.",
        ));
    }
    if !profile.enabled {
        return Err(CommandError::new(
            "ai_profile_disabled",
            "Enable this AI provider profile before refreshing its models.",
        ));
    }

    let credential = resolve_credential(runtime, &request)?;
    let bearer = credential
        .as_ref()
        .map(|value| value.as_utf8())
        .transpose()?;
    let models = providers::list_models(&runtime.gateway, profile, bearer)
        .await
        .map_err(gateway_to_command)?;
    let retrieved_at = unix_time_millis()?;
    repository::persist_model_declarations(database, profile, &models, retrieved_at)?;
    Ok(AiProviderModelsDto {
        profile_id: profile.id.clone(),
        profile_revision: profile.revision,
        provider_kind: profile.provider_kind,
        models,
        retrieved_at,
    })
}

pub(super) async fn test_profile(
    database: &Database,
    runtime: &AiRuntimeState,
    input: AiProfileReferenceInput,
) -> Result<AiProviderProbeResultDto, CommandError> {
    validate_profile_reference(&input)?;
    let mut request = load_profile_request(database, &input.profile_id)?;
    if request.profile.revision != input.revision {
        return Err(profile_changed());
    }
    if let Some(model_id) = input.model_id.as_deref() {
        request.profile.model_id = super::super::profiles::policy::validate_model_id(model_id)?;
        request.profile.reasoning_effort = super::super::reasoning::validate_reasoning_effort(
            input.reasoning_effort.as_deref().unwrap_or("automatic"),
        )?;
    }
    let profile = &request.profile;
    profile.validate_for_provider_request()?;

    let tested_at = unix_time_millis()?;
    let mut phases = vec![passed_phase(
        "profile_validation",
        "Provider and model configuration",
        "The saved provider connection and selected model passed local validation.",
        None,
    )];
    let credential = resolve_credential(runtime, &request)?;
    phases.push(passed_phase(
        "credential_resolution",
        "Credential",
        if profile.auth_mode.requires_credential() {
            "A credential is available to the native provider gateway."
        } else {
            "This provider does not require authentication."
        },
        None,
    ));
    let bearer = credential
        .as_ref()
        .map(|value| value.as_utf8())
        .transpose()?;

    let mut optional_failure = false;
    let mut request_profile = profile.clone();
    let model_start = Instant::now();
    let models = match providers::list_models(&runtime.gateway, profile, bearer).await {
        Ok(models) => {
            phases.push(passed_phase(
                "model_discovery",
                "Model discovery",
                &format!("The provider returned {} available model(s).", models.len()),
                Some(elapsed_ms(model_start)),
            ));
            repository::persist_model_declarations(database, profile, &models, tested_at)?;
            Some(models)
        }
        Err(error_value)
            if profile.provider_kind == ProviderKind::LlamaCpp
                && error_value.safe.code == "ai_http_not_found" =>
        {
            optional_failure = true;
            phases.push(failed_phase(
                "model_discovery",
                "Model discovery",
                "This llama.cpp server does not expose model discovery; the selected model ID will be tested directly.",
                Some(elapsed_ms(model_start)),
                error_value.safe,
            ));
            None
        }
        Err(error_value) => {
            phases.push(failed_phase(
                "model_discovery",
                "Endpoint and model discovery",
                "The provider could not complete model discovery.",
                Some(elapsed_ms(model_start)),
                error_value.safe,
            ));
            return repository::finish_and_persist(
                database,
                profile,
                ProbeOverallStatus::Failed,
                phases,
                default_unknown_capabilities(profile.provider_kind),
                tested_at,
            );
        }
    };

    if let Some(models) = &models {
        match classify_model_match(profile, models) {
            ModelMatch::Exact => phases.push(passed_phase(
                "model_verification",
                "Selected model",
                "The selected model is available.",
                None,
            )),
            ModelMatch::SoleLlamaServerId(server_model_id) => {
                optional_failure = true;
                request_profile.model_id = server_model_id.clone();
                phases.push(failed_phase(
                    "model_verification",
                    "Selected model",
                    "llama.cpp reported one loaded model under a different API model ID. ostudy continued this connectivity test with the server-reported ID.",
                    None,
                    SafeAiError {
                        code: "ai_llama_model_id_mismatch".to_string(),
                        message: format!(
                            "llama.cpp reports the loaded model as `{server_model_id}`. Refresh models and select that ID, or start llama.cpp with `--alias` matching your preferred ID."
                        ),
                        retryable: false,
                        retry_after_seconds: None,
                    },
                ));
            }
            ModelMatch::Missing => {
                phases.push(failed_phase(
                    "model_verification",
                    "Selected model",
                    "The selected model was not present in the provider's model list.",
                    None,
                    SafeAiError {
                        code: "ai_model_not_found".to_string(),
                        message: "The selected model is not available from this provider. Refresh models and choose one of the returned model IDs.".to_string(),
                        retryable: false,
                        retry_after_seconds: None,
                    },
                ));
                let mut capabilities = default_unknown_capabilities(profile.provider_kind);
                set_capability(
                    &mut capabilities,
                    "model_listing",
                    CapabilityStatus::Supported,
                    CapabilitySource::Probed,
                    None,
                );
                return repository::finish_and_persist(
                    database,
                    profile,
                    ProbeOverallStatus::Failed,
                    phases,
                    capabilities,
                    tested_at,
                );
            }
        }
    } else {
        phases.push(skipped_phase(
            "model_verification",
            "Selected model",
            "The selected model will be verified by the completion probe.",
        ));
    }

    // Capability probes should spend their small output budgets validating the
    // requested contract, not a provider-default hidden reasoning trace. For an
    // OpenRouter model that declares optional reasoning, explicitly disable it.
    // Models with mandatory reasoning retain their configured/default effort;
    // models with no reasoning metadata receive no reasoning parameter at all.
    request_profile.reasoning_effort = probe_reasoning_effort(&request_profile, models.as_deref());

    let mut properties = ProviderProperties::default();
    if profile.provider_kind == ProviderKind::LlamaCpp {
        let property_start = Instant::now();
        match providers::load_properties(&runtime.gateway, &request_profile, bearer).await {
            Ok(loaded) => {
                properties = loaded;
                phases.push(passed_phase(
                    "server_properties",
                    "Server/model properties",
                    "llama.cpp server properties were read without retaining the chat template text.",
                    Some(elapsed_ms(property_start)),
                ));
            }
            Err(error_value) => {
                optional_failure = true;
                phases.push(failed_phase(
                    "server_properties",
                    "Server/model properties",
                    "The optional llama.cpp properties endpoint was unavailable.",
                    Some(elapsed_ms(property_start)),
                    error_value.safe,
                ));
            }
        }
    } else {
        phases.push(skipped_phase(
            "server_properties",
            "Server/model properties",
            "This provider manages model chat formatting internally.",
        ));
    }

    let text_start = Instant::now();
    let text_outcome = match providers::run_text_probe(&runtime.gateway, &request_profile, bearer)
        .await
    {
        Ok(outcome) => {
            let finish = outcome.finish_reason.as_deref().unwrap_or("unreported");
            phases.push(passed_phase(
                "text_completion",
                "Text completion",
                &format!("The selected model completed the minimal text probe (finish: {finish})."),
                Some(elapsed_ms(text_start)),
            ));
            outcome
        }
        Err(error_value) => {
            phases.push(failed_phase(
                "text_completion",
                "Text completion",
                "The selected model did not complete the minimal text probe.",
                Some(elapsed_ms(text_start)),
                error_value.safe,
            ));
            let capabilities = build_capabilities(
                profile,
                CapabilityEvidence {
                    models: models.as_deref(),
                    model_listing: models.is_some(),
                    text_completion: false,
                    structured_output: false,
                    document_generation_schema: None,
                    usage_reported: false,
                    properties: &properties,
                },
            );
            return repository::finish_and_persist(
                database,
                profile,
                ProbeOverallStatus::Failed,
                phases,
                capabilities,
                tested_at,
            );
        }
    };

    let echo = format!("{}-{tested_at}", &profile.id[..8]);
    let structured_start = Instant::now();
    let structured_outcome = match providers::run_structured_probe(
        &runtime.gateway,
        &request_profile,
        bearer,
        &echo,
    )
    .await
    {
        Ok(outcome) => {
            let finish = outcome.finish_reason.as_deref().unwrap_or("unreported");
            phases.push(passed_phase(
                "structured_output",
                "Structured output",
                &format!(
                    "The provider returned structured output that passed local type and semantic validation (finish: {finish})."
                ),
                Some(elapsed_ms(structured_start)),
            ));
            Some(outcome)
        }
        Err(error_value) => {
            optional_failure = true;
            phases.push(failed_phase(
                "structured_output",
                "Structured output",
                "Text generation works, but the basic structured-output probe did not validate.",
                Some(elapsed_ms(structured_start)),
                error_value.safe,
            ));
            None
        }
    };

    let document_started = Instant::now();
    let (document_schema_outcome, document_schema_supported) =
        match providers::run_document_schema_probe(&runtime.gateway, &request_profile, bearer).await
        {
            Ok(outcome) => {
                let finish = outcome.finish_reason.as_deref().unwrap_or("unreported");
                phases.push(passed_phase(
                    "document_generation_schema",
                    "Document generation schema",
                    &format!(
                        "The selected model satisfied ostudy's representative document-generation contract through its configured provider dialect (finish: {finish})."
                    ),
                    Some(elapsed_ms(document_started)),
                ));
                (Some(outcome), Some(true))
            }
            Err(error_value) => {
                optional_failure = true;
                phases.push(failed_phase(
                    "document_generation_schema",
                    "Document generation schema",
                    "The selected model did not satisfy ostudy's representative document-generation contract through the same structured-output path used by real document work.",
                    Some(elapsed_ms(document_started)),
                    error_value.safe,
                ));
                (None, Some(false))
            }
        };

    let usage_reported = text_outcome.prompt_tokens.is_some()
        || text_outcome.completion_tokens.is_some()
        || structured_outcome.as_ref().is_some_and(|outcome| {
            outcome.prompt_tokens.is_some() || outcome.completion_tokens.is_some()
        })
        || document_schema_outcome.as_ref().is_some_and(|outcome| {
            outcome.prompt_tokens.is_some() || outcome.completion_tokens.is_some()
        });
    let capabilities = build_capabilities(
        profile,
        CapabilityEvidence {
            models: models.as_deref(),
            model_listing: models.is_some(),
            text_completion: true,
            structured_output: structured_outcome.is_some(),
            document_generation_schema: document_schema_supported,
            usage_reported,
            properties: &properties,
        },
    );
    repository::finish_and_persist(
        database,
        profile,
        if optional_failure {
            ProbeOverallStatus::Partial
        } else {
            ProbeOverallStatus::Verified
        },
        phases,
        capabilities,
        tested_at,
    )
}

fn resolve_credential(
    runtime: &AiRuntimeState,
    request: &ProfileRequestSnapshot,
) -> Result<Option<super::super::secrets::ResolvedCredential>, CommandError> {
    runtime.secrets.resolve(
        &request.profile.id,
        request.profile.auth_mode,
        request.credential_storage,
    )
}

fn classify_model_match(profile: &ProfileSnapshot, models: &[ProviderModel]) -> ModelMatch {
    if models.iter().any(|model| model.id == profile.model_id) {
        return ModelMatch::Exact;
    }
    if profile.provider_kind == ProviderKind::LlamaCpp && models.len() == 1 {
        return ModelMatch::SoleLlamaServerId(models[0].id.clone());
    }
    ModelMatch::Missing
}

fn probe_reasoning_effort(profile: &ProfileSnapshot, models: Option<&[ProviderModel]>) -> String {
    if profile.provider_kind != ProviderKind::Openrouter {
        return profile.reasoning_effort.clone();
    }
    let Some(reasoning) = models
        .and_then(|models| models.iter().find(|model| model.id == profile.model_id))
        .and_then(|model| model.reasoning.as_ref())
    else {
        return "automatic".to_string();
    };
    if reasoning.mandatory {
        if profile.reasoning_effort == "none" {
            "automatic".to_string()
        } else {
            profile.reasoning_effort.clone()
        }
    } else {
        "none".to_string()
    }
}

fn validate_profile_reference(input: &AiProfileReferenceInput) -> Result<(), CommandError> {
    crate::util::validate_entity_id(&input.profile_id, "AI provider profile")?;
    if input.revision < 1 {
        return Err(CommandError::new(
            "invalid_revision",
            "The AI provider profile revision is invalid.",
        ));
    }
    if let Some(model_id) = input.model_id.as_deref() {
        if super::super::profiles::policy::validate_model_id(model_id)?.is_empty() {
            return Err(CommandError::new(
                "invalid_ai_model_id",
                "Choose a model to test.",
            ));
        }
        super::super::reasoning::validate_reasoning_effort(
            input.reasoning_effort.as_deref().unwrap_or("automatic"),
        )?;
    }
    Ok(())
}

fn gateway_to_command(error_value: AiGatewayError) -> CommandError {
    CommandError::new(error_value.safe.code, error_value.safe.message)
}

fn profile_changed() -> CommandError {
    CommandError::new(
        "ai_profile_changed",
        "This AI provider connection changed before the request started. Reload it and try again.",
    )
}

#[cfg(test)]
pub(super) fn classify_model_match_for_test(
    profile: &ProfileSnapshot,
    models: &[ProviderModel],
) -> String {
    match classify_model_match(profile, models) {
        ModelMatch::Exact => "exact".to_string(),
        ModelMatch::SoleLlamaServerId(id) => format!("sole:{id}"),
        ModelMatch::Missing => "missing".to_string(),
    }
}

#[cfg(test)]
pub(super) fn probe_reasoning_effort_for_test(
    profile: &ProfileSnapshot,
    models: Option<&[ProviderModel]>,
) -> String {
    probe_reasoning_effort(profile, models)
}
