use std::{collections::BTreeSet, path::Path};

use crate::{
    pdf_backend::{self, ExtractedDocument, ExtractionRecoveryPlan, RecoveredPageText},
    pdf_render_backend::{self, PdfRenderRuntime},
};

use super::validation::bounded_diagnostic;

pub(crate) fn rescue_alternate_native_text(
    render_runtime: &PdfRenderRuntime,
    document_id: &str,
    source_path: &Path,
    semantic: &mut ExtractedDocument,
    mut recovery_plan: ExtractionRecoveryPlan,
) -> ExtractionRecoveryPlan {
    let candidate_pages = recovery_plan
        .ocr_pages
        .iter()
        .map(|candidate| candidate.page_number)
        .collect::<Vec<_>>();
    let mut native_rescued_pages = BTreeSet::new();
    if !candidate_pages.is_empty() {
        match pdf_render_backend::extract_native_text_pages(
            render_runtime,
            source_path,
            &candidate_pages,
        ) {
            Ok(alternatives) => {
                for (page_number, alternative) in alternatives {
                    let assessment = pdf_backend::assess_text_quality(&alternative);
                    if alternative.trim().is_empty()
                        || pdf_backend::native_page_requires_ocr(&alternative, &assessment)
                    {
                        continue;
                    }
                    if let Some(page) = semantic
                        .pages
                        .iter_mut()
                        .find(|page| page.page_number == page_number)
                    {
                        page.text = alternative;
                        semantic
                            .native_text_failed_pages
                            .retain(|failed| *failed != page_number);
                        native_rescued_pages.insert(page_number);
                        crate::logging::info(
                            "document_pdfium_native_text_rescued",
                            format!("document={document_id} page={page_number}"),
                        );
                    }
                }
            }
            Err(error) => crate::logging::info(
                "document_pdfium_native_text_rescue_unavailable",
                format!(
                    "document={document_id} detail={}",
                    bounded_diagnostic(&error.to_string(), 300)
                ),
            ),
        }
    }
    if native_rescued_pages.is_empty() {
        return recovery_plan;
    }

    recovery_plan = pdf_backend::plan_text_recovery(semantic);
    let still_ocr = recovery_plan
        .ocr_pages
        .iter()
        .map(|candidate| candidate.page_number)
        .collect::<BTreeSet<_>>();
    let mut fallback = recovery_plan
        .native_semantic_fallback_pages
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    fallback.extend(
        native_rescued_pages
            .iter()
            .copied()
            .filter(|page| !still_ocr.contains(page)),
    );
    recovery_plan.native_semantic_fallback_pages = fallback.into_iter().collect();
    recovery_plan
}

pub(crate) fn expand_semantic_recovery_pages(
    document: &ExtractedDocument,
    recovered_pages: &[RecoveredPageText],
    native_fallback_pages: &[usize],
) -> Vec<usize> {
    let mut pages = native_fallback_pages
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    pages.extend(recovered_pages.iter().map(|page| page.page_number));
    if pages.is_empty() {
        return Vec::new();
    }

    loop {
        let before = pages.len();
        for chunk in &document.chunks {
            if chunk.page_numbers.iter().any(|page| pages.contains(page)) {
                pages.extend(chunk.page_numbers.iter().copied());
            }
        }
        if pages.len() == before {
            break;
        }
    }
    pages.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_recovery_expands_across_cross_page_chunks() {
        let semantic = ExtractedDocument {
            page_count: 3,
            pages: Vec::new(),
            elements: Vec::new(),
            chunks: vec![
                pdf_backend::ExtractedSemanticChunk {
                    chunk_index: 0,
                    text: "pages one and two".to_string(),
                    full_text: "pages one and two".to_string(),
                    page_numbers: vec![1, 2],
                    bounding_boxes: Vec::new(),
                    element_types: vec!["paragraph".to_string()],
                    heading_context: None,
                    token_estimate: 4,
                    is_oversized: false,
                },
                pdf_backend::ExtractedSemanticChunk {
                    chunk_index: 1,
                    text: "pages two and three".to_string(),
                    full_text: "pages two and three".to_string(),
                    page_numbers: vec![2, 3],
                    bounding_boxes: Vec::new(),
                    element_types: vec!["paragraph".to_string()],
                    heading_context: None,
                    token_estimate: 4,
                    is_oversized: false,
                },
            ],
            native_text_failed_pages: Vec::new(),
            semantic_analysis_degraded: false,
        };
        let recovered = [RecoveredPageText {
            page_number: 1,
            text: "recovered page one".to_string(),
            confidence: 0.9,
        }];
        assert_eq!(
            expand_semantic_recovery_pages(&semantic, &recovered, &[]),
            vec![1, 2, 3]
        );
    }
}
