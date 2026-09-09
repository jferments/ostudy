use rusqlite::{params, Connection, OptionalExtension, Transaction};

use crate::{error::CommandError, util::unix_time_millis};

use super::{
    super::{
        catalog::{AiTaskKind, ProviderKind},
        reasoning::validate_reasoning_effort,
    },
    contracts::{AiTaskDefaultsDto, ProfileRequestSnapshot, SetAiTaskDefaultInput, StoredProfile},
    policy::validate_model_id,
    repository::{load_profile, operation_replay, record_operation},
};

#[derive(Clone, Debug)]
struct TaskAssignment {
    profile_id: String,
    model_id: String,
    reasoning_effort: String,
}

pub(super) fn load_task_defaults(
    connection: &mut Connection,
) -> Result<AiTaskDefaultsDto, CommandError> {
    let flashcards = task_assignment(connection, AiTaskKind::FlashcardGeneration)?;
    let tutor = task_assignment(connection, AiTaskKind::Tutor)?;
    let embeddings = task_assignment(connection, AiTaskKind::Embeddings)?;
    Ok(AiTaskDefaultsDto {
        flashcard_generation_profile_id: flashcards.as_ref().map(|value| value.profile_id.clone()),
        flashcard_generation_model_id: flashcards.as_ref().map(|value| value.model_id.clone()),
        flashcard_generation_reasoning_effort: flashcards
            .as_ref()
            .map(|value| value.reasoning_effort.clone()),
        tutor_profile_id: tutor.as_ref().map(|value| value.profile_id.clone()),
        tutor_model_id: tutor.as_ref().map(|value| value.model_id.clone()),
        tutor_reasoning_effort: tutor.as_ref().map(|value| value.reasoning_effort.clone()),
        embeddings_profile_id: embeddings.as_ref().map(|value| value.profile_id.clone()),
        embeddings_model_id: embeddings.as_ref().map(|value| value.model_id.clone()),
        embeddings_reasoning_effort: embeddings
            .as_ref()
            .map(|value| value.reasoning_effort.clone()),
    })
}

fn task_assignment(
    connection: &Connection,
    task: AiTaskKind,
) -> Result<Option<TaskAssignment>, CommandError> {
    connection
        .query_row(
            "SELECT profile_id, model_id, reasoning_effort FROM ai_task_defaults
             WHERE task_kind = ?1 AND profile_id IS NOT NULL AND length(trim(model_id)) > 0",
            [task.as_str()],
            |row| {
                Ok(TaskAssignment {
                    profile_id: row.get(0)?,
                    model_id: row.get(1)?,
                    reasoning_effort: row.get(2)?,
                })
            },
        )
        .optional()
        .map_err(CommandError::database)
}

pub(crate) fn is_device_transfer_table(table: &str) -> bool {
    matches!(table, "ai_task_defaults" | "ai_embedding_options")
}

/// Provider/model defaults and embedding options transfer only with explicit API
/// configuration consent. Vector caches are never provider configuration.
pub(crate) fn sanitize_for_device_transfer(
    connection: &Connection,
    include_defaults: bool,
) -> Result<(), CommandError> {
    if !include_defaults {
        connection
            .execute("DELETE FROM ai_task_defaults", [])
            .map_err(CommandError::database)?;
        connection
            .execute("DELETE FROM ai_embedding_options", [])
            .map_err(CommandError::database)?;
    }
    Ok(())
}

pub(super) fn set_task_default(
    connection: &mut Connection,
    input: SetAiTaskDefaultInput,
) -> Result<(), CommandError> {
    let normalized = normalize_input(connection, &input)?;
    if operation_replay(
        connection,
        &input.operation_id,
        "set_task_default",
        normalized.as_ref().map(|value| value.profile_id.as_str()),
    )?
    .is_some()
    {
        let saved: (Option<String>, Option<String>, Option<String>) = connection
            .query_row(
                "SELECT task_kind, model_id, reasoning_effort
                 FROM ai_profile_operations WHERE operation_id = ?1",
                [&input.operation_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .map_err(CommandError::database)?;
        let expected_model = normalized.as_ref().map(|value| value.model_id.as_str());
        let expected_reasoning = normalized
            .as_ref()
            .map(|value| value.reasoning_effort.as_str());
        if saved.0.as_deref().unwrap_or("flashcard_generation") != input.task_kind.as_str()
            || saved.1.as_deref() != expected_model
            || saved.2.as_deref() != expected_reasoning
        {
            return Err(CommandError::new(
                "ai_operation_invalid",
                "This operation identity was already used for a different AI task model assignment.",
            ));
        }
        return Ok(());
    }

    let now = unix_time_millis()?;
    let transaction = connection
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    match normalized.as_ref() {
        Some(assignment) => {
            transaction
                .execute(
                    "INSERT INTO ai_task_defaults (task_kind, profile_id, model_id, reasoning_effort, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5)
                     ON CONFLICT(task_kind) DO UPDATE SET
                         profile_id = excluded.profile_id,
                         model_id = excluded.model_id,
                         reasoning_effort = excluded.reasoning_effort,
                         updated_at = excluded.updated_at",
                    params![
                        input.task_kind.as_str(),
                        assignment.profile_id,
                        assignment.model_id,
                        assignment.reasoning_effort,
                        now,
                    ],
                )
                .map_err(CommandError::database)?;
        }
        None => {
            transaction
                .execute(
                    "INSERT INTO ai_task_defaults (task_kind, profile_id, model_id, reasoning_effort, updated_at)
                     VALUES (?1, NULL, '', 'automatic', ?2)
                     ON CONFLICT(task_kind) DO UPDATE SET profile_id = NULL, model_id = '',
                         reasoning_effort = 'automatic', updated_at = excluded.updated_at",
                    params![input.task_kind.as_str(), now],
                )
                .map_err(CommandError::database)?;
        }
    }
    record_operation(
        &transaction,
        &input.operation_id,
        "set_task_default",
        normalized.as_ref().map(|value| value.profile_id.as_str()),
        now,
    )?;
    transaction
        .execute(
            "UPDATE ai_profile_operations
             SET task_kind = ?1, model_id = ?2, reasoning_effort = ?3
             WHERE operation_id = ?4",
            params![
                input.task_kind.as_str(),
                normalized.as_ref().map(|value| value.model_id.as_str()),
                normalized
                    .as_ref()
                    .map(|value| value.reasoning_effort.as_str()),
                input.operation_id,
            ],
        )
        .map_err(CommandError::database)?;
    transaction.commit().map_err(CommandError::database)
}

fn normalize_input(
    connection: &Connection,
    input: &SetAiTaskDefaultInput,
) -> Result<Option<TaskAssignment>, CommandError> {
    let Some(profile_id) = input.profile_id.as_deref() else {
        if input
            .model_id
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
        {
            return Err(CommandError::new(
                "invalid_ai_task_default",
                "Clear the model when clearing an AI provider assignment.",
            ));
        }
        return Ok(None);
    };
    crate::util::validate_entity_id(profile_id, "AI provider profile")?;
    let model_id = validate_model_id(input.model_id.as_deref().unwrap_or_default())?;
    if model_id.is_empty() {
        return Err(CommandError::new(
            "invalid_ai_task_default",
            "Choose a model for the selected AI provider.",
        ));
    }
    let profile = load_profile(connection, profile_id)?;
    validate_profile_for_task(&profile, input.task_kind)?;
    let requested_reasoning = if input.task_kind == AiTaskKind::Embeddings {
        "none"
    } else {
        input.reasoning_effort.as_deref().unwrap_or("automatic")
    };
    let reasoning_effort = validate_reasoning_effort(requested_reasoning)?;
    Ok(Some(TaskAssignment {
        profile_id: profile.id,
        model_id,
        reasoning_effort,
    }))
}

fn validate_profile_for_task(
    profile: &StoredProfile,
    task: AiTaskKind,
) -> Result<(), CommandError> {
    if !profile.provider_kind.implemented() {
        return Err(CommandError::new(
            "ai_provider_not_implemented",
            "That provider cannot be assigned until its adapter is implemented.",
        ));
    }
    if task == AiTaskKind::Embeddings
        && !matches!(
            profile.provider_kind,
            ProviderKind::Openrouter | ProviderKind::LlamaCpp
        )
    {
        return Err(CommandError::new(
            "ai_embeddings_unsupported",
            "Choose an OpenRouter or external llama.cpp provider for semantic embeddings.",
        ));
    }
    Ok(())
}

pub(super) fn clear_profile_defaults(
    transaction: &Transaction<'_>,
    profile_id: &str,
    now: i64,
) -> Result<(), CommandError> {
    transaction
        .execute(
            "UPDATE ai_task_defaults
             SET profile_id = NULL, model_id = '', reasoning_effort = 'automatic', updated_at = ?1
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
    let assignment = task_assignment(connection, task)?.ok_or_else(|| {
        CommandError::new(
            "ai_task_profile_required",
            match task {
                AiTaskKind::FlashcardGeneration => "Choose a default Flashcard generation model in Settings → AI/LLM before analyzing a document.",
                AiTaskKind::Tutor => "Choose a default Tutor model in Settings → AI/LLM before starting a conversation.",
                AiTaskKind::Embeddings => "Choose and test a Semantic embeddings model in Settings → AI/LLM before semantic preparation.",
            },
        )
    })?;
    let profile = load_profile(connection, &assignment.profile_id)?;
    Ok(ProfileRequestSnapshot {
        profile: profile.request_snapshot(&assignment.model_id, &assignment.reasoning_effort),
        credential_storage: profile.credential_storage,
    })
}
