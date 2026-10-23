//! Cost planning mirrors canonical segmentation and semantic batch packing.
//! Counts describe an uncached attempt, not a promise about model tokenization.
use super::{
    adapters::split_text,
    contracts::MAX_SOURCE_CHARS,
    extraction::{
        extraction_complexity_units, EXTRACTION_BATCH_CHARS, EXTRACTION_BATCH_COMPLEXITY_UNITS,
        EXTRACTION_BATCH_SOURCES, MAX_WINDOW_PAGE_IMAGES,
    },
};
use crate::document_semantics::SemanticChunk;
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct SemanticWorkEstimate {
    pub requests: usize,
    pub characters: usize,
    pub images: usize,
}

pub(crate) fn estimate(chunks: &[SemanticChunk], attach_images: bool) -> SemanticWorkEstimate {
    let mut result = SemanticWorkEstimate::default();
    let mut sources = Vec::<(String, &[String])>::new();
    for chunk in chunks {
        let parts = split_text(&chunk.full_text, MAX_SOURCE_CHARS);
        let parts = if parts.is_empty() && !chunk.source_locator.asset_ids.is_empty() {
            vec![String::new()]
        } else {
            parts
        };
        let assets = &chunk.source_locator.asset_ids;
        let image_groups = assets.chunks(6).collect::<Vec<_>>();
        let image_groups = if image_groups.is_empty() {
            vec![&assets[..]]
        } else {
            image_groups
        };
        for part in parts {
            for images in &image_groups {
                sources.push((part.clone(), images));
            }
        }
    }
    // As in `extraction::plan_windows`, a page is shown with the sources that
    // start on it, and a page none starts on with the first that reaches it.
    let starts = sources
        .iter()
        .filter_map(|(_, images)| images.first())
        .collect::<HashSet<_>>();
    let mut placed = HashSet::new();
    let mut window = Window::default();
    for (part, images) in &sources {
        let shown = images
            .iter()
            .enumerate()
            .filter(|(index, image)| {
                *index == 0 || (!starts.contains(image) && placed.insert(*image))
            })
            .map(|(_, image)| image)
            .collect::<Vec<_>>();
        result.characters = result.characters.saturating_add(part.chars().count());
        if !window.admits(part, &shown) {
            window.flush(&mut result, attach_images);
        }
        window.push(part, &shown);
    }
    window.flush(&mut result, attach_images);
    result
}

/// Mirrors `extraction::plan_windows`: neighbouring sources share a window
/// while its text, complexity and distinct shown pages stay within budget. It
/// does not move a split page whole to the next window, so it may count a
/// request more than the planner sends.
#[derive(Default)]
struct Window<'a> {
    sources: usize,
    chars: usize,
    complexity: usize,
    images: Vec<&'a String>,
}

impl<'a> Window<'a> {
    fn new_images(&self, images: &[&'a String]) -> usize {
        images
            .iter()
            .filter(|image| !self.images.contains(image))
            .count()
    }

    fn admits(&self, part: &str, images: &[&'a String]) -> bool {
        let new_images = self.new_images(images);
        self.sources == 0
            || (self.sources < EXTRACTION_BATCH_SOURCES
                && self.images.len() + new_images <= MAX_WINDOW_PAGE_IMAGES
                && self.chars + part.chars().count() <= EXTRACTION_BATCH_CHARS
                && self
                    .complexity
                    .saturating_add(extraction_complexity_units(part, new_images))
                    <= EXTRACTION_BATCH_COMPLEXITY_UNITS)
    }

    fn push(&mut self, part: &str, images: &[&'a String]) {
        let new_images = self.new_images(images);
        self.sources += 1;
        self.chars += part.chars().count();
        self.complexity = self
            .complexity
            .saturating_add(extraction_complexity_units(part, new_images));
        for image in images {
            if !self.images.contains(image) {
                self.images.push(image);
            }
        }
    }

    fn flush(&mut self, result: &mut SemanticWorkEstimate, attach_images: bool) {
        if self.sources > 0 {
            result.requests += 1;
            if attach_images {
                result.images += self.images.len();
            }
        }
        *self = Self::default();
    }
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

    #[test]
    fn chunks_of_one_rendered_page_are_one_request_with_one_image() {
        let chunk = |index: i64, page: i64| SemanticChunk {
            chunk_index: index,
            text: String::new(),
            full_text: "x".repeat(500),
            source_locator: SourceLocator::with_evidence(
                SourceUnitKind::Page,
                vec![page],
                None,
                vec![format!("unit-{page}-render")],
                None,
            )
            .unwrap(),
            element_types: vec![],
            heading_context: None,
            token_estimate: 125,
        };
        let chunks = [chunk(0, 3), chunk(1, 3), chunk(2, 4), chunk(3, 5)];
        let work = estimate(&chunks, true);
        assert_eq!(work.requests, 2);
        assert_eq!(work.images, 3);
    }
}
