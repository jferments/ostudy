use crate::{
    ai::{
        catalog::{AiTaskKind, ProviderKind},
        profiles::{contracts::{SetAiProviderCredentialInput, SetAiTaskDefaultInput}, service},
        secrets::CredentialPersistence,
        AiRuntimeState,
    },
    database::Database,
};

use super::default_input;

fn runtime() -> AiRuntimeState {
    AiRuntimeState::new(crate::platform::unavailable_secret_store())
        .expect("runtime should initialize")
}

#[test]
fn create_service_replays_without_duplicate_profiles() {
    let database = Database::open_in_memory().expect("database should open");
    let runtime = runtime();
    let mut first = default_input(ProviderKind::LlamaCpp);
    first.operation_id = "55555555555555555555555555555555".to_string();
    first.display_name = "Replay service".to_string();
    first.model_id = "model".to_string();
    let created = service::create_profile(&database, &runtime, first).unwrap();

    let mut retry = default_input(ProviderKind::LlamaCpp);
    retry.operation_id = "55555555555555555555555555555555".to_string();
    retry.display_name = "Ignored retry payload".to_string();
    let replayed = service::create_profile(&database, &runtime, retry).unwrap();
    assert_eq!(created.id, replayed.id);
    assert_eq!(service::list_profiles(&database, &runtime).unwrap().len(), 1);
}

#[test]
fn session_credential_updates_epoch_without_exposing_value() {
    let database = Database::open_in_memory().expect("database should open");
    let runtime = runtime();
    let mut input = default_input(ProviderKind::Openrouter);
    input.operation_id = "66666666666666666666666666666666".to_string();
    input.display_name = "OpenRouter".to_string();
    input.model_id = "openai/example".to_string();
    let created = service::create_profile(&database, &runtime, input).unwrap();
    let saved = service::set_credential(
        &database,
        &runtime,
        SetAiProviderCredentialInput {
            operation_id: "77777777777777777777777777777777".to_string(),
            profile_id: created.id.clone(),
            revision: created.revision,
            credential: "credential-sentinel".to_string(),
            persistence: CredentialPersistence::Session,
        },
    )
    .unwrap();
    assert_eq!(saved.credential_epoch, 1);
    assert_eq!(saved.revision, 2);
    let serialized = serde_json::to_string(&saved).unwrap();
    assert!(!serialized.contains("credential-sentinel"));
}

#[test]
fn task_default_assignment_is_idempotent() {
    let database = Database::open_in_memory().expect("database should open");
    let runtime = runtime();
    let mut input = default_input(ProviderKind::LlamaCpp);
    input.operation_id = "88888888888888888888888888888888".to_string();
    input.display_name = "Default".to_string();
    input.model_id = "model".to_string();
    let profile = service::create_profile(&database, &runtime, input).unwrap();
    let operation_id = "99999999999999999999999999999999".to_string();
    let defaults = service::set_task_default(
        &database,
        &runtime,
        SetAiTaskDefaultInput {
            operation_id: operation_id.clone(),
            task_kind: AiTaskKind::FlashcardGeneration,
            profile_id: Some(profile.id.clone()),
        },
    )
    .unwrap();
    assert_eq!(
        defaults.flashcard_generation_profile_id,
        Some(profile.id.clone())
    );
    let replayed = service::set_task_default(
        &database,
        &runtime,
        SetAiTaskDefaultInput {
            operation_id,
            task_kind: AiTaskKind::FlashcardGeneration,
            profile_id: Some(profile.id),
        },
    )
    .unwrap();
    assert_eq!(
        replayed.flashcard_generation_profile_id,
        defaults.flashcard_generation_profile_id
    );
}
