//! Native structured PDF extraction boundary.
//!
//! The document subsystem owns storage, limits, hashing, persistence, AI calls,
//! and retry behavior. This module owns only PDF interpretation. It converts the
//! active parser's types into ostudy-owned page/element/chunk structures so a
//! future extraction engine can be swapped without changing durable document or
//! AI contracts.

use std::{fmt, path::Path};

use oxidize_pdf::{
    parser::PdfDocument,
    pipeline::{ExtractionProfile, HybridChunker, RagChunk},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PdfBackendErrorKind {
    Open,
    PageCount,
    ExtractText,
    SemanticAnalysis,
}

#[derive(Debug)]
pub(crate) struct PdfBackendError {
    pub kind: PdfBackendErrorKind,
    pub page_number: Option<usize>,
    detail: String,
}

impl PdfBackendError {
    fn new(
        kind: PdfBackendErrorKind,
        page_number: Option<usize>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            page_number,
            detail: detail.into(),
        }
    }
}

impl fmt::Display for PdfBackendError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.detail)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SemanticBBox {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Debug)]
pub(crate) struct ExtractedTextPage {
    pub page_number: usize,
    pub text: String,
}

#[derive(Clone, Debug)]
pub(crate) struct ExtractedSemanticElement {
    pub element_index: usize,
    pub page_number: usize,
    pub element_type: String,
    pub text: String,
    pub parent_heading: Option<String>,
    pub heading_path: Vec<String>,
    pub bbox: SemanticBBox,
    pub confidence: f64,
    pub font_name: Option<String>,
    pub font_size: Option<f64>,
    pub is_bold: bool,
    pub is_italic: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct ExtractedSemanticChunk {
    pub chunk_index: usize,
    pub text: String,
    pub full_text: String,
    pub page_numbers: Vec<usize>,
    pub bounding_boxes: Vec<SemanticBBox>,
    pub element_types: Vec<String>,
    pub heading_context: Option<String>,
    pub token_estimate: usize,
    pub is_oversized: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct ExtractedDocument {
    pub page_count: usize,
    pub pages: Vec<ExtractedTextPage>,
    pub elements: Vec<ExtractedSemanticElement>,
    pub chunks: Vec<ExtractedSemanticChunk>,
}

pub(crate) fn active_backend_id() -> &'static str {
    "oxidize-pdf-4.2.2-rag"
}

/// Parse one immutable source PDF into ostudy-owned structured data.
///
/// `ExtractionProfile::Rag` preserves reading order, headings, tables, lists,
/// geometry, and structure-aware chunks. OCR is intentionally not enabled by
/// ostudy's Cargo feature set; image-only pages therefore have empty extracted
/// text but remain available to the independent page renderer and multimodal
/// provider path.
pub(crate) fn extract_document(source_path: &Path) -> Result<ExtractedDocument, PdfBackendError> {
    let document = PdfDocument::open(source_path).map_err(|error| {
        PdfBackendError::new(PdfBackendErrorKind::Open, None, error.to_string())
    })?;
    let page_count_u32 = document.page_count().map_err(|error| {
        PdfBackendError::new(PdfBackendErrorKind::PageCount, None, error.to_string())
    })?;
    let page_count = usize::try_from(page_count_u32).map_err(|_| {
        PdfBackendError::new(
            PdfBackendErrorKind::PageCount,
            None,
            "PDF page count exceeded the supported platform range.",
        )
    })?;

    let mut pages = Vec::with_capacity(page_count);
    for page_index in 0..page_count {
        let page_index_u32 = u32::try_from(page_index).map_err(|_| {
            PdfBackendError::new(
                PdfBackendErrorKind::ExtractText,
                Some(page_index + 1),
                "PDF page index exceeded the supported range.",
            )
        })?;
        let extracted = document
            .extract_text_from_page(page_index_u32)
            .map_err(|error| {
                PdfBackendError::new(
                    PdfBackendErrorKind::ExtractText,
                    Some(page_index + 1),
                    error.to_string(),
                )
            })?;
        pages.push(ExtractedTextPage {
            page_number: page_index + 1,
            text: extracted.text,
        });
    }

    let elements = document
        .partition_with_profile(ExtractionProfile::Rag)
        .map_err(|error| {
            PdfBackendError::new(
                PdfBackendErrorKind::SemanticAnalysis,
                None,
                error.to_string(),
            )
        })?;

    let semantic_elements = elements
        .iter()
        .enumerate()
        .map(|(element_index, element)| {
            let metadata = element.metadata();
            let bbox = element.bbox();
            ExtractedSemanticElement {
                element_index,
                page_number: usize::try_from(element.page())
                    .unwrap_or(usize::MAX)
                    .saturating_add(1),
                element_type: element.type_name().to_string(),
                text: element.display_text(),
                parent_heading: metadata.parent_heading.clone(),
                heading_path: metadata.heading_path.clone(),
                bbox: SemanticBBox {
                    x: bbox.x,
                    y: bbox.y,
                    width: bbox.width,
                    height: bbox.height,
                },
                confidence: metadata.confidence.clamp(0.0, 1.0),
                font_name: metadata.font_name.clone(),
                font_size: metadata.font_size,
                is_bold: metadata.is_bold,
                is_italic: metadata.is_italic,
            }
        })
        .collect::<Vec<_>>();

    // Build chunks from the exact same partitioned elements instead of asking
    // oxidize-pdf to partition the document a second time.
    let chunker = HybridChunker::default();
    let hybrid_chunks = chunker.chunk(&elements);
    let chunks = hybrid_chunks
        .iter()
        .enumerate()
        .map(|(chunk_index, hybrid)| RagChunk::from_hybrid_chunk(chunk_index, hybrid))
        .map(convert_chunk)
        .collect::<Vec<_>>();

    Ok(ExtractedDocument {
        page_count,
        pages,
        elements: semantic_elements,
        chunks,
    })
}

fn convert_chunk(chunk: RagChunk) -> ExtractedSemanticChunk {
    ExtractedSemanticChunk {
        chunk_index: chunk.chunk_index,
        text: chunk.text,
        full_text: chunk.full_text,
        page_numbers: chunk
            .page_numbers
            .into_iter()
            .map(|page| {
                usize::try_from(page)
                    .unwrap_or(usize::MAX)
                    .saturating_add(1)
            })
            .collect(),
        bounding_boxes: chunk
            .bounding_boxes
            .into_iter()
            .map(|bbox| SemanticBBox {
                x: bbox.x,
                y: bbox.y,
                width: bbox.width,
                height: bbox.height,
            })
            .collect(),
        element_types: chunk.element_types,
        heading_context: chunk.heading_context,
        token_estimate: chunk.token_estimate,
        is_oversized: chunk.is_oversized,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_backend_identifies_structure_aware_oxidize_extraction() {
        assert_eq!(active_backend_id(), "oxidize-pdf-4.2.2-rag");
    }
}
