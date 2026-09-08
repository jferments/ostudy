use rusqlite::{params, Connection, OptionalExtension, Transaction};

use crate::{error::CommandError, util::unix_time_millis};

use super::{
    super::catalog::{AiTaskKind, ProviderKind},
    contracts::{AiTaskDefaultsDto, ProfileRequestSnapshot, SetAiTaskDefaultInput},
    repository::{load_profile, operation_replay, record_operation},
};

pub(super) fn load_task_defaults(
    connection: &mut Connection,
) -> Result<AiTaskDefaultsDto, CommandError> {
    Ok(AiTaskDefaultsDto {
        flashcard_generation_profile_id: task_profile_id(connection, AiTaskKind::FlashcardGeneration)?,
        tutor_profile_id: task_profile_id(connection, AiTaskKind::Tutor)?,
        embeddings_profile_id: task_profile_id(connection, AiTaskKind::Embeddings)?,
    })
}

fn task_profile_id(
    connection: &Connection,
    task: AiTaskKind,
) -> Result<Option<String>, CommandError> {
    connection
        .query_row(
            "SELECT profile_id FROM ai_task_defaults WHERE task_kind = ?1",
            [task.as_str()],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()
        .map(Option::flatten)
        .map_err(CommandError::database)
}

pub(crate) fn is_device_transfer_table(table: &str) -> bool {
    matches!(table, "ai_task_defaults" | "ai_embedding_options")
}

/// Provider defaults and embedding options transfer only with explicit API
/// configuration consent. Vector caches are never provider configuration.
pub(crate) fn sanitize_for_device_transfer(
    connection: &Connection,
    include_defaults: bool,
) -> Result<(), CommandError> {
    if !include_defaults {
        connection.execute("DELETE FROM ai_task_defaults", []).map_err(CommandError::database)?;
        connection.execute("DELETE FROM ai_embedding_options", []).map_err(CommandError::database)?;
    }
    Ok(())
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
        let saved_task: Option<String> = connection
            .query_row(
                "SELECT task_kind FROM ai_profile_operations WHERE operation_id = ?1",
                [&input.operation_id],
                |row| row.get(0),
            )
            .map_err(CommandError::database)?;
        // Pre-tutor operations could only select flashcard generation.
        if saved_task.as_deref().unwrap_or("flashcard_generation") != input.task_kind.as_str() {
            return Err(CommandError::new(
                "ai_operation_invalid",
                "This operation identity was already used for a different AI task.",
            ));
        }
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
        if input.task_kind == AiTaskKind::Embeddings
            && !matches!(profile.provider_kind, ProviderKind::Openrouter | ProviderKind::LlamaCpp)
        {
            return Err(CommandError::new(
                "ai_embeddings_unsupported",
                "Choose an OpenRouter or external llama.cpp embedding-model profile. A chat profile does not automatically support embeddings.",
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
    transaction
        .execute(
            "UPDATE ai_profile_operations SET task_kind = ?1 WHERE operation_id = ?2",
            params![input.task_kind.as_str(), input.operation_id],
        )
        .map_err(CommandError::database)?;
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
    connection: &mut Connection,
) -> Result<ProfileRequestSnapshot, CommandError> {
    load_task_profile_request(connection, AiTaskKind::FlashcardGeneration)
}

pub(crate) fn load_task_profile_request(
    connection: &mut Connection,
    task: AiTaskKind,
) -> Result<ProfileRequestSnapshot, CommandError> {
    let profile_id = task_profile_id(connection, task)?.ok_or_else(|| {
        CommandError::new(
            "ai_task_profile_required",
            match task {
                AiTaskKind::FlashcardGeneration => "Choose a default Flashcard generation profile in Settings → AI/LLM before analyzing a document.",
                AiTaskKind::Tutor => "Choose a default Tutor model in Settings → AI/LLM before starting a conversation.",
                AiTaskKind::Embeddings => "Choose and test an Embeddings model in Settings → AI/LLM before semantic preparation.",
            },
        )
    })?;
    let profile = load_profile(connection, &profile_id)?;
    Ok(ProfileRequestSnapshot {
        profile: (&profile).into(),
        credential_storage: profile.credential_storage,
    })
}
