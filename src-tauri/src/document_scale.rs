//! Provider-independent document scaling policy.
//!
//! The model context window is a per-request limit, not a cumulative allowance
//! for the whole PDF. Large documents therefore use more, smaller requests and
//! durable checkpoints rather than allowing one request or one failed final
//! pass to invalidate all earlier work.

pub(crate) const LARGE_DOCUMENT_PAGE_THRESHOLD: usize = 50;
pub(crate) const MAX_SUPPORTED_PDF_PAGES: usize = 300;

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

impl DocumentScalePlan {
    pub(crate) fn for_page_count(page_count: usize) -> Self {
        if page_count > LARGE_DOCUMENT_PAGE_THRESHOLD {
            Self {
                class: DocumentScaleClass::Large,
                partition_target_tokens: 9_000,
                partition_max_tokens: 13_000,
                partition_target_pages: 14,
                partition_max_pages: 20,
                outline_window_max_tokens: 1_800,
                outline_window_max_pages: 4,
                outline_window_overlap_tokens: 160,
                generation_window_max_tokens: 2_800,
                generation_window_max_pages: 6,
                generation_window_overlap_tokens: 300,
                max_images_per_window: 3,
                max_cards_per_window: 8,
            }
        } else {
            Self {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fifty_pages_keeps_the_standard_plan() {
        let plan = DocumentScalePlan::for_page_count(50);
        assert_eq!(plan.class, DocumentScaleClass::Standard);
        assert_eq!(plan.generation_window_max_tokens, 5_000);
    }

    #[test]
    fn large_documents_use_smaller_independent_contexts() {
        let plan = DocumentScalePlan::for_page_count(138);
        assert_eq!(plan.class, DocumentScaleClass::Large);
        assert!(plan.outline_window_max_tokens < 3_200);
        assert!(plan.generation_window_max_tokens < 5_000);
        assert!(plan.partition_max_pages < LARGE_DOCUMENT_PAGE_THRESHOLD);
        assert!(plan.max_cards_per_window <= 8);
    }
}
