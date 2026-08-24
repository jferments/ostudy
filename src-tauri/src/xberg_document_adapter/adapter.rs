use std::{collections::HashMap, path::Path};

use crate::{
    document_canonical::{
        CanonicalBlock, CanonicalBlockRole, CanonicalDocument, CanonicalExtractionReport,
        CanonicalIngestion, CanonicalSourceFile, CanonicalSourceUnit,
        CANONICAL_DOCUMENT_SCHEMA_VERSION,
    },
    document_domain::{DocumentFormat, DocumentSourceDescriptor},
    document_ingestion::{
        canonical::{build_semantic_chunks, unit_text_projection, NormalizedBlock, UnitDraft},
        contracts::{
            DocumentCancellation, IngestionProgress, IngestionProgressSink, IngestionStage,
        },
        support::{normalize_semantic_text, sha256_hex},
    },
    error::CommandError,
    paths::AppPaths,
};

use super::{
    assets::persist_images,
    config::extraction_config,
    detection::{detect_document, read_bounded_source},
    metadata::map_metadata,
    normalization::normalize_document,
    package_security::reject_encrypted_package,
    runtime::extract_one,
    support::{capitalize, media_type_for_format},
    XBERG_ADAPTER_BACKEND, XBERG_EXTRACTION_BACKEND,
};

const MAX_BLOCK_TEXT_CHARS: usize = 250_000;
const MAX_FILE_NAME_CHARS: usize = 255;
const MAX_TITLE_CHARS: usize = 1_000;

#[allow(clippy::too_many_arguments)]
pub(crate) fn extract(
    paths: &AppPaths,
    document_id: &str,
    source_path: &Path,
    format: DocumentFormat,
    maximum_source_units: usize,
    cancellation: &dyn DocumentCancellation,
    progress: &mut dyn IngestionProgressSink,
) -> Result<CanonicalIngestion, CommandError> {
    cancellation.check()?;
    progress.report(IngestionProgress::Stage(IngestionStage::DetectingFormat))?;
    let bytes = read_bounded_source(source_path)?;
    let filename = source_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    let detected = detect_document(&bytes, filename)?;
    if detected.format != format {
        return Err(CommandError::new(
            "document_source_format_mismatch",
            "The managed source bytes do not match the document format recorded by oStudy.",
        ));
    }
    reject_encrypted_package(&bytes, format)?;
    let source_byte_size = i64::try_from(bytes.len()).unwrap_or(i64::MAX);
    let source_sha256 = sha256_hex(&bytes);
    let used_ooxml_path_normalization = detected.used_ooxml_path_normalization();
    let extraction_bytes = detected.prepare_extraction_bytes(bytes)?;

    progress.report(IngestionProgress::Stage(IngestionStage::ExtractingContent))?;
    let extracted = extract_one(
        extraction_bytes,
        detected.media_type.clone(),
        Some(filename.to_string()),
        extraction_config(),
        cancellation,
    )?;
    cancellation.check()?;

    progress.report(IngestionProgress::Stage(IngestionStage::SavingAssets))?;
    let persisted_images = persist_images(paths, document_id, &extracted)?;
    let mut assets = persisted_images.assets;
    let asset_ids = persisted_images.asset_id_by_image_index;
    let skipped_image_count = persisted_images.skipped_image_count;
    cancellation.check()?;

    progress.report(IngestionProgress::Stage(IngestionStage::MappingStructure))?;
    let mut normalized = normalize_document(&extracted, format, &asset_ids)?;
    attach_unreferenced_assets(&mut normalized.units, &assets);
    if normalized.units.is_empty() {
        return Err(CommandError::new(
            "document_has_no_semantic_content",
            "The document did not contain structured text or visual content that oStudy could analyze.",
        ));
    }
    if normalized.units.len() > maximum_source_units {
        return Err(CommandError::new(
            "unsupported_document_unit_count",
            format!(
                "This document produced {} source units; automatic processing supports at most {maximum_source_units}.",
                normalized.units.len()
            ),
        ));
    }

    progress.report(IngestionProgress::Stage(
        IngestionStage::BuildingSourceUnits,
    ))?;
    progress.report(IngestionProgress::Extracting {
        completed_units: 0,
        total_units: normalized.units.len(),
    })?;
    let unit_kind = format.source_unit_kind();
    let mut canonical_units = Vec::with_capacity(normalized.units.len());
    let mut canonical_blocks = Vec::new();
    let mut first_asset_unit = HashMap::<String, i64>::new();
    for (unit_index, unit) in normalized.units.iter().enumerate() {
        let unit_number = i64::try_from(unit_index + 1).unwrap_or(i64::MAX);
        let label = unit.label.clone().unwrap_or_else(|| {
            format!("{} {}", capitalize(unit_kind.singular_label()), unit_number)
        });
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
                normalize_semantic_text(&block.text, MAX_BLOCK_TEXT_CHARS),
                block.parent_heading.clone(),
                block.heading_path.clone(),
                None,
                None,
                block.asset_ids.clone(),
                block.attributes.clone(),
            ));
        }
        progress.report(IngestionProgress::Extracting {
            completed_units: unit_index + 1,
            total_units: normalized.units.len(),
        })?;
        cancellation.check()?;
    }
    for asset in &mut assets {
        asset.source_unit_number = first_asset_unit.get(&asset.asset_id).copied();
    }

    progress.report(IngestionProgress::Stage(
        IngestionStage::BuildingSemanticChunks,
    ))?;
    let semantic_chunks = build_semantic_chunks(unit_kind, &canonical_units, &canonical_blocks)?;
    let original_filename = source_path
        .file_name()
        .and_then(|value| value.to_str())
        .map(|value| normalize_semantic_text(value, MAX_FILE_NAME_CHARS))
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
    let mut metadata = map_metadata(&extracted.metadata, None);
    if metadata.title.is_none() {
        metadata.title = canonical_units
            .iter()
            .find_map(|unit| unit.title.clone())
            .or_else(|| {
                Path::new(&original_filename)
                    .file_stem()
                    .and_then(|value| value.to_str())
                    .map(|value| normalize_semantic_text(value, MAX_TITLE_CHARS))
                    .filter(|value| !value.is_empty())
            });
    }
    metadata.attributes.insert(
        "parserBackend".to_string(),
        XBERG_EXTRACTION_BACKEND.to_string(),
    );
    if used_ooxml_path_normalization {
        metadata.attributes.insert(
            "packagePathNormalization".to_string(),
            "opc_office_document_root_to_xberg_standard_paths".to_string(),
        );
    }
    metadata.attributes.insert(
        "sourceUnitSegmentation".to_string(),
        if normalized.exact_unit_boundaries {
            "xberg_authored_units_preserved_as_sections"
        } else {
            "heading_and_capacity_boundaries"
        }
        .to_string(),
    );
    if normalized.skipped_node_count > 0 {
        metadata.attributes.insert(
            "skippedUnsupportedNodeCount".to_string(),
            normalized.skipped_node_count.to_string(),
        );
    }
    if normalized.external_image_reference_count > 0 {
        metadata.attributes.insert(
            "externalImageReferenceCount".to_string(),
            normalized.external_image_reference_count.to_string(),
        );
    }
    if skipped_image_count > 0 {
        metadata.attributes.insert(
            "skippedImageCount".to_string(),
            skipped_image_count.to_string(),
        );
    }
    if !extracted.processing_warnings.is_empty() {
        metadata.attributes.insert(
            "processingWarningCount".to_string(),
            extracted.processing_warnings.len().to_string(),
        );
    }

    progress.report(IngestionProgress::Stage(IngestionStage::ValidatingResult))?;
    let semantic_analysis_degraded = normalized.skipped_node_count > 0
        || normalized.external_image_reference_count > 0
        || skipped_image_count > 0
        || !extracted.processing_warnings.is_empty()
        || matches!(
            format,
            DocumentFormat::Epub | DocumentFormat::Doc | DocumentFormat::Ppt
        );
    let canonical = CanonicalIngestion {
        document: CanonicalDocument {
            schema_version: CANONICAL_DOCUMENT_SCHEMA_VERSION,
            source: DocumentSourceDescriptor::new(format, canonical_units.len()),
            source_files: vec![CanonicalSourceFile {
                file_ordinal: 1,
                original_filename,
                media_type: if detected.media_type.is_empty() {
                    media_type_for_format(format).to_string()
                } else {
                    detected.media_type
                },
                relative_path,
                byte_size: source_byte_size,
                sha256: source_sha256,
            }],
            metadata,
            units: canonical_units,
            blocks: canonical_blocks,
            assets,
            extraction: CanonicalExtractionReport {
                adapter_backend: XBERG_ADAPTER_BACKEND.to_string(),
                extraction_backend: XBERG_EXTRACTION_BACKEND.to_string(),
                render_backend: None,
                extraction_mode: "native".to_string(),
                semantic_analysis_degraded,
                ocr_unit_count: 0,
                ocr_mean_confidence: None,
                ocr_engine: None,
            },
        },
        semantic_chunks,
    };
    cancellation.check()?;
    canonical.validate()?;
    crate::logging::debug_target(
        "document.adapter.xberg",
        "xberg_canonicalized",
        "Converted a non-PDF document into canonical records without logging source content.",
        serde_json::json!({
            "document": document_id,
            "format": format.as_str(),
            "sourceUnits": canonical.document.units.len(),
            "blocks": canonical.document.blocks.len(),
            "assets": canonical.document.assets.len(),
            "semanticChunks": canonical.semantic_chunks.len(),
            "exactUnitBoundaries": normalized.exact_unit_boundaries,
            "skippedNodes": normalized.skipped_node_count,
            "externalImages": normalized.external_image_reference_count,
            "warnings": extracted.processing_warnings.len(),
        }),
    );
    Ok(canonical)
}

fn attach_unreferenced_assets(
    units: &mut Vec<UnitDraft>,
    assets: &[crate::document_canonical::CanonicalAsset],
) {
    let referenced = units
        .iter()
        .flat_map(|unit| unit.blocks.iter())
        .flat_map(|block| block.asset_ids.iter().cloned())
        .collect::<std::collections::HashSet<_>>();
    let missing = assets
        .iter()
        .filter(|asset| !referenced.contains(&asset.asset_id))
        .map(|asset| asset.asset_id.clone())
        .collect::<Vec<_>>();
    if missing.is_empty() {
        return;
    }
    if units.is_empty() {
        units.push(UnitDraft {
            label: Some("Visual content".to_string()),
            title: Some("Visual content".to_string()),
            ..UnitDraft::default()
        });
    }
    if let Some(first) = units.first_mut() {
        first.push(NormalizedBlock {
            role: CanonicalBlockRole::Image,
            text: String::new(),
            heading_level: None,
            parent_heading: None,
            heading_path: Vec::new(),
            asset_ids: missing,
            attributes: std::collections::BTreeMap::new(),
        });
    }
}
