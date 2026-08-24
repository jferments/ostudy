use std::{
    collections::{BTreeMap, HashMap},
    path::Path,
};

use crate::{
    document_canonical::{
        CanonicalBlock, CanonicalBlockRole, CanonicalDocument, CanonicalExtractionReport,
        CanonicalIngestion, CanonicalSourceFile, CanonicalSourceUnit, DocumentMetadata,
        CANONICAL_DOCUMENT_SCHEMA_VERSION,
    },
    document_domain::{DocumentFormat, DocumentSourceDescriptor},
    error::CommandError,
    paths::AppPaths,
    task_cancellation,
};

use super::{
    assets::persist_image_assets,
    chunking::build_semantic_chunks,
    detection::{detect_anydoc_format, expected_format, read_bounded_source},
    errors::map_anydoc_error,
    model::{NormalizedBlock, UnitDraft},
    normalization::normalize_document_blocks,
    segmentation::{segment_units, unit_text_projection},
    support::{bounded_text, capitalize, hash_bytes, media_type_for_format, MAX_BLOCK_TEXT_CHARS},
    ANYDOC_ADAPTER_BACKEND, ANYDOC_EXTRACTION_BACKEND,
};

pub(crate) fn extract(
    paths: &AppPaths,
    document_id: &str,
    source_path: &Path,
    format: DocumentFormat,
    maximum_source_units: usize,
    mut report_units: impl FnMut(usize, usize) -> Result<(), CommandError>,
) -> Result<CanonicalIngestion, CommandError> {
    ensure_not_canceled(document_id)?;
    let bytes = read_bounded_source(source_path)?;
    let anydoc_format = expected_format(format)?;
    let filename = source_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    if detect_anydoc_format(&bytes, filename) != Some(anydoc_format) {
        return Err(CommandError::new(
            "document_source_format_mismatch",
            "The managed source bytes do not match the document format recorded by oStudy.",
        ));
    }
    let parsed = anydoc::to_document(&bytes, anydoc_format).map_err(map_anydoc_error)?;
    ensure_not_canceled(document_id)?;

    let (image_assets, asset_identity) = persist_image_assets(paths, document_id, &parsed)?;
    let normalized = normalize_document_blocks(&parsed, format, &asset_identity);
    let mut units = segment_units(format, normalized);
    ensure_visual_only_unit(&mut units, &image_assets);
    if units.is_empty() {
        return Err(CommandError::new(
            "document_has_no_semantic_content",
            "The document did not contain structured text or visual content that oStudy could analyze.",
        ));
    }
    if units.len() > maximum_source_units {
        return Err(CommandError::new(
            "unsupported_document_unit_count",
            format!(
                "This document produced {} source units; automatic processing supports at most {maximum_source_units}.",
                units.len()
            ),
        ));
    }
    report_units(0, units.len())?;

    let unit_kind = format.source_unit_kind();
    let mut canonical_units = Vec::with_capacity(units.len());
    let mut canonical_blocks = Vec::new();
    let mut first_asset_unit = HashMap::<String, i64>::new();
    for (unit_index, unit) in units.iter().enumerate() {
        let unit_number = i64::try_from(unit_index + 1).unwrap_or(i64::MAX);
        let label = format!("{} {}", capitalize(unit_kind.singular_label()), unit_number);
        canonical_units.push(CanonicalSourceUnit::new(
            unit_number,
            unit_kind,
            label.clone(),
            unit.title.clone(),
            unit_text_projection(unit, &label),
            1,
        ));
        for (ordinal, block) in unit.blocks.iter().enumerate() {
            for asset_id in &block.asset_ids {
                first_asset_unit
                    .entry(asset_id.clone())
                    .or_insert(unit_number);
            }
            canonical_blocks.push(CanonicalBlock::new(
                i64::try_from(canonical_blocks.len()).unwrap_or(i64::MAX),
                i64::try_from(ordinal).unwrap_or(i64::MAX),
                unit_number,
                block.role,
                bounded_text(&block.text, MAX_BLOCK_TEXT_CHARS),
                block.parent_heading.clone(),
                block.heading_path.clone(),
                None,
                None,
                block.asset_ids.clone(),
                block.attributes.clone(),
            ));
        }
        report_units(unit_index + 1, units.len())?;
        ensure_not_canceled(document_id)?;
    }

    let mut canonical_assets = image_assets;
    for asset in &mut canonical_assets {
        asset.source_unit_number = first_asset_unit.get(&asset.asset_id).copied();
    }
    let semantic_chunks = build_semantic_chunks(unit_kind, &canonical_units, &canonical_blocks)?;
    let (source_byte_size, source_sha256) = hash_bytes(&bytes);
    let original_filename = source_path
        .file_name()
        .and_then(|value| value.to_str())
        .map(|value| bounded_text(value, 255))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| format!("source.{}", format.default_extension()));
    let relative_path = source_path
        .strip_prefix(paths.app_data_dir())
        .ok()
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|| {
            AppPaths::document_source_relative_path_with_extension(
                document_id,
                format.default_extension(),
            )
        });
    let title = canonical_units
        .iter()
        .find_map(|unit| unit.title.clone())
        .or_else(|| {
            Path::new(&original_filename)
                .file_stem()
                .and_then(|value| value.to_str())
                .map(|value| bounded_text(value, 1_000))
                .filter(|value| !value.is_empty())
        });
    let skipped_non_image_assets = parsed.assets.len().saturating_sub(canonical_assets.len());
    let inferred_units = matches!(
        format,
        DocumentFormat::Ppt | DocumentFormat::Pptx | DocumentFormat::Odp | DocumentFormat::Epub
    );
    let mut metadata_attributes = BTreeMap::new();
    metadata_attributes.insert(
        "parserBackend".to_string(),
        ANYDOC_EXTRACTION_BACKEND.to_string(),
    );
    metadata_attributes.insert(
        "sourceUnitSegmentation".to_string(),
        if inferred_units {
            "inferred_from_heading_and_capacity_boundaries"
        } else {
            "heading_and_capacity_boundaries"
        }
        .to_string(),
    );
    if skipped_non_image_assets > 0 {
        metadata_attributes.insert(
            "skippedNonImageAssetCount".to_string(),
            skipped_non_image_assets.to_string(),
        );
    }

    let canonical = CanonicalIngestion {
        document: CanonicalDocument {
            schema_version: CANONICAL_DOCUMENT_SCHEMA_VERSION,
            source: DocumentSourceDescriptor::new(format, canonical_units.len()),
            source_files: vec![CanonicalSourceFile {
                file_ordinal: 1,
                original_filename,
                media_type: media_type_for_format(format).to_string(),
                relative_path,
                byte_size: source_byte_size,
                sha256: source_sha256,
            }],
            metadata: DocumentMetadata {
                title,
                attributes: metadata_attributes,
                ..DocumentMetadata::default()
            },
            units: canonical_units,
            blocks: canonical_blocks,
            assets: canonical_assets,
            extraction: CanonicalExtractionReport {
                adapter_backend: ANYDOC_ADAPTER_BACKEND.to_string(),
                extraction_backend: ANYDOC_EXTRACTION_BACKEND.to_string(),
                render_backend: None,
                extraction_mode: "native".to_string(),
                semantic_analysis_degraded: inferred_units || skipped_non_image_assets > 0,
                ocr_unit_count: 0,
                ocr_mean_confidence: None,
                ocr_engine: None,
            },
        },
        semantic_chunks,
    };
    canonical.validate()?;
    crate::logging::debug_target(
        "document.adapter.anydoc",
        "anydoc_canonicalized",
        "Converted a non-PDF document into canonical records without logging source content.",
        serde_json::json!({
            "document": document_id,
            "format": format.as_str(),
            "sourceUnits": canonical.document.units.len(),
            "blocks": canonical.document.blocks.len(),
            "assets": canonical.document.assets.len(),
            "semanticChunks": canonical.semantic_chunks.len(),
            "inferredUnitBoundaries": inferred_units,
            "skippedNonImageAssets": skipped_non_image_assets,
        }),
    );
    Ok(canonical)
}

fn ensure_visual_only_unit(
    units: &mut Vec<UnitDraft>,
    image_assets: &[crate::document_canonical::CanonicalAsset],
) {
    if !units.is_empty() || image_assets.is_empty() {
        return;
    }
    units.push(UnitDraft {
        title: Some("Visual content".to_string()),
        blocks: vec![NormalizedBlock {
            role: CanonicalBlockRole::Image,
            text: String::new(),
            heading_level: None,
            parent_heading: None,
            heading_path: Vec::new(),
            asset_ids: image_assets
                .iter()
                .map(|asset| asset.asset_id.clone())
                .collect(),
            attributes: BTreeMap::new(),
        }],
        text_chars: 0,
    });
}

fn ensure_not_canceled(document_id: &str) -> Result<(), CommandError> {
    if task_cancellation::is_document_task_canceled(document_id) {
        Err(CommandError::new(
            "ai_request_canceled",
            "The document generation request was canceled.",
        ))
    } else {
        Ok(())
    }
}
