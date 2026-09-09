use crate::{
    ai::{
        catalog::{AiTaskKind, ProviderKind},
        profiles::{
            contracts::{SetAiProviderCredentialInput, SetAiTaskDefaultInput},
            service,
        },
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
    assert_eq!(
        service::list_profiles(&database, &runtime).unwrap().len(),
        1
    );
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
fn one_provider_connection_can_back_three_independent_task_models() {
    let database = Database::open_in_memory().expect("database should open");
    let runtime = runtime();
    let mut input = default_input(ProviderKind::Openrouter);
    input.operation_id = "88888888888888888888888888888888".to_string();
    input.display_name = "OpenRouter connection".to_string();
    input.model_id = "legacy/model".to_string();
    let profile = service::create_profile(&database, &runtime, input).unwrap();

    let defaults = service::set_task_default(
        &database,
        &runtime,
        SetAiTaskDefaultInput {
            operation_id: "99999999999999999999999999999991".to_string(),
            task_kind: AiTaskKind::FlashcardGeneration,
            profile_id: Some(profile.id.clone()),
            model_id: Some("deepseek/deepseek-v4-flash-0731".to_string()),
            reasoning_effort: Some("high".to_string()),
        },
    )
    .unwrap();
    assert_eq!(
        defaults.flashcard_generation_profile_id,
        Some(profile.id.clone())
    );
    assert_eq!(
        defaults.flashcard_generation_model_id.as_deref(),
        Some("deepseek/deepseek-v4-flash-0731")
    );

    let defaults = service::set_task_default(
        &database,
        &runtime,
        SetAiTaskDefaultInput {
            operation_id: "99999999999999999999999999999992".to_string(),
            task_kind: AiTaskKind::Tutor,
            profile_id: Some(profile.id.clone()),
            model_id: Some("anthropic/claude-sonnet".to_string()),
            reasoning_effort: Some("medium".to_string()),
        },
    )
    .unwrap();
    let defaults = service::set_task_default(
        &database,
        &runtime,
        SetAiTaskDefaultInput {
            operation_id: "99999999999999999999999999999993".to_string(),
            task_kind: AiTaskKind::Embeddings,
            profile_id: Some(profile.id.clone()),
            model_id: Some("qwen/qwen3-embedding-0.6b".to_string()),
            reasoning_effort: Some("none".to_string()),
        },
    )
    .unwrap();

    assert_eq!(defaults.tutor_profile_id, Some(profile.id.clone()));
    assert_eq!(
        defaults.tutor_model_id.as_deref(),
        Some("anthropic/claude-sonnet")
    );
    assert_eq!(defaults.embeddings_profile_id, Some(profile.id.clone()));
    assert_eq!(
        defaults.embeddings_model_id.as_deref(),
        Some("qwen/qwen3-embedding-0.6b")
    );
    assert_eq!(
        service::list_profiles(&database, &runtime).unwrap().len(),
        1,
        "task model assignments must not duplicate the provider/API-key connection"
    );

    let replay_operation = "99999999999999999999999999999994".to_string();
    let first = service::set_task_default(
        &database,
        &runtime,
        SetAiTaskDefaultInput {
            operation_id: replay_operation.clone(),
            task_kind: AiTaskKind::Tutor,
            profile_id: Some(profile.id.clone()),
            model_id: Some("openai/gpt-5.1".to_string()),
            reasoning_effort: Some("automatic".to_string()),
        },
    )
    .unwrap();
    let replayed = service::set_task_default(
        &database,
        &runtime,
        SetAiTaskDefaultInput {
            operation_id: replay_operation,
            task_kind: AiTaskKind::Tutor,
            profile_id: Some(profile.id),
            model_id: Some("openai/gpt-5.1".to_string()),
            reasoning_effort: Some("automatic".to_string()),
        },
    )
    .unwrap();
    assert_eq!(replayed.tutor_model_id, first.tutor_model_id);
}
