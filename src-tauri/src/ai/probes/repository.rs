use rusqlite::{params, OptionalExtension, TransactionBehavior};

use crate::{database::Database, error::CommandError};

use super::{
    contracts::{
        AiProviderProbeResultDto, DocumentProbeEvidence, PersistedCapability, ProbeOverallStatus,
        ADAPTER_REVISION, MAX_PERSISTED_RESULT_BYTES, RESULT_SCHEMA_VERSION,
    },
    super::{profiles::{load_profile_snapshot, ProfileSnapshot}, providers::ProviderModel},
};

pub(super) fn persist_selected_model_declarations(
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

pub(super) fn load_probe_result(
    database: &Database,
    profile_id: &str,
) -> Result<Option<AiProviderProbeResultDto>, CommandError> {
    database.with_connection(|connection| {
        let serialized: Option<String> = connection
            .query_row(
                "SELECT result_json FROM ai_provider_probe_results WHERE profile_id = ?1",
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
        let current = load_profile_snapshot(connection, profile_id)?;
        result.stale = !snapshot_matches_result(&current, &result);
        result.persisted = true;
        Ok(Some(result))
    })
}

pub(crate) fn load_capabilities(
    database: &Database,
    profile_id: &str,
) -> Result<Vec<PersistedCapability>, CommandError> {
    database.with_connection(|connection| {
        let mut statement = connection
            .prepare(
                "SELECT capability_key, status, value_json
                 FROM ai_provider_capabilities
                 WHERE profile_id = ?1
                 ORDER BY capability_key",
            )
            .map_err(CommandError::database)?;
        let rows = statement
            .query_map([profile_id], |row| {
                Ok(PersistedCapability {
                    key: row.get(0)?,
                    status: row.get(1)?,
                    value_json: row.get(2)?,
                })
            })
            .map_err(CommandError::database)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)
    })
}

pub(crate) fn load_document_probe_evidence(
    database: &Database,
    profile: &ProfileSnapshot,
) -> Result<DocumentProbeEvidence, CommandError> {
    database.with_connection(|connection| {
        let probe_matches = connection
            .query_row(
                "SELECT EXISTS(
                     SELECT 1 FROM ai_provider_probe_results
                     WHERE profile_id = ?1
                       AND profile_revision = ?2
                       AND credential_epoch = ?3
                       AND tested_model_id = ?4
                       AND overall_status IN ('verified', 'partial')
                 )",
                params![
                    profile.id,
                    profile.revision,
                    profile.credential_epoch,
                    profile.model_id,
                ],
                |row| row.get::<_, i64>(0),
            )
            .map(|value| value != 0)
            .map_err(CommandError::database)?;
        let image_status: Option<String> = connection
            .query_row(
                "SELECT status FROM ai_provider_capabilities
                 WHERE profile_id = ?1 AND capability_key = 'image_input'",
                [profile.id.as_str()],
                |row| row.get(0),
            )
            .optional()
            .map_err(CommandError::database)?;
        let document_schema_status: Option<String> = connection
            .query_row(
                "SELECT status FROM ai_provider_capabilities
                 WHERE profile_id = ?1 AND capability_key = 'document_generation_schema'",
                [profile.id.as_str()],
                |row| row.get(0),
            )
            .optional()
            .map_err(CommandError::database)?;
        Ok(DocumentProbeEvidence {
            probe_matches,
            image_input_supported: image_status.as_deref() == Some("supported"),
            document_generation_schema_supported: document_schema_status.as_deref()
                == Some("supported"),
        })
    })
}

pub(super) fn snapshot_matches_result(
    profile: &ProfileSnapshot,
    result: &AiProviderProbeResultDto,
) -> bool {
    profile.revision == result.profile_revision
        && profile.credential_epoch == result.credential_epoch
        && profile.model_id == result.tested_model_id
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
