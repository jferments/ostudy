use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde_json::Value;

use crate::{database::Database, error::CommandError};

use super::{
    super::{
        profiles::{load_profile_snapshot, ProfileSnapshot},
        providers::ProviderModel,
    },
    contracts::{
        AiProviderProbeResultDto, CapabilityStatus, DocumentProbeEvidence, PersistedCapability,
        ProbeOverallStatus, ADAPTER_REVISION, MAX_PERSISTED_RESULT_BYTES, RESULT_SCHEMA_VERSION,
    },
};

pub(crate) fn sanitize_for_device_transfer(
    connection: &rusqlite::Connection,
) -> Result<(), CommandError> {
    connection
        .execute_batch(
            "DELETE FROM ai_provider_probe_results;
             DELETE FROM ai_provider_capabilities;",
        )
        .map_err(CommandError::database)
}

/// Cache provider-declared model metadata per model. Model discovery is a read
/// and never changes the provider connection or task assignments.
pub(super) fn persist_model_declarations(
    database: &Database,
    profile: &ProfileSnapshot,
    models: &[ProviderModel],
    checked_at: i64,
) -> Result<(), CommandError> {
    database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        transaction
            .execute(
                "DELETE FROM ai_provider_capabilities
                 WHERE profile_id = ?1 AND source_kind = 'declared'
                   AND capability_key IN ('context_window_tokens', 'reasoning_control')",
                [profile.id.as_str()],
            )
            .map_err(CommandError::database)?;
        for model in models {
            if let Some(context) = model.context_window_tokens {
                transaction
                    .execute(
                        "INSERT INTO ai_provider_capabilities (
                             profile_id, model_id, capability_key, status, source_kind, value_json, checked_at
                         ) VALUES (?1, ?2, 'context_window_tokens', 'supported', 'declared', ?3, ?4)
                         ON CONFLICT(profile_id, model_id, capability_key) DO UPDATE SET
                             status = excluded.status, source_kind = excluded.source_kind,
                             value_json = excluded.value_json, checked_at = excluded.checked_at",
                        params![profile.id, model.id, context.to_string(), checked_at],
                    )
                    .map_err(CommandError::database)?;
            }
            if let Some(reasoning) = model.reasoning.as_ref() {
                let value = serde_json::to_string(&Some(reasoning))
                    .map_err(CommandError::serialization)?;
                transaction
                    .execute(
                        "INSERT INTO ai_provider_capabilities (
                             profile_id, model_id, capability_key, status, source_kind, value_json, checked_at
                         ) VALUES (?1, ?2, 'reasoning_control', 'supported', 'declared', ?3, ?4)
                         ON CONFLICT(profile_id, model_id, capability_key) DO UPDATE SET
                             status = excluded.status, source_kind = excluded.source_kind,
                             value_json = excluded.value_json, checked_at = excluded.checked_at",
                        params![profile.id, model.id, value, checked_at],
                    )
                    .map_err(CommandError::database)?;
            }
        }
        transaction.commit().map_err(CommandError::database)
    })
}

pub(super) fn finish_and_persist(
    database: &Database,
    profile: &ProfileSnapshot,
    overall_status: ProbeOverallStatus,
    phases: Vec<super::contracts::ProbePhaseDto>,
    capabilities: Vec<super::contracts::AiCapabilityDto>,
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
        let mut current = load_profile_snapshot(connection, &profile.id)?;
        current.model_id.clone_from(&result.tested_model_id);
        current.reasoning_effort.clone_from(&profile.reasoning_effort);
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

/// Legacy profile-detail UI asks for one probe result. Return the most recent
/// model probe for that connection; task execution uses model-specific lookups.
pub(super) fn load_probe_result(
    database: &Database,
    profile_id: &str,
) -> Result<Option<AiProviderProbeResultDto>, CommandError> {
    database.with_connection(|connection| {
        let serialized: Option<String> = connection
            .query_row(
                "SELECT result_json FROM ai_provider_probe_results
                 WHERE profile_id = ?1 ORDER BY tested_at DESC, tested_model_id LIMIT 1",
                [profile_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(CommandError::database)?;
        let Some(serialized) = serialized else {
            return Ok(None);
        };
        let mut result: AiProviderProbeResultDto =
            serde_json::from_str(&serialized).map_err(CommandError::serialization)?;
        let mut current = load_profile_snapshot(connection, profile_id)?;
        current.model_id.clone_from(&result.tested_model_id);
        result.stale = !snapshot_matches_result(&current, &result);
        result.persisted = true;
        Ok(Some(result))
    })
}

pub(crate) fn load_capability_records(
    database: &Database,
    profile_id: &str,
    model_id: &str,
) -> Result<Vec<PersistedCapability>, CommandError> {
    database.with_connection(|connection| {
        let mut profile = load_profile_snapshot(connection, profile_id)?;
        profile.model_id = model_id.to_string();
        if !probe_matches_current(connection, &profile)? {
            return Ok(Vec::new());
        }
        let mut statement = connection
            .prepare(
                "SELECT capability_key, status, value_json
                 FROM ai_provider_capabilities
                 WHERE profile_id = ?1 AND model_id = ?2
                 ORDER BY capability_key",
            )
            .map_err(CommandError::database)?;
        let records = statement
            .query_map(params![profile_id, model_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        records
            .into_iter()
            .map(|(key, status, value_json)| {
                let status = CapabilityStatus::from_db(&status)?;
                let value: Value = serde_json::from_str(&value_json)
                    .map_err(CommandError::serialization)?;
                Ok(PersistedCapability {
                    key,
                    status,
                    value: (!value.is_null()).then_some(value),
                })
            })
            .collect()
    })
}

pub(crate) fn load_document_probe_evidence(
    database: &Database,
    profile: &ProfileSnapshot,
) -> Result<DocumentProbeEvidence, CommandError> {
    database.with_connection(|connection| {
        let probe_matches = probe_matches_current(connection, profile)?;
        let image_status: Option<String> = connection
            .query_row(
                "SELECT status FROM ai_provider_capabilities
                 WHERE profile_id = ?1 AND model_id = ?2 AND capability_key = 'image_input'",
                params![profile.id, profile.model_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(CommandError::database)?;
        let document_schema_status: Option<String> = connection
            .query_row(
                "SELECT status FROM ai_provider_capabilities
                 WHERE profile_id = ?1 AND model_id = ?2
                   AND capability_key = 'document_generation_schema'",
                params![profile.id, profile.model_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(CommandError::database)?;
        Ok(DocumentProbeEvidence {
            probe_matches,
            image_input_supported: probe_matches && image_status.as_deref() == Some("supported"),
            document_generation_schema_supported: probe_matches
                && document_schema_status.as_deref() == Some("supported"),
        })
    })
}

fn probe_matches_current(
    connection: &rusqlite::Connection,
    profile: &ProfileSnapshot,
) -> Result<bool, CommandError> {
    connection
        .query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM ai_provider_probe_results
                 WHERE profile_id = ?1 AND tested_model_id = ?2
                   AND profile_revision = ?3 AND credential_epoch = ?4
                   AND adapter_revision = ?5 AND result_schema_version = ?6
                   AND overall_status IN ('verified', 'partial')
             )",
            params![
                profile.id,
                profile.model_id,
                profile.revision,
                profile.credential_epoch,
                ADAPTER_REVISION,
                RESULT_SCHEMA_VERSION,
            ],
            |row| row.get::<_, i64>(0),
        )
        .map(|value| value != 0)
        .map_err(CommandError::database)
}

pub(super) fn snapshot_matches_result(
    profile: &ProfileSnapshot,
    result: &AiProviderProbeResultDto,
) -> bool {
    profile.revision == result.profile_revision
        && profile.credential_epoch == result.credential_epoch
        && profile.model_id == result.tested_model_id
        && result.adapter_revision == ADAPTER_REVISION
        && result.result_schema_version == RESULT_SCHEMA_VERSION
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
                 adapter_revision, result_schema_version, overall_status, result_json, tested_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(profile_id, tested_model_id) DO UPDATE SET
                 profile_revision = excluded.profile_revision,
                 credential_epoch = excluded.credential_epoch,
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
            "DELETE FROM ai_provider_capabilities WHERE profile_id = ?1 AND model_id = ?2",
            params![result.profile_id, result.tested_model_id],
        )
        .map_err(CommandError::database)?;
    for capability in &result.capabilities {
        let value_json = serde_json::to_string(&capability.value)
            .map_err(CommandError::serialization)?;
        transaction
            .execute(
                "INSERT INTO ai_provider_capabilities (
                     profile_id, model_id, capability_key, status, source_kind,
                     value_json, checked_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    result.profile_id,
                    result.tested_model_id,
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
