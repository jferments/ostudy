use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use crate::{
    document_canonical::{
        unit_render_asset_id, CanonicalAsset, CanonicalAssetRole, CanonicalBlock,
        CanonicalBlockRole, CanonicalDocument, CanonicalExtractionReport, CanonicalIngestion,
        CanonicalSemanticChunk, CanonicalSourceFile, CanonicalSourceUnit, DocumentMetadata,
        NormalizedBoundingBox, CANONICAL_DOCUMENT_SCHEMA_VERSION,
    },
    document_domain::SourceLocator,
    error::CommandError,
    paths::AppPaths,
    pdf_backend::ExtractedSemanticElement,
    pdf_document_ingestion::IngestedDocument,
};

use super::super::{
    contracts::DocumentAdapterIdentity,
    support::{
        format_source_units, hash_file, normalize_optional_semantic_text, normalize_semantic_text,
        to_i64,
    },
};

const MAX_CANONICAL_FILE_NAME_CHARS: usize = 255;
const MAX_CANONICAL_UNIT_TEXT_CHARS: usize = 250_000;
const MAX_CANONICAL_BLOCK_TEXT_CHARS: usize = 250_000;
const MAX_CANONICAL_HEADING_CHARS: usize = 500;
const MAX_CANONICAL_HEADING_ITEMS: usize = 64;
const MAX_CANONICAL_ATTRIBUTE_VALUE_CHARS: usize = 2_000;
const MAX_CANONICAL_CHUNK_TEXT_CHARS: usize = 1_000_000;
const MAX_CANONICAL_CHUNK_FULL_TEXT_CHARS: usize = 1_100_000;
const MAX_CANONICAL_HEADING_CONTEXT_CHARS: usize = 2_000;

pub(crate) fn canonicalize_pdf(
    document_id: &str,
    source_path: &Path,
    identity: DocumentAdapterIdentity,
    processed: &IngestedDocument,
) -> Result<CanonicalIngestion, CommandError> {
    let (source_byte_size, source_sha256) = hash_file(source_path)?;
    let original_filename = source_path
        .file_name()
        .and_then(|value| value.to_str())
        .map(|value| normalize_semantic_text(value, MAX_CANONICAL_FILE_NAME_CHARS))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "source.pdf".to_string());
    let source_file = CanonicalSourceFile {
        file_ordinal: 1,
        original_filename,
        media_type: "application/pdf".to_string(),
        relative_path: AppPaths::document_source_relative_path(document_id),
        byte_size: source_byte_size,
        sha256: source_sha256,
    };

    let units = processed
        .pages
        .iter()
        .map(|page| {
            CanonicalSourceUnit::new(
                page.unit_number,
                processed.source.unit_kind,
                format!("Page {}", page.unit_number),
                None,
                normalize_semantic_text(&page.plain_text, MAX_CANONICAL_UNIT_TEXT_CHARS),
                1,
            )
        })
        .collect::<Vec<_>>();
    let assets = processed
        .pages
        .iter()
        .map(|page| CanonicalAsset {
            asset_id: unit_render_asset_id(page.unit_number),
            source_unit_number: Some(page.unit_number),
            block_index: None,
            role: CanonicalAssetRole::UnitRender,
            media_type: "image/png".to_string(),
            relative_path: page.render_relative_path.clone(),
            byte_size: page.render_byte_size,
            sha256: page.render_sha256.clone(),
            width: None,
            height: None,
            alt_text: None,
        })
        .collect::<Vec<_>>();

    let mut normalized_block_count = 0_usize;
    let mut block_ordinals = BTreeMap::<i64, i64>::new();
    let mut blocks = Vec::with_capacity(processed.semantic.elements.len());
    for (block_index, element) in processed.semantic.elements.iter().enumerate() {
        let source_unit_number = to_i64(element.page_number);
        let block_ordinal = block_ordinals.entry(source_unit_number).or_insert(0);
        let role = CanonicalBlockRole::from_adapter(&element.element_type)?;
        let geometry = NormalizedBoundingBox::new(
            element.bbox.x,
            element.bbox.y,
            element.bbox.width.max(0.0),
            element.bbox.height.max(0.0),
        );
        let confidence = element
            .confidence
            .is_finite()
            .then_some(element.confidence.clamp(0.0, 1.0));
        let mut attributes = BTreeMap::new();
        attributes.insert(
            "nativeElementIndex".to_string(),
            element.element_index.to_string(),
        );
        if let Some(font_name) = &element.font_name {
            let font_name = normalize_semantic_text(font_name, MAX_CANONICAL_ATTRIBUTE_VALUE_CHARS);
            if !font_name.is_empty() {
                attributes.insert("fontName".to_string(), font_name);
            }
        }
        if let Some(font_size) = element.font_size.filter(|value| value.is_finite()) {
            attributes.insert("fontSize".to_string(), font_size.to_string());
        }
        attributes.insert("isBold".to_string(), element.is_bold.to_string());
        attributes.insert("isItalic".to_string(), element.is_italic.to_string());
        let asset_ids = if role.benefits_from_visual_evidence() {
            vec![unit_render_asset_id(source_unit_number)]
        } else {
            Vec::new()
        };
        let text = normalize_semantic_text(&element.text, MAX_CANONICAL_BLOCK_TEXT_CHARS);
        let parent_heading = normalize_optional_semantic_text(
            element.parent_heading.as_deref(),
            MAX_CANONICAL_HEADING_CHARS,
        );
        let heading_path = element
            .heading_path
            .iter()
            .take(MAX_CANONICAL_HEADING_ITEMS)
            .map(|value| normalize_semantic_text(value, MAX_CANONICAL_HEADING_CHARS))
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();
        if text != element.text
            || parent_heading != element.parent_heading
            || heading_path != element.heading_path
        {
            normalized_block_count = normalized_block_count.saturating_add(1);
        }
        blocks.push(CanonicalBlock::new(
            to_i64(block_index),
            *block_ordinal,
            source_unit_number,
            role,
            text,
            parent_heading,
            heading_path,
            geometry,
            confidence,
            asset_ids,
            attributes,
        ));
        *block_ordinal = block_ordinal.saturating_add(1);
    }

    let visual_pages = layout_dependent_pages(&processed.semantic.elements);
    let mut normalized_chunk_count = 0_usize;
    let mut visual_only_chunk_count = 0_usize;
    let mut semantic_chunks = Vec::with_capacity(processed.semantic.chunks.len());
    for (chunk_index, chunk) in processed.semantic.chunks.iter().enumerate() {
        let unit_numbers = chunk
            .page_numbers
            .iter()
            .map(|unit| to_i64(*unit))
            .collect::<Vec<_>>();
        let heading_context = normalize_optional_semantic_text(
            chunk.heading_context.as_deref(),
            MAX_CANONICAL_HEADING_CONTEXT_CHARS,
        );
        let mut text = normalize_semantic_text(&chunk.text, MAX_CANONICAL_CHUNK_TEXT_CHARS);
        let mut full_text =
            normalize_semantic_text(&chunk.full_text, MAX_CANONICAL_CHUNK_FULL_TEXT_CHARS);
        if text.is_empty() && !full_text.is_empty() {
            text = full_text.clone();
        } else if full_text.is_empty() && !text.is_empty() {
            full_text = text.clone();
        }
        let empty_after_normalization = text.is_empty() && full_text.is_empty();
        if empty_after_normalization {
            let label = format_source_units(&unit_numbers);
            full_text = format!(
                "Source {label} contains visual or non-text content with no extractable semantic text. Use the attached source image as evidence and do not infer unsupported details."
            );
            text = full_text.clone();
            visual_only_chunk_count = visual_only_chunk_count.saturating_add(1);
        }
        let needs_visual_grounding = empty_after_normalization
            || chunk_needs_visual_grounding(&chunk.element_types, &full_text)
            || chunk
                .page_numbers
                .iter()
                .any(|page| visual_pages.contains(page));
        let visual_asset_ids = if needs_visual_grounding {
            unit_numbers
                .iter()
                .map(|unit| unit_render_asset_id(*unit))
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let locator = SourceLocator::with_evidence(
            processed.source.unit_kind,
            unit_numbers,
            heading_context.clone(),
            visual_asset_ids,
            None,
        )?;
        if text != chunk.text
            || full_text != chunk.full_text
            || heading_context != chunk.heading_context
        {
            normalized_chunk_count = normalized_chunk_count.saturating_add(1);
        }
        semantic_chunks.push(CanonicalSemanticChunk::new(
            to_i64(chunk_index),
            text,
            full_text,
            locator,
            chunk.element_types.clone(),
            heading_context,
            chunk.token_estimate.max(1),
            chunk.is_oversized,
        ));
    }

    crate::logging::debug_target(
        "document.canonical",
        "canonical_adapter_text_normalized",
        "Normalized parser-owned text before constructing canonical document records.",
        serde_json::json!({
            "document": document_id,
            "normalizedBlocks": normalized_block_count,
            "normalizedChunks": normalized_chunk_count,
            "visualOnlyChunks": visual_only_chunk_count,
        }),
    );

    let canonical = CanonicalIngestion {
        document: CanonicalDocument {
            schema_version: CANONICAL_DOCUMENT_SCHEMA_VERSION,
            source: processed.source.clone(),
            source_files: vec![source_file],
            metadata: DocumentMetadata::default(),
            units,
            blocks,
            assets,
            extraction: CanonicalExtractionReport {
                adapter_backend: identity.adapter_backend.to_string(),
                extraction_backend: processed.extraction_backend.to_string(),
                render_backend: Some(processed.render_backend.to_string()),
                extraction_mode: processed.extraction_mode.as_str().to_string(),
                semantic_analysis_degraded: processed.semantic.semantic_analysis_degraded,
                ocr_unit_count: processed.ocr.page_count,
                ocr_mean_confidence: processed.ocr.mean_confidence,
                ocr_engine: (!processed.ocr.engine.is_empty())
                    .then(|| processed.ocr.engine.clone()),
            },
        },
        semantic_chunks,
    };
    canonical.validate()?;
    Ok(canonical)
}

fn chunk_needs_visual_grounding(element_types: &[String], full_text: &str) -> bool {
    element_types.iter().any(|element_type| {
        matches!(
            element_type.as_str(),
            "image"
                | "table"
                | "key_value"
                | "equation"
                | "chart"
                | "diagram"
                | "page_text_fallback"
                | "ocr_text"
        )
    }) || full_text.contains("has no extractable structured text")
}

// Text extraction often misreads a table or figure as ordinary paragraphs, so
// element types alone miss the pages where the rendered page matters most. A
// page whose text names a table or figure, or whose short blocks line up in a
// grid of at least two rows of three, sends its render with every chunk on it.
const GRID_ROW_TOLERANCE: f64 = 2.0;
const GRID_MIN_COLUMNS: usize = 3;
const GRID_MIN_ROWS: usize = 2;

fn layout_dependent_pages(elements: &[ExtractedSemanticElement]) -> BTreeSet<usize> {
    let mut by_page = BTreeMap::<usize, Vec<&ExtractedSemanticElement>>::new();
    for element in elements {
        by_page
            .entry(element.page_number)
            .or_default()
            .push(element);
    }
    by_page
        .into_iter()
        .filter(|(_, elements)| {
            elements
                .iter()
                .any(|element| names_table_or_figure(&element.text))
                || has_grid_layout(elements)
        })
        .map(|(page, _)| page)
        .collect()
}

fn names_table_or_figure(text: &str) -> bool {
    let label = text.trim_start_matches(|character: char| {
        character.is_whitespace() || matches!(character, '•' | '·' | '*' | '-' | '–')
    });
    [
        "Table", "TABLE", "Figure", "FIGURE", "Fig.", "FIG.", "Chart", "Diagram",
    ]
    .iter()
    .any(|prefix| {
        label.strip_prefix(prefix).is_some_and(|rest| {
            rest.trim_start_matches([' ', '\u{a0}', '\n'])
                .starts_with(|character: char| character.is_ascii_digit())
        })
    })
}

fn has_grid_layout(elements: &[&ExtractedSemanticElement]) -> bool {
    let mut rows = Vec::<Vec<&ExtractedSemanticElement>>::new();
    for element in elements {
        if !(element.bbox.width > 0.0 && element.bbox.height > 0.0) {
            continue;
        }
        let top = element.bbox.y + element.bbox.height;
        match rows
            .iter_mut()
            .find(|row| (row[0].bbox.y + row[0].bbox.height - top).abs() <= GRID_ROW_TOLERANCE)
        {
            Some(row) => row.push(element),
            None => rows.push(vec![element]),
        }
    }
    rows.iter()
        .filter(|row| horizontally_disjoint_count(row) >= GRID_MIN_COLUMNS)
        .count()
        >= GRID_MIN_ROWS
}

fn horizontally_disjoint_count(row: &[&ExtractedSemanticElement]) -> usize {
    let mut spans = row
        .iter()
        .map(|element| (element.bbox.x, element.bbox.x + element.bbox.width))
        .collect::<Vec<_>>();
    spans.sort_by(|left, right| left.1.total_cmp(&right.1));
    let mut count = 0;
    let mut right_edge = f64::NEG_INFINITY;
    for (left, right) in spans {
        if left >= right_edge {
            count += 1;
            right_edge = right;
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    fn element(page: usize, x: f64, y: f64, width: f64, text: &str) -> ExtractedSemanticElement {
        ExtractedSemanticElement {
            element_index: 0,
            page_number: page,
            element_type: "paragraph".to_string(),
            text: text.to_string(),
            parent_heading: None,
            heading_path: Vec::new(),
            bbox: crate::pdf_backend::SemanticBBox {
                x,
                y,
                width,
                height: 20.0,
            },
            confidence: 1.0,
            font_name: None,
            font_size: None,
            is_bold: false,
            is_italic: false,
        }
    }

    #[test]
    fn misread_table_and_captioned_figure_pages_send_their_render() {
        let table_cells = [45.0, 134.0, 241.0, 442.0].into_iter().flat_map(|x| {
            [
                element(3, x, 700.0, 80.0, "cell"),
                element(3, x, 640.0, 80.0, "cell"),
            ]
        });
        let elements = table_cells
            .chain([
                element(
                    5,
                    40.0,
                    500.0,
                    520.0,
                    "\u{2022} Fig. 1.3 The normal trachea.",
                ),
                element(6, 40.0, 700.0, 250.0, "Left column of prose."),
                element(6, 320.0, 700.0, 250.0, "Right column of prose."),
                element(6, 40.0, 600.0, 250.0, "Tables 3 and 4 compare the drugs."),
            ])
            .collect::<Vec<_>>();
        assert_eq!(layout_dependent_pages(&elements), BTreeSet::from([3, 5]));
    }

    #[test]
    fn table_and_figure_labels_need_a_number() {
        assert!(names_table_or_figure("Table 24.5  Major Minerals"));
        assert!(names_table_or_figure(
            "TABLE \n1.1 Structural Characteristics"
        ));
        assert!(names_table_or_figure("Figure 19.3 The heart"));
        assert!(!names_table_or_figure("Table salt, milk, beets"));
        assert!(!names_table_or_figure("Figures show the trend."));
    }

    #[test]
    fn visual_semantic_types_request_grounding() {
        assert!(chunk_needs_visual_grounding(
            &["table".to_string()],
            "values"
        ));
        assert!(chunk_needs_visual_grounding(
            &["paragraph".to_string()],
            "source has no extractable structured text"
        ));
        assert!(!chunk_needs_visual_grounding(
            &["paragraph".to_string()],
            "ordinary text"
        ));
    }
}
