use super::super::super::{
    catalog::{AuthMode, ProviderKind},
    profiles::ProfileSnapshot,
    providers::{ProviderModel, ProviderModelReasoning},
};
use super::super::runner::{
    classify_model_match_for_test, document_reasoning_effort_for_test,
    probe_reasoning_effort_for_test,
};

fn profile(provider_kind: ProviderKind, model_id: &str) -> ProfileSnapshot {
    ProfileSnapshot {
        id: "0123456789abcdef0123456789abcdef".to_string(),
        provider_kind,
        base_url: match provider_kind {
            ProviderKind::LlamaCpp => "http://127.0.0.1:8080/".to_string(),
            ProviderKind::Openrouter => "https://openrouter.ai/api/v1/".to_string(),
            _ => "https://api.deepseek.com/".to_string(),
        },
        model_id: model_id.to_string(),
        reasoning_effort: "low".to_string(),
        auth_mode: if matches!(
            provider_kind,
            ProviderKind::Deepseek | ProviderKind::Openrouter
        ) {
            AuthMode::Bearer
        } else {
            AuthMode::None
        },
        enabled: true,
        allow_insecure_http: false,
        require_zero_data_retention: false,
        request_timeout_ms: 60_000,
        credential_epoch: 2,
        revision: 3,
    }
}

fn model(id: &str, reasoning: Option<ProviderModelReasoning>) -> ProviderModel {
    ProviderModel {
        id: id.to_string(),
        display_name: id.to_string(),
        owned_by: None,
        context_window_tokens: None,
        reasoning,
        image_input: None,
    }
}

#[test]
fn sole_llama_model_mismatch_uses_server_reported_id_for_probe() {
    let profile = profile(ProviderKind::LlamaCpp, "unsloth/repository-name");
    let models = vec![ProviderModel {
        id: "/models/Qwen3.6-27B-Q4_K_M.gguf".to_string(),
        display_name: "Qwen3.6".to_string(),
        owned_by: Some("llamacpp".to_string()),
        context_window_tokens: None,
        reasoning: None,
        image_input: None,
    }];
    assert_eq!(
        classify_model_match_for_test(&profile, &models),
        "sole:/models/Qwen3.6-27B-Q4_K_M.gguf"
    );
}

#[test]
fn cloud_model_mismatch_remains_a_hard_failure() {
    let profile = profile(ProviderKind::Deepseek, "deepseek-v4-flash");
    let models = vec![ProviderModel {
        id: "deepseek-v4-pro".to_string(),
        display_name: "deepseek-v4-pro".to_string(),
        owned_by: Some("deepseek".to_string()),
        context_window_tokens: None,
        reasoning: None,
        image_input: None,
    }];
    assert_eq!(classify_model_match_for_test(&profile, &models), "missing");
}

#[test]
fn exact_model_match_remains_exact() {
    let profile = profile(ProviderKind::Openrouter, "openai/example");
    let models = vec![ProviderModel {
        id: "openai/example".to_string(),
        display_name: "Example".to_string(),
        owned_by: Some("openai".to_string()),
        context_window_tokens: Some(128_000),
        reasoning: None,
        image_input: None,
    }];
    assert_eq!(classify_model_match_for_test(&profile, &models), "exact");
}

#[test]
fn openrouter_probe_uses_the_same_effective_reasoning_as_document_generation() {
    let optional = profile(ProviderKind::Openrouter, "deepseek/deepseek-v4-flash");
    let optional_models = vec![model(
        "deepseek/deepseek-v4-flash",
        Some(ProviderModelReasoning {
            supported_efforts: Some(vec!["low".to_string(), "high".to_string()]),
            default_effort: Some("high".to_string()),
            default_enabled: Some(true),
            supports_max_tokens: true,
            mandatory: false,
        }),
    )];
    assert_eq!(
        probe_reasoning_effort_for_test(&optional, Some(&optional_models)),
        "low"
    );

    let mandatory = profile(ProviderKind::Openrouter, "reasoning/mandatory");
    let mandatory_models = vec![model(
        "reasoning/mandatory",
        Some(ProviderModelReasoning {
            supported_efforts: Some(vec!["low".to_string(), "high".to_string()]),
            default_effort: Some("low".to_string()),
            default_enabled: Some(true),
            supports_max_tokens: true,
            mandatory: true,
        }),
    )];
    assert_eq!(
        probe_reasoning_effort_for_test(&mandatory, Some(&mandatory_models)),
        "low"
    );
}

#[test]
fn openrouter_probe_matches_runtime_none_when_catalog_has_no_reasoning_contract() {
    let profile = profile(ProviderKind::Openrouter, "openai/example");
    let models = vec![model("openai/example", None)];
    assert_eq!(
        probe_reasoning_effort_for_test(&profile, Some(&models)),
        "none"
    );
}

#[test]
fn openrouter_image_probe_reasons_as_document_work_does() {
    let mut optional = profile(ProviderKind::Openrouter, "reasoning/optional");
    optional.reasoning_effort = "high".to_string();
    let optional_models = vec![model(
        "reasoning/optional",
        Some(ProviderModelReasoning {
            supported_efforts: Some(vec!["low".to_string(), "high".to_string()]),
            default_effort: Some("high".to_string()),
            default_enabled: Some(true),
            supports_max_tokens: true,
            mandatory: false,
        }),
    )];
    assert_eq!(
        document_reasoning_effort_for_test(&optional, Some(&optional_models)),
        "none"
    );

    let mut mandatory = profile(ProviderKind::Openrouter, "reasoning/mandatory");
    mandatory.reasoning_effort = "high".to_string();
    let mandatory_models = vec![model(
        "reasoning/mandatory",
        Some(ProviderModelReasoning {
            supported_efforts: Some(vec!["low".to_string(), "high".to_string()]),
            default_effort: Some("high".to_string()),
            default_enabled: Some(true),
            supports_max_tokens: true,
            mandatory: true,
        }),
    )];
    assert_eq!(
        document_reasoning_effort_for_test(&mandatory, Some(&mandatory_models)),
        "low"
    );
}
