//! Native structured PDF extraction boundary.
//!
//! The document subsystem owns storage, limits, hashing, persistence, AI calls,
//! and retry behavior. This module owns only PDF interpretation. It converts the
//! active parser's types into ostudy-owned page/element/chunk structures so a
//! future extraction engine can be swapped without changing durable document or
//! AI contracts.

use std::{collections::HashSet, fmt, path::Path};

use oxidize_pdf::{
    parser::PdfDocument,
    pipeline::{ExtractionProfile, HybridChunker, RagChunk},
};
use serde::{Deserialize, Serialize};

const MAX_PARENT_HEADING_CHARS: usize = 1_000;
const MAX_HEADING_PATH_ITEMS: usize = 32;
const MAX_HEADING_PATH_ITEM_CHARS: usize = 400;
const MAX_FONT_NAME_CHARS: usize = 500;
const MAX_CHUNK_BOUNDING_BOXES: usize = 512;
const MAX_CHUNK_ELEMENT_TYPES: usize = 128;
const MAX_CHUNK_ELEMENT_TYPE_CHARS: usize = 80;
const MAX_HEADING_CONTEXT_CHARS: usize = 2_000;

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
/// ostudy's Cargo feature set. Pages without extractable structure receive a
/// bounded page-evidence chunk so they remain represented in semantic windows;
/// an image-capable provider can then use the independently rendered page.
///
/// Third-party semantic metadata is normalized at this boundary to the durable
/// database contract. Complex textbooks can contain extremely long heading
/// paths, thousands of geometry fragments, unusual font names, or non-finite
/// numeric metadata. Those are useful hints but must never be able to turn an
/// otherwise valid PDF into an opaque SQLite constraint failure.
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
                parent_heading: bounded_optional_text(
                    metadata.parent_heading.as_deref(),
                    MAX_PARENT_HEADING_CHARS,
                ),
                heading_path: metadata
                    .heading_path
                    .iter()
                    .take(MAX_HEADING_PATH_ITEMS)
                    .map(|value| bounded_text(value, MAX_HEADING_PATH_ITEM_CHARS))
                    .filter(|value| !value.is_empty())
                    .collect(),
                bbox: sanitize_bbox(SemanticBBox {
                    x: bbox.x,
                    y: bbox.y,
                    width: bbox.width,
                    height: bbox.height,
                }),
                confidence: finite_or(metadata.confidence, 0.0).clamp(0.0, 1.0),
                font_name: bounded_optional_text(
                    metadata.font_name.as_deref(),
                    MAX_FONT_NAME_CHARS,
                ),
                font_size: metadata.font_size.filter(|value| value.is_finite()),
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
    let chunks = cover_unstructured_pages(chunks, &pages);

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
            .take(MAX_CHUNK_BOUNDING_BOXES)
            .map(|bbox| {
                sanitize_bbox(SemanticBBox {
                    x: bbox.x,
                    y: bbox.y,
                    width: bbox.width,
                    height: bbox.height,
                })
            })
            .collect(),
        element_types: chunk
            .element_types
            .into_iter()
            .take(MAX_CHUNK_ELEMENT_TYPES)
            .map(|value| bounded_text(&value, MAX_CHUNK_ELEMENT_TYPE_CHARS))
            .filter(|value| !value.is_empty())
            .collect(),
        heading_context: bounded_optional_text(
            chunk.heading_context.as_deref(),
            MAX_HEADING_CONTEXT_CHARS,
        ),
        token_estimate: chunk.token_estimate,
        is_oversized: chunk.is_oversized,
    }
}

fn cover_unstructured_pages(
    mut chunks: Vec<ExtractedSemanticChunk>,
    pages: &[ExtractedTextPage],
) -> Vec<ExtractedSemanticChunk> {
    let covered = chunks
        .iter()
        .flat_map(|chunk| chunk.page_numbers.iter().copied())
        .collect::<HashSet<_>>();

    for page in pages {
        if covered.contains(&page.page_number) {
            continue;
        }
        let extracted = page.text.trim();
        let (text, element_type) = if extracted.is_empty() {
            (
                format!(
                    "Rendered PDF page {} has no extractable structured text. Use an attached page image as source evidence when available; otherwise do not infer page content.",
                    page.page_number
                ),
                "image".to_string(),
            )
        } else {
            (extracted.to_string(), "paragraph".to_string())
        };
        chunks.push(ExtractedSemanticChunk {
            chunk_index: chunks.len(),
            text: text.clone(),
            full_text: text.clone(),
            page_numbers: vec![page.page_number],
            bounding_boxes: Vec::new(),
            element_types: vec![element_type],
            heading_context: None,
            token_estimate: (text.chars().count() / 4).max(1),
            is_oversized: false,
        });
    }

    chunks.sort_by_key(|chunk| {
        (
            chunk.page_numbers.first().copied().unwrap_or(usize::MAX),
            chunk.chunk_index,
        )
    });
    for (index, chunk) in chunks.iter_mut().enumerate() {
        chunk.chunk_index = index;
    }
    chunks
}

fn sanitize_bbox(bbox: SemanticBBox) -> SemanticBBox {
    SemanticBBox {
        x: finite_or(bbox.x, 0.0),
        y: finite_or(bbox.y, 0.0),
        width: finite_or(bbox.width, 0.0).max(0.0),
        height: finite_or(bbox.height, 0.0).max(0.0),
    }
}

fn finite_or(value: f64, fallback: f64) -> f64 {
    if value.is_finite() {
        value
    } else {
        fallback
    }
}

fn bounded_optional_text(value: Option<&str>, maximum: usize) -> Option<String> {
    value
        .map(|value| bounded_text(value, maximum))
        .filter(|value| !value.is_empty())
}

fn bounded_text(value: &str, maximum: usize) -> String {
    value
        .chars()
        .filter(|character| !character.is_control() || matches!(character, '\n' | '\t'))
        .take(maximum)
        .collect::<String>()
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_backend_identifies_structure_aware_oxidize_extraction() {
        assert_eq!(active_backend_id(), "oxidize-pdf-4.2.2-rag");
    }

    #[test]
    fn unstructured_pages_remain_available_as_visual_evidence_chunks() {
        let pages = vec![
            ExtractedTextPage {
                page_number: 1,
                text: "Structured text".to_string(),
            },
            ExtractedTextPage {
                page_number: 2,
                text: String::new(),
            },
        ];
        let chunks = vec![ExtractedSemanticChunk {
            chunk_index: 0,
            text: "Structured text".to_string(),
            full_text: "Structured text".to_string(),
            page_numbers: vec![1],
            bounding_boxes: Vec::new(),
            element_types: vec!["paragraph".to_string()],
            heading_context: None,
            token_estimate: 4,
            is_oversized: false,
        }];

        let covered = cover_unstructured_pages(chunks, &pages);
        assert_eq!(covered.len(), 2);
        assert_eq!(covered[1].page_numbers, vec![2]);
        assert_eq!(covered[1].element_types, vec!["image"]);
        assert!(covered[1].full_text.contains("attached page image"));
    }

    #[test]
    fn semantic_metadata_is_bounded_and_finite_before_sqlite() {
        let bounded = bounded_optional_text(Some(&"x".repeat(5_000)), 1_000).unwrap();
        assert_eq!(bounded.chars().count(), 1_000);
        assert_eq!(finite_or(f64::NAN, 0.0), 0.0);
        assert_eq!(finite_or(f64::INFINITY, 7.0), 7.0);
        let bbox = sanitize_bbox(SemanticBBox {
            x: f64::NAN,
            y: f64::INFINITY,
            width: -4.0,
            height: 12.0,
        });
        assert_eq!(bbox.x, 0.0);
        assert_eq!(bbox.y, 0.0);
        assert_eq!(bbox.width, 0.0);
        assert_eq!(bbox.height, 12.0);
    }
}
