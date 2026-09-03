use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

use crate::{
    database::Database,
    error::CommandError,
    util::{bool_to_integer, generate_id, unix_time_millis},
};

use super::{
    super::{
        catalog::{AuthMode, ProviderKind},
        secrets::CredentialStorage,
    },
    contracts::{
        CreateAiProviderProfileInput, DeleteAiProviderProfileInput, ProfileSnapshot, StoredProfile,
        UpdateAiProviderProfileInput,
    },
    policy::{profile_changed, ValidatedProfileFields, MAX_ACTIVE_PROFILES},
};

#[derive(Debug)]
struct RawProfileRow {
    id: String,
    display_name: String,
    provider_kind: String,
    base_url: String,
    model_id: String,
    reasoning_effort: String,
    auth_mode: String,
    enabled: i64,
    allow_insecure_http: i64,
    require_zero_data_retention: i64,
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
            reasoning_effort: row.reasoning_effort,
            auth_mode: AuthMode::from_db(&row.auth_mode)?,
            enabled: row.enabled != 0,
            allow_insecure_http: row.allow_insecure_http != 0,
            require_zero_data_retention: row.require_zero_data_retention != 0,
            request_timeout_ms: row.request_timeout_ms,
            credential_storage: CredentialStorage::from_db(&row.credential_storage)?,
            credential_epoch: row.credential_epoch,
            revision: row.revision,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }
}

pub(super) fn list_profiles(
    connection: &mut Connection,
) -> Result<Vec<StoredProfile>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT id, display_name, provider_kind, base_url,
                    model_id, reasoning_effort, auth_mode, enabled,
                    allow_insecure_http, require_zero_data_retention,
                    request_timeout_ms, credential_storage, credential_epoch,
                    revision, created_at, updated_at
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

pub(super) fn load_profile(
    connection: &Connection,
    profile_id: &str,
) -> Result<StoredProfile, CommandError> {
    let raw = connection
        .query_row(
            "SELECT id, display_name, provider_kind, base_url,
                    model_id, reasoning_effort, auth_mode, enabled,
                    allow_insecure_http, require_zero_data_retention,
                    request_timeout_ms, credential_storage, credential_epoch,
                    revision, created_at, updated_at
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

pub(crate) fn load_profile_snapshot(
    connection: &Connection,
    profile_id: &str,
) -> Result<ProfileSnapshot, CommandError> {
    load_profile(connection, profile_id).map(|profile| ProfileSnapshot::from(&profile))
}

pub(super) fn create_profile(
    connection: &mut Connection,
    input: CreateAiProviderProfileInput,
    fields: ValidatedProfileFields,
) -> Result<String, CommandError> {
    ensure_profile_capacity(connection)?;
    ensure_name_available(connection, &fields.normalized_name, None)?;
    let now = unix_time_millis()?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    let id = generate_id(&transaction)?;
    transaction
        .execute(
            "INSERT INTO ai_provider_profiles (
                 id, display_name, normalized_name, provider_kind, base_url,
                 model_id, reasoning_effort, auth_mode, enabled,
                 allow_insecure_http, require_zero_data_retention,
                 request_timeout_ms, credential_storage, credential_epoch,
                 revision, created_at, updated_at, deleted_at
             ) VALUES (
                 ?1, ?2, ?3, ?4, ?5,
                 ?6, ?7, ?8, ?9,
                 ?10, ?11, ?12, 'none', 0,
                 1, ?13, ?13, NULL
             )",
            params![
                id,
                fields.display_name,
                fields.normalized_name,
                input.provider_kind.as_str(),
                fields.endpoint.normalized_url,
                fields.model_id,
                fields.reasoning_effort,
                input.auth_mode.as_str(),
                bool_to_integer(input.enabled),
                bool_to_integer(input.allow_insecure_http),
                bool_to_integer(input.require_zero_data_retention),
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
    crate::editor_drafts::commit_canonical_editor_draft(
        &transaction,
        input.editor_draft.as_ref(),
        crate::editor_drafts::EditorDraftKind::AiProfile,
        Some(&id),
        None,
        None,
        now,
    )?;
    transaction.commit().map_err(CommandError::database)?;
    Ok(id)
}

pub(super) fn update_profile(
    connection: &mut Connection,
    input: UpdateAiProviderProfileInput,
    fields: ValidatedProfileFields,
) -> Result<String, CommandError> {
    ensure_name_available(connection, &fields.normalized_name, Some(&input.profile_id))?;
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
                 reasoning_effort = ?5,
                 auth_mode = ?6,
                 enabled = ?7,
                 allow_insecure_http = ?8,
                 require_zero_data_retention = ?9,
                 request_timeout_ms = ?10,
                 updated_at = ?11,
                 revision = revision + 1
             WHERE id = ?12 AND revision = ?13 AND deleted_at IS NULL",
            params![
                fields.display_name,
                fields.normalized_name,
                fields.endpoint.normalized_url,
                fields.model_id,
                fields.reasoning_effort,
                input.auth_mode.as_str(),
                bool_to_integer(input.enabled),
                bool_to_integer(input.allow_insecure_http),
                bool_to_integer(input.require_zero_data_retention),
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
    invalidate_probe_result(&transaction, &input.profile_id)?;
    record_operation(
        &transaction,
        &input.operation_id,
        "update_profile",
        Some(&input.profile_id),
        now,
    )?;
    crate::editor_drafts::commit_canonical_editor_draft(
        &transaction,
        input.editor_draft.as_ref(),
        crate::editor_drafts::EditorDraftKind::AiProfile,
        Some(&input.profile_id),
        Some(input.revision),
        None,
        now,
    )?;
    transaction.commit().map_err(CommandError::database)?;
    Ok(input.profile_id)
}

pub(super) fn delete_profile(
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
    super::defaults::clear_profile_defaults(&transaction, &input.profile_id, now)?;
    record_operation(
        &transaction,
        &input.operation_id,
        "delete_profile",
        Some(&input.profile_id),
        now,
    )?;
    transaction.commit().map_err(CommandError::database)
}

pub(super) fn commit_credential_epoch(
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
    invalidate_probe_result(&transaction, profile_id)?;
    record_operation(
        &transaction,
        operation_id,
        operation_kind,
        Some(profile_id),
        now,
    )?;
    transaction.commit().map_err(CommandError::database)
}

pub(super) fn operation_was_replayed(
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

pub(super) fn operation_replay(
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

pub(super) fn record_operation(
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

fn invalidate_probe_result(connection: &Connection, profile_id: &str) -> Result<(), CommandError> {
    connection
        .execute(
            "DELETE FROM ai_provider_probe_results WHERE profile_id = ?1",
            [profile_id],
        )
        .map_err(CommandError::database)?;
    Ok(())
}

fn raw_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RawProfileRow> {
    Ok(RawProfileRow {
        id: row.get(0)?,
        display_name: row.get(1)?,
        provider_kind: row.get(2)?,
        base_url: row.get(3)?,
        model_id: row.get(4)?,
        reasoning_effort: row.get(5)?,
        auth_mode: row.get(6)?,
        enabled: row.get(7)?,
        allow_insecure_http: row.get(8)?,
        require_zero_data_retention: row.get(9)?,
        request_timeout_ms: row.get(10)?,
        credential_storage: row.get(11)?,
        credential_epoch: row.get(12)?,
        revision: row.get(13)?,
        created_at: row.get(14)?,
        updated_at: row.get(15)?,
    })
}
