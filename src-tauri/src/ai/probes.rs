use std::time::Instant;

use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::State;

use crate::{
    database::Database,
    error::CommandError,
    util::{unix_time_millis, validate_entity_id},
};

use super::{
    catalog::ProviderKind,
    gateway::{AiGatewayError, SafeAiError},
    profiles::{load_profile_snapshot, ProfileSnapshot},
    providers::{self, ProviderModel, ProviderProperties},
    secrets::CredentialStorage,
    AiRuntimeState,
};

const ADAPTER_REVISION: i64 = 6;
const RESULT_SCHEMA_VERSION: i64 = 1;
const MAX_PERSISTED_RESULT_BYTES: usize = 32 * 1024;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AiProfileReferenceInput {
    pub profile_id: String,
    pub revision: i64,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProbeOverallStatus {
    Verified,
    Partial,
    Failed,
}

impl ProbeOverallStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Verified => "verified",
            Self::Partial => "partial",
            Self::Failed => "failed",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProbePhaseStatus {
    Passed,
    Failed,
    Skipped,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbePhaseDto {
    pub phase: String,
    pub label: String,
    pub status: ProbePhaseStatus,
    pub message: String,
    pub latency_ms: Option<i64>,
    pub error: Option<SafeAiError>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityStatus {
    Supported,
    Unsupported,
    Unknown,
}

impl CapabilityStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Supported => "supported",
            Self::Unsupported => "unsupported",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CapabilitySource {
    Declared,
    Probed,
}

impl CapabilitySource {
    fn as_str(self) -> &'static str {
        match self {
            Self::Declared => "declared",
            Self::Probed => "probed",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiCapabilityDto {
    pub key: String,
    pub status: CapabilityStatus,
    pub source: CapabilitySource,
    pub value: Option<Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiProviderProbeResultDto {
    pub profile_id: String,
    pub profile_revision: i64,
    pub credential_epoch: i64,
    pub tested_model_id: String,
    pub adapter_revision: i64,
    pub result_schema_version: i64,
    pub overall_status: ProbeOverallStatus,
    pub phases: Vec<ProbePhaseDto>,
    pub capabilities: Vec<AiCapabilityDto>,
    pub tested_at: i64,
    pub persisted: bool,
    pub stale: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiProviderModelsDto {
    pub profile_id: String,
    pub profile_revision: i64,
    pub provider_kind: ProviderKind,
    pub models: Vec<ProviderModel>,
    pub retrieved_at: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ModelMatch {
    Exact,
    SoleLlamaServerId(String),
    Missing,
}

#[tauri::command]
pub async fn list_ai_provider_models(
    database: State<'_, Database>,
    runtime: State<'_, AiRuntimeState>,
    input: AiProfileReferenceInput,
) -> Result<AiProviderModelsDto, CommandError> {
    validate_profile_reference(&input)?;
    let (profile, credential_storage) = database.with_connection(|connection| {
        Ok((
            load_profile_snapshot(connection, &input.profile_id)?,
            load_credential_storage(connection, &input.profile_id)?,
        ))
    })?;
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

    let credential = runtime
        .secrets
        .resolve(&profile.id, profile.auth_mode, credential_storage)?;
    let bearer = credential
        .as_ref()
        .map(|value| value.as_utf8())
        .transpose()?;
    let models = providers::list_models(&runtime.gateway, &profile, bearer)
        .await
        .map_err(gateway_to_command)?;
    let retrieved_at = unix_time_millis()?;
    persist_selected_model_declarations(database.inner(), &profile, &models, retrieved_at)?;
    Ok(AiProviderModelsDto {
        profile_id: profile.id.clone(),
        profile_revision: profile.revision,
        provider_kind: profile.provider_kind,
        models,
        retrieved_at,
    })
}

#[tauri::command]
pub async fn test_ai_provider_profile(
    database: State<'_, Database>,
    runtime: State<'_, AiRuntimeState>,
    input: AiProfileReferenceInput,
) -> Result<AiProviderProbeResultDto, CommandError> {
    validate_profile_reference(&input)?;
    let (profile, credential_storage) = database.with_connection(|connection| {
        Ok((
            load_profile_snapshot(connection, &input.profile_id)?,
            load_credential_storage(connection, &input.profile_id)?,
        ))
    })?;
    if profile.revision != input.revision {
        return Err(profile_changed());
    }
    profile.validate_for_provider_request()?;

    let tested_at = unix_time_millis()?;
    let mut phases = vec![passed_phase(
        "profile_validation",
        "Profile configuration",
        "The saved provider profile passed local validation.",
        None,
    )];
    let credential = runtime
        .secrets
        .resolve(&profile.id, profile.auth_mode, credential_storage)?;
    phases.push(passed_phase(
        "credential_resolution",
        "Credential",
        if profile.auth_mode.requires_credential() {
            "A credential is available to the native provider gateway."
        } else {
            "This profile does not require authentication."
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
    let models = match providers::list_models(&runtime.gateway, &profile, bearer).await {
        Ok(models) => {
            phases.push(passed_phase(
                "model_discovery",
                "Model discovery",
                &format!("The provider returned {} available model(s).", models.len()),
                Some(elapsed_ms(model_start)),
            ));
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
                "This llama.cpp server does not expose model discovery; the saved model ID will be tested directly.",
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
            return finish_and_persist(
                database.inner(),
                &profile,
                ProbeOverallStatus::Failed,
                phases,
                default_unknown_capabilities(profile.provider_kind),
                tested_at,
            );
        }
    };

    if let Some(models) = &models {
        match classify_model_match(&profile, models) {
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
                            "llama.cpp reports the loaded model as `{server_model_id}`. Refresh models and save that ID, or start llama.cpp with `--alias` matching your preferred ID."
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
                        message: "The selected model is not available from this provider's document-compatible model list. Refresh models and save one of the returned model IDs.".to_string(),
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
                return finish_and_persist(
                    database.inner(),
                    &profile,
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
                &profile,
                models.as_deref(),
                models.is_some(),
                false,
                false,
                None,
                false,
                &properties,
            );
            return finish_and_persist(
                database.inner(),
                &profile,
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
                "Text generation works, but the structured-output probe did not validate.",
                Some(elapsed_ms(structured_start)),
                error_value.safe,
            ));
            None
        }
    };

    let (document_schema_outcome, document_schema_supported) =
        if profile.provider_kind == ProviderKind::Openrouter {
            if structured_outcome.is_some() {
                let started = Instant::now();
                match providers::run_document_schema_probe(
                    &runtime.gateway,
                    &request_profile,
                    bearer,
                )
                .await
                {
                    Ok(outcome) => {
                        let finish = outcome.finish_reason.as_deref().unwrap_or("unreported");
                        phases.push(passed_phase(
                            "document_generation_schema",
                            "Document generation schema",
                            &format!(
                                "The selected OpenRouter model satisfied ostudy's representative flashcard union contract (finish: {finish})."
                            ),
                            Some(elapsed_ms(started)),
                        ));
                        (Some(outcome), Some(true))
                    }
                    Err(error_value) => {
                        optional_failure = true;
                        phases.push(failed_phase(
                            "document_generation_schema",
                            "Document generation schema",
                            "Structured JSON works, but the model did not reliably satisfy ostudy's representative flashcard generation contract.",
                            Some(elapsed_ms(started)),
                            error_value.safe,
                        ));
                        (None, Some(false))
                    }
                }
            } else {
                phases.push(skipped_phase(
                    "document_generation_schema",
                    "Document generation schema",
                    "The document contract probe was skipped because the basic structured-output probe did not pass.",
                ));
                (None, Some(false))
            }
        } else {
            phases.push(skipped_phase(
                "document_generation_schema",
                "Document generation schema",
                "This provider uses its existing provider-specific document structured-output contract.",
            ));
            (None, None)
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
        &profile,
        models.as_deref(),
        models.is_some(),
        true,
        structured_outcome.is_some(),
        document_schema_supported,
        usage_reported,
        &properties,
    );
    let overall_status = if optional_failure {
        ProbeOverallStatus::Partial
    } else {
        ProbeOverallStatus::Verified
    };
    finish_and_persist(
        database.inner(),
        &profile,
        overall_status,
        phases,
        capabilities,
        tested_at,
    )
}

#[tauri::command]
pub fn get_ai_provider_probe_result(
    database: State<'_, Database>,
    profile_id: String,
) -> Result<Option<AiProviderProbeResultDto>, CommandError> {
    validate_entity_id(&profile_id, "AI provider profile")?;
    database.with_connection(|connection| {
        let serialized: Option<String> = connection
            .query_row(
                "SELECT result_json FROM ai_provider_probe_results WHERE profile_id = ?1",
                [profile_id.as_str()],
                |row| row.get(0),
            )
            .optional()
            .map_err(CommandError::database)?;
        let Some(serialized) = serialized else {
            return Ok(None);
        };
        let mut result: AiProviderProbeResultDto =
            serde_json::from_str(&serialized).map_err(CommandError::serialization)?;
        let current = load_profile_snapshot(connection, &profile_id)?;
        result.stale = !snapshot_matches_result(&current, &result);
        result.persisted = true;
        Ok(Some(result))
    })
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

fn load_credential_storage(
    connection: &rusqlite::Connection,
    profile_id: &str,
) -> Result<CredentialStorage, CommandError> {
    let storage: String = connection
        .query_row(
            "SELECT credential_storage
             FROM ai_provider_profiles
             WHERE id = ?1 AND deleted_at IS NULL",
            [profile_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(CommandError::database)?
        .ok_or_else(|| {
            CommandError::new(
                "ai_profile_not_found",
                "That AI provider profile no longer exists.",
            )
        })?;
    CredentialStorage::from_db(&storage)
}

fn persist_selected_model_declarations(
    database: &Database,
    profile: &ProfileSnapshot,
    models: &[ProviderModel],
    checked_at: i64,
) -> Result<(), CommandError> {
    let selected = models.iter().find(|model| model.id == profile.model_id);
    database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        transaction
            .execute(
                "DELETE FROM ai_provider_capabilities
                 WHERE profile_id = ?1 AND capability_key IN ('context_window_tokens', 'reasoning_control')",
                [profile.id.as_str()],
            )
            .map_err(CommandError::database)?;
        if let Some(context) = selected.and_then(|model| model.context_window_tokens) {
            transaction
                .execute(
                    "INSERT INTO ai_provider_capabilities (
                         profile_id, capability_key, status, source_kind, value_json, checked_at
                     ) VALUES (?1, 'context_window_tokens', 'supported', 'declared', ?2, ?3)",
                    params![profile.id, context.to_string(), checked_at],
                )
                .map_err(CommandError::database)?;
        }
        if let Some(reasoning) = selected.and_then(|model| model.reasoning.as_ref()) {
            let value = serde_json::to_string(&Some(reasoning)).map_err(CommandError::serialization)?;
            transaction
                .execute(
                    "INSERT INTO ai_provider_capabilities (
                         profile_id, capability_key, status, source_kind, value_json, checked_at
                     ) VALUES (?1, 'reasoning_control', 'supported', 'declared', ?2, ?3)",
                    params![profile.id, value, checked_at],
                )
                .map_err(CommandError::database)?;
        }
        transaction.commit().map_err(CommandError::database)
    })
}

fn finish_and_persist(
    database: &Database,
    profile: &ProfileSnapshot,
    overall_status: ProbeOverallStatus,
    phases: Vec<ProbePhaseDto>,
    capabilities: Vec<AiCapabilityDto>,
    tested_at: i64,
) -> Result<AiProviderProbeResultDto, CommandError> {
    let mut result = AiProviderProbeResultDto {
        profile_id: profile.id.clone(),
        profile_revision: profile.revision,
        credential_epoch: profile.credential_epoch,
        tested_model_id: profile.model_id.clone(),
        adapter_revision: ADAPTER_REVISION,
        result_schema_version: RESULT_SCHEMA_VERSION,
        overall_status,
        phases,
        capabilities,
        tested_at,
        persisted: false,
        stale: false,
    };
    let persisted = database.with_connection(|connection| {
        let current = load_profile_snapshot(connection, &profile.id)?;
        if !snapshot_matches_result(&current, &result) {
            return Ok(false);
        }
        persist_result(connection, &result)?;
        Ok(true)
    })?;
    result.persisted = persisted;
    result.stale = !persisted;
    Ok(result)
}

fn persist_result(
    connection: &mut rusqlite::Connection,
    result: &AiProviderProbeResultDto,
) -> Result<(), CommandError> {
    let serialized = serde_json::to_string(result).map_err(CommandError::serialization)?;
    if serialized.len() > MAX_PERSISTED_RESULT_BYTES {
        return Err(CommandError::new(
            "ai_probe_result_too_large",
            "The redacted provider verification result exceeded ostudy's storage limit.",
        ));
    }
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    transaction
        .execute(
            "INSERT INTO ai_provider_probe_results (
                 profile_id, profile_revision, credential_epoch, tested_model_id,
                 adapter_revision, result_schema_version, overall_status,
                 result_json, tested_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(profile_id) DO UPDATE SET
                 profile_revision = excluded.profile_revision,
                 credential_epoch = excluded.credential_epoch,
                 tested_model_id = excluded.tested_model_id,
                 adapter_revision = excluded.adapter_revision,
                 result_schema_version = excluded.result_schema_version,
                 overall_status = excluded.overall_status,
                 result_json = excluded.result_json,
                 tested_at = excluded.tested_at",
            params![
                result.profile_id,
                result.profile_revision,
                result.credential_epoch,
                result.tested_model_id,
                result.adapter_revision,
                result.result_schema_version,
                result.overall_status.as_str(),
                serialized,
                result.tested_at,
            ],
        )
        .map_err(CommandError::database)?;
    transaction
        .execute(
            "DELETE FROM ai_provider_capabilities WHERE profile_id = ?1",
            [result.profile_id.as_str()],
        )
        .map_err(CommandError::database)?;
    for capability in &result.capabilities {
        let value_json =
            serde_json::to_string(&capability.value).map_err(CommandError::serialization)?;
        transaction
            .execute(
                "INSERT INTO ai_provider_capabilities (
                     profile_id, capability_key, status, source_kind,
                     value_json, checked_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    result.profile_id,
                    capability.key,
                    capability.status.as_str(),
                    capability.source.as_str(),
                    value_json,
                    result.tested_at,
                ],
            )
            .map_err(CommandError::database)?;
    }
    transaction.commit().map_err(CommandError::database)
}

fn snapshot_matches_result(profile: &ProfileSnapshot, result: &AiProviderProbeResultDto) -> bool {
    profile.revision == result.profile_revision
        && profile.credential_epoch == result.credential_epoch
        && profile.model_id == result.tested_model_id
}

fn build_capabilities(
    profile: &ProfileSnapshot,
    models: Option<&[ProviderModel]>,
    model_listing: bool,
    text_completion: bool,
    structured_output: bool,
    document_generation_schema: Option<bool>,
    usage_reported: bool,
    properties: &ProviderProperties,
) -> Vec<AiCapabilityDto> {
    let mut capabilities = default_unknown_capabilities(profile.provider_kind);
    set_capability(
        &mut capabilities,
        "model_listing",
        if model_listing {
            CapabilityStatus::Supported
        } else {
            CapabilityStatus::Unknown
        },
        CapabilitySource::Probed,
        None,
    );
    set_capability(
        &mut capabilities,
        "text_input",
        if text_completion {
            CapabilityStatus::Supported
        } else {
            CapabilityStatus::Unknown
        },
        CapabilitySource::Probed,
        None,
    );
    set_capability(
        &mut capabilities,
        "json_object_output",
        if structured_output {
            CapabilityStatus::Supported
        } else {
            CapabilityStatus::Unknown
        },
        CapabilitySource::Probed,
        None,
    );
    set_capability(
        &mut capabilities,
        "json_schema_output",
        match profile.provider_kind {
            ProviderKind::LlamaCpp | ProviderKind::Openrouter if structured_output => {
                CapabilityStatus::Supported
            }
            ProviderKind::LlamaCpp | ProviderKind::Openrouter => CapabilityStatus::Unknown,
            ProviderKind::Deepseek => CapabilityStatus::Unsupported,
            _ => CapabilityStatus::Unknown,
        },
        CapabilitySource::Probed,
        None,
    );
    if let Some(supported) = document_generation_schema {
        set_capability(
            &mut capabilities,
            "document_generation_schema",
            if supported {
                CapabilityStatus::Supported
            } else {
                CapabilityStatus::Unsupported
            },
            CapabilitySource::Probed,
            None,
        );
    }
    set_capability(
        &mut capabilities,
        "local_schema_validation",
        if structured_output {
            CapabilityStatus::Supported
        } else {
            CapabilityStatus::Unknown
        },
        CapabilitySource::Probed,
        None,
    );
    set_capability(
        &mut capabilities,
        "usage_reporting",
        if usage_reported {
            CapabilityStatus::Supported
        } else {
            CapabilityStatus::Unknown
        },
        CapabilitySource::Probed,
        None,
    );

    if let Some(model) =
        models.and_then(|models| models.iter().find(|model| model.id == profile.model_id))
    {
        if let Some(context_window_tokens) = model.context_window_tokens {
            set_capability(
                &mut capabilities,
                "context_window_tokens",
                CapabilityStatus::Supported,
                CapabilitySource::Declared,
                Some(json!(context_window_tokens)),
            );
        }
        if let Some(reasoning) = model.reasoning.as_ref() {
            set_capability(
                &mut capabilities,
                "reasoning_control",
                CapabilityStatus::Supported,
                CapabilitySource::Declared,
                serde_json::to_value(reasoning).ok(),
            );
        }
    }

    if let Some(present) = properties.server_chat_template_present {
        set_capability(
            &mut capabilities,
            "server_chat_template",
            if present {
                CapabilityStatus::Supported
            } else {
                CapabilityStatus::Unsupported
            },
            CapabilitySource::Declared,
            properties
                .server_chat_template_hash
                .as_ref()
                .map(|value| json!({ "sha256": value })),
        );
    }
    if let Some(image_input) = properties.image_input_declared {
        set_capability(
            &mut capabilities,
            "image_input",
            if image_input {
                CapabilityStatus::Supported
            } else {
                CapabilityStatus::Unsupported
            },
            CapabilitySource::Declared,
            None,
        );
    }
    if let Some(context_window_tokens) = properties.context_window_tokens {
        set_capability(
            &mut capabilities,
            "context_window_tokens",
            CapabilityStatus::Supported,
            CapabilitySource::Declared,
            Some(json!(context_window_tokens)),
        );
    }
    capabilities
}

fn default_unknown_capabilities(provider_kind: ProviderKind) -> Vec<AiCapabilityDto> {
    [
        "text_input",
        "image_input",
        "json_object_output",
        "json_schema_output",
        "document_generation_schema",
        "local_schema_validation",
        "model_listing",
        "usage_reporting",
        "server_chat_template",
        "streaming",
        "reasoning_control",
        "embeddings",
        "context_window_tokens",
    ]
    .into_iter()
    .map(|key| AiCapabilityDto {
        key: key.to_string(),
        status: if key == "json_schema_output" && provider_kind == ProviderKind::Deepseek {
            CapabilityStatus::Unsupported
        } else {
            CapabilityStatus::Unknown
        },
        source: CapabilitySource::Probed,
        value: None,
    })
    .collect()
}

fn set_capability(
    capabilities: &mut [AiCapabilityDto],
    key: &str,
    status: CapabilityStatus,
    source: CapabilitySource,
    value: Option<Value>,
) {
    if let Some(capability) = capabilities.iter_mut().find(|item| item.key == key) {
        capability.status = status;
        capability.source = source;
        capability.value = value;
    }
}

fn validate_profile_reference(input: &AiProfileReferenceInput) -> Result<(), CommandError> {
    validate_entity_id(&input.profile_id, "AI provider profile")?;
    if input.revision < 1 {
        return Err(CommandError::new(
            "invalid_revision",
            "The AI provider profile revision is invalid.",
        ));
    }
    Ok(())
}

fn elapsed_ms(started: Instant) -> i64 {
    i64::try_from(started.elapsed().as_millis()).unwrap_or(i64::MAX)
}

fn passed_phase(phase: &str, label: &str, message: &str, latency_ms: Option<i64>) -> ProbePhaseDto {
    ProbePhaseDto {
        phase: phase.to_string(),
        label: label.to_string(),
        status: ProbePhaseStatus::Passed,
        message: message.to_string(),
        latency_ms,
        error: None,
    }
}

fn failed_phase(
    phase: &str,
    label: &str,
    message: &str,
    latency_ms: Option<i64>,
    error: SafeAiError,
) -> ProbePhaseDto {
    ProbePhaseDto {
        phase: phase.to_string(),
        label: label.to_string(),
        status: ProbePhaseStatus::Failed,
        message: message.to_string(),
        latency_ms,
        error: Some(error),
    }
}

fn skipped_phase(phase: &str, label: &str, message: &str) -> ProbePhaseDto {
    ProbePhaseDto {
        phase: phase.to_string(),
        label: label.to_string(),
        status: ProbePhaseStatus::Skipped,
        message: message.to_string(),
        latency_ms: None,
        error: None,
    }
}

fn gateway_to_command(error_value: AiGatewayError) -> CommandError {
    CommandError::new(error_value.safe.code, error_value.safe.message)
}

fn profile_changed() -> CommandError {
    CommandError::new(
        "ai_profile_changed",
        "This AI provider profile changed before the request started. Reload it and try again.",
    )
}

#[cfg(test)]
mod tests {
    use super::super::catalog::AuthMode;
    use super::*;

    fn profile(provider_kind: ProviderKind, model_id: &str) -> ProfileSnapshot {
        ProfileSnapshot {
            id: "0123456789abcdef0123456789abcdef".to_string(),
            provider_kind,
            base_url: match provider_kind {
                ProviderKind::LlamaCpp => "http://127.0.0.1:8080/".to_string(),
                ProviderKind::Openrouter => "https://openrouter.ai/api/v1/".to_string(),
                _ => "https://api.deepseek.com/".to_string(),
            },
            model_id: model_id.to_string(),
            auth_mode: if matches!(
                provider_kind,
                ProviderKind::Deepseek | ProviderKind::Openrouter
            ) {
                AuthMode::Bearer
            } else {
                AuthMode::None
            },
            enabled: true,
            allow_insecure_http: false,
            require_zero_data_retention: false,
            request_timeout_ms: 60_000,
            credential_epoch: 2,
            revision: 3,
        }
    }

    #[test]
    fn deepseek_never_claims_provider_enforced_json_schema() {
        let capabilities = default_unknown_capabilities(ProviderKind::Deepseek);
        let schema = capabilities
            .iter()
            .find(|capability| capability.key == "json_schema_output")
            .expect("schema capability should exist");
        assert_eq!(schema.status, CapabilityStatus::Unsupported);
    }

    #[test]
    fn openrouter_probes_can_report_provider_enforced_json_schema_and_document_contract() {
        let profile = profile(ProviderKind::Openrouter, "openai/example");
        let capabilities = build_capabilities(
            &profile,
            None,
            true,
            true,
            true,
            Some(true),
            true,
            &ProviderProperties::default(),
        );
        let schema = capabilities
            .iter()
            .find(|capability| capability.key == "json_schema_output")
            .expect("schema capability should exist");
        assert_eq!(schema.status, CapabilityStatus::Supported);
        let document = capabilities
            .iter()
            .find(|capability| capability.key == "document_generation_schema")
            .expect("document capability should exist");
        assert_eq!(document.status, CapabilityStatus::Supported);
    }

    #[test]
    fn sole_llama_model_mismatch_uses_server_reported_id_for_probe() {
        let profile = profile(ProviderKind::LlamaCpp, "unsloth/repository-name");
        let models = vec![ProviderModel {
            id: "/models/Qwen3.6-27B-Q4_K_M.gguf".to_string(),
            display_name: "Qwen3.6".to_string(),
            owned_by: Some("llamacpp".to_string()),
            context_window_tokens: None,
            reasoning: None,
        }];
        assert_eq!(
            classify_model_match(&profile, &models),
            ModelMatch::SoleLlamaServerId("/models/Qwen3.6-27B-Q4_K_M.gguf".to_string())
        );
    }

    #[test]
    fn deepseek_model_mismatch_remains_a_hard_failure() {
        let profile = profile(ProviderKind::Deepseek, "deepseek-v4-flash");
        let models = vec![ProviderModel {
            id: "deepseek-v4-pro".to_string(),
            display_name: "deepseek-v4-pro".to_string(),
            owned_by: Some("deepseek".to_string()),
            context_window_tokens: None,
            reasoning: None,
        }];
        assert_eq!(classify_model_match(&profile, &models), ModelMatch::Missing);
    }

    #[test]
    fn probe_results_serialize_without_raw_provider_payloads() {
        let result = AiProviderProbeResultDto {
            profile_id: "0123456789abcdef0123456789abcdef".to_string(),
            profile_revision: 1,
            credential_epoch: 0,
            tested_model_id: "model".to_string(),
            adapter_revision: ADAPTER_REVISION,
            result_schema_version: RESULT_SCHEMA_VERSION,
            overall_status: ProbeOverallStatus::Verified,
            phases: vec![passed_phase("text", "Text", "Passed.", Some(4))],
            capabilities: default_unknown_capabilities(ProviderKind::LlamaCpp),
            tested_at: 1,
            persisted: false,
            stale: false,
        };
        let serialized = serde_json::to_string(&result).expect("result should serialize");
        assert!(!serialized.contains("authorization"));
        assert!(!serialized.contains("reasoning_content"));
        assert!(serialized.len() < MAX_PERSISTED_RESULT_BYTES);
    }

    #[test]
    fn revision_model_and_credential_epoch_bind_probe_freshness() {
        let profile = profile(ProviderKind::LlamaCpp, "model");
        let mut result = AiProviderProbeResultDto {
            profile_id: profile.id.clone(),
            profile_revision: 3,
            credential_epoch: 2,
            tested_model_id: "model".to_string(),
            adapter_revision: ADAPTER_REVISION,
            result_schema_version: RESULT_SCHEMA_VERSION,
            overall_status: ProbeOverallStatus::Verified,
            phases: Vec::new(),
            capabilities: Vec::new(),
            tested_at: 1,
            persisted: true,
            stale: false,
        };
        assert!(snapshot_matches_result(&profile, &result));
        result.credential_epoch = 1;
        assert!(!snapshot_matches_result(&profile, &result));
    }
}
