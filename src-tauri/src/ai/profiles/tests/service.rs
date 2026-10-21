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

    let defaults = service::get_task_defaults(&database).unwrap();
    assert_eq!(
        defaults.flashcard_generation_model_id.as_deref(),
        Some("deepseek/deepseek-v4.1-flash-1017")
    );
    assert_eq!(
        defaults.flashcard_generation_reasoning_effort.as_deref(),
        Some("none")
    );
    assert_eq!(
        defaults.tutor_model_id.as_deref(),
        Some("deepseek/deepseek-v4.1-flash-1017")
    );
    assert_eq!(defaults.tutor_reasoning_effort.as_deref(), Some("low"));
    assert_eq!(
        defaults.embeddings_model_id.as_deref(),
        Some("qwen/qwen3-embedding-8b")
    );
    assert_eq!(
        defaults.embeddings_reasoning_effort.as_deref(),
        Some("none")
    );
}

#[test]
fn first_openrouter_key_seeds_only_unassigned_defaults_and_later_keys_do_not_replace_them() {
    let database = Database::open_in_memory().expect("database should open");
    let runtime = runtime();

    let mut local_input = default_input(ProviderKind::LlamaCpp);
    local_input.operation_id = "12121212121212121212121212121212".to_string();
    local_input.display_name = "Local generation".to_string();
    local_input.model_id = "local/model".to_string();
    let local = service::create_profile(&database, &runtime, local_input).unwrap();
    service::set_task_default(
        &database,
        &runtime,
        SetAiTaskDefaultInput {
            operation_id: "13131313131313131313131313131313".to_string(),
            task_kind: AiTaskKind::FlashcardGeneration,
            profile_id: Some(local.id.clone()),
            model_id: Some("local/model".to_string()),
            reasoning_effort: None,
        },
    )
    .unwrap();

    let mut first_input = default_input(ProviderKind::Openrouter);
    first_input.operation_id = "14141414141414141414141414141414".to_string();
    first_input.display_name = "OpenRouter first".to_string();
    first_input.model_id = String::new();
    let first = service::create_profile(&database, &runtime, first_input).unwrap();
    service::set_credential(
        &database,
        &runtime,
        SetAiProviderCredentialInput {
            operation_id: "15151515151515151515151515151515".to_string(),
            profile_id: first.id.clone(),
            revision: first.revision,
            credential: "first-openrouter-key".to_string(),
            persistence: CredentialPersistence::Session,
        },
    )
    .unwrap();

    let seeded = service::get_task_defaults(&database).unwrap();
    assert_eq!(
        seeded.flashcard_generation_profile_id.as_deref(),
        Some(local.id.as_str()),
        "a deliberate existing generation assignment must not be replaced"
    );
    assert_eq!(
        seeded.flashcard_generation_model_id.as_deref(),
        Some("local/model")
    );
    assert_eq!(
        seeded.flashcard_generation_reasoning_effort.as_deref(),
        Some("none"),
        "new generation assignments default to reasoning off"
    );
    assert_eq!(seeded.tutor_profile_id.as_deref(), Some(first.id.as_str()));
    assert_eq!(
        seeded.tutor_model_id.as_deref(),
        Some("deepseek/deepseek-v4.1-flash-1017")
    );
    assert_eq!(seeded.tutor_reasoning_effort.as_deref(), Some("low"));
    assert_eq!(
        seeded.embeddings_model_id.as_deref(),
        Some("qwen/qwen3-embedding-8b")
    );

    let mut second_input = default_input(ProviderKind::Openrouter);
    second_input.operation_id = "16161616161616161616161616161616".to_string();
    second_input.display_name = "OpenRouter second".to_string();
    second_input.model_id = String::new();
    let second = service::create_profile(&database, &runtime, second_input).unwrap();
    service::set_credential(
        &database,
        &runtime,
        SetAiProviderCredentialInput {
            operation_id: "18181818181818181818181818181818".to_string(),
            profile_id: second.id,
            revision: second.revision,
            credential: "second-openrouter-key".to_string(),
            persistence: CredentialPersistence::Session,
        },
    )
    .unwrap();

    let after_second = service::get_task_defaults(&database).unwrap();
    assert_eq!(after_second.tutor_profile_id, seeded.tutor_profile_id);
    assert_eq!(after_second.tutor_model_id, seeded.tutor_model_id);
    assert_eq!(
        after_second.embeddings_profile_id,
        seeded.embeddings_profile_id
    );
    assert_eq!(after_second.embeddings_model_id, seeded.embeddings_model_id);
}

#[test]
fn first_text_only_key_seeds_writing_and_tutor_but_leaves_search_unassigned() {
    let database = Database::open_in_memory().expect("database should open");
    let runtime = runtime();
    for (kind, operation, credential_operation, model) in [
        (
            ProviderKind::Claude,
            "21212121212121212121212121212121",
            "22222222222222222222222222222223",
            "claude-sonnet-5-5",
        ),
        (
            ProviderKind::Deepseek,
            "23232323232323232323232323232323",
            "24242424242424242424242424242424",
            "deepseek-v4-flash",
        ),
    ] {
        let mut input = default_input(kind);
        input.operation_id = operation.to_string();
        let created = service::create_profile(&database, &runtime, input).unwrap();
        service::set_credential(
            &database,
            &runtime,
            SetAiProviderCredentialInput {
                operation_id: credential_operation.to_string(),
                profile_id: created.id.clone(),
                revision: created.revision,
                credential: "text-only-key".to_string(),
                persistence: CredentialPersistence::Session,
            },
        )
        .unwrap();
        let defaults = service::get_task_defaults(&database).unwrap();
        if kind == ProviderKind::Claude {
            assert_eq!(
                defaults.flashcard_generation_profile_id.as_deref(),
                Some(created.id.as_str())
            );
            assert_eq!(
                defaults.flashcard_generation_model_id.as_deref(),
                Some(model)
            );
            assert_eq!(defaults.tutor_model_id.as_deref(), Some(model));
            assert_eq!(defaults.tutor_reasoning_effort.as_deref(), Some("low"));
        } else {
            assert_ne!(
                defaults.tutor_profile_id.as_deref(),
                Some(created.id.as_str()),
                "a later service must not replace the first service's assignments"
            );
        }
        assert_eq!(defaults.embeddings_profile_id, None);
        assert_eq!(defaults.embeddings_model_id, None);
    }
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
    assert_eq!(defaults.tutor_profile_id, Some(profile.id.clone()));
    assert_eq!(
        defaults.tutor_model_id.as_deref(),
        Some("anthropic/claude-sonnet")
    );

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
