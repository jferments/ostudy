//! Provider-aware scaling policy for managed PDF generation.
//!
//! A model context window is a per-request bound, not a cumulative budget for
//! a whole document. Large documents therefore use semantic partitions,
//! bounded provider windows, durable checkpoints, and an explicit whole-job
//! preflight budget. The policy deliberately does not impose a small global
//! card-count ceiling: card capacity scales with the number of generation
//! windows, while every individual request remains bounded.

use crate::error::CommandError;

pub(crate) const LARGE_DOCUMENT_PAGE_THRESHOLD: usize = 50;
pub(crate) const MAX_SUPPORTED_PDF_PAGES: usize = 300;
pub(crate) const MAX_AUTOMATIC_SOURCE_TOKENS: usize = 250_000;
const DEEPSEEK_JOB_TOKEN_CEILING: usize = 650_000;
const LOCAL_JOB_TOKEN_CEILING: usize = 1_200_000;
const DEFAULT_CLOUD_JOB_TOKEN_CEILING: usize = 650_000;

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
    pub partition_target_pages: usize,
    pub partition_max_pages: usize,
    pub outline_window_max_tokens: usize,
    pub outline_window_max_pages: usize,
    pub outline_window_overlap_tokens: usize,
    pub generation_window_max_tokens: usize,
    pub generation_window_max_pages: usize,
    pub generation_window_overlap_tokens: usize,
    pub max_images_per_window: usize,
    pub max_cards_per_window: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DocumentJobBudget {
    pub estimated_tokens: usize,
    pub ceiling_tokens: usize,
    pub generation_card_capacity: usize,
}

impl DocumentScalePlan {
    /// Compatibility helper for provider-neutral tests and callers. Large
    /// provider jobs should use `for_provider` so cloud and local runtimes can
    /// make different context/overhead tradeoffs.
    #[cfg(test)]
    pub(crate) fn for_page_count(page_count: usize) -> Self {
        Self::for_provider(page_count, "llama_cpp")
    }

    pub(crate) fn for_provider(page_count: usize, provider_kind: &str) -> Self {
        if page_count <= LARGE_DOCUMENT_PAGE_THRESHOLD {
            return Self {
                class: DocumentScaleClass::Standard,
                partition_target_tokens: 16_000,
                partition_max_tokens: 24_000,
                partition_target_pages: 24,
                partition_max_pages: 32,
                outline_window_max_tokens: 3_200,
                outline_window_max_pages: 8,
                outline_window_overlap_tokens: 350,
                generation_window_max_tokens: 5_000,
                generation_window_max_pages: 10,
                generation_window_overlap_tokens: 700,
                max_images_per_window: 4,
                max_cards_per_window: 12,
            };
        }

        if provider_kind == "deepseek" {
            // DeepSeek can safely amortize the repeated strict-tool schema and
            // planning prompt across moderately larger source windows. Card
            // generation remains finer-grained than outline analysis so a
            // 100–200 page chapter can still yield hundreds of cards without
            // one enormous completion.
            Self {
                class: DocumentScaleClass::Large,
                partition_target_tokens: 32_000,
                partition_max_tokens: 48_000,
                partition_target_pages: 48,
                partition_max_pages: 60,
                outline_window_max_tokens: 10_000,
                outline_window_max_pages: 20,
                outline_window_overlap_tokens: 500,
                generation_window_max_tokens: 6_000,
                generation_window_max_pages: 8,
                generation_window_overlap_tokens: 400,
                max_images_per_window: 0,
                max_cards_per_window: 16,
            }
        } else {
            // Local llama.cpp context/KV-cache capacity varies with the server,
            // model and accelerator. Keep local requests conservative, but do
            // not impose a document-wide card cap.
            Self {
                class: DocumentScaleClass::Large,
                partition_target_tokens: 18_000,
                partition_max_tokens: 28_000,
                partition_target_pages: 20,
                partition_max_pages: 30,
                outline_window_max_tokens: 3_200,
                outline_window_max_pages: 8,
                outline_window_overlap_tokens: 300,
                generation_window_max_tokens: 5_000,
                generation_window_max_pages: 10,
                generation_window_overlap_tokens: 500,
                max_images_per_window: 3,
                max_cards_per_window: 16,
            }
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self.class {
            DocumentScaleClass::Standard => "standard",
            DocumentScaleClass::Large => "large",
        }
    }
}

pub(crate) fn validate_automatic_job(
    page_count: usize,
    source_tokens: usize,
    plan: DocumentScalePlan,
    outline_window_count: usize,
    generation_window_count: usize,
    provider_kind: &str,
) -> Result<DocumentJobBudget, CommandError> {
    if page_count > MAX_SUPPORTED_PDF_PAGES {
        return Err(CommandError::new(
            "document_generation_page_limit",
            format!(
                "Automatic flashcard generation supports up to {MAX_SUPPORTED_PDF_PAGES} PDF pages at a time. Split this source at chapter or section boundaries and import the parts separately."
            ),
        ));
    }
    if source_tokens > MAX_AUTOMATIC_SOURCE_TOKENS {
        return Err(CommandError::new(
            "document_generation_source_limit",
            format!(
                "This PDF contains too much extracted source material for one automatic generation job. Split it into chapter or section files so each part stays below about {MAX_AUTOMATIC_SOURCE_TOKENS} estimated source tokens."
            ),
        ));
    }

    let ceiling_tokens = match provider_kind {
        "llama_cpp" => LOCAL_JOB_TOKEN_CEILING,
        "deepseek" => DEEPSEEK_JOB_TOKEN_CEILING,
        _ => DEFAULT_CLOUD_JOB_TOKEN_CEILING,
    };
    let generation_card_capacity = generation_window_count
        .saturating_mul(plan.max_cards_per_window)
        .min(10_000);

    // This is deliberately a conservative transport estimate, not a claim
    // about provider billing/tokenization. It includes source overlap plus a
    // fixed strict-schema/prompt allowance, completion allowance, one compact
    // repair allowance per stage, whole-document synthesis, and bounded final
    // curation. It exists to stop a doomed job before spending another large
    // amount of API traffic.
    let outline_source = source_tokens
        .saturating_add(outline_window_count.saturating_mul(plan.outline_window_overlap_tokens));
    let outline_fixed = outline_window_count.saturating_mul(2_400 + 2_600);
    let generation_source = source_tokens.saturating_add(
        generation_window_count.saturating_mul(plan.generation_window_overlap_tokens),
    );
    let generation_completion_per_window = (1_024_usize
        .saturating_add(plan.max_cards_per_window.saturating_mul(384)))
    .clamp(1_536, 5_120);
    let generation_fixed = generation_window_count
        .saturating_mul(2_800_usize.saturating_add(generation_completion_per_window));
    let compact_repair_reserve = outline_window_count
        .saturating_mul(900)
        .saturating_add(generation_window_count.saturating_mul(1_200));
    let curation_batches = generation_card_capacity.div_ceil(32);
    let curation_reserve = curation_batches.saturating_mul(2_500);
    let global_synthesis_reserve = 12_000;
    let estimated_tokens = outline_source
        .saturating_add(outline_fixed)
        .saturating_add(generation_source)
        .saturating_add(generation_fixed)
        .saturating_add(compact_repair_reserve)
        .saturating_add(curation_reserve)
        .saturating_add(global_synthesis_reserve);

    if estimated_tokens > ceiling_tokens {
        return Err(CommandError::new(
            "document_ai_budget_exceeded",
            format!(
                "This document's conservative AI-job estimate is about {estimated_tokens} tokens, above the automatic {ceiling_tokens}-token safety ceiling for the selected provider. Split the PDF at chapter or section boundaries; completed local extraction remains available in Documents."
            ),
        ));
    }

    Ok(DocumentJobBudget {
        estimated_tokens,
        ceiling_tokens,
        generation_card_capacity,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fifty_pages_keeps_the_standard_plan() {
        let plan = DocumentScalePlan::for_provider(50, "deepseek");
        assert_eq!(plan.class, DocumentScaleClass::Standard);
        assert_eq!(plan.generation_window_max_tokens, 5_000);
    }

    #[test]
    fn deepseek_large_documents_use_chapter_scale_partitions_and_many_card_slots() {
        let plan = DocumentScalePlan::for_provider(138, "deepseek");
        assert_eq!(plan.class, DocumentScaleClass::Large);
        assert_eq!(plan.partition_target_pages, 48);
        assert_eq!(plan.partition_max_pages, 60);
        assert_eq!(plan.generation_window_max_pages, 8);
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
        assert_eq!(plan.generation_window_max_pages, 10);
        assert_eq!(plan.max_cards_per_window, 16);
    }

    #[test]
    fn whole_job_budget_fails_before_an_excessive_cloud_run() {
        let plan = DocumentScalePlan::for_provider(250, "deepseek");
        let error = validate_automatic_job(250, 220_000, plan, 20, 40, "deepseek").unwrap_err();
        assert_eq!(error.code, "document_ai_budget_exceeded");
    }

    #[test]
    fn automatic_source_and_page_limits_are_explicit() {
        let plan = DocumentScalePlan::for_provider(301, "deepseek");
        assert_eq!(
            validate_automatic_job(301, 10_000, plan, 1, 1, "deepseek")
                .unwrap_err()
                .code,
            "document_generation_page_limit"
        );
        let plan = DocumentScalePlan::for_provider(100, "deepseek");
        assert_eq!(
            validate_automatic_job(100, MAX_AUTOMATIC_SOURCE_TOKENS + 1, plan, 1, 1, "deepseek")
                .unwrap_err()
                .code,
            "document_generation_source_limit"
        );
    }
}
