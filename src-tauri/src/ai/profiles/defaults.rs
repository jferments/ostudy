use rusqlite::{params, Connection, OptionalExtension, Transaction};

use crate::{error::CommandError, util::unix_time_millis};

use super::{
    super::catalog::AiTaskKind,
    contracts::{AiTaskDefaultsDto, ProfileRequestSnapshot, SetAiTaskDefaultInput},
    repository::{load_profile, operation_replay, record_operation},
};

pub(super) fn load_task_defaults(
    connection: &mut Connection,
) -> Result<AiTaskDefaultsDto, CommandError> {
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

pub(super) fn set_task_default(
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
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
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

pub(super) fn clear_profile_defaults(
    transaction: &Transaction<'_>,
    profile_id: &str,
    now: i64,
) -> Result<(), CommandError> {
    transaction
        .execute(
            "UPDATE ai_task_defaults
             SET profile_id = NULL, updated_at = ?1
             WHERE profile_id = ?2",
            params![now, profile_id],
        )
        .map_err(CommandError::database)?;
    Ok(())
}

pub(crate) fn load_default_profile_request(
    connection: &Connection,
) -> Result<ProfileRequestSnapshot, CommandError> {
    let profile_id: Option<Option<String>> = connection
        .query_row(
            "SELECT profile_id FROM ai_task_defaults WHERE task_kind = ?1",
            [AiTaskKind::FlashcardGeneration.as_str()],
            |row| row.get(0),
        )
        .optional()
        .map_err(CommandError::database)?;
    let profile_id = profile_id.flatten().ok_or_else(|| {
        CommandError::new(
            "ai_task_profile_required",
            "Choose a default Flashcard generation profile in Settings → AI/LLM before analyzing a document.",
        )
    })?;
    let profile = load_profile(connection, &profile_id)?;
    Ok(ProfileRequestSnapshot {
        profile: (&profile).into(),
        credential_storage: profile.credential_storage,
    })
}
