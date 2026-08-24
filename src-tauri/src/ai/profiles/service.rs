use crate::{
    database::Database,
    error::CommandError,
    util::validate_entity_id,
};

use super::{
    contracts::{
        AiProviderProfileDetailDto, AiProviderProfileSummaryDto, AiTaskDefaultsDto,
        ClearAiProviderCredentialInput, CreateAiProviderProfileInput,
        DeleteAiProviderProfileInput, DeleteAiProviderProfileResultDto, ProfileRequestSnapshot,
        SetAiProviderCredentialInput, SetAiTaskDefaultInput, UpdateAiProviderProfileInput,
    },
    defaults,
    policy::{
        ensure_trust_boundary_can_change, profile_changed, validate_create_profile,
        validate_operation_id, validate_profile_reference, validate_update_profile,
    },
    projections::{build_detail, build_summary},
    repository,
    super::{secrets::CredentialStorage, AiRuntimeState},
};

pub(super) fn list_profiles(
    database: &Database,
    runtime: &AiRuntimeState,
) -> Result<Vec<AiProviderProfileSummaryDto>, CommandError> {
    let profiles = database.with_connection(repository::list_profiles)?;
    profiles
        .iter()
        .map(|profile| build_summary(profile, runtime))
        .collect()
}

pub(super) fn get_profile(
    database: &Database,
    runtime: &AiRuntimeState,
    profile_id: &str,
) -> Result<AiProviderProfileDetailDto, CommandError> {
    validate_entity_id(profile_id, "AI provider profile")?;
    load_detail(database, runtime, profile_id)
}

pub(super) fn create_profile(
    database: &Database,
    runtime: &AiRuntimeState,
    input: CreateAiProviderProfileInput,
) -> Result<AiProviderProfileDetailDto, CommandError> {
    validate_operation_id(&input.operation_id)?;
    let _guard = lock_profile_mutations(runtime)?;
    if let Some(profile_id) = database.with_connection(|connection| {
        repository::operation_replay(connection, &input.operation_id, "create_profile", None)
    })? {
        let profile_id = profile_id.ok_or_else(|| {
            CommandError::new(
                "ai_operation_invalid",
                "The saved create-profile operation has no profile identity.",
            )
        })?;
        return load_detail(database, runtime, &profile_id);
    }
    let fields = validate_create_profile(&input)?;
    let profile_id =
        database.with_connection(|connection| repository::create_profile(connection, input, fields))?;
    load_detail(database, runtime, &profile_id)
}

pub(super) fn update_profile(
    database: &Database,
    runtime: &AiRuntimeState,
    input: UpdateAiProviderProfileInput,
) -> Result<AiProviderProfileDetailDto, CommandError> {
    validate_operation_id(&input.operation_id)?;
    validate_profile_reference(&input.profile_id, input.revision)?;
    let _guard = lock_profile_mutations(runtime)?;
    if repository::operation_was_replayed(
        database,
        &input.operation_id,
        "update_profile",
        Some(&input.profile_id),
    )? {
        return load_detail(database, runtime, &input.profile_id);
    }

    let current =
        database.with_connection(|connection| repository::load_profile(connection, &input.profile_id))?;
    if current.revision != input.revision {
        return Err(profile_changed());
    }
    let fields = validate_update_profile(current.provider_kind, &input)?;
    ensure_trust_boundary_can_change(&current, &fields.endpoint, input.auth_mode)?;
    let profile_id =
        database.with_connection(|connection| repository::update_profile(connection, input, fields))?;
    load_detail(database, runtime, &profile_id)
}

pub(super) fn delete_profile(
    database: &Database,
    runtime: &AiRuntimeState,
    input: DeleteAiProviderProfileInput,
) -> Result<DeleteAiProviderProfileResultDto, CommandError> {
    validate_operation_id(&input.operation_id)?;
    validate_profile_reference(&input.profile_id, input.revision)?;
    let _guard = lock_profile_mutations(runtime)?;
    if repository::operation_was_replayed(
        database,
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
        database.with_connection(|connection| repository::load_profile(connection, &input.profile_id))?;
    if current.revision != input.revision {
        return Err(profile_changed());
    }
    runtime
        .secrets
        .clear(&input.profile_id, current.credential_storage)?;
    database.with_connection(|connection| repository::delete_profile(connection, &input))?;
    Ok(DeleteAiProviderProfileResultDto {
        profile_id: input.profile_id,
        deleted: true,
    })
}

pub(super) fn set_credential(
    database: &Database,
    runtime: &AiRuntimeState,
    input: SetAiProviderCredentialInput,
) -> Result<AiProviderProfileDetailDto, CommandError> {
    validate_operation_id(&input.operation_id)?;
    validate_profile_reference(&input.profile_id, input.revision)?;
    let _guard = lock_profile_mutations(runtime)?;
    if repository::operation_was_replayed(
        database,
        &input.operation_id,
        "set_credential",
        Some(&input.profile_id),
    )? {
        return load_detail(database, runtime, &input.profile_id);
    }

    let current =
        database.with_connection(|connection| repository::load_profile(connection, &input.profile_id))?;
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
        repository::commit_credential_epoch(
            connection,
            &input.profile_id,
            input.revision,
            &input.operation_id,
            "set_credential",
            storage,
        )
    })?;
    load_detail(database, runtime, &input.profile_id)
}

pub(super) fn clear_credential(
    database: &Database,
    runtime: &AiRuntimeState,
    input: ClearAiProviderCredentialInput,
) -> Result<AiProviderProfileDetailDto, CommandError> {
    validate_operation_id(&input.operation_id)?;
    validate_profile_reference(&input.profile_id, input.revision)?;
    let _guard = lock_profile_mutations(runtime)?;
    if repository::operation_was_replayed(
        database,
        &input.operation_id,
        "clear_credential",
        Some(&input.profile_id),
    )? {
        return load_detail(database, runtime, &input.profile_id);
    }

    let current =
        database.with_connection(|connection| repository::load_profile(connection, &input.profile_id))?;
    if current.revision != input.revision {
        return Err(profile_changed());
    }
    runtime
        .secrets
        .clear(&input.profile_id, current.credential_storage)?;
    database.with_connection(|connection| {
        repository::commit_credential_epoch(
            connection,
            &input.profile_id,
            input.revision,
            &input.operation_id,
            "clear_credential",
            CredentialStorage::None,
        )
    })?;
    load_detail(database, runtime, &input.profile_id)
}

pub(super) fn get_task_defaults(database: &Database) -> Result<AiTaskDefaultsDto, CommandError> {
    database.with_connection(defaults::load_task_defaults)
}

pub(super) fn set_task_default(
    database: &Database,
    runtime: &AiRuntimeState,
    input: SetAiTaskDefaultInput,
) -> Result<AiTaskDefaultsDto, CommandError> {
    validate_operation_id(&input.operation_id)?;
    if let Some(profile_id) = &input.profile_id {
        validate_entity_id(profile_id, "AI provider profile")?;
    }
    let _guard = lock_profile_mutations(runtime)?;
    database.with_connection(|connection| defaults::set_task_default(connection, input))?;
    database.with_connection(defaults::load_task_defaults)
}

pub(crate) fn load_profile_request(
    database: &Database,
    profile_id: &str,
) -> Result<ProfileRequestSnapshot, CommandError> {
    validate_entity_id(profile_id, "AI provider profile")?;
    database.with_connection(|connection| {
        let profile = repository::load_profile(connection, profile_id)?;
        Ok(ProfileRequestSnapshot {
            profile: (&profile).into(),
            credential_storage: profile.credential_storage,
        })
    })
}

pub(crate) fn load_default_profile_request(
    database: &Database,
) -> Result<ProfileRequestSnapshot, CommandError> {
    database.with_connection(defaults::load_default_profile_request)
}

fn load_detail(
    database: &Database,
    runtime: &AiRuntimeState,
    profile_id: &str,
) -> Result<AiProviderProfileDetailDto, CommandError> {
    let profile = database.with_connection(|connection| repository::load_profile(connection, profile_id))?;
    build_detail(&profile, runtime)
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
