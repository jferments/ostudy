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
            || chunk_needs_visual_grounding(&chunk.element_types, &full_text);
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
            "image" | "table" | "key_value" | "equation" | "chart" | "diagram"
        )
    }) || full_text.contains("has no extractable structured text")
}

#[cfg(test)]
mod tests {
    use super::*;

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
