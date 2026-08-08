use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    database::Database,
    error::CommandError,
    util::{bool_to_integer, generate_id, unix_time_millis, validate_entity_id},
};

use super::{
    catalog::{
        provider_catalog, AiTaskKind, AuthMode, ProviderCatalogEntryDto, ProviderKind,
        MAX_TIMEOUT_MS, MIN_TIMEOUT_MS,
    },
    endpoint_policy::{
        endpoint_origin_from_str, validate_endpoint, NetworkScope, ValidatedEndpoint,
    },
    secrets::{
        CredentialPersistence, CredentialStatus, CredentialStatusDto, CredentialStorage,
    },
    AiRuntimeState,
};

const MAX_ACTIVE_PROFILES: usize = 32;
const MAX_PROFILE_NAME_CHARS: usize = 100;
const MAX_MODEL_ID_CHARS: usize = 300;

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
    pub request_timeout_ms: i64,
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
    pub request_timeout_ms: i64,
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
    pub auth_mode: AuthMode,
    pub enabled: bool,
    pub allow_insecure_http: bool,
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
    pub auth_mode: AuthMode,
    pub enabled: bool,
    pub allow_insecure_http: bool,
    pub request_timeout_ms: i64,
    pub credential_storage: CredentialStorage,
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
        validate_endpoint(self.provider_kind, &self.base_url, self.allow_insecure_http)?;
        Ok(())
    }
}

#[derive(Clone, Debug)]
struct StoredProfile {
    id: String,
    display_name: String,
    provider_kind: ProviderKind,
    base_url: String,
    model_id: String,
    auth_mode: AuthMode,
    enabled: bool,
    allow_insecure_http: bool,
    request_timeout_ms: i64,
    credential_storage: CredentialStorage,
    credential_epoch: i64,
    revision: i64,
    created_at: i64,
    updated_at: i64,
}

#[derive(Debug)]
struct RawProfileRow {
    id: String,
    display_name: String,
    provider_kind: String,
    base_url: String,
    model_id: String,
    auth_mode: String,
    enabled: i64,
    allow_insecure_http: i64,
    request_timeout_ms: i64,
    credential_storage: String,
    credential_epoch: i64,
    revision: i64,
    created_at: i64,
    updated_at: i64,
}

impl TryFrom<RawProfileRow> for StoredProfile {
    type Error = CommandError;

    fn try_from(row: RawProfileRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.id,
            display_name: row.display_name,
            provider_kind: ProviderKind::from_db(&row.provider_kind)?,
            base_url: row.base_url,
            model_id: row.model_id,
            auth_mode: AuthMode::from_db(&row.auth_mode)?,
            enabled: row.enabled != 0,
            allow_insecure_http: row.allow_insecure_http != 0,
            request_timeout_ms: row.request_timeout_ms,
            credential_storage: CredentialStorage::from_db(&row.credential_storage)?,
            credential_epoch: row.credential_epoch,
            revision: row.revision,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }
}

impl From<&StoredProfile> for ProfileSnapshot {
    fn from(profile: &StoredProfile) -> Self {
        Self {
            id: profile.id.clone(),
            provider_kind: profile.provider_kind,
            base_url: profile.base_url.clone(),
            model_id: profile.model_id.clone(),
            auth_mode: profile.auth_mode,
            enabled: profile.enabled,
            allow_insecure_http: profile.allow_insecure_http,
            request_timeout_ms: profile.request_timeout_ms,
            credential_storage: profile.credential_storage,
            credential_epoch: profile.credential_epoch,
            revision: profile.revision,
        }
    }
}

#[tauri::command]
pub fn get_ai_provider_catalog() -> Vec<ProviderCatalogEntryDto> {
    provider_catalog()
}

#[tauri::command]
pub fn list_ai_provider_profiles(
    database: State<'_, Database>,
    runtime: State<'_, AiRuntimeState>,
) -> Result<Vec<AiProviderProfileSummaryDto>, CommandError> {
    let profiles = database.with_connection(list_profiles)?;
    profiles
        .iter()
        .map(|profile| build_summary(profile, runtime.inner()))
        .collect()
}

#[tauri::command]
pub fn get_ai_provider_profile(
    database: State<'_, Database>,
    runtime: State<'_, AiRuntimeState>,
    profile_id: String,
) -> Result<AiProviderProfileDetailDto, CommandError> {
    validate_entity_id(&profile_id, "AI provider profile")?;
    let profile = database.with_connection(|connection| load_profile(connection, &profile_id))?;
    build_detail(&profile, runtime.inner())
}

#[tauri::command]
pub fn create_ai_provider_profile(
    database: State<'_, Database>,
    runtime: State<'_, AiRuntimeState>,
    input: CreateAiProviderProfileInput,
) -> Result<AiProviderProfileDetailDto, CommandError> {
    validate_operation_id(&input.operation_id)?;
    let _guard = lock_profile_mutations(runtime.inner())?;
    let profile_id = database.with_connection(|connection| create_profile(connection, input))?;
    let profile = database.with_connection(|connection| load_profile(connection, &profile_id))?;
    build_detail(&profile, runtime.inner())
}

#[tauri::command]
pub fn update_ai_provider_profile(
    database: State<'_, Database>,
    runtime: State<'_, AiRuntimeState>,
    input: UpdateAiProviderProfileInput,
) -> Result<AiProviderProfileDetailDto, CommandError> {
    validate_operation_id(&input.operation_id)?;
    validate_profile_reference(&input.profile_id, input.revision)?;
    let _guard = lock_profile_mutations(runtime.inner())?;

    if operation_was_replayed(
        database.inner(),
        &input.operation_id,
        "update_profile",
        Some(&input.profile_id),
    )? {
        let profile =
            database.with_connection(|connection| load_profile(connection, &input.profile_id))?;
        return build_detail(&profile, runtime.inner());
    }

    let current =
        database.with_connection(|connection| load_profile(connection, &input.profile_id))?;
    if current.revision != input.revision {
        return Err(profile_changed());
    }
    let next_endpoint = validate_endpoint(
        current.provider_kind,
        &input.base_url,
        input.allow_insecure_http,
    )?;
    ensure_trust_boundary_can_change(&current, &next_endpoint, input.auth_mode)?;

    let profile_id = database.with_connection(|connection| {
        update_profile(connection, current.provider_kind, next_endpoint, input)
    })?;
    let profile = database.with_connection(|connection| load_profile(connection, &profile_id))?;
    build_detail(&profile, runtime.inner())
}

#[tauri::command]
pub fn delete_ai_provider_profile(
    database: State<'_, Database>,
    runtime: State<'_, AiRuntimeState>,
    input: DeleteAiProviderProfileInput,
) -> Result<DeleteAiProviderProfileResultDto, CommandError> {
    validate_operation_id(&input.operation_id)?;
    validate_profile_reference(&input.profile_id, input.revision)?;
    let _guard = lock_profile_mutations(runtime.inner())?;

    if operation_was_replayed(
        database.inner(),
        &input.operation_id,
        "delete_profile",
        Some(&input.profile_id),
    )? {
        return Ok(DeleteAiProviderProfileResultDto {
            profile_id: input.profile_id,
            deleted: true,
        });
    }

    let current =
        database.with_connection(|connection| load_profile(connection, &input.profile_id))?;
    if current.revision != input.revision {
        return Err(profile_changed());
    }
    runtime
        .secrets
        .clear(&input.profile_id, current.credential_storage)?;
    database.with_connection(|connection| delete_profile(connection, &input))?;
    Ok(DeleteAiProviderProfileResultDto {
        profile_id: input.profile_id,
        deleted: true,
    })
}

#[tauri::command]
pub fn set_ai_provider_credential(
    database: State<'_, Database>,
    runtime: State<'_, AiRuntimeState>,
    input: SetAiProviderCredentialInput,
) -> Result<AiProviderProfileDetailDto, CommandError> {
    validate_operation_id(&input.operation_id)?;
    validate_profile_reference(&input.profile_id, input.revision)?;
    let _guard = lock_profile_mutations(runtime.inner())?;

    if operation_was_replayed(
        database.inner(),
        &input.operation_id,
        "set_credential",
        Some(&input.profile_id),
    )? {
        let profile =
            database.with_connection(|connection| load_profile(connection, &input.profile_id))?;
        return build_detail(&profile, runtime.inner());
    }

    let current =
        database.with_connection(|connection| load_profile(connection, &input.profile_id))?;
    if current.revision != input.revision {
        return Err(profile_changed());
    }
    if !current.auth_mode.requires_credential() {
        return Err(CommandError::new(
            "ai_credential_not_required",
            "This AI provider profile is configured without authentication.",
        ));
    }

    let storage = CredentialStorage::from(input.persistence);
    runtime.secrets.set(
        &input.profile_id,
        input.credential,
        input.persistence,
        current.credential_storage,
    )?;
    database.with_connection(|connection| {
        commit_credential_epoch(
            connection,
            &input.profile_id,
            input.revision,
            &input.operation_id,
            "set_credential",
            storage,
        )
    })?;
    let profile =
        database.with_connection(|connection| load_profile(connection, &input.profile_id))?;
    build_detail(&profile, runtime.inner())
}

#[tauri::command]
pub fn clear_ai_provider_credential(
    database: State<'_, Database>,
    runtime: State<'_, AiRuntimeState>,
    input: ClearAiProviderCredentialInput,
) -> Result<AiProviderProfileDetailDto, CommandError> {
    validate_operation_id(&input.operation_id)?;
    validate_profile_reference(&input.profile_id, input.revision)?;
    let _guard = lock_profile_mutations(runtime.inner())?;

    if operation_was_replayed(
        database.inner(),
        &input.operation_id,
        "clear_credential",
        Some(&input.profile_id),
    )? {
        let profile =
            database.with_connection(|connection| load_profile(connection, &input.profile_id))?;
        return build_detail(&profile, runtime.inner());
    }

    let current =
        database.with_connection(|connection| load_profile(connection, &input.profile_id))?;
    if current.revision != input.revision {
        return Err(profile_changed());
    }

    runtime
        .secrets
        .clear(&input.profile_id, current.credential_storage)?;
    database.with_connection(|connection| {
        commit_credential_epoch(
            connection,
            &input.profile_id,
            input.revision,
            &input.operation_id,
            "clear_credential",
            CredentialStorage::None,
        )
    })?;
    let profile =
        database.with_connection(|connection| load_profile(connection, &input.profile_id))?;
    build_detail(&profile, runtime.inner())
}

#[tauri::command]
pub fn get_ai_task_defaults(
    database: State<'_, Database>,
) -> Result<AiTaskDefaultsDto, CommandError> {
    database.with_connection(load_task_defaults)
}

#[tauri::command]
pub fn set_ai_task_default(
    database: State<'_, Database>,
    runtime: State<'_, AiRuntimeState>,
    input: SetAiTaskDefaultInput,
) -> Result<AiTaskDefaultsDto, CommandError> {
    validate_operation_id(&input.operation_id)?;
    if let Some(profile_id) = &input.profile_id {
        validate_entity_id(profile_id, "AI provider profile")?;
    }
    let _guard = lock_profile_mutations(runtime.inner())?;
    database.with_connection(|connection| set_task_default(connection, input))?;
    database.with_connection(load_task_defaults)
}

pub(crate) fn load_profile_snapshot(
    connection: &Connection,
    profile_id: &str,
) -> Result<ProfileSnapshot, CommandError> {
    load_profile(connection, profile_id).map(|profile| ProfileSnapshot::from(&profile))
}

fn create_profile(
    connection: &mut Connection,
    input: CreateAiProviderProfileInput,
) -> Result<String, CommandError> {
    if let Some(profile_id) =
        operation_replay(connection, &input.operation_id, "create_profile", None)?
    {
        return profile_id.ok_or_else(|| {
            CommandError::new(
                "ai_operation_invalid",
                "The saved create-profile operation has no profile identity.",
            )
        });
    }

    validate_provider_configuration(input.provider_kind, input.auth_mode)?;
    let display_name = validate_display_name(input.display_name)?;
    let normalized_name = normalize_name(&display_name);
    let model_id = validate_model_id(input.model_id)?;
    validate_timeout(input.request_timeout_ms)?;
    let endpoint = validate_endpoint(
        input.provider_kind,
        &input.base_url,
        input.allow_insecure_http,
    )?;
    ensure_profile_capacity(connection)?;
    ensure_name_available(connection, &normalized_name, None)?;
    let now = unix_time_millis()?;

    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    let id = generate_id(&transaction)?;
    transaction
        .execute(
            "INSERT INTO ai_provider_profiles (
                 id, display_name, normalized_name, provider_kind, base_url,
                 model_id, auth_mode, enabled, allow_insecure_http,
                 request_timeout_ms, credential_storage, credential_epoch, revision,
                 created_at, updated_at, deleted_at
             ) VALUES (
                 ?1, ?2, ?3, ?4, ?5,
                 ?6, ?7, ?8, ?9,
                 ?10, 'none', 0, 1,
                 ?11, ?11, NULL
             )",
            params![
                id,
                display_name,
                normalized_name,
                input.provider_kind.as_str(),
                endpoint.normalized_url,
                model_id,
                input.auth_mode.as_str(),
                bool_to_integer(input.enabled),
                bool_to_integer(input.allow_insecure_http),
                input.request_timeout_ms,
                now,
            ],
        )
        .map_err(CommandError::database)?;
    record_operation(
        &transaction,
        &input.operation_id,
        "create_profile",
        Some(&id),
        now,
    )?;
    transaction.commit().map_err(CommandError::database)?;
    Ok(id)
}

fn update_profile(
    connection: &mut Connection,
    provider_kind: ProviderKind,
    endpoint: ValidatedEndpoint,
    input: UpdateAiProviderProfileInput,
) -> Result<String, CommandError> {
    validate_provider_configuration(provider_kind, input.auth_mode)?;
    let display_name = validate_display_name(input.display_name)?;
    let normalized_name = normalize_name(&display_name);
    let model_id = validate_model_id(input.model_id)?;
    validate_timeout(input.request_timeout_ms)?;
    ensure_name_available(connection, &normalized_name, Some(&input.profile_id))?;
    let now = unix_time_millis()?;

    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    let changed = transaction
        .execute(
            "UPDATE ai_provider_profiles
             SET display_name = ?1,
                 normalized_name = ?2,
                 base_url = ?3,
                 model_id = ?4,
                 auth_mode = ?5,
                 enabled = ?6,
                 allow_insecure_http = ?7,
                 request_timeout_ms = ?8,
                 updated_at = ?9,
                 revision = revision + 1
             WHERE id = ?10 AND revision = ?11 AND deleted_at IS NULL",
            params![
                display_name,
                normalized_name,
                endpoint.normalized_url,
                model_id,
                input.auth_mode.as_str(),
                bool_to_integer(input.enabled),
                bool_to_integer(input.allow_insecure_http),
                input.request_timeout_ms,
                now,
                input.profile_id,
                input.revision,
            ],
        )
        .map_err(CommandError::database)?;
    if changed != 1 {
        return Err(profile_changed());
    }
    transaction
        .execute(
            "DELETE FROM ai_provider_probe_results WHERE profile_id = ?1",
            [input.profile_id.as_str()],
        )
        .map_err(CommandError::database)?;
    record_operation(
        &transaction,
        &input.operation_id,
        "update_profile",
        Some(&input.profile_id),
        now,
    )?;
    transaction.commit().map_err(CommandError::database)?;
    Ok(input.profile_id)
}

fn delete_profile(
    connection: &mut Connection,
    input: &DeleteAiProviderProfileInput,
) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    let changed = transaction
        .execute(
            "UPDATE ai_provider_profiles
             SET deleted_at = ?1,
                 enabled = 0,
                 credential_storage = 'none',
                 updated_at = ?1,
                 revision = revision + 1
             WHERE id = ?2 AND revision = ?3 AND deleted_at IS NULL",
            params![now, input.profile_id, input.revision],
        )
        .map_err(CommandError::database)?;
    if changed != 1 {
        return Err(profile_changed());
    }
    transaction
        .execute(
            "UPDATE ai_task_defaults
             SET profile_id = NULL, updated_at = ?1
             WHERE profile_id = ?2",
            params![now, input.profile_id],
        )
        .map_err(CommandError::database)?;
    record_operation(
        &transaction,
        &input.operation_id,
        "delete_profile",
        Some(&input.profile_id),
        now,
    )?;
    transaction.commit().map_err(CommandError::database)
}

fn commit_credential_epoch(
    connection: &mut Connection,
    profile_id: &str,
    revision: i64,
    operation_id: &str,
    operation_kind: &str,
    storage: CredentialStorage,
) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    let changed = transaction
        .execute(
            "UPDATE ai_provider_profiles
             SET credential_storage = ?1,
                 credential_epoch = credential_epoch + 1,
                 updated_at = ?2,
                 revision = revision + 1
             WHERE id = ?3 AND revision = ?4 AND deleted_at IS NULL",
            params![storage.as_str(), now, profile_id, revision],
        )
        .map_err(CommandError::database)?;
    if changed != 1 {
        return Err(profile_changed());
    }
    transaction
        .execute(
            "DELETE FROM ai_provider_probe_results WHERE profile_id = ?1",
            [profile_id],
        )
        .map_err(CommandError::database)?;
    record_operation(
        &transaction,
        operation_id,
        operation_kind,
        Some(profile_id),
        now,
    )?;
    transaction.commit().map_err(CommandError::database)
}

fn set_task_default(
    connection: &mut Connection,
    input: SetAiTaskDefaultInput,
) -> Result<(), CommandError> {
    if operation_replay(
        connection,
        &input.operation_id,
        "set_task_default",
        input.profile_id.as_deref(),
    )?
    .is_some()
    {
        return Ok(());
    }

    if let Some(profile_id) = &input.profile_id {
        let profile = load_profile(connection, profile_id)?;
        if !profile.provider_kind.implemented() {
            return Err(CommandError::new(
                "ai_provider_not_implemented",
                "That provider cannot be assigned until its adapter is implemented.",
            ));
        }
    }

    let now = unix_time_millis()?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    transaction
        .execute(
            "INSERT INTO ai_task_defaults (task_kind, profile_id, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(task_kind) DO UPDATE SET
                 profile_id = excluded.profile_id,
                 updated_at = excluded.updated_at",
            params![input.task_kind.as_str(), input.profile_id, now],
        )
        .map_err(CommandError::database)?;
    record_operation(
        &transaction,
        &input.operation_id,
        "set_task_default",
        input.profile_id.as_deref(),
        now,
    )?;
    transaction.commit().map_err(CommandError::database)
}

fn list_profiles(connection: &mut Connection) -> Result<Vec<StoredProfile>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT id, display_name, provider_kind, base_url,
                    model_id, auth_mode, enabled, allow_insecure_http,
                    request_timeout_ms, credential_storage, credential_epoch, revision,
                    created_at, updated_at
             FROM ai_provider_profiles
             WHERE deleted_at IS NULL
             ORDER BY updated_at DESC, id
             LIMIT ?1",
        )
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map(
            [i64::try_from(MAX_ACTIVE_PROFILES + 1).unwrap_or(33)],
            raw_row,
        )
        .map_err(CommandError::database)?;
    let raw = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    if raw.len() > MAX_ACTIVE_PROFILES {
        return Err(CommandError::new(
            "ai_profile_limit_exceeded",
            format!("ostudy supports at most {MAX_ACTIVE_PROFILES} active AI provider profiles."),
        ));
    }
    raw.into_iter().map(StoredProfile::try_from).collect()
}

fn load_profile(connection: &Connection, profile_id: &str) -> Result<StoredProfile, CommandError> {
    let raw = connection
        .query_row(
            "SELECT id, display_name, provider_kind, base_url,
                    model_id, auth_mode, enabled, allow_insecure_http,
                    request_timeout_ms, credential_storage, credential_epoch, revision,
                    created_at, updated_at
             FROM ai_provider_profiles
             WHERE id = ?1 AND deleted_at IS NULL",
            [profile_id],
            raw_row,
        )
        .optional()
        .map_err(CommandError::database)?;
    raw.map(StoredProfile::try_from)
        .transpose()?
        .ok_or_else(|| {
            CommandError::new(
                "ai_profile_not_found",
                "That AI provider profile no longer exists.",
            )
        })
}

fn raw_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RawProfileRow> {
    Ok(RawProfileRow {
        id: row.get(0)?,
        display_name: row.get(1)?,
        provider_kind: row.get(2)?,
        base_url: row.get(3)?,
        model_id: row.get(4)?,
        auth_mode: row.get(5)?,
        enabled: row.get(6)?,
        allow_insecure_http: row.get(7)?,
        request_timeout_ms: row.get(8)?,
        credential_storage: row.get(9)?,
        credential_epoch: row.get(10)?,
        revision: row.get(11)?,
        created_at: row.get(12)?,
        updated_at: row.get(13)?,
    })
}

fn load_task_defaults(connection: &mut Connection) -> Result<AiTaskDefaultsDto, CommandError> {
    let row: Option<Option<String>> = connection
        .query_row(
            "SELECT profile_id FROM ai_task_defaults WHERE task_kind = ?1",
            [AiTaskKind::FlashcardGeneration.as_str()],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()
        .map_err(CommandError::database)?;
    Ok(AiTaskDefaultsDto {
        flashcard_generation_profile_id: row.flatten(),
    })
}

fn build_summary(
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
        enabled: detail.enabled,
        network_scope: detail.network_scope,
        credential_status: detail.credential_status,
        configuration_status: detail.configuration_status,
        revision: detail.revision,
        updated_at: detail.updated_at,
    })
}

fn build_detail(
    profile: &StoredProfile,
    runtime: &AiRuntimeState,
) -> Result<AiProviderProfileDetailDto, CommandError> {
    let endpoint = validate_endpoint(
        profile.provider_kind,
        &profile.base_url,
        profile.allow_insecure_http,
    )?;
    let credential_status = runtime
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
        auth_mode: profile.auth_mode,
        enabled: profile.enabled,
        allow_insecure_http: profile.allow_insecure_http,
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
    } else if profile.model_id.trim().is_empty()
        || (profile.auth_mode.requires_credential()
            && !matches!(
                credential_status,
                CredentialStatus::SavedSecurely | CredentialStatus::SessionOnly
            ))
    {
        ProfileConfigurationStatus::Incomplete
    } else {
        ProfileConfigurationStatus::Ready
    }
}

fn ensure_trust_boundary_can_change(
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

fn validate_provider_configuration(
    provider_kind: ProviderKind,
    auth_mode: AuthMode,
) -> Result<(), CommandError> {
    if !provider_kind.implemented() {
        return Err(CommandError::new(
            "ai_provider_not_implemented",
            "That AI provider is planned but is not available in this ostudy build.",
        ));
    }
    if provider_kind == ProviderKind::Deepseek && auth_mode != AuthMode::Bearer {
        return Err(CommandError::new(
            "invalid_ai_auth_mode",
            "DeepSeek profiles require bearer-token authentication.",
        ));
    }
    Ok(())
}

fn validate_display_name(value: String) -> Result<String, CommandError> {
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

fn normalize_name(value: &str) -> String {
    value.to_lowercase()
}

fn validate_model_id(value: String) -> Result<String, CommandError> {
    let trimmed = value.trim().to_string();
    if trimmed.chars().count() > MAX_MODEL_ID_CHARS || trimmed.chars().any(char::is_control) {
        return Err(CommandError::new(
            "invalid_ai_model_id",
            format!("Model IDs cannot exceed {MAX_MODEL_ID_CHARS} characters or contain controls."),
        ));
    }
    Ok(trimmed)
}

fn validate_timeout(value: i64) -> Result<(), CommandError> {
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

fn validate_profile_reference(profile_id: &str, revision: i64) -> Result<(), CommandError> {
    validate_entity_id(profile_id, "AI provider profile")?;
    if revision < 1 {
        return Err(CommandError::new(
            "invalid_revision",
            "The AI provider profile revision is invalid.",
        ));
    }
    Ok(())
}

fn validate_operation_id(operation_id: &str) -> Result<(), CommandError> {
    validate_entity_id(operation_id, "operation")
}

fn lock_profile_mutations(
    runtime: &AiRuntimeState,
) -> Result<std::sync::MutexGuard<'_, ()>, CommandError> {
    runtime.profile_mutations.lock().map_err(|_| {
        CommandError::new(
            "ai_profile_mutation_lock_poisoned",
            "AI provider profile mutations are unavailable. Restart ostudy.",
        )
    })
}

fn operation_was_replayed(
    database: &Database,
    operation_id: &str,
    operation_kind: &str,
    profile_id: Option<&str>,
) -> Result<bool, CommandError> {
    Ok(database
        .with_connection(|connection| {
            operation_replay(connection, operation_id, operation_kind, profile_id)
        })?
        .is_some())
}

fn ensure_profile_capacity(connection: &Connection) -> Result<(), CommandError> {
    let count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM ai_provider_profiles WHERE deleted_at IS NULL",
            [],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    if count >= i64::try_from(MAX_ACTIVE_PROFILES).unwrap_or(32) {
        return Err(CommandError::new(
            "ai_profile_limit_exceeded",
            format!("ostudy supports at most {MAX_ACTIVE_PROFILES} active AI provider profiles."),
        ));
    }
    Ok(())
}

fn ensure_name_available(
    connection: &Connection,
    normalized_name: &str,
    excluding_id: Option<&str>,
) -> Result<(), CommandError> {
    let duplicate: i64 = connection
        .query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM ai_provider_profiles
                 WHERE normalized_name = ?1
                   AND deleted_at IS NULL
                   AND (?2 IS NULL OR id <> ?2)
             )",
            params![normalized_name, excluding_id],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    if duplicate != 0 {
        return Err(CommandError::new(
            "ai_profile_name_exists",
            "Another active AI provider profile already uses that name.",
        ));
    }
    Ok(())
}

fn operation_replay(
    connection: &Connection,
    operation_id: &str,
    expected_kind: &str,
    expected_profile_id: Option<&str>,
) -> Result<Option<Option<String>>, CommandError> {
    let existing: Option<(String, Option<String>)> = connection
        .query_row(
            "SELECT operation_kind, profile_id
             FROM ai_profile_operations
             WHERE operation_id = ?1",
            [operation_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(CommandError::database)?;
    let Some((operation_kind, profile_id)) = existing else {
        return Ok(None);
    };

    if operation_kind == "create_profile" && expected_kind == "create_profile" {
        return Ok(Some(profile_id));
    }
    if operation_kind != expected_kind || profile_id.as_deref() != expected_profile_id {
        return Err(CommandError::new(
            "ai_operation_conflict",
            "That operation identity was already used for a different AI profile mutation.",
        ));
    }
    Ok(Some(profile_id))
}

fn record_operation(
    connection: &Connection,
    operation_id: &str,
    operation_kind: &str,
    profile_id: Option<&str>,
    committed_at: i64,
) -> Result<(), CommandError> {
    connection
        .execute(
            "INSERT INTO ai_profile_operations (
                 operation_id, operation_kind, profile_id, committed_at
             ) VALUES (?1, ?2, ?3, ?4)",
            params![operation_id, operation_kind, profile_id, committed_at],
        )
        .map_err(CommandError::database)?;
    Ok(())
}

fn profile_changed() -> CommandError {
    CommandError::new(
        "ai_profile_changed",
        "This AI provider profile changed in another operation. Reload it and try again.",
    )
}

#[cfg(test)]
fn default_profile_input(provider_kind: ProviderKind) -> CreateAiProviderProfileInput {
    CreateAiProviderProfileInput {
        operation_id: String::new(),
        display_name: provider_kind.display_name().to_string(),
        provider_kind,
        base_url: provider_kind.default_base_url().to_string(),
        model_id: String::new(),
        auth_mode: provider_kind.default_auth_mode(),
        enabled: true,
        allow_insecure_http: false,
        request_timeout_ms: super::catalog::DEFAULT_TIMEOUT_MS,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_profile(database: &Database, provider_kind: ProviderKind) -> String {
        let mut input = default_profile_input(provider_kind);
        input.operation_id = "11111111111111111111111111111111".to_string();
        input.display_name = "Local model".to_string();
        input.model_id = "test-model".to_string();
        database
            .with_connection(|connection| create_profile(connection, input))
            .expect("profile should create")
    }

    #[test]
    fn creates_profiles_without_persisting_credentials() {
        let database = Database::open_in_memory().expect("database should open");
        let profile_id = create_test_profile(&database, ProviderKind::LlamaCpp);
        database
            .with_connection(|connection| {
                let columns: Vec<String> = connection
                    .prepare("SELECT name FROM pragma_table_info('ai_provider_profiles')")
                    .map_err(CommandError::database)?
                    .query_map([], |row| row.get(0))
                    .map_err(CommandError::database)?
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(CommandError::database)?;
                assert!(!columns.iter().any(|column| {
                    column.contains("secret")
                        || column.contains("token")
                        || column.contains("api_key")
                }));
                let storage: String = connection
                    .query_row(
                        "SELECT credential_storage FROM ai_provider_profiles WHERE id = ?1",
                        [profile_id.as_str()],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(storage, "none");
                let snapshot = load_profile_snapshot(connection, &profile_id)?;
                assert_eq!(snapshot.provider_kind, ProviderKind::LlamaCpp);
                assert_eq!(snapshot.credential_storage, CredentialStorage::None);
                Ok(())
            })
            .expect("profile should load");
    }

    #[test]
    fn replays_create_operations_without_duplicate_profiles() {
        let database = Database::open_in_memory().expect("database should open");
        let mut input = default_profile_input(ProviderKind::LlamaCpp);
        input.operation_id = "22222222222222222222222222222222".to_string();
        input.display_name = "Replay profile".to_string();
        let first = database
            .with_connection(|connection| create_profile(connection, input))
            .expect("profile should create");

        let mut retry = default_profile_input(ProviderKind::LlamaCpp);
        retry.operation_id = "22222222222222222222222222222222".to_string();
        retry.display_name = "Ignored retry payload".to_string();
        let second = database
            .with_connection(|connection| create_profile(connection, retry))
            .expect("operation should replay");
        assert_eq!(first, second);
    }

    #[test]
    fn credential_epoch_operations_are_idempotent_and_redacted() {
        let database = Database::open_in_memory().expect("database should open");
        let profile_id = create_test_profile(&database, ProviderKind::LlamaCpp);
        database
            .with_connection(|connection| {
                commit_credential_epoch(
                    connection,
                    &profile_id,
                    1,
                    "44444444444444444444444444444444",
                    "set_credential",
                    CredentialStorage::Session,
                )
            })
            .expect("credential epoch should commit");
        assert!(operation_was_replayed(
            &database,
            "44444444444444444444444444444444",
            "set_credential",
            Some(&profile_id),
        )
        .expect("operation replay should load"));
        database
            .with_connection(|connection| {
                let (storage, epoch): (String, i64) = connection
                    .query_row(
                        "SELECT credential_storage, credential_epoch
                         FROM ai_provider_profiles WHERE id = ?1",
                        [profile_id.as_str()],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(storage, "session");
                assert_eq!(epoch, 1);
                let operation_columns: Vec<String> = connection
                    .prepare("SELECT name FROM pragma_table_info('ai_profile_operations')")
                    .map_err(CommandError::database)?
                    .query_map([], |row| row.get(0))
                    .map_err(CommandError::database)?
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(CommandError::database)?;
                assert!(
                    !operation_columns
                        .iter()
                        .any(|column| column.contains("credential_value")
                            || column.contains("secret"))
                );
                Ok(())
            })
            .expect("credential operation should remain redacted");
    }

    #[test]
    fn rejects_planned_provider_profiles() {
        let database = Database::open_in_memory().expect("database should open");
        let mut input = default_profile_input(ProviderKind::Openai);
        input.operation_id = "33333333333333333333333333333333".to_string();
        assert!(database
            .with_connection(|connection| create_profile(connection, input))
            .is_err());
    }

    #[test]
    fn normalizes_profile_names_and_bounds_timeouts() {
        assert_eq!(
            validate_display_name("  Local   Model  ".to_string()).unwrap(),
            "Local Model"
        );
        assert!(validate_timeout(MIN_TIMEOUT_MS - 1).is_err());
        assert!(validate_timeout(MAX_TIMEOUT_MS + 1).is_err());
    }

    #[test]
    fn task_defaults_decode_nullable_profile_ids() {
        let database = Database::open_in_memory().expect("database should open");
        let defaults = database
            .with_connection(load_task_defaults)
            .expect("defaults should load");
        assert_eq!(defaults.flashcard_generation_profile_id, None);
    }
}
