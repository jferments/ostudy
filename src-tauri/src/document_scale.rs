use crate::{
    ai::document_provider::DocumentProviderCapabilities, document_cost::DocumentCostCalibration,
    error::CommandError,
};

pub(crate) const LARGE_DOCUMENT_UNIT_THRESHOLD: usize = 50;
pub(crate) const MAX_SUPPORTED_SOURCE_UNITS: usize = 2_000;
const MIN_CONTEXT_BOUNDED_SOURCE_TOKENS: usize = 1_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DocumentScaleClass {
    Standard,
    Large,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AggregateTokenCeiling {
    Windowed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DocumentBudgetEstimate {
    pub estimated_tokens: usize,
    pub ceiling_tokens: AggregateTokenCeiling,
    pub generation_card_capacity: usize,
    pub context_window_tokens: Option<usize>,
    pub calibration_sample_count: usize,
}

impl DocumentScalePlan {
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

        plan.outline_window_max_tokens = context_bounded_source_tokens(
            plan.outline_window_max_tokens,
            context_window,
            capabilities.outline_completion_tokens,
            capabilities.fixed_prompt_reserve_tokens,
            0,
            calibration,
            MIN_CONTEXT_BOUNDED_SOURCE_TOKENS,
        )?;
        plan.generation_window_max_tokens = context_bounded_source_tokens(
            plan.generation_window_max_tokens,
            context_window,
            capabilities.generation_completion_tokens,
            capabilities.fixed_prompt_reserve_tokens,
            plan.max_images_per_window,
            calibration,
            MIN_CONTEXT_BOUNDED_SOURCE_TOKENS,
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
            max_cards_per_window: 10,
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
        max_cards_per_window: 10,
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
            "The selected model's reported context window is too small for safe document generation after reserving prompt, schema, output, and image capacity.",
        ));
    }
    Ok(bounded)
}

pub(crate) fn validate_automatic_job(
    source_unit_count: usize,
    source_tokens: usize,
    plan: DocumentScalePlan,
    outline_windows: usize,
    generation_windows: usize,
    provider_kind: &str,
) -> Result<DocumentBudgetEstimate, CommandError> {
    validate_automatic_job_calibrated(
        source_unit_count,
        source_tokens,
        plan,
        outline_windows,
        generation_windows,
        &DocumentProviderCapabilities::baseline(provider_kind),
        DocumentCostCalibration::default(),
    )
}

pub(crate) fn validate_automatic_job_calibrated(
    source_unit_count: usize,
    source_tokens: usize,
    plan: DocumentScalePlan,
    outline_windows: usize,
    generation_windows: usize,
    capabilities: &DocumentProviderCapabilities,
    calibration: DocumentCostCalibration,
) -> Result<DocumentBudgetEstimate, CommandError> {
    if source_unit_count == 0 || source_unit_count > MAX_SUPPORTED_SOURCE_UNITS {
        return Err(CommandError::new(
            "document_size_unsupported",
            format!(
                "Automatic document generation supports 1–{MAX_SUPPORTED_SOURCE_UNITS} source units per document."
            ),
        ));
    }
    if outline_windows == 0 || generation_windows == 0 {
        return Err(CommandError::new(
            "document_window_plan_invalid",
            "The document did not produce a complete bounded outline and generation plan.",
        ));
    }

    let outline_input = source_tokens
        .saturating_add(outline_windows.saturating_mul(capabilities.fixed_prompt_reserve_tokens));
    let generation_input = source_tokens.saturating_add(
        generation_windows.saturating_mul(capabilities.fixed_prompt_reserve_tokens),
    );
    let calibrated_input = calibration
        .scale_text_tokens(outline_input)
        .saturating_add(calibration.scale_text_tokens(generation_input));
    let outline_output = outline_windows.saturating_mul(capabilities.outline_completion_tokens);
    let generation_output =
        generation_windows.saturating_mul(capabilities.generation_completion_tokens);
    let image_tokens = generation_windows
        .saturating_mul(plan.max_images_per_window)
        .saturating_mul(calibration.image_prompt_tokens);
    let estimated_tokens = calibrated_input
        .saturating_add(outline_output)
        .saturating_add(generation_output)
        .saturating_add(image_tokens);
    Ok(DocumentBudgetEstimate {
        estimated_tokens,
        ceiling_tokens: AggregateTokenCeiling::Windowed,
        generation_card_capacity: generation_windows.saturating_mul(plan.max_cards_per_window),
        context_window_tokens: capabilities.context_window_tokens,
        calibration_sample_count: calibration.sample_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn capabilities(
        provider_kind: &str,
        context_window_tokens: Option<usize>,
    ) -> DocumentProviderCapabilities {
        DocumentProviderCapabilities {
            context_window_tokens,
            ..DocumentProviderCapabilities::baseline(provider_kind)
        }
    }

    #[test]
    fn fifty_units_keep_the_standard_plan() {
        let plan = DocumentScalePlan::for_provider(50, "llama_cpp");
        assert_eq!(plan.class, DocumentScaleClass::Standard);
        assert_eq!(plan.partition_target_units, 24);
        assert_eq!(plan.partition_max_units, 32);
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
            assert_eq!(plan.max_cards_per_window, 10);
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
    fn source_unit_limit_remains_a_provider_neutral_ingestion_bound() {
        for provider in ["llama_cpp", "deepseek", "openrouter"] {
            let plan = DocumentScalePlan::for_provider(MAX_SUPPORTED_SOURCE_UNITS + 1, provider);
            let error = validate_automatic_job(
                MAX_SUPPORTED_SOURCE_UNITS + 1,
                10_000,
                plan,
                1,
                1,
                provider,
            )
            .unwrap_err();
            assert_eq!(error.code, "document_size_unsupported");
        }
    }

    #[test]
    fn aggregate_source_tokens_do_not_create_provider_specific_rejections() {
        for provider in ["llama_cpp", "deepseek", "openrouter"] {
            let plan = DocumentScalePlan::for_provider(100, provider);
            let accepted = validate_automatic_job(100, usize::MAX / 4, plan, 20, 20, provider)
                .expect("aggregate source tokens are diagnostic only");
            assert_eq!(accepted.ceiling_tokens, AggregateTokenCeiling::Windowed);
        }
    }

    #[test]
    fn openrouter_55_page_job_above_old_650k_guard_is_allowed() {
        let plan = DocumentScalePlan::for_provider(55, "openrouter");
        let accepted = validate_automatic_job(55, 560_000, plan, 7, 9, "openrouter")
            .expect("OpenRouter uses bounded source windows, not whole-job rejection");
        assert!(accepted.estimated_tokens > 650_000);
        assert_eq!(accepted.ceiling_tokens, AggregateTokenCeiling::Windowed);
    }
}
