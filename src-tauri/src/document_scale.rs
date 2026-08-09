//! Format-neutral, provider-aware document planning.
//!
//! Provider context windows are hard per-request constraints. Empirical usage
//! samples refine conservative prompt-overhead estimates, but never increase a
//! provider-declared hard limit. Large sources scale through more bounded
//! semantic windows rather than ever-growing requests.

use crate::{
    ai::document_provider::DocumentProviderCapabilities, document_cost::DocumentCostCalibration,
    error::CommandError,
};

pub(crate) const LARGE_DOCUMENT_UNIT_THRESHOLD: usize = 50;
pub(crate) const MAX_SUPPORTED_SOURCE_UNITS: usize = 300;
pub(crate) const MAX_AUTOMATIC_SOURCE_TOKENS: usize = 250_000;
const DEEPSEEK_JOB_TOKEN_CEILING: usize = 650_000;
const LOCAL_JOB_TOKEN_CEILING: usize = 1_200_000;
const DEFAULT_CLOUD_JOB_TOKEN_CEILING: usize = 650_000;
const OUTLINE_FIXED_PROMPT_RESERVE: usize = 1_200;
const GENERATION_FIXED_PROMPT_RESERVE: usize = 1_800;
const MIN_OUTLINE_SOURCE_TOKENS: usize = 600;
const MIN_GENERATION_SOURCE_TOKENS: usize = 800;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DocumentScaleClass {
    Standard,
    Large,
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
    pub ceiling_tokens: usize,
    pub generation_card_capacity: usize,
    pub context_window_tokens: Option<usize>,
    pub calibration_sample_count: usize,
}

impl DocumentScalePlan {
    #[cfg(test)]
    pub(crate) fn for_unit_count(source_unit_count: usize) -> Self {
        Self::for_provider(source_unit_count, "llama_cpp")
    }

    /// Conservative provider defaults used when no current capability snapshot
    /// is available. This preserves offline/test determinism and fails toward
    /// smaller request windows rather than assuming a large advertised context.
    pub(crate) fn for_provider(source_unit_count: usize, provider_kind: &str) -> Self {
        baseline_plan(source_unit_count, provider_kind)
    }

    pub(crate) fn for_capabilities(
        source_unit_count: usize,
        capabilities: &DocumentProviderCapabilities,
        calibration: DocumentCostCalibration,
    ) -> Result<Self, CommandError> {
        let mut plan = baseline_plan(source_unit_count, &capabilities.provider_kind);
        let Some(context_window) = capabilities.context_window_tokens else {
            return Ok(plan);
        };

        let outline_output = outline_completion_reserve(&capabilities.provider_kind);
        plan.outline_window_max_tokens = context_bounded_source_tokens(
            plan.outline_window_max_tokens,
            context_window,
            outline_output,
            OUTLINE_FIXED_PROMPT_RESERVE,
            0,
            calibration,
            MIN_OUTLINE_SOURCE_TOKENS,
        )?;
        let generation_output = generation_completion_reserve(plan.max_cards_per_window);
        plan.generation_window_max_tokens = context_bounded_source_tokens(
            plan.generation_window_max_tokens,
            context_window,
            generation_output,
            GENERATION_FIXED_PROMPT_RESERVE,
            plan.max_images_per_window,
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
            outline_window_max_tokens: 3_200,
            outline_window_max_units: 8,
            outline_window_overlap_tokens: 350,
            generation_window_max_tokens: 5_000,
            generation_window_max_units: 10,
            generation_window_overlap_tokens: 700,
            max_images_per_window: if provider_kind == "deepseek" { 0 } else { 4 },
            max_cards_per_window: 12,
        };
    }

    if provider_kind == "deepseek" {
        DocumentScalePlan {
            class: DocumentScaleClass::Large,
            partition_target_tokens: 32_000,
            partition_max_tokens: 48_000,
            partition_target_units: 48,
            partition_max_units: 60,
            outline_window_max_tokens: 10_000,
            outline_window_max_units: 20,
            outline_window_overlap_tokens: 500,
            generation_window_max_tokens: 6_000,
            generation_window_max_units: 8,
            generation_window_overlap_tokens: 400,
            max_images_per_window: 0,
            max_cards_per_window: 16,
        }
    } else {
        DocumentScalePlan {
            class: DocumentScaleClass::Large,
            partition_target_tokens: 18_000,
            partition_max_tokens: 28_000,
            partition_target_units: 20,
            partition_max_units: 30,
            outline_window_max_tokens: 3_200,
            outline_window_max_units: 8,
            outline_window_overlap_tokens: 300,
            generation_window_max_tokens: 5_000,
            generation_window_max_units: 10,
            generation_window_overlap_tokens: 500,
            max_images_per_window: 3,
            max_cards_per_window: 16,
        }
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

fn outline_completion_reserve(provider_kind: &str) -> usize {
    if provider_kind == "deepseek" {
        3_072
    } else {
        2_048
    }
}

fn generation_completion_reserve(maximum_cards: usize) -> usize {
    1_024_usize
        .saturating_add(maximum_cards.saturating_mul(384))
        .clamp(1_536, 5_120)
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
        &DocumentProviderCapabilities::conservative(provider_kind, "unknown"),
        DocumentCostCalibration::default(),
    )
}

pub(crate) fn validate_automatic_job_calibrated(
    source_unit_count: usize,
    source_tokens: usize,
    plan: DocumentScalePlan,
    outline_window_count: usize,
    generation_window_count: usize,
    capabilities: &DocumentProviderCapabilities,
    calibration: DocumentCostCalibration,
) -> Result<DocumentJobBudget, CommandError> {
    if source_unit_count > MAX_SUPPORTED_SOURCE_UNITS {
        return Err(CommandError::new(
            "document_generation_unit_limit",
            format!(
                "Automatic generation supports up to {MAX_SUPPORTED_SOURCE_UNITS} source units at a time. Split this source at chapter or section boundaries and import the parts separately."
            ),
        ));
    }
    if source_tokens > MAX_AUTOMATIC_SOURCE_TOKENS {
        return Err(CommandError::new(
            "document_generation_source_limit",
            format!(
                "This document contains too much extracted source material for one automatic generation job. Split it at chapter or section boundaries so each part stays below about {MAX_AUTOMATIC_SOURCE_TOKENS} estimated source tokens."
            ),
        ));
    }

    let ceiling_tokens = match capabilities.provider_kind.as_str() {
        "llama_cpp" => LOCAL_JOB_TOKEN_CEILING,
        "deepseek" => DEEPSEEK_JOB_TOKEN_CEILING,
        _ => DEFAULT_CLOUD_JOB_TOKEN_CEILING,
    };
    let generation_card_capacity = generation_window_count
        .saturating_mul(plan.max_cards_per_window)
        .min(10_000);

    let outline_source_estimate = source_tokens.saturating_add(
        outline_window_count.saturating_mul(plan.outline_window_overlap_tokens),
    );
    let outline_prompt = calibration.calibrated_prompt_tokens(outline_source_estimate, 0);
    let outline_fixed = outline_window_count.saturating_mul(
        1_500_usize.saturating_add(outline_completion_reserve(&capabilities.provider_kind)),
    );

    let generation_source_estimate = source_tokens.saturating_add(
        generation_window_count.saturating_mul(plan.generation_window_overlap_tokens),
    );
    let maximum_images = generation_window_count.saturating_mul(plan.max_images_per_window);
    let generation_prompt =
        calibration.calibrated_prompt_tokens(generation_source_estimate, maximum_images);
    let generation_fixed = generation_window_count.saturating_mul(
        1_800_usize.saturating_add(generation_completion_reserve(plan.max_cards_per_window)),
    );
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

    if estimated_tokens > ceiling_tokens {
        return Err(CommandError::new(
            "document_ai_budget_exceeded",
            format!(
                "This document's conservative AI-job estimate is about {estimated_tokens} tokens, above the automatic {ceiling_tokens}-token safety ceiling for the selected provider. Split the source at chapter or section boundaries; completed local extraction remains available in Documents."
            ),
        ));
    }

    Ok(DocumentJobBudget {
        estimated_tokens,
        ceiling_tokens,
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
        }
    }

    #[test]
    fn fifty_units_keep_the_standard_plan() {
        let plan = DocumentScalePlan::for_provider(50, "deepseek");
        assert_eq!(plan.class, DocumentScaleClass::Standard);
        assert_eq!(plan.generation_window_max_tokens, 5_000);
    }

    #[test]
    fn reported_context_can_only_shrink_the_conservative_source_window() {
        let calibration = DocumentCostCalibration {
            prompt_multiplier_milli: 1_500,
            image_prompt_tokens: 0,
            sample_count: 5,
        };
        let plan = DocumentScalePlan::for_capabilities(
            20,
            &capabilities("llama_cpp", Some(8_192)),
            calibration,
        )
        .unwrap();
        assert!(plan.outline_window_max_tokens <= 3_200);
        assert!(plan.generation_window_max_tokens < 5_000);
    }

    #[test]
    fn too_small_reported_context_fails_before_provider_traffic() {
        let error = DocumentScalePlan::for_capabilities(
            10,
            &capabilities("llama_cpp", Some(2_048)),
            DocumentCostCalibration::default(),
        )
        .unwrap_err();
        assert_eq!(error.code, "document_ai_context_too_small");
    }

    #[test]
    fn deepseek_large_documents_use_chapter_scale_partitions_and_many_card_slots() {
        let plan = DocumentScalePlan::for_provider(138, "deepseek");
        assert_eq!(plan.class, DocumentScaleClass::Large);
        assert_eq!(plan.partition_target_units, 48);
        assert_eq!(plan.partition_max_units, 60);
        assert_eq!(plan.generation_window_max_units, 8);
        assert_eq!(plan.max_cards_per_window, 16);
        let budget = validate_automatic_job(138, 100_000, plan, 8, 18, "deepseek").unwrap();
        assert_eq!(budget.generation_card_capacity, 288);
        assert!(budget.estimated_tokens < budget.ceiling_tokens);
    }

    #[test]
    fn local_large_documents_keep_conservative_request_windows() {
        let plan = DocumentScalePlan::for_provider(138, "llama_cpp");
        assert_eq!(plan.class, DocumentScaleClass::Large);
        assert_eq!(plan.outline_window_max_tokens, 3_200);
        assert_eq!(plan.generation_window_max_units, 10);
        assert_eq!(plan.max_cards_per_window, 16);
    }

    #[test]
    fn whole_job_budget_fails_before_an_excessive_cloud_run() {
        let plan = DocumentScalePlan::for_provider(250, "deepseek");
        let error = validate_automatic_job(250, 220_000, plan, 20, 40, "deepseek").unwrap_err();
        assert_eq!(error.code, "document_ai_budget_exceeded");
    }

    #[test]
    fn automatic_source_and_unit_limits_are_explicit() {
        let plan = DocumentScalePlan::for_provider(301, "deepseek");
        assert_eq!(
            validate_automatic_job(301, 10_000, plan, 1, 1, "deepseek")
                .unwrap_err()
                .code,
            "document_generation_unit_limit"
        );
        let plan = DocumentScalePlan::for_provider(100, "deepseek");
        assert_eq!(
            validate_automatic_job(
                100,
                MAX_AUTOMATIC_SOURCE_TOKENS + 1,
                plan,
                1,
                1,
                "deepseek"
            )
            .unwrap_err()
            .code,
            "document_generation_source_limit"
        );
    }
}
