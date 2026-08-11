//! Native structured PDF extraction boundary.
//!
//! The document subsystem owns storage, limits, hashing, persistence, AI calls,
//! and retry behavior. This module owns PDF interpretation plus extraction-
//! quality assessment. A visually valid PDF may contain a missing, malformed, or
//! intentionally obfuscated font-to-Unicode map; those pages must never be
//! treated as trustworthy model input merely because the parser returned a
//! non-empty string.

use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    fmt,
    io::{Read, Seek},
    path::Path,
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc, Arc,
    },
};

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
const MIN_SUBSTANTIVE_NONSPACE_CHARS: usize = 24;
const MIN_SUBSTANTIVE_ALPHA_CHARS: usize = 8;
const MIN_SUBSTITUTION_ALPHA_CHARS: usize = 48;
const TEXT_EXTRACTION_WORKER_MEMORY_BYTES: u64 = 96 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PdfBackendErrorKind {
    Open,
    PageCount,
    ExtractText,
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
    pub native_text_failed_pages: Vec<usize>,
    pub semantic_analysis_degraded: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct OcrPageCandidate {
    pub page_number: usize,
    pub required: bool,
    pub reasons: Vec<&'static str>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ExtractionRecoveryPlan {
    pub ocr_pages: Vec<OcrPageCandidate>,
    pub native_semantic_fallback_pages: Vec<usize>,
}

#[derive(Clone, Debug)]
pub(crate) struct RecoveredPageText {
    pub page_number: usize,
    pub text: String,
    pub confidence: f64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TextQualityAssessment {
    pub suspicious: bool,
    pub substantive: bool,
    pub reasons: Vec<&'static str>,
}

pub(crate) fn active_backend_id() -> &'static str {
    "oxidize-pdf-4.2.2-rag-quality-ocr-v2"
}

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

    let (pages, native_text_failed_pages) = extract_page_texts(source_path, &document, page_count)?;

    let partitioned = document.partition_with_profile(ExtractionProfile::Rag);
    let (elements, semantic_analysis_degraded) = match partitioned {
        Ok(elements) => (elements, false),
        Err(error) => {
            crate::logging::info(
                "pdf_semantic_analysis_native_fallback",
                format!("detail={}", bounded_text(&error.to_string(), 400)),
            );
            (Vec::new(), true)
        }
    };

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

    let chunks = if elements.is_empty() {
        Vec::new()
    } else {
        let chunker = HybridChunker::default();
        chunker
            .chunk(&elements)
            .iter()
            .enumerate()
            .map(|(chunk_index, hybrid)| RagChunk::from_hybrid_chunk(chunk_index, hybrid))
            .map(convert_chunk)
            .collect::<Vec<_>>()
    };
    let chunks = cover_unstructured_pages(chunks, &pages);

    Ok(ExtractedDocument {
        page_count,
        pages,
        elements: semantic_elements,
        chunks,
        native_text_failed_pages,
        semantic_analysis_degraded,
    })
}

fn extract_page_texts<R: Read + Seek>(
    source_path: &Path,
    canonical_document: &PdfDocument<R>,
    page_count: usize,
) -> Result<(Vec<ExtractedTextPage>, Vec<usize>), PdfBackendError> {
    if page_count == 0 {
        return Ok((Vec::new(), Vec::new()));
    }
    let resource_plan = crate::system_resources::plan_parallel_workers(
        page_count,
        TEXT_EXTRACTION_WORKER_MEMORY_BYTES,
        "OSTUDY_PDF_TEXT_WORKERS",
    );
    let worker_count = resource_plan.worker_count;
    crate::logging::info(
        "pdf_text_extraction_resources",
        format!(
            "pages={page_count} logical_cpus={} available_memory_mb={} working_budget_mb={} per_worker_mb={} workers={worker_count}",
            resource_plan.logical_cpus,
            resource_plan.available_memory_bytes / (1024 * 1024),
            resource_plan.working_memory_budget_bytes / (1024 * 1024),
            resource_plan.per_worker_memory_bytes / (1024 * 1024),
        ),
    );
    let next_index = Arc::new(AtomicUsize::new(0));
    let (sender, receiver) = mpsc::channel::<(usize, Result<String, String>)>();
    let mut results = (0..page_count)
        .map(|_| None::<Result<String, String>>)
        .collect::<Vec<_>>();

    std::thread::scope(|scope| {
        for worker_index in 0..worker_count {
            let sender = sender.clone();
            let next_index = Arc::clone(&next_index);
            scope.spawn(move || {
                let worker_document = match PdfDocument::open(source_path) {
                    Ok(document) => document,
                    Err(error) => {
                        crate::logging::info(
                            "pdf_text_worker_open_failed",
                            format!(
                                "worker={worker_index} detail={}",
                                bounded_text(&error.to_string(), 320)
                            ),
                        );
                        return;
                    }
                };
                loop {
                    let page_index = next_index.fetch_add(1, Ordering::AcqRel);
                    if page_index >= page_count {
                        break;
                    }
                    let page_index_u32 = match u32::try_from(page_index) {
                        Ok(value) => value,
                        Err(_) => {
                            let _ = sender.send((
                                page_index,
                                Err("PDF page index exceeded the supported range.".to_string()),
                            ));
                            continue;
                        }
                    };
                    let result = worker_document
                        .extract_text_from_page(page_index_u32)
                        .map(|extracted| extracted.text)
                        .map_err(|error| error.to_string());
                    if sender.send((page_index, result)).is_err() {
                        break;
                    }
                }
            });
        }
        drop(sender);
        for (page_index, result) in receiver {
            if page_index < results.len() {
                results[page_index] = Some(result);
            }
        }
    });

    let mut pages = Vec::with_capacity(page_count);
    let mut failed_pages = Vec::new();
    for (page_index, worker_result) in results.into_iter().enumerate() {
        let page_number = page_index + 1;
        let result = match worker_result {
            Some(result) => result,
            None => {
                let page_index_u32 = u32::try_from(page_index).map_err(|_| {
                    PdfBackendError::new(
                        PdfBackendErrorKind::ExtractText,
                        Some(page_number),
                        "PDF page index exceeded the supported range.",
                    )
                })?;
                canonical_document
                    .extract_text_from_page(page_index_u32)
                    .map(|extracted| extracted.text)
                    .map_err(|error| error.to_string())
            }
        };
        let text = match result {
            Ok(text) => text,
            Err(detail) => {
                failed_pages.push(page_number);
                crate::logging::info(
                    "pdf_native_text_page_failed",
                    format!("page={page_number} detail={}", bounded_text(&detail, 400)),
                );
                String::new()
            }
        };
        pages.push(ExtractedTextPage { page_number, text });
    }

    crate::logging::info(
        "pdf_text_extraction_parallel",
        format!(
            "pages={page_count} workers={worker_count} failed_pages={}",
            failed_pages.len()
        ),
    );
    Ok((pages, failed_pages))
}

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

        if extraction_failed || raw.suspicious {
            let mut reasons = raw.reasons.clone();
            if extraction_failed {
                reasons.push("native_extraction_failed");
            }
            reasons.sort_unstable();
            reasons.dedup();
            ocr_pages.push(OcrPageCandidate {
                page_number: page.page_number,
                required: true,
                reasons,
            });
            continue;
        }

        if !raw.substantive && !has_good_semantic {
            ocr_pages.push(OcrPageCandidate {
                page_number: page.page_number,
                required: false,
                reasons: vec!["sparse_or_missing_text"],
            });
            continue;
        }

        if structured_is_suspicious || document.semantic_analysis_degraded {
            native_semantic_fallback_pages.insert(page.page_number);
        }
    }

    ExtractionRecoveryPlan {
        ocr_pages,
        native_semantic_fallback_pages: native_semantic_fallback_pages.into_iter().collect(),
    }
}

pub(crate) fn assess_text_quality(text: &str) -> TextQualityAssessment {
    let mut nonspace = 0_usize;
    let mut alphabetic = 0_usize;
    let mut ascii_alphabetic = 0_usize;
    let mut latin1_alphabetic = 0_usize;
    let mut private_use = 0_usize;
    let mut replacement = 0_usize;
    let mut invalid_controls = 0_usize;

    for character in text.chars() {
        if !character.is_whitespace() {
            nonspace = nonspace.saturating_add(1);
        }
        if character.is_alphabetic() {
            alphabetic = alphabetic.saturating_add(1);
            if character.is_ascii_alphabetic() {
                ascii_alphabetic = ascii_alphabetic.saturating_add(1);
            } else if ('\u{00C0}'..='\u{00FF}').contains(&character) {
                latin1_alphabetic = latin1_alphabetic.saturating_add(1);
            }
        }
        if is_private_use(character) {
            private_use = private_use.saturating_add(1);
        }
        if character == '\u{FFFD}' {
            replacement = replacement.saturating_add(1);
        }
        if character.is_control() && !matches!(character, '\n' | '\r' | '\t') {
            invalid_controls = invalid_controls.saturating_add(1);
        }
    }

    let mut reasons = Vec::new();
    if private_use >= 2
        && (nonspace < 50 || private_use.saturating_mul(100) >= nonspace.saturating_mul(1))
    {
        reasons.push("private_use_unicode");
    }
    if replacement >= 3 && replacement.saturating_mul(100) >= nonspace.max(1) {
        reasons.push("unicode_replacement_character");
    }
    if invalid_controls > 0 {
        reasons.push("invalid_control_characters");
    }
    if alphabetic >= MIN_SUBSTITUTION_ALPHA_CHARS
        && latin1_alphabetic.saturating_mul(100) >= alphabetic.saturating_mul(55)
        && ascii_alphabetic.saturating_mul(100) <= alphabetic.saturating_mul(25)
    {
        reasons.push("probable_font_substitution");
    }

    TextQualityAssessment {
        suspicious: !reasons.is_empty(),
        substantive: nonspace >= MIN_SUBSTANTIVE_NONSPACE_CHARS
            && alphabetic >= MIN_SUBSTANTIVE_ALPHA_CHARS,
        reasons,
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

fn is_private_use(character: char) -> bool {
    matches!(
        character as u32,
        0xE000..=0xF8FF | 0xF0000..=0xFFFFD | 0x100000..=0x10FFFD
    )
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
            token_estimate: text.chars().count().div_ceil(4).max(1),
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
    fn active_backend_identifies_quality_checked_oxidize_extraction() {
        assert_eq!(active_backend_id(), "oxidize-pdf-4.2.2-rag-quality-ocr-v2");
    }

    #[test]
    fn text_extraction_declares_a_conservative_per_worker_memory_estimate() {
        assert_eq!(TEXT_EXTRACTION_WORKER_MEMORY_BYTES, 96 * 1024 * 1024);
    }

    #[test]
    fn private_use_font_mapping_is_detected_as_corrupt_text() {
        let assessment = assess_text_quality(
            "Th\u{e01c} purp\u{e01d}\u{e01e}\u{e01c} of HDL\u{e01e} appears to transport cholesterol.",
        );
        assert!(assessment.suspicious);
        assert!(assessment.reasons.contains(&"private_use_unicode"));
    }

    #[test]
    fn latin1_glyph_substitution_is_detected_without_flagging_normal_accents() {
        let corrupt = assess_text_quality(
            "Õâóïáäèå ÖîããóÛó îì ÖèïÝáîãîÛ× èåÝ çôó Õâóïáäèå ëóèïç Õííîäáèçáîå ñðæãáíôóÝ åóØ ÛðáÝóãáåóí",
        );
        assert!(corrupt.suspicious);
        assert!(corrupt.reasons.contains(&"probable_font_substitution"));

        let normal = assess_text_quality(
            "L'étude cardiovasculaire évalue la pression artérielle et l'activité physique régulière.",
        );
        assert!(!normal.suspicious);
    }

    #[test]
    fn isolated_replacement_glyph_does_not_force_ocr_on_readable_native_text() {
        let assessment = assess_text_quality(
            "A readable native text layer may contain one unsupported glyph � without making the page unusable for study.",
        );
        assert!(!assessment.suspicious);
    }

    #[test]
    fn repeated_replacement_glyphs_still_fail_native_text_quality() {
        let assessment = assess_text_quality("Cardiac �� �� �� �� �� physiology text");
        assert!(assessment.suspicious);
        assert!(assessment
            .reasons
            .contains(&"unicode_replacement_character"));
    }

    #[test]
    fn empty_native_page_still_selects_ocr_recovery() {
        let document = base_document("", "");
        let plan = plan_text_recovery(&document);
        assert_eq!(plan.ocr_pages.len(), 1);
        assert_eq!(plan.ocr_pages[0].page_number, 1);
    }

    #[test]
    fn corrupt_semantics_with_good_raw_text_use_native_page_fallback_not_ocr() {
        let document = base_document(
            "The American College of Cardiology published updated clinical guidelines.",
            "Õâóïáäèå ÖîããóÛó îì ÖèïÝáîãîÛ× ñðæãáíôóÝ åóØ ÛðáÝóãáåóí",
        );
        let plan = plan_text_recovery(&document);
        assert!(plan.ocr_pages.is_empty());
        assert_eq!(plan.native_semantic_fallback_pages, vec![1]);
    }

    #[test]
    fn recovered_ocr_text_replaces_corrupt_page_and_semantic_content() {
        let mut document = base_document(
            "Õâóïáäèå ÖîããóÛó îì ÖèïÝáîãîÛ× ñðæãáíôóÝ åóØ ÛðáÝóãáåóí",
            "Õâóïáäèå ÖîããóÛó îì ÖèïÝáîãîÛ× ñðæãáíôóÝ åóØ ÛðáÝóãáåóí",
        );
        let recovered = RecoveredPageText {
            page_number: 1,
            text: "The American College of Cardiology published updated clinical guidelines."
                .to_string(),
            confidence: 0.93,
        };
        apply_text_recovery(&mut document, &[recovered], &[]);
        assert!(document.pages[0].text.starts_with("The American"));
        assert_eq!(document.elements.len(), 1);
        assert_eq!(document.elements[0].element_type, "paragraph");
        assert_eq!(document.chunks[0].element_types, vec!["ocr_text"]);
        assert!(document.chunks[0].full_text.starts_with("The American"));
        assert!(!document.chunks[0].full_text.contains('Õ'));
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
