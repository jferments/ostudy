//! Cost planning mirrors canonical segmentation and semantic batch packing.
//! Counts describe an uncached attempt, not a promise about model tokenization.
use super::{
    adapters::split_text,
    contracts::MAX_SOURCE_CHARS,
    extraction::{EXTRACTION_BATCH_CHARS, EXTRACTION_BATCH_SOURCES},
};
use crate::document_semantics::SemanticChunk;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct SemanticWorkEstimate {
    pub requests: usize,
    pub characters: usize,
    pub images: usize,
}

pub(crate) fn estimate(chunks: &[SemanticChunk], attach_images: bool) -> SemanticWorkEstimate {
    let mut result = SemanticWorkEstimate::default();
    let mut packed_chars = 0usize;
    let mut packed_sources = 0usize;
    for chunk in chunks {
        let parts = split_text(&chunk.full_text, MAX_SOURCE_CHARS);
        let parts = if parts.is_empty() && !chunk.source_locator.asset_ids.is_empty() {
            vec![String::new()]
        } else {
            parts
        };
        let image_groups = chunk.source_locator.asset_ids.len().div_ceil(6).max(1);
        for part in parts {
            let chars = part.chars().count();
            for _ in 0..image_groups {
                result.characters = result.characters.saturating_add(chars);
                if !chunk.source_locator.asset_ids.is_empty() {
                    if packed_sources > 0 {
                        result.requests += 1;
                        packed_sources = 0;
                        packed_chars = 0;
                    }
                    result.requests += 1;
                } else {
                    if packed_sources > 0
                        && (packed_sources == EXTRACTION_BATCH_SOURCES
                            || packed_chars + chars > EXTRACTION_BATCH_CHARS)
                    {
                        result.requests += 1;
                        packed_sources = 0;
                        packed_chars = 0;
                    }
                    packed_sources += 1;
                    packed_chars += chars;
                }
            }
            if attach_images {
                result.images += chunk.source_locator.asset_ids.len();
            }
        }
    }
    if packed_sources > 0 {
        result.requests += 1;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document_domain::{SourceLocator, SourceUnitKind};
    #[test]
    fn packing_counts_the_actual_138_page_semantic_phase() {
        let chunks = (0..138)
            .map(|index| SemanticChunk {
                chunk_index: index,
                text: String::new(),
                full_text: "x".repeat(5000),
                source_locator: SourceLocator::new(SourceUnitKind::Page, vec![index + 1], None)
                    .unwrap(),
                element_types: vec![],
                heading_context: None,
                token_estimate: 1250,
            })
            .collect::<Vec<_>>();
        let work = estimate(&chunks, false);
        assert_eq!(work.requests, 69);
        assert_eq!(work.characters, 690000);
        assert_eq!(work.images, 0);
    }
}
