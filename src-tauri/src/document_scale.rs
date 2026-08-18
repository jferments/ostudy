//! Format-neutral, provider-aware document planning.
//!
//! Provider context windows are hard per-request constraints. Empirical usage
//! samples refine conservative prompt-overhead estimates, but never increase a
//! provider-declared hard limit. Large sources scale through more bounded
//! semantic windows rather than ever-growing requests. Aggregate job estimates
//! are diagnostic/cost information only and never reject an otherwise
//! context-safe sequence of resumable windows based on provider type.

use std::fmt;

use crate::{
    ai::document_provider::DocumentProviderCapabilities, document_cost::DocumentCostCalibration,
    error::CommandError,
};

pub(crate) const LARGE_DOCUMENT_UNIT_THRESHOLD: usize = 50;
pub(crate) const MAX_SUPPORTED_SOURCE_UNITS: usize = 300;
const OUTLINE_FIXED_PROMPT_RESERVE: usize = 1_200;
const GENERATION_FIXED_PROMPT_RESERVE: usize = 1_800;
const MIN_OUTLINE_SOURCE_TOKENS: usize = 600;
const MIN_GENERATION_SOURCE_TOKENS: usize = 800;
pub(crate) const OUTLINE_VISIBLE_COMPLETION_TOKENS: usize = 2_048;
pub(crate) const GLOBAL_OUTLINE_VISIBLE_COMPLETION_TOKENS: usize = 3_072;
pub(crate) const CURATION_VISIBLE_COMPLETION_TOKENS: usize = 2_048;

pub(crate) fn generation_visible_completion_tokens(maximum_cards: usize) -> usize {
    2_048_usize
        .saturating_add(maximum_cards.saturating_mul(384))
        .clamp(2_048, 8_192)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DocumentScaleClass {
    Standard,
    Large,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AggregateTokenCeiling {
    /// Every supported provider executes large jobs as a sequence of bounded,
    /// checkpointed requests. Only per-request model/context constraints are
    /// hard; whole-document token totals are informational.
    Windowed,
}

impl fmt::Display for AggregateTokenCeiling {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("windowed")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DocumentScalePlan {
    pub class: DocumentScaleClass,
    pub partition_target_tokens: usize,
    pub partition_max_tokens: usize,
    pub partition_target_units: usize,
    pub partition_max_units: usize,
    pub outline_window_max_tokens: usize,
    pub outline_window_max_units: usize,
    pub outline_window_overlap_tokens: usize,
    pub generation_window_max_tokens: usize,
    pub generation_window_max_units: usize,
    pub generation_window_overlap_tokens: usize,
    pub max_images_per_window: usize,
    pub max_cards_per_window: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DocumentJobBudget {
    pub estimated_tokens: usize,
    pub ceiling_tokens: AggregateTokenCeiling,
    pub generation_card_capacity: usize,
    pub context_window_tokens: Option<usize>,
    pub calibration_sample_count: usize,
}

impl DocumentScalePlan {
    #[cfg(test)]
    pub(crate) fn for_unit_count(source_unit_count: usize) -> Self {
        Self::for_provider(source_unit_count, "llama_cpp")
    }

    /// Conservative provider defaults used by deterministic tests when no
    /// current capability snapshot is available.
    #[cfg(test)]
    pub(crate) fn for_provider(source_unit_count: usize, provider_kind: &str) -> Self {
        baseline_plan(source_unit_count, provider_kind)
    }

    pub(crate) fn for_capabilities(
        source_unit_count: usize,
        capabilities: &DocumentProviderCapabilities,
        effective_reasoning_effort: &str,
        calibration: DocumentCostCalibration,
    ) -> Result<Self, CommandError> {
        let mut plan = baseline_plan(source_unit_count, &capabilities.provider_kind);
        let Some(context_window) = capabilities.context_window_tokens else {
            return Ok(plan);
        };

        let outline_output = capabilities.completion_token_ceiling(
            effective_reasoning_effort,
            OUTLINE_VISIBLE_COMPLETION_TOKENS,
        );
        plan.outline_window_max_tokens = context_bounded_source_tokens(
            plan.outline_window_max_tokens,
            context_window,
            outline_output,
            OUTLINE_FIXED_PROMPT_RESERVE,
            0,
            calibration,
            MIN_OUTLINE_SOURCE_TOKENS,
        )?;
        let generation_output = capabilities.completion_token_ceiling(
            effective_reasoning_effort,
            generation_visible_completion_tokens(plan.max_cards_per_window),
        );
        let generation_image_count = if capabilities.image_input {
            plan.max_images_per_window
        } else {
            0
        };
        plan.generation_window_max_tokens = context_bounded_source_tokens(
            plan.generation_window_max_tokens,
            context_window,
            generation_output,
            GENERATION_FIXED_PROMPT_RESERVE,
            generation_image_count,
            calibration,
            MIN_GENERATION_SOURCE_TOKENS,
        )?;
        Ok(plan)
    }

    pub(crate) fn label(self) -> &'static str {
        match self.class {
            DocumentScaleClass::Standard => "standard",
            DocumentScaleClass::Large => "large",
        }
    }

    pub(crate) fn planning_fingerprint(self) -> String {
        format!(
            "scale={}|partition={}/{}/{}:{}|outline={}/{}/{}|generation={}/{}/{}|images={}|cards={}",
            self.label(),
            self.partition_target_tokens,
            self.partition_max_tokens,
            self.partition_target_units,
            self.partition_max_units,
            self.outline_window_max_tokens,
            self.outline_window_max_units,
            self.outline_window_overlap_tokens,
            self.generation_window_max_tokens,
            self.generation_window_max_units,
            self.generation_window_overlap_tokens,
            self.max_images_per_window,
            self.max_cards_per_window,
        )
    }
}

fn baseline_plan(source_unit_count: usize, provider_kind: &str) -> DocumentScalePlan {
    if source_unit_count <= LARGE_DOCUMENT_UNIT_THRESHOLD {
        return DocumentScalePlan {
            class: DocumentScaleClass::Standard,
            partition_target_tokens: 16_000,
            partition_max_tokens: 24_000,
            partition_target_units: 24,
            partition_max_units: 32,
            outline_window_max_tokens: 6_000,
            outline_window_max_units: 12,
            outline_window_overlap_tokens: 300,
            generation_window_max_tokens: 6_500,
            generation_window_max_units: 14,
            generation_window_overlap_tokens: 500,
            max_images_per_window: if provider_kind == "deepseek" { 0 } else { 4 },
            max_cards_per_window: 12,
        };
    }

    // Large-document request sizing is provider-neutral. Hierarchy metadata is
    // retained inside each semantic chunk, while network request packing uses
    // useful multi-page batches. A provider-declared context window can only
    // shrink these targets below; it can never make an unsafe request larger.
    DocumentScalePlan {
        class: DocumentScaleClass::Large,
        partition_target_tokens: 32_000,
        partition_max_tokens: 48_000,
        partition_target_units: 40,
        partition_max_units: 60,
        outline_window_max_tokens: 14_000,
        outline_window_max_units: 24,
        outline_window_overlap_tokens: 400,
        generation_window_max_tokens: 10_000,
        generation_window_max_units: 18,
        generation_window_overlap_tokens: 500,
        max_images_per_window: if provider_kind == "deepseek" { 0 } else { 3 },
        max_cards_per_window: 16,
    }
}

fn context_bounded_source_tokens(
    desired_source_tokens: usize,
    context_window: usize,
    completion_reserve: usize,
    fixed_prompt_reserve: usize,
    image_count: usize,
    calibration: DocumentCostCalibration,
    minimum_source_tokens: usize,
) -> Result<usize, CommandError> {
    let image_reserve = image_count.saturating_mul(calibration.image_prompt_tokens);
    let non_source = completion_reserve
        .saturating_add(fixed_prompt_reserve)
        .saturating_add(image_reserve);
    let prompt_room = context_window.saturating_sub(non_source);
    let multiplier = calibration.prompt_multiplier_milli.max(1_000);
    let provider_source_limit = prompt_room.saturating_mul(1_000) / multiplier;
    let bounded = desired_source_tokens.min(provider_source_limit);
    if bounded < minimum_source_tokens {
        return Err(CommandError::new(
            "document_ai_context_too_small",
            format!(
                "The selected model reports a {context_window}-token context window, which leaves too little room for ostudy's structured document request and completion. Choose a model/profile with a larger context window."
            ),
        ));
    }
    Ok(bounded)
}

pub(crate) struct DocumentBudgetContext<'a> {
    pub capabilities: &'a DocumentProviderCapabilities,
    pub reasoning_effort: &'a str,
    pub calibration: DocumentCostCalibration,
}

#[cfg(test)]
pub(crate) fn validate_automatic_job(
    source_unit_count: usize,
    source_tokens: usize,
    plan: DocumentScalePlan,
    outline_window_count: usize,
    generation_window_count: usize,
    provider_kind: &str,
) -> Result<DocumentJobBudget, CommandError> {
    validate_automatic_job_calibrated(
        source_unit_count,
        source_tokens,
        plan,
        outline_window_count,
        generation_window_count,
        DocumentBudgetContext {
            capabilities: &DocumentProviderCapabilities::conservative(provider_kind, "unknown"),
            reasoning_effort: "none",
            calibration: DocumentCostCalibration::default(),
        },
    )
}

pub(crate) fn validate_automatic_job_calibrated(
    source_unit_count: usize,
    source_tokens: usize,
    plan: DocumentScalePlan,
    outline_window_count: usize,
    generation_window_count: usize,
    context: DocumentBudgetContext<'_>,
) -> Result<DocumentJobBudget, CommandError> {
    let DocumentBudgetContext {
        capabilities,
        reasoning_effort: effective_reasoning_effort,
        calibration,
    } = context;
    if source_unit_count > MAX_SUPPORTED_SOURCE_UNITS {
        return Err(CommandError::new(
            "document_generation_unit_limit",
            format!(
                "Automatic generation supports up to {MAX_SUPPORTED_SOURCE_UNITS} source units at a time. Split this source at chapter or section boundaries and import the parts separately."
            ),
        ));
    }

    let generation_card_capacity = generation_window_count
        .saturating_mul(plan.max_cards_per_window)
        .min(10_000);

    let outline_source_estimate = source_tokens
        .saturating_add(outline_window_count.saturating_mul(plan.outline_window_overlap_tokens));
    let outline_prompt = calibration.calibrated_prompt_tokens(outline_source_estimate, 0);
    let outline_completion = capabilities.completion_token_ceiling(
        effective_reasoning_effort,
        OUTLINE_VISIBLE_COMPLETION_TOKENS,
    );
    let outline_fixed =
        outline_window_count.saturating_mul(1_500_usize.saturating_add(outline_completion));

    let generation_source_estimate = source_tokens.saturating_add(
        generation_window_count.saturating_mul(plan.generation_window_overlap_tokens),
    );
    let maximum_images = generation_window_count.saturating_mul(plan.max_images_per_window);
    let generation_prompt =
        calibration.calibrated_prompt_tokens(generation_source_estimate, maximum_images);
    let generation_completion = capabilities.completion_token_ceiling(
        effective_reasoning_effort,
        generation_visible_completion_tokens(plan.max_cards_per_window),
    );
    let generation_fixed =
        generation_window_count.saturating_mul(1_800_usize.saturating_add(generation_completion));
    let compact_repair_reserve = outline_window_count
        .saturating_mul(900)
        .saturating_add(generation_window_count.saturating_mul(1_200));
    let curation_batches = generation_card_capacity.div_ceil(32);
    let curation_reserve = curation_batches.saturating_mul(2_500);
    let global_synthesis_reserve = 12_000;
    let estimated_tokens = outline_prompt
        .saturating_add(outline_fixed)
        .saturating_add(generation_prompt)
        .saturating_add(generation_fixed)
        .saturating_add(compact_repair_reserve)
        .saturating_add(curation_reserve)
        .saturating_add(global_synthesis_reserve);

    Ok(DocumentJobBudget {
        estimated_tokens,
        ceiling_tokens: AggregateTokenCeiling::Windowed,
        generation_card_capacity,
        context_window_tokens: capabilities.context_window_tokens,
        calibration_sample_count: calibration.sample_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::document_provider::{DocumentProviderCapabilities, StructuredOutputStrategy};

    fn capabilities(provider: &str, context: Option<usize>) -> DocumentProviderCapabilities {
        DocumentProviderCapabilities {
            provider_kind: provider.to_string(),
            model_id: "test-model".to_string(),
            context_window_tokens: context,
            image_input: false,
            usage_reporting: true,
            structured_output: StructuredOutputStrategy::JsonSchema,
            reasoning: None,
        }
    }

    #[test]
    fn fifty_units_keep_the_standard_plan() {
        let plan = DocumentScalePlan::for_provider(50, "deepseek");
        assert_eq!(plan.class, DocumentScaleClass::Standard);
        assert_eq!(plan.generation_window_max_tokens, 6_500);
    }

    #[test]
    fn reported_context_can_only_shrink_the_source_window() {
        let calibration = DocumentCostCalibration {
            prompt_multiplier_milli: 1_500,
            image_prompt_tokens: 0,
            sample_count: 5,
        };
        let plan = DocumentScalePlan::for_capabilities(
            55,
            &capabilities("openrouter", Some(12_000)),
            "none",
            calibration,
        )
        .unwrap();
        assert!(plan.outline_window_max_tokens < 14_000);
        assert!(plan.generation_window_max_tokens < 10_000);
    }

    #[test]
    fn too_small_reported_context_fails_before_provider_traffic() {
        let error = DocumentScalePlan::for_capabilities(
            10,
            &capabilities("llama_cpp", Some(2_048)),
            "none",
            DocumentCostCalibration::default(),
        )
        .unwrap_err();
        assert_eq!(error.code, "document_ai_context_too_small");
    }

    #[test]
    fn large_document_request_sizes_are_provider_neutral() {
        let llama = DocumentScalePlan::for_provider(138, "llama_cpp");
        let deepseek = DocumentScalePlan::for_provider(138, "deepseek");
        let openrouter = DocumentScalePlan::for_provider(138, "openrouter");
        for plan in [llama, deepseek, openrouter] {
            assert_eq!(plan.class, DocumentScaleClass::Large);
            assert_eq!(plan.partition_target_units, 40);
            assert_eq!(plan.partition_max_units, 60);
            assert_eq!(plan.outline_window_max_tokens, 14_000);
            assert_eq!(plan.outline_window_max_units, 24);
            assert_eq!(plan.generation_window_max_tokens, 10_000);
            assert_eq!(plan.generation_window_max_units, 18);
            assert_eq!(plan.max_cards_per_window, 16);
        }
        assert_eq!(deepseek.max_images_per_window, 0);
        assert_eq!(llama.max_images_per_window, 3);
        assert_eq!(openrouter.max_images_per_window, 3);
    }

    #[test]
    fn openrouter_55_page_plan_uses_fewer_larger_windows_than_old_cloud_policy() {
        let plan = DocumentScalePlan::for_provider(55, "openrouter");
        assert_eq!(plan.outline_window_max_tokens, 14_000);
        assert_eq!(plan.outline_window_max_units, 24);
        assert_eq!(plan.generation_window_max_tokens, 10_000);
        assert_eq!(plan.generation_window_max_units, 18);
    }

    #[test]
    fn all_providers_continue_through_windows_past_old_aggregate_ceilings() {
        for provider in ["llama_cpp", "deepseek", "openrouter"] {
            let plan = DocumentScalePlan::for_provider(138, provider);
            let budget = validate_automatic_job(138, 220_000, plan, 20, 35, provider)
                .expect("large jobs should be window-bounded, not whole-job-bounded");
            assert!(budget.estimated_tokens > 650_000);
            assert_eq!(budget.ceiling_tokens, AggregateTokenCeiling::Windowed);
        }
    }

    #[test]
    fn openrouter_55_page_job_above_old_650k_guard_is_allowed() {
        let plan = DocumentScalePlan::for_provider(55, "openrouter");
        let budget = validate_automatic_job(55, 180_000, plan, 18, 28, "openrouter")
            .expect("OpenRouter should process large sources through bounded windows");
        assert!(budget.estimated_tokens > 650_000);
        assert_eq!(budget.ceiling_tokens, AggregateTokenCeiling::Windowed);
    }

    #[test]
    fn aggregate_source_tokens_do_not_create_provider_specific_rejections() {
        for provider in ["llama_cpp", "deepseek", "openrouter"] {
            let plan = DocumentScalePlan::for_provider(100, provider);
            let budget = validate_automatic_job(100, 350_000, plan, 50, 60, provider)
                .expect("large source totals should scale by adding bounded windows");
            assert_eq!(budget.ceiling_tokens, AggregateTokenCeiling::Windowed);
        }
    }

    #[test]
    fn source_unit_limit_remains_a_provider_neutral_ingestion_bound() {
        for provider in ["llama_cpp", "deepseek", "openrouter"] {
            let plan = DocumentScalePlan::for_provider(301, provider);
            assert_eq!(
                validate_automatic_job(301, 10_000, plan, 1, 1, provider)
                    .unwrap_err()
                    .code,
                "document_generation_unit_limit"
            );
        }
    }
}
