use std::collections::{BTreeMap, BTreeSet, HashSet};

use super::{
    contracts::{
        ExtractedDocument, ExtractedSemanticChunk, ExtractedSemanticElement,
        ExtractionRecoveryPlan, OcrPageCandidate, RecoveredPageText, SemanticBBox,
    },
    extraction::cover_unstructured_pages,
    quality::{assess_text_quality, native_page_requires_ocr},
};

pub(crate) fn plan_text_recovery(document: &ExtractedDocument) -> ExtractionRecoveryPlan {
    let failed_pages = document
        .native_text_failed_pages
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    let mut semantic_suspicious = BTreeSet::<usize>::new();
    let mut semantic_good = BTreeSet::<usize>::new();

    for element in &document.elements {
        let assessment = assess_text_quality(&element.text);
        if assessment.suspicious {
            semantic_suspicious.insert(element.page_number);
        } else if assessment.substantive {
            semantic_good.insert(element.page_number);
        }
        if element
            .parent_heading
            .as_deref()
            .is_some_and(|value| assess_text_quality(value).suspicious)
            || element
                .heading_path
                .iter()
                .any(|value| assess_text_quality(value).suspicious)
        {
            semantic_suspicious.insert(element.page_number);
        }
    }
    for chunk in &document.chunks {
        let suspicious = assess_text_quality(&chunk.text).suspicious
            || assess_text_quality(&chunk.full_text).suspicious
            || chunk
                .heading_context
                .as_deref()
                .is_some_and(|value| assess_text_quality(value).suspicious);
        if suspicious {
            semantic_suspicious.extend(chunk.page_numbers.iter().copied());
        }
    }

    let mut ocr_pages = Vec::new();
    let mut native_semantic_fallback_pages = BTreeSet::new();
    for page in &document.pages {
        let raw = assess_text_quality(&page.text);
        let extraction_failed = failed_pages.contains(&page.page_number);
        let structured_is_suspicious = semantic_suspicious.contains(&page.page_number);
        let has_good_semantic = semantic_good.contains(&page.page_number);

        if extraction_failed {
            let mut reasons = raw.reasons.clone();
            reasons.push("native_extraction_failed");
            reasons.sort_unstable();
            reasons.dedup();
            ocr_pages.push(OcrPageCandidate {
                page_number: page.page_number,
                required: true,
                reasons,
            });
            continue;
        }
        if page.text.trim().is_empty() && !has_good_semantic {
            ocr_pages.push(OcrPageCandidate {
                page_number: page.page_number,
                required: false,
                reasons: vec!["native_text_empty"],
            });
            continue;
        }
        if native_page_requires_ocr(&page.text, &raw) {
            ocr_pages.push(OcrPageCandidate {
                page_number: page.page_number,
                required: true,
                reasons: raw.reasons.clone(),
            });
            continue;
        }
        if raw.suspicious
            || !raw.substantive
            || structured_is_suspicious
            || document.semantic_analysis_degraded
        {
            native_semantic_fallback_pages.insert(page.page_number);
        }
    }

    ExtractionRecoveryPlan {
        ocr_pages,
        native_semantic_fallback_pages: native_semantic_fallback_pages.into_iter().collect(),
    }
}

pub(crate) fn apply_text_recovery(
    document: &mut ExtractedDocument,
    recovered_pages: &[RecoveredPageText],
    native_semantic_fallback_pages: &[usize],
) {
    let recovered = recovered_pages
        .iter()
        .map(|page| (page.page_number, page))
        .collect::<BTreeMap<_, _>>();
    for page in &mut document.pages {
        if let Some(replacement) = recovered.get(&page.page_number) {
            page.text = replacement.text.clone();
        }
    }

    let mut rebuilt_pages = native_semantic_fallback_pages
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    rebuilt_pages.extend(recovered.keys().copied());
    if rebuilt_pages.is_empty() {
        return;
    }

    document
        .elements
        .retain(|element| !rebuilt_pages.contains(&element.page_number));
    document.chunks.retain(|chunk| {
        !chunk
            .page_numbers
            .iter()
            .any(|page| rebuilt_pages.contains(page))
    });

    for page_number in &rebuilt_pages {
        let Some(page) = document
            .pages
            .iter()
            .find(|page| page.page_number == *page_number)
        else {
            continue;
        };
        let text = page.text.trim();
        if text.is_empty() {
            continue;
        }
        let is_ocr = recovered.contains_key(page_number);
        let confidence = recovered
            .get(page_number)
            .map_or(1.0, |page| page.confidence.clamp(0.0, 1.0));
        let source_kind = if is_ocr {
            "ocr_text"
        } else {
            "page_text_fallback"
        };
        document.elements.push(ExtractedSemanticElement {
            element_index: document.elements.len(),
            page_number: *page_number,
            element_type: "paragraph".to_string(),
            text: text.to_string(),
            parent_heading: None,
            heading_path: Vec::new(),
            bbox: SemanticBBox {
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 0.0,
            },
            confidence,
            font_name: None,
            font_size: None,
            is_bold: false,
            is_italic: false,
        });
        document.chunks.push(ExtractedSemanticChunk {
            chunk_index: document.chunks.len(),
            text: text.to_string(),
            full_text: text.to_string(),
            page_numbers: vec![*page_number],
            bounding_boxes: Vec::new(),
            element_types: vec![source_kind.to_string()],
            heading_context: None,
            token_estimate: text.chars().count().div_ceil(4).max(1),
            is_oversized: false,
        });
    }

    document.elements.sort_by_key(|element| {
        (
            element.page_number,
            element.element_index,
            element.element_type.clone(),
        )
    });
    for (index, element) in document.elements.iter_mut().enumerate() {
        element.element_index = index;
    }
    document.chunks =
        cover_unstructured_pages(std::mem::take(&mut document.chunks), &document.pages);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdf_backend::ExtractedTextPage;

    fn base_document(page_text: &str, semantic_text: &str) -> ExtractedDocument {
        ExtractedDocument {
            page_count: 1,
            pages: vec![ExtractedTextPage {
                page_number: 1,
                text: page_text.to_string(),
            }],
            elements: vec![ExtractedSemanticElement {
                element_index: 0,
                page_number: 1,
                element_type: "paragraph".to_string(),
                text: semantic_text.to_string(),
                parent_heading: None,
                heading_path: Vec::new(),
                bbox: SemanticBBox {
                    x: 0.0,
                    y: 0.0,
                    width: 1.0,
                    height: 1.0,
                },
                confidence: 1.0,
                font_name: None,
                font_size: None,
                is_bold: false,
                is_italic: false,
            }],
            chunks: vec![ExtractedSemanticChunk {
                chunk_index: 0,
                text: semantic_text.to_string(),
                full_text: semantic_text.to_string(),
                page_numbers: vec![1],
                bounding_boxes: Vec::new(),
                element_types: vec!["paragraph".to_string()],
                heading_context: None,
                token_estimate: 20,
                is_oversized: false,
            }],
            native_text_failed_pages: Vec::new(),
            semantic_analysis_degraded: false,
        }
    }

    #[test]
    fn empty_native_page_selects_optional_ocr() {
        let plan = plan_text_recovery(&base_document("", ""));
        assert_eq!(plan.ocr_pages.len(), 1);
        assert!(!plan.ocr_pages[0].required);
    }

    #[test]
    fn corrupt_semantics_with_good_raw_text_use_native_fallback() {
        let document = base_document(
            "The American College of Cardiology published updated clinical guidelines.",
            "Õâóïáäèå ÖîããóÛó îì ÖèïÝáîãîÛ× ñðæãáíôóÝ åóØ ÛðáÝóãáåóí",
        );
        let plan = plan_text_recovery(&document);
        assert!(plan.ocr_pages.is_empty());
        assert_eq!(plan.native_semantic_fallback_pages, vec![1]);
    }

    #[test]
    fn recovered_ocr_text_replaces_semantics() {
        let mut document = base_document("corrupt", "corrupt");
        apply_text_recovery(
            &mut document,
            &[RecoveredPageText {
                page_number: 1,
                text: "Recovered physiology text".to_string(),
                confidence: 0.93,
            }],
            &[],
        );
        assert_eq!(document.chunks[0].element_types, vec!["ocr_text"]);
        assert!(document.chunks[0].full_text.starts_with("Recovered"));
    }
}
