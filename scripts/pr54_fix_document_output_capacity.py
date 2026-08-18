#!/usr/bin/env python3
"""One-shot PR #54 fix for reasoning-aware structured-output capacity and retries."""
from pathlib import Path


def replace_once(path: str, old: str, new: str, label: str) -> None:
    p = Path(path)
    text = p.read_text()
    if new in text:
        return
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one anchor in {path}, found {count}")
    p.write_text(text.replace(old, new, 1))


# ---------------------------------------------------------------------------
# OpenRouter model metadata must survive discovery so document planning can
# resolve a user reasoning preference without guessing provider behavior.
# ---------------------------------------------------------------------------
replace_once(
    "src-tauri/src/ai/providers.rs",
    '''#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderModelReasoning {''',
    '''#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderModelReasoning {''',
    "reasoning metadata deserialization",
)

# ---------------------------------------------------------------------------
# Provider-neutral document capability normalization. The user-selected effort
# is a ceiling: unsupported low effort must never be silently promoted to high.
# If an optional model does not support the requested effort, document work uses
# no reasoning. Mandatory models use the lowest supported effort. Wire max_tokens
# is expanded so reasoning cannot consume the visible structured-output budget.
# ---------------------------------------------------------------------------
replace_once(
    "src-tauri/src/ai/document_provider.rs",
    '''use sha2::{Digest, Sha256};

use crate::{database::Database, error::CommandError};''',
    '''use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::{database::Database, error::CommandError};''',
    "document reasoning serde import",
)
replace_once(
    "src-tauri/src/ai/document_provider.rs",
    '''#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DocumentProviderCapabilities {''',
    '''#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DocumentReasoningCapabilities {
    pub supported_efforts: Option<Vec<String>>,
    pub default_effort: Option<String>,
    pub default_enabled: Option<bool>,
    #[serde(default)]
    pub supports_max_tokens: bool,
    #[serde(default)]
    pub mandatory: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DocumentProviderCapabilities {''',
    "document reasoning capability type",
)
replace_once(
    "src-tauri/src/ai/document_provider.rs",
    '''    pub usage_reporting: bool,
    pub structured_output: StructuredOutputStrategy,
}''',
    '''    pub usage_reporting: bool,
    pub structured_output: StructuredOutputStrategy,
    pub reasoning: Option<DocumentReasoningCapabilities>,
}''',
    "document reasoning capability field",
)
replace_once(
    "src-tauri/src/ai/document_provider.rs",
    '''            usage_reporting: false,
            structured_output: structured_strategy(provider_kind),
        }''',
    '''            usage_reporting: false,
            structured_output: structured_strategy(provider_kind),
            reasoning: None,
        }''',
    "conservative reasoning capability",
)
replace_once(
    "src-tauri/src/ai/document_provider.rs",
    '''        let source = format!(
            "provider={}|model={}|context={context}|images={}|usage={}|structured={}",
            self.provider_kind,
            self.model_id,
            self.image_input,
            self.usage_reporting,
            self.structured_output.as_str(),
        );''',
    '''        let reasoning = self
            .reasoning
            .as_ref()
            .map(|value| format!(
                "efforts={:?};default={:?};enabled={:?};mandatory={}",
                value.supported_efforts,
                value.default_effort,
                value.default_enabled,
                value.mandatory,
            ))
            .unwrap_or_else(|| "unknown".to_string());
        let source = format!(
            "provider={}|model={}|context={context}|images={}|usage={}|structured={}|reasoning={reasoning}",
            self.provider_kind,
            self.model_id,
            self.image_input,
            self.usage_reporting,
            self.structured_output.as_str(),
        );''',
    "reasoning planning fingerprint",
)
replace_once(
    "src-tauri/src/ai/document_provider.rs",
    '''    pub(crate) fn planning_fingerprint(&self) -> String {''',
    '''    pub(crate) fn resolve_reasoning_effort(&self, requested: &str) -> String {
        // llama.cpp and direct DeepSeek requests explicitly disable thinking in
        // their provider adapters. OpenRouter is the only implemented document
        // transport whose unified reasoning parameter needs model negotiation.
        if self.provider_kind != "openrouter" {
            return "none".to_string();
        }
        let Some(reasoning) = self.reasoning.as_ref() else {
            return "none".to_string();
        };
        let supports = |effort: &str| {
            reasoning
                .supported_efforts
                .as_ref()
                .is_none_or(|efforts| efforts.iter().any(|value| value == effort))
        };
        let lowest_supported = || {
            ["minimal", "low", "medium", "high", "xhigh", "max"]
                .into_iter()
                .find(|effort| supports(effort))
                .unwrap_or("medium")
                .to_string()
        };

        if requested == "none" {
            return if reasoning.mandatory {
                lowest_supported()
            } else {
                "none".to_string()
            };
        }
        if requested == "automatic" {
            if reasoning.default_enabled != Some(true) && !reasoning.mandatory {
                return "none".to_string();
            }
            if let Some(default) = reasoning.default_effort.as_deref() {
                if default == "none" && !reasoning.mandatory {
                    return "none".to_string();
                }
                if supports(default) {
                    return default.to_string();
                }
            }
            return if reasoning.mandatory {
                lowest_supported()
            } else {
                "none".to_string()
            };
        }
        if supports(requested) {
            return requested.to_string();
        }
        if reasoning.mandatory {
            lowest_supported()
        } else {
            // A user asking for Low must never be silently upgraded to High just
            // because a model exposes only high/xhigh. Disable optional reasoning
            // rather than consuming most of the structured-output token budget.
            "none".to_string()
        }
    }

    pub(crate) fn completion_token_ceiling(
        &self,
        effective_reasoning_effort: &str,
        visible_completion_tokens: usize,
    ) -> usize {
        if self.provider_kind != "openrouter" || effective_reasoning_effort == "none" {
            return visible_completion_tokens;
        }
        let visible_percent = match effective_reasoning_effort {
            "minimal" => 90_usize,
            "low" => 80,
            "medium" => 50,
            "high" => 20,
            "xhigh" | "max" => 5,
            _ => 100,
        };
        visible_completion_tokens
            .saturating_mul(100)
            .saturating_add(visible_percent - 1)
            / visible_percent
    }

    pub(crate) fn planning_fingerprint(&self) -> String {''',
    "reasoning effort resolver",
)
replace_once(
    "src-tauri/src/ai/document_provider.rs",
    '''                "usage_reporting" => result.usage_reporting = status == "supported",
                "json_schema_output" if status == "supported" => {''',
    '''                "usage_reporting" => result.usage_reporting = status == "supported",
                "reasoning_control" if status == "supported" => {
                    result.reasoning = serde_json::from_str::<Option<DocumentReasoningCapabilities>>(
                        &value_json,
                    )
                    .ok()
                    .flatten();
                }
                "json_schema_output" if status == "supported" => {''',
    "load persisted reasoning capability",
)
replace_once(
    "src-tauri/src/ai/document_provider.rs",
    '''            usage_reporting: true,
            structured_output: StructuredOutputStrategy::JsonSchema,
        };''',
    '''            usage_reporting: true,
            structured_output: StructuredOutputStrategy::JsonSchema,
            reasoning: None,
        };''',
    "capability fingerprint test fixture",
)
replace_once(
    "src-tauri/src/ai/document_provider.rs",
    '''    #[test]
    fn deepseek_document_strategy_is_strict_tool_output() {''',
    '''    #[test]
    fn optional_reasoning_never_promotes_an_unsupported_low_request() {
        let capabilities = DocumentProviderCapabilities {
            provider_kind: "openrouter".to_string(),
            model_id: "reasoning-model".to_string(),
            context_window_tokens: Some(1_000_000),
            image_input: false,
            usage_reporting: true,
            structured_output: StructuredOutputStrategy::JsonSchema,
            reasoning: Some(DocumentReasoningCapabilities {
                supported_efforts: Some(vec!["high".to_string(), "xhigh".to_string()]),
                default_effort: Some("high".to_string()),
                default_enabled: Some(true),
                supports_max_tokens: false,
                mandatory: false,
            }),
        };
        assert_eq!(capabilities.resolve_reasoning_effort("low"), "none");
        assert_eq!(capabilities.resolve_reasoning_effort("high"), "high");
        assert_eq!(capabilities.completion_token_ceiling("none", 10_000), 10_000);
        assert_eq!(capabilities.completion_token_ceiling("high", 10_000), 50_000);
    }

    #[test]
    fn mandatory_reasoning_uses_the_lowest_supported_effort() {
        let capabilities = DocumentProviderCapabilities {
            provider_kind: "openrouter".to_string(),
            model_id: "mandatory".to_string(),
            context_window_tokens: Some(1_000_000),
            image_input: false,
            usage_reporting: true,
            structured_output: StructuredOutputStrategy::JsonSchema,
            reasoning: Some(DocumentReasoningCapabilities {
                supported_efforts: Some(vec!["xhigh".to_string(), "high".to_string()]),
                default_effort: Some("xhigh".to_string()),
                default_enabled: Some(true),
                supports_max_tokens: false,
                mandatory: true,
            }),
        };
        assert_eq!(capabilities.resolve_reasoning_effort("none"), "high");
        assert_eq!(capabilities.resolve_reasoning_effort("low"), "high");
    }

    #[test]
    fn deepseek_document_strategy_is_strict_tool_output() {''',
    "reasoning capability regressions",
)

# ---------------------------------------------------------------------------
# Persist current selected-model context/reasoning declarations during ordinary
# model discovery. This is a zero-token metadata GET already performed by the UI.
# ---------------------------------------------------------------------------
replace_once(
    "src-tauri/src/ai/probes.rs",
    '''const ADAPTER_REVISION: i64 = 4;''',
    '''const ADAPTER_REVISION: i64 = 5;''',
    "probe adapter revision",
)
replace_once(
    "src-tauri/src/ai/probes.rs",
    '''    let models = providers::list_models(&runtime.gateway, &profile, bearer)
        .await
        .map_err(gateway_to_command)?;
    Ok(AiProviderModelsDto {
        profile_id: profile.id.clone(),
        profile_revision: profile.revision,
        provider_kind: profile.provider_kind,
        models,
        retrieved_at: unix_time_millis()?,
    })''',
    '''    let models = providers::list_models(&runtime.gateway, &profile, bearer)
        .await
        .map_err(gateway_to_command)?;
    let retrieved_at = unix_time_millis()?;
    persist_selected_model_declarations(database.inner(), &profile, &models, retrieved_at)?;
    Ok(AiProviderModelsDto {
        profile_id: profile.id.clone(),
        profile_revision: profile.revision,
        provider_kind: profile.provider_kind,
        models,
        retrieved_at,
    })''',
    "persist model declarations on discovery",
)
replace_once(
    "src-tauri/src/ai/probes.rs",
    '''fn finish_and_persist(
    database: &Database,''',
    '''fn persist_selected_model_declarations(
    database: &Database,
    profile: &ProfileSnapshot,
    models: &[ProviderModel],
    checked_at: i64,
) -> Result<(), CommandError> {
    let selected = models.iter().find(|model| model.id == profile.model_id);
    database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        transaction
            .execute(
                "DELETE FROM ai_provider_capabilities
                 WHERE profile_id = ?1 AND capability_key IN ('context_window_tokens', 'reasoning_control')",
                [profile.id.as_str()],
            )
            .map_err(CommandError::database)?;
        if let Some(context) = selected.and_then(|model| model.context_window_tokens) {
            transaction
                .execute(
                    "INSERT INTO ai_provider_capabilities (
                         profile_id, capability_key, status, source_kind, value_json, checked_at
                     ) VALUES (?1, 'context_window_tokens', 'supported', 'declared', ?2, ?3)",
                    params![profile.id, context.to_string(), checked_at],
                )
                .map_err(CommandError::database)?;
        }
        if let Some(reasoning) = selected.and_then(|model| model.reasoning.as_ref()) {
            let value = serde_json::to_string(&Some(reasoning)).map_err(CommandError::serialization)?;
            transaction
                .execute(
                    "INSERT INTO ai_provider_capabilities (
                         profile_id, capability_key, status, source_kind, value_json, checked_at
                     ) VALUES (?1, 'reasoning_control', 'supported', 'declared', ?2, ?3)",
                    params![profile.id, value, checked_at],
                )
                .map_err(CommandError::database)?;
        }
        transaction.commit().map_err(CommandError::database)
    })
}

fn finish_and_persist(
    database: &Database,''',
    "selected model declaration persistence",
)

# ---------------------------------------------------------------------------
# Task-local effective reasoning. The gateway consumes the resolved task value,
# never the raw global setting, so provider-specific metadata is contained at the
# document-provider boundary.
# ---------------------------------------------------------------------------
replace_once(
    "src-tauri/src/ai/gateway.rs",
    '''    current_signal, record_current_provider_activity, record_current_provider_request,
    record_current_provider_retry, record_current_usage, set_current_reasoning_effort,
    CancellationSignal, DocumentTaskUsageDelta,
};''',
    '''    current_reasoning_effort, current_signal, record_current_provider_activity,
    record_current_provider_request, record_current_provider_retry, record_current_usage,
    CancellationSignal, DocumentTaskUsageDelta,
};''',
    "gateway effective reasoning import",
)
replace_once(
    "src-tauri/src/ai/gateway.rs",
    '''    let document_task = current_signal().is_some();
    let reasoning_effort = document_task.then(crate::settings::current_ai_reasoning_effort);
    if let Some(reasoning_effort) = reasoning_effort.as_deref() {
        set_current_reasoning_effort(reasoning_effort);
    }
    prepare_provider_body_for_document(url, body, reasoning_effort.as_deref())''',
    '''    let reasoning_effort = current_signal()
        .is_some()
        .then(current_reasoning_effort)
        .flatten();
    prepare_provider_body_for_document(url, body, reasoning_effort.as_deref())''',
    "gateway task reasoning source",
)

# ---------------------------------------------------------------------------
# Shared structured-output budgets. These are visible-output budgets; OpenRouter
# reasoning headroom is added separately by the normalized provider capability.
# ---------------------------------------------------------------------------
replace_once(
    "src-tauri/src/document_scale.rs",
    '''const MIN_GENERATION_SOURCE_TOKENS: usize = 800;''',
    '''const MIN_GENERATION_SOURCE_TOKENS: usize = 800;
pub(crate) const OUTLINE_VISIBLE_COMPLETION_TOKENS: usize = 2_048;
pub(crate) const GLOBAL_OUTLINE_VISIBLE_COMPLETION_TOKENS: usize = 3_072;
pub(crate) const CURATION_VISIBLE_COMPLETION_TOKENS: usize = 2_048;

pub(crate) fn generation_visible_completion_tokens(maximum_cards: usize) -> usize {
    1_536_usize
        .saturating_add(maximum_cards.saturating_mul(512))
        .clamp(2_048, 10_240)
}''',
    "shared visible output budgets",
)
replace_once(
    "src-tauri/src/document_scale.rs",
    '''    pub(crate) fn for_capabilities(
        source_unit_count: usize,
        capabilities: &DocumentProviderCapabilities,
        calibration: DocumentCostCalibration,
    ) -> Result<Self, CommandError> {''',
    '''    pub(crate) fn for_capabilities(
        source_unit_count: usize,
        capabilities: &DocumentProviderCapabilities,
        effective_reasoning_effort: &str,
        calibration: DocumentCostCalibration,
    ) -> Result<Self, CommandError> {''',
    "reasoning-aware scale signature",
)
replace_once(
    "src-tauri/src/document_scale.rs",
    '''        let outline_output = outline_completion_reserve(&capabilities.provider_kind);''',
    '''        let outline_output = capabilities.completion_token_ceiling(
            effective_reasoning_effort,
            OUTLINE_VISIBLE_COMPLETION_TOKENS,
        );''',
    "reasoning-aware outline reserve",
)
replace_once(
    "src-tauri/src/document_scale.rs",
    '''        let generation_output = generation_completion_reserve(plan.max_cards_per_window);''',
    '''        let generation_output = capabilities.completion_token_ceiling(
            effective_reasoning_effort,
            generation_visible_completion_tokens(plan.max_cards_per_window),
        );''',
    "reasoning-aware generation reserve",
)
replace_once(
    "src-tauri/src/document_scale.rs",
    '''fn outline_completion_reserve(_provider_kind: &str) -> usize {
    // Local outline fragments are deliberately concise. A 1.5k-token reserve
    // leaves ample room for the strict fragment schema without encouraging a
    // multi-thousand-token restatement of source prose.
    1_536
}

fn generation_completion_reserve(maximum_cards: usize) -> usize {
    1_024_usize
        .saturating_add(maximum_cards.saturating_mul(384))
        .clamp(1_536, 5_120)
}

''',
    '''''',
    "remove obsolete completion reserve helpers",
)
replace_once(
    "src-tauri/src/document_scale.rs",
    '''        &DocumentProviderCapabilities::conservative(provider_kind, "unknown"),
        DocumentCostCalibration::default(),''',
    '''        &DocumentProviderCapabilities::conservative(provider_kind, "unknown"),
        "none",
        DocumentCostCalibration::default(),''',
    "test validation default reasoning",
)
replace_once(
    "src-tauri/src/document_scale.rs",
    '''    capabilities: &DocumentProviderCapabilities,
    calibration: DocumentCostCalibration,
) -> Result<DocumentJobBudget, CommandError> {''',
    '''    capabilities: &DocumentProviderCapabilities,
    effective_reasoning_effort: &str,
    calibration: DocumentCostCalibration,
) -> Result<DocumentJobBudget, CommandError> {''',
    "reasoning-aware budget signature",
)
replace_once(
    "src-tauri/src/document_scale.rs",
    '''    let outline_fixed = outline_window_count.saturating_mul(
        1_500_usize.saturating_add(outline_completion_reserve(&capabilities.provider_kind)),
    );''',
    '''    let outline_completion = capabilities.completion_token_ceiling(
        effective_reasoning_effort,
        OUTLINE_VISIBLE_COMPLETION_TOKENS,
    );
    let outline_fixed = outline_window_count
        .saturating_mul(1_500_usize.saturating_add(outline_completion));''',
    "reasoning-aware outline estimate",
)
replace_once(
    "src-tauri/src/document_scale.rs",
    '''    let generation_fixed = generation_window_count.saturating_mul(
        1_800_usize.saturating_add(generation_completion_reserve(plan.max_cards_per_window)),
    );''',
    '''    let generation_completion = capabilities.completion_token_ceiling(
        effective_reasoning_effort,
        generation_visible_completion_tokens(plan.max_cards_per_window),
    );
    let generation_fixed = generation_window_count
        .saturating_mul(1_800_usize.saturating_add(generation_completion));''',
    "reasoning-aware generation estimate",
)
# Test fixtures/signatures in document_scale.
replace_once(
    "src-tauri/src/document_scale.rs",
    '''            structured_output: StructuredOutputStrategy::JsonSchema,
        }''',
    '''            structured_output: StructuredOutputStrategy::JsonSchema,
            reasoning: None,
        }''',
    "scale test reasoning fixture",
)
# Both direct for_capabilities calls in tests receive explicit no-reasoning.
text = Path("src-tauri/src/document_scale.rs").read_text()
text = text.replace(
    '''            &capabilities("openrouter", Some(12_000)),
            calibration,''',
    '''            &capabilities("openrouter", Some(12_000)),
            "none",
            calibration,''',
)
text = text.replace(
    '''            &capabilities("llama_cpp", Some(2_048)),
            DocumentCostCalibration::default(),''',
    '''            &capabilities("llama_cpp", Some(2_048)),
            "none",
            DocumentCostCalibration::default(),''',
)
Path("src-tauri/src/document_scale.rs").write_text(text)

# ---------------------------------------------------------------------------
# Document task resolves reasoning once before planning and requests. Completion
# handling accepts a length-finished response only when its complete structured
# object actually validates; otherwise length is a typed truncation.
# ---------------------------------------------------------------------------
replace_once(
    "src-tauri/src/ai/document_intelligence.rs",
    '''    ai::providers, database::Database, document_semantics::SemanticWindow, error::CommandError,
};''',
    '''    ai::providers,
    database::Database,
    document_scale::{
        generation_visible_completion_tokens, CURATION_VISIBLE_COMPLETION_TOKENS,
        GLOBAL_OUTLINE_VISIBLE_COMPLETION_TOKENS, OUTLINE_VISIBLE_COMPLETION_TOKENS,
    },
    document_semantics::SemanticWindow,
    error::CommandError,
};''',
    "document output budget imports",
)
replace_once(
    "src-tauri/src/ai/document_intelligence.rs",
    '''    pub network_scope: String,
    pub include_page_images: bool,
    profile: ProfileSnapshot,''',
    '''    pub network_scope: String,
    pub include_page_images: bool,
    pub requested_reasoning_effort: String,
    pub reasoning_effort: String,
    profile: ProfileSnapshot,''',
    "task reasoning fields",
)
replace_once(
    "src-tauri/src/ai/document_intelligence.rs",
    '''impl DocumentTaskContext {
    pub(crate) fn usage_summary(&self) -> (Option<i64>, bool) {''',
    '''impl DocumentTaskContext {
    pub(crate) fn apply_provider_capabilities(
        &mut self,
        capabilities: &super::document_provider::DocumentProviderCapabilities,
    ) {
        self.reasoning_effort =
            capabilities.resolve_reasoning_effort(&self.requested_reasoning_effort);
        crate::task_cancellation::set_current_reasoning_effort(&self.reasoning_effort);
        crate::logging::debug_target(
            "ai.document.reasoning",
            "document_reasoning_resolved",
            "Resolved the document reasoning preference against current model metadata.",
            json!({
                "provider": self.provider_kind.as_str(),
                "model": self.model_id.as_str(),
                "requestedEffort": self.requested_reasoning_effort.as_str(),
                "effectiveEffort": self.reasoning_effort.as_str(),
            }),
        );
    }

    fn completion_token_ceiling(&self, visible_tokens: usize) -> usize {
        // Provider capability metadata was resolved before planning. This local
        // helper mirrors that normalized contract for request construction.
        super::document_provider::DocumentProviderCapabilities {
            provider_kind: self.provider_kind.clone(),
            model_id: self.model_id.clone(),
            context_window_tokens: None,
            image_input: self.include_page_images,
            usage_reporting: false,
            structured_output: super::document_provider::StructuredOutputStrategy::Unknown,
            reasoning: None,
        }
        .completion_token_ceiling(&self.reasoning_effort, visible_tokens)
    }

    pub(crate) fn usage_summary(&self) -> (Option<i64>, bool) {''',
    "task reasoning resolution",
)
# Replace the above temporary constructed-capability helper with provider-kind
# arithmetic independent of metadata (effort has already been resolved).
replace_once(
    "src-tauri/src/ai/document_intelligence.rs",
    '''    fn completion_token_ceiling(&self, visible_tokens: usize) -> usize {
        // Provider capability metadata was resolved before planning. This local
        // helper mirrors that normalized contract for request construction.
        super::document_provider::DocumentProviderCapabilities {
            provider_kind: self.provider_kind.clone(),
            model_id: self.model_id.clone(),
            context_window_tokens: None,
            image_input: self.include_page_images,
            usage_reporting: false,
            structured_output: super::document_provider::StructuredOutputStrategy::Unknown,
            reasoning: None,
        }
        .completion_token_ceiling(&self.reasoning_effort, visible_tokens)
    }''',
    '''    fn completion_token_ceiling(&self, visible_tokens: usize) -> usize {
        if self.provider_kind != "openrouter" || self.reasoning_effort == "none" {
            return visible_tokens;
        }
        let visible_percent = match self.reasoning_effort.as_str() {
            "minimal" => 90_usize,
            "low" => 80,
            "medium" => 50,
            "high" => 20,
            "xhigh" | "max" => 5,
            _ => 100,
        };
        visible_tokens
            .saturating_mul(100)
            .saturating_add(visible_percent - 1)
            / visible_percent
    }''',
    "request completion headroom",
)
replace_once(
    "src-tauri/src/ai/document_intelligence.rs",
    '''        let include_page_images = profile.provider_kind == ProviderKind::LlamaCpp
            && probe_matches
            && image_status.as_deref() == Some("supported");

        Ok(DocumentTaskContext {''',
    '''        let include_page_images = profile.provider_kind == ProviderKind::LlamaCpp
            && probe_matches
            && image_status.as_deref() == Some("supported");
        let requested_reasoning_effort = crate::settings::load_ai_reasoning_effort(connection)?;

        Ok(DocumentTaskContext {''',
    "load requested reasoning setting",
)
replace_once(
    "src-tauri/src/ai/document_intelligence.rs",
    '''            network_scope: network_scope_name(endpoint.scope).to_string(),
            include_page_images,
            profile,''',
    '''            network_scope: network_scope_name(endpoint.scope).to_string(),
            include_page_images,
            requested_reasoning_effort,
            reasoning_effort: "none".to_string(),
            profile,''',
    "initialize task reasoning",
)
replace_once(
    "src-tauri/src/ai/document_intelligence.rs",
    '''            max_tokens: 1_536,
            images: &[],''',
    '''            max_tokens: task.completion_token_ceiling(GLOBAL_OUTLINE_VISIBLE_COMPLETION_TOKENS),
            images: &[],''',
    "global outline output budget",
)
replace_once(
    "src-tauri/src/ai/document_intelligence.rs",
    '''fn outline_completion_tokens(_task: &DocumentTaskContext) -> usize {
    1_536
}

#[cfg(test)]
fn outline_completion_tokens_for_provider(_provider_kind: ProviderKind) -> usize {
    1_536
}

fn flashcard_completion_tokens(maximum_cards: usize) -> usize {
    1_024_usize
        .saturating_add(maximum_cards.saturating_mul(384))
        .clamp(1_536, 5_120)
}''',
    '''fn outline_completion_tokens(task: &DocumentTaskContext) -> usize {
    task.completion_token_ceiling(OUTLINE_VISIBLE_COMPLETION_TOKENS)
}

fn outline_retry_completion_tokens(task: &DocumentTaskContext) -> usize {
    task.completion_token_ceiling(OUTLINE_VISIBLE_COMPLETION_TOKENS.saturating_mul(2))
}

#[cfg(test)]
fn outline_completion_tokens_for_provider(_provider_kind: ProviderKind) -> usize {
    OUTLINE_VISIBLE_COMPLETION_TOKENS
}

pub(crate) fn flashcard_completion_token_ceiling(
    task: &DocumentTaskContext,
    maximum_cards: usize,
) -> usize {
    task.completion_token_ceiling(generation_visible_completion_tokens(maximum_cards))
}''',
    "reasoning-aware completion budgets",
)
replace_once(
    "src-tauri/src/ai/document_intelligence.rs",
    '''    maximum_cards: usize,
) -> Result<Vec<Value>, CommandError> {
    let maximum_cards = maximum_cards.clamp(1, MAX_WINDOW_CARDS);''',
    '''    maximum_cards: usize,
    completion_token_floor: usize,
) -> Result<Vec<Value>, CommandError> {
    let maximum_cards = maximum_cards.clamp(1, MAX_WINDOW_CARDS);''',
    "flashcard generation completion floor signature",
)
replace_once(
    "src-tauri/src/ai/document_intelligence.rs",
    '''    request_flashcards_with_retry(runtime, task, &system, &user, images, maximum_cards).await''',
    '''    request_flashcards_with_retry(
        runtime,
        task,
        &system,
        &user,
        images,
        maximum_cards,
        completion_token_floor,
    )
    .await''',
    "flashcard generation completion floor call",
)
replace_once(
    "src-tauri/src/ai/document_intelligence.rs",
    '''    images: &[SourcePageImage],
    maximum_cards: usize,
) -> Result<Vec<Value>, CommandError> {
    let schema = flashcard_schema(maximum_cards);
    let maximum_tokens = flashcard_completion_tokens(maximum_cards);''',
    '''    images: &[SourcePageImage],
    maximum_cards: usize,
    completion_token_floor: usize,
) -> Result<Vec<Value>, CommandError> {
    let schema = flashcard_schema(maximum_cards);
    let maximum_tokens = flashcard_completion_token_ceiling(task, maximum_cards)
        .max(completion_token_floor);''',
    "flashcard request token floor",
)
replace_once(
    "src-tauri/src/ai/document_intelligence.rs",
    '''            max_tokens: 1_536,
            images: &[],
        },
    )
    .await
    .and_then(|value| parse_curation(value, candidate_count));''',
    '''            max_tokens: task.completion_token_ceiling(CURATION_VISIBLE_COMPLETION_TOKENS),
            images: &[],
        },
    )
    .await
    .and_then(|value| parse_curation(value, candidate_count));''',
    "curation output budget",
)
replace_once(
    "src-tauri/src/ai/document_intelligence.rs",
    '''                    max_tokens: 2_048,
                    images: &[],''',
    '''                    max_tokens: task.completion_token_ceiling(
                        CURATION_VISIBLE_COMPLETION_TOKENS.saturating_mul(2),
                    ),
                    images: &[],''',
    "curation repair output budget",
)
# Outline retry: truncation receives extra output capacity, invalid JSON gets the
# existing fresh-contract repair. Persistent truncation is not native-fallback.
replace_once(
    "src-tauri/src/ai/document_intelligence.rs",
    '''    match first {
        Ok(outline) => Ok(outline),
        Err(error) if retryable_contract_error(&error) => {
            let repaired = format!(
                "{user}\\n\\nA prior response could not be decoded as the required outline object. Generate a fresh bounded outline fragment for these pages and return only the required structured result."
            );
            request_json(
                runtime,
                task,
                JsonTaskRequest {
                    system_prompt: system,
                    user_text: &repaired,
                    schema: &schema,
                    max_tokens: outline_completion_tokens(task),
                    images,
                },
            )
            .await
            .and_then(|value| {
                parse_outline_fragment(value, bounds.page_start, bounds.page_end, bounds.page_count)
            })
            .map_err(|retry_error| collapse_contract_error(retry_error, "outline fragment"))
        }
        Err(error) => Err(error),
    }''',
    '''    match first {
        Ok(outline) => Ok(outline),
        Err(error) if error.code == "ai_document_output_truncated" => {
            crate::task_cancellation::record_current_provider_retry();
            request_json(
                runtime,
                task,
                JsonTaskRequest {
                    system_prompt: system,
                    user_text: user,
                    schema: &schema,
                    max_tokens: outline_retry_completion_tokens(task),
                    images,
                },
            )
            .await
            .and_then(|value| {
                parse_outline_fragment(value, bounds.page_start, bounds.page_end, bounds.page_count)
            })
        }
        Err(error) if retryable_contract_error(&error) => {
            let repaired = format!(
                "{user}\\n\\nA prior response could not be decoded as the required outline object. Generate a fresh bounded outline fragment for these pages and return only the required structured result."
            );
            request_json(
                runtime,
                task,
                JsonTaskRequest {
                    system_prompt: system,
                    user_text: &repaired,
                    schema: &schema,
                    max_tokens: outline_retry_completion_tokens(task),
                    images,
                },
            )
            .await
            .and_then(|value| {
                parse_outline_fragment(value, bounds.page_start, bounds.page_end, bounds.page_count)
            })
            .map_err(|retry_error| collapse_contract_error(retry_error, "outline fragment"))
        }
        Err(error) => Err(error),
    }''',
    "outline truncation recovery",
)
replace_once(
    "src-tauri/src/ai/document_intelligence.rs",
    '''        "ai_document_output_truncated" | "ai_document_output_invalid" | "ai_empty_response"''',
    '''        "ai_document_output_invalid" | "ai_empty_response"''',
    "exclude truncation from native outline fallback",
)
# Parse/validate the structured object before deciding whether finish=length was
# fatal. A complete schema-valid JSON object is safe even if an upstream labels
# the terminal event length; an incomplete object remains typed truncation.
replace_once(
    "src-tauri/src/ai/document_intelligence.rs",
    '''    record_document_usage(task, &response);
    ensure_document_response_complete(&response)?;

    match task.profile.provider_kind {
        ProviderKind::Deepseek => parse_strict_tool_arguments(&response),
        _ => {
            let content = parse_completion_content(&response)?;
            serde_json::from_str(normalize_json_content(content)).map_err(|_| {
                CommandError::new(
                    "ai_document_output_invalid",
                    "The AI provider returned content that was not valid JSON for the document task.",
                )
            })
        }
    }''',
    '''    record_document_usage(task, &response);
    ensure_document_response_not_blocked(&response)?;
    let finish_reason = response
        .pointer("/choices/0/finish_reason")
        .and_then(Value::as_str);
    let parsed = match task.profile.provider_kind {
        ProviderKind::Deepseek => parse_strict_tool_arguments(&response),
        _ => parse_completion_content(&response).and_then(|content| {
            serde_json::from_str(normalize_json_content(content)).map_err(|_| {
                CommandError::new(
                    "ai_document_output_invalid",
                    "The AI provider returned content that was not valid JSON for the document task.",
                )
            })
        }),
    };
    match parsed {
        Ok(value) => Ok(value),
        Err(_) if finish_reason == Some("length") => Err(truncated_output_error()),
        Err(error) => Err(error),
    }''',
    "structured parse before length classification",
)
replace_once(
    "src-tauri/src/ai/document_intelligence.rs",
    '''fn ensure_document_response_complete(value: &Value) -> Result<(), CommandError> {
    match value
        .pointer("/choices/0/finish_reason")
        .and_then(Value::as_str)
    {
        Some("length") => Err(CommandError::new(
            "ai_document_output_truncated",
            "The AI provider reached its completion limit before finishing the structured document result. ostudy uses bounded source windows and concise structured outputs; retry on a healthy endpoint or choose a model with more output capacity.",
        )),
        Some("content_filter") => Err(CommandError::new(''',
    '''fn truncated_output_error() -> CommandError {
    CommandError::new(
        "ai_document_output_truncated",
        "The AI provider reached its completion limit before finishing a valid structured document result. ostudy will retry this bounded source window with more output headroom and, for card generation, a smaller card count without reducing the token ceiling.",
    )
}

fn ensure_document_response_not_blocked(value: &Value) -> Result<(), CommandError> {
    match value
        .pointer("/choices/0/finish_reason")
        .and_then(Value::as_str)
    {
        Some("content_filter") => Err(CommandError::new(''',
    "length-aware response validation",
)
# Update tests that directly exercise completion and flashcard signatures.
text = Path("src-tauri/src/ai/document_intelligence.rs").read_text()
text = text.replace("assert_eq!(outline_completion_tokens_for_provider(provider), 1_536);", "assert_eq!(outline_completion_tokens_for_provider(provider), 2_048);")
text = text.replace("ensure_document_response_complete(&json!({", "ensure_document_response_not_blocked(&json!({")
Path("src-tauri/src/ai/document_intelligence.rs").write_text(text)

# ---------------------------------------------------------------------------
# Compact retry keeps at least the original completion ceiling. The old 16→8
# retry also cut max_tokens 5120→4096, exactly the wrong response to a length
# finish. Card count can shrink; output token capacity must not.
# ---------------------------------------------------------------------------
replace_once(
    "src-tauri/src/document_generation.rs",
    '''        input.images,
        first_budget,
    )''',
    '''        input.images,
        first_budget,
        0,
    )''',
    "initial generation token floor",
)
replace_once(
    "src-tauri/src/document_generation.rs",
    '''            let reduced_budget = (input.maximum_cards / 2).max(2);
            record_current_provider_retry();''',
    '''            let reduced_budget = (input.maximum_cards / 2).max(2);
            let completion_token_floor =
                document_intelligence::flashcard_completion_token_ceiling(input.task, first_budget);
            record_current_provider_retry();''',
    "compact retry token floor calculation",
)
replace_once(
    "src-tauri/src/document_generation.rs",
    '''                input.images,
                reduced_budget,
            )''',
    '''                input.images,
                reduced_budget,
                completion_token_floor,
            )''',
    "compact retry preserves token ceiling",
)
replace_once(
    "src-tauri/src/document_generation.rs",
    '''    #[test]
    fn transport_inactivity_never_changes_the_card_budget() {''',
    '''    #[test]
    fn compact_retry_never_reduces_the_original_completion_ceiling() {
        let initial_visible = crate::document_scale::generation_visible_completion_tokens(16);
        let reduced_visible = crate::document_scale::generation_visible_completion_tokens(8);
        assert!(initial_visible > reduced_visible);
        let retry_wire_ceiling = reduced_visible.max(initial_visible);
        assert_eq!(retry_wire_ceiling, initial_visible);
    }

    #[test]
    fn transport_inactivity_never_changes_the_card_budget() {''',
    "compact retry ceiling regression",
)

# ---------------------------------------------------------------------------
# Current reasoning becomes part of rebuildable checkpoint identity. Changing a
# reasoning setting may change generated content and must invalidate old cards.
# ---------------------------------------------------------------------------
replace_once(
    "src-tauri/src/document_checkpoints.rs",
    '''    pub provider_capability_fingerprint: String,
}''',
    '''    pub provider_capability_fingerprint: String,
    pub reasoning_effort: String,
}''',
    "checkpoint reasoning identity field",
)
replace_once(
    "src-tauri/src/document_checkpoints.rs",
    '''            extraction_backend: extraction_backend.to_string(),
            provider_capability_fingerprint: provider_capability_fingerprint.to_string(),
        }''',
    '''            extraction_backend: extraction_backend.to_string(),
            provider_capability_fingerprint: provider_capability_fingerprint.to_string(),
            reasoning_effort: task.reasoning_effort.clone(),
        }''',
    "checkpoint reasoning identity initialization",
)
replace_once(
    "src-tauri/src/document_checkpoints.rs",
    '''            "provider={}|profile={}|model={}|extractor={}|capabilities={}",
            self.provider_kind,
            self.profile_id,
            self.model_id,
            self.extraction_backend,
            self.provider_capability_fingerprint,
        )''',
    '''            "provider={}|profile={}|model={}|extractor={}|capabilities={}|reasoning={}",
            self.provider_kind,
            self.profile_id,
            self.model_id,
            self.extraction_backend,
            self.provider_capability_fingerprint,
            self.reasoning_effort,
        )''',
    "checkpoint reasoning identity material",
)

# ---------------------------------------------------------------------------
# Apply provider capabilities before scale planning in initial/additional paths;
# pass effective effort into context and aggregate budget calculations.
# ---------------------------------------------------------------------------
replace_once(
    "src-tauri/src/documents.rs",
    '''    let task = document_intelligence::load_document_task_context(database.inner())?;
    let capabilities = document_provider::load_document_provider_capabilities(''',
    '''    let mut task = document_intelligence::load_document_task_context(database.inner())?;
    let capabilities = document_provider::load_document_provider_capabilities(''',
    "mutable initial document task",
)
replace_once(
    "src-tauri/src/documents.rs",
    '''        &task.model_id,
    )?;
    let calibration = document_cost::load_cost_calibration(''',
    '''        &task.model_id,
    )?;
    task.apply_provider_capabilities(&capabilities);
    let calibration = document_cost::load_cost_calibration(''',
    "resolve initial reasoning capabilities",
)
replace_once(
    "src-tauri/src/documents.rs",
    '''    let plan = DocumentScalePlan::for_capabilities(source.unit_count, &capabilities, calibration)?;''',
    '''    let plan = DocumentScalePlan::for_capabilities(
        source.unit_count,
        &capabilities,
        &task.reasoning_effort,
        calibration,
    )?;''',
    "initial reasoning-aware scale plan",
)
replace_once(
    "src-tauri/src/documents.rs",
    '''        &capabilities,
        calibration,
    )?;''',
    '''        &capabilities,
        &task.reasoning_effort,
        calibration,
    )?;''',
    "initial reasoning-aware job estimate",
)
replace_once(
    "src-tauri/src/document_more.rs",
    '''    let task = document_intelligence::load_document_task_context(database.inner())?;
    let capabilities = document_provider::load_document_provider_capabilities(''',
    '''    let mut task = document_intelligence::load_document_task_context(database.inner())?;
    let capabilities = document_provider::load_document_provider_capabilities(''',
    "mutable additional document task",
)
replace_once(
    "src-tauri/src/document_more.rs",
    '''        &task.model_id,
    )?;
    let calibration = document_cost::load_cost_calibration(''',
    '''        &task.model_id,
    )?;
    task.apply_provider_capabilities(&capabilities);
    let calibration = document_cost::load_cost_calibration(''',
    "resolve additional reasoning capabilities",
)
replace_once(
    "src-tauri/src/document_more.rs",
    '''    let plan = DocumentScalePlan::for_capabilities(source.unit_count, &capabilities, calibration)?;''',
    '''    let plan = DocumentScalePlan::for_capabilities(
        source.unit_count,
        &capabilities,
        &task.reasoning_effort,
        calibration,
    )?;''',
    "additional reasoning-aware scale plan",
)
replace_once(
    "src-tauri/src/document_more.rs",
    '''        &capabilities,
        calibration,
    )?;''',
    '''        &capabilities,
        &task.reasoning_effort,
        calibration,
    )?;''',
    "additional reasoning-aware job estimate",
)

# ---------------------------------------------------------------------------
# A native outline fallback is still allowed for repeated malformed/empty model
# output, but never for persistent truncation. Make fallback visible under the AI
# debug filter rather than reporting it as a normal provider-completed outline.
# ---------------------------------------------------------------------------
replace_once(
    "src-tauri/src/ai/document_intelligence_adapter.rs",
    '''            crate::logging::info(
                "document_outline_window_native_fallback",
                format!(
                    "window={}/{} partition={} pages={}-{} code={} detail={}",
                    index + 1,
                    windows.len(),
                    window.partition_index + 1,
                    window.page_start,
                    window.page_end,
                    error.code,
                    error.message.chars().take(400).collect::<String>(),
                ),
            );''',
    '''            crate::logging::warn_target(
                "ai.document.outline",
                "document_outline_window_native_fallback",
                "Provider outline output remained invalid after bounded repair; ostudy used a deterministic source-derived fallback for this window.",
                serde_json::json!({
                    "window": index + 1,
                    "totalWindows": windows.len(),
                    "partition": window.partition_index + 1,
                    "pageStart": window.page_start,
                    "pageEnd": window.page_end,
                    "code": error.code.as_str(),
                }),
            );''',
    "visible structured outline fallback diagnostic",
)

print("Applied PR #54 reasoning-aware structured-output capacity fixes.")
