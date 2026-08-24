use std::{
    collections::HashSet,
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

use super::contracts::{
    ExtractedDocument, ExtractedSemanticChunk, ExtractedSemanticElement, ExtractedTextPage,
    PdfBackendError, PdfBackendErrorKind, SemanticBBox,
};

const MAX_PARENT_HEADING_CHARS: usize = 1_000;
const MAX_HEADING_PATH_ITEMS: usize = 32;
const MAX_HEADING_PATH_ITEM_CHARS: usize = 400;
const MAX_FONT_NAME_CHARS: usize = 500;
const MAX_CHUNK_BOUNDING_BOXES: usize = 512;
const MAX_CHUNK_ELEMENT_TYPES: usize = 128;
const MAX_CHUNK_ELEMENT_TYPE_CHARS: usize = 80;
const MAX_HEADING_CONTEXT_CHARS: usize = 2_000;
const TEXT_EXTRACTION_WORKER_MEMORY_BYTES: u64 = 96 * 1024 * 1024;

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
        HybridChunker::default()
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

pub(crate) fn cover_unstructured_pages(
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

    #[test]
    fn worker_memory_estimate_remains_conservative() {
        assert_eq!(TEXT_EXTRACTION_WORKER_MEMORY_BYTES, 96 * 1024 * 1024);
    }

    #[test]
    fn unstructured_pages_remain_visual_evidence_chunks() {
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
        assert_eq!(covered[1].element_types, vec!["image"]);
    }

    #[test]
    fn metadata_helpers_bound_and_sanitize_values() {
        assert_eq!(
            bounded_optional_text(Some(&"x".repeat(5_000)), 1_000)
                .unwrap()
                .chars()
                .count(),
            1_000
        );
        let bbox = sanitize_bbox(SemanticBBox {
            x: f64::NAN,
            y: f64::INFINITY,
            width: -4.0,
            height: 12.0,
        });
        assert_eq!(bbox.x, 0.0);
        assert_eq!(bbox.width, 0.0);
        assert_eq!(bbox.height, 12.0);
    }
}
