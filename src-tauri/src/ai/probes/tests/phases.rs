use super::{
    super::{
        super::super::{catalog::ProviderKind, providers::ProviderModel},
        contracts::ProbePhaseStatus,
        phases::{classify_model_match, passed_phase, skipped_phase, ModelMatch},
    },
    profile,
};

#[test]
fn sole_llama_model_mismatch_uses_the_server_id() {
    let profile = profile(ProviderKind::LlamaCpp, "unsloth/repository-name");
    let models = vec![ProviderModel {
        id: "/models/Qwen3.6-27B-Q4_K_M.gguf".to_string(),
        display_name: "Qwen3.6".to_string(),
        owned_by: Some("llamacpp".to_string()),
        context_window_tokens: None,
        reasoning: None,
    }];
    assert_eq!(
        classify_model_match(&profile, &models),
        ModelMatch::SoleLlamaServerId("/models/Qwen3.6-27B-Q4_K_M.gguf".to_string())
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
    }];
    assert_eq!(classify_model_match(&profile, &models), ModelMatch::Missing);
}

#[test]
fn phase_builders_preserve_terminal_labels() {
    let passed = passed_phase("text", "Text", "Passed.", Some(4));
    assert_eq!(passed.status, ProbePhaseStatus::Passed);
    assert_eq!(passed.latency_ms, Some(4));
    let skipped = skipped_phase("optional", "Optional", "Not required.");
    assert_eq!(skipped.status, ProbePhaseStatus::Skipped);
    assert_eq!(skipped.latency_ms, None);
}
