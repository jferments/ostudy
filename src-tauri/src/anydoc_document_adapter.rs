//! AnyDoc-backed adapters for non-PDF document families.
//!
//! AnyDoc remains a private parser backend. Its block/inline/asset types are
//! normalized immediately into oStudy-owned canonical source units, blocks,
//! assets, and semantic chunks. PDF is intentionally absent from this module.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fs::{self, File, OpenOptions},
    io::Write,
    path::Path,
};

use anydoc::{
    model::{
        AssetId, Block, CellSlot, Document, ImageSource, Inline, List, NoteKind, Table, TableKind,
    },
    ConvertError, Format,
};
use sha2::{Digest, Sha256};

use crate::{
    document_canonical::{
        CanonicalAsset, CanonicalAssetRole, CanonicalBlock, CanonicalBlockRole, CanonicalDocument,
        CanonicalExtractionReport, CanonicalIngestion, CanonicalSemanticChunk, CanonicalSourceFile,
        CanonicalSourceUnit, DocumentMetadata, CANONICAL_DOCUMENT_SCHEMA_VERSION,
    },
    document_domain::{DocumentFormat, DocumentSourceDescriptor, SourceLocator, SourceUnitKind},
    error::CommandError,
    paths::AppPaths,
    task_cancellation,
};

pub(crate) const ANYDOC_ADAPTER_BACKEND: &str = "anydoc-canonical-adapter-v1";
pub(crate) const ANYDOC_EXTRACTION_BACKEND: &str = "anydoc-0.2.3";

const MAX_ANYDOC_SOURCE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_ANYDOC_ASSET_BYTES: usize = 32 * 1024 * 1024;
const MAX_ANYDOC_RETAINED_ASSET_BYTES: usize = 128 * 1024 * 1024;
const MAX_UNIT_TARGET_CHARS: usize = 60_000;
const MAX_UNIT_BLOCKS: usize = 160;
const MAX_UNIT_TEXT_CHARS: usize = 250_000;
const MAX_BLOCK_TEXT_CHARS: usize = 250_000;
const MAX_HEADING_CHARS: usize = 500;
const MAX_CHUNK_TARGET_CHARS: usize = 24_000;
const MAX_CHUNK_TEXT_CHARS: usize = 1_000_000;
const MAX_ATTRIBUTE_VALUE_CHARS: usize = 2_000;

#[derive(Clone, Debug)]
struct NormalizedBlock {
    role: CanonicalBlockRole,
    text: String,
    heading_level: Option<u8>,
    parent_heading: Option<String>,
    heading_path: Vec<String>,
    asset_ids: Vec<String>,
    attributes: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Default)]
struct UnitDraft {
    title: Option<String>,
    blocks: Vec<NormalizedBlock>,
    text_chars: usize,
}

impl UnitDraft {
    fn push(&mut self, block: NormalizedBlock) {
        if self.title.is_none()
            && matches!(
                block.role,
                CanonicalBlockRole::Title | CanonicalBlockRole::Heading
            )
        {
            self.title = (!block.text.is_empty()).then(|| block.text.clone());
        }
        self.text_chars = self.text_chars.saturating_add(block.text.chars().count());
        self.blocks.push(block);
    }

    fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }
}

#[derive(Default)]
struct InlineProjection {
    text: String,
    asset_ids: Vec<String>,
    external_image_count: usize,
}

pub(crate) fn supports(format: DocumentFormat) -> bool {
    format.is_anydoc()
}

pub(crate) fn expected_format(format: DocumentFormat) -> Result<Format, CommandError> {
    match format {
        DocumentFormat::Doc => Ok(Format::Doc),
        DocumentFormat::Docx => Ok(Format::Docx),
        DocumentFormat::Ppt => Ok(Format::Ppt),
        DocumentFormat::Pptx => Ok(Format::Pptx),
        DocumentFormat::Odt => Ok(Format::Odt),
        DocumentFormat::Odp => Ok(Format::Odp),
        DocumentFormat::Rtf => Ok(Format::Rtf),
        DocumentFormat::Epub => Ok(Format::Epub),
        DocumentFormat::Excel => Ok(Format::Excel),
        DocumentFormat::Ods => Ok(Format::Ods),
        DocumentFormat::Csv => Ok(Format::Csv),
        DocumentFormat::Pdf => Err(CommandError::new(
            "document_adapter_boundary_violation",
            "PDF documents must use oStudy's native PDF adapter.",
        )),
        _ => Err(CommandError::new(
            "document_ingestion_adapter_unavailable",
            "This document format is not handled by the AnyDoc adapter family.",
        )),
    }
}

pub(crate) fn detected_document_format(
    bytes: &[u8],
    filename: &str,
) -> Result<DocumentFormat, CommandError> {
    if bytes.starts_with(b"%PDF-") {
        return Ok(DocumentFormat::Pdf);
    }
    detect_anydoc_format(bytes, filename)
        .ok_or_else(|| {
            CommandError::new(
                "unsupported_document_format",
                "The selected file is not a supported document type.",
            )
        })
        .and_then(map_anydoc_format)
}

pub(crate) fn probe(format: DocumentFormat, source_path: &Path) -> Result<bool, CommandError> {
    let bytes = read_bounded_source(source_path)?;
    let filename = source_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    let detected = detected_document_format(&bytes, filename)?;
    Ok(detected == format)
}

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

    let image_assets = persist_image_assets(paths, document_id, &parsed)?;
    let asset_identity = image_assets
        .iter()
        .enumerate()
        .map(|(index, asset)| (index, asset.asset_id.clone()))
        .collect::<HashMap<_, _>>();
    let normalized = normalize_document_blocks(&parsed, format, &asset_identity);
    let mut units = segment_units(format, normalized);
    if units.is_empty() && !image_assets.is_empty() {
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
        let plain_text = unit_text_projection(unit, &label);
        canonical_units.push(CanonicalSourceUnit::new(
            unit_number,
            unit_kind,
            label,
            unit.title.clone(),
            plain_text,
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

fn detect_anydoc_format(bytes: &[u8], filename: &str) -> Option<Format> {
    Format::from_bytes(bytes).or_else(|| {
        Path::new(filename)
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("csv"))
            .then_some(Format::Csv)
    })
}

fn map_anydoc_format(format: Format) -> Result<DocumentFormat, CommandError> {
    match format {
        Format::Doc => Ok(DocumentFormat::Doc),
        Format::Docx => Ok(DocumentFormat::Docx),
        Format::Odt => Ok(DocumentFormat::Odt),
        Format::Pdf => Ok(DocumentFormat::Pdf),
        Format::Ppt => Ok(DocumentFormat::Ppt),
        Format::Pptx => Ok(DocumentFormat::Pptx),
        Format::Rtf => Ok(DocumentFormat::Rtf),
        Format::Epub => Ok(DocumentFormat::Epub),
        Format::Excel => Ok(DocumentFormat::Excel),
        Format::Ods => Ok(DocumentFormat::Ods),
        Format::Odp => Ok(DocumentFormat::Odp),
        Format::Csv => Ok(DocumentFormat::Csv),
    }
}

fn normalize_document_blocks(
    document: &Document,
    format: DocumentFormat,
    asset_identity: &HashMap<usize, String>,
) -> Vec<NormalizedBlock> {
    let mut output = Vec::new();
    let mut headings = Vec::<String>::new();
    for block in &document.blocks {
        normalize_block(block, format, asset_identity, &mut headings, &mut output, 0);
    }
    for note in &document.notes {
        let mut projection = InlineProjection::default();
        let text = blocks_plain_text(&note.blocks, asset_identity, &mut projection);
        if text.is_empty() && projection.asset_ids.is_empty() {
            continue;
        }
        let mut attributes = BTreeMap::new();
        attributes.insert(
            "noteId".to_string(),
            bounded_text(&note.id, MAX_ATTRIBUTE_VALUE_CHARS),
        );
        output.push(NormalizedBlock {
            role: match note.kind {
                NoteKind::Footnote => CanonicalBlockRole::Footnote,
                NoteKind::Endnote => CanonicalBlockRole::Endnote,
            },
            text,
            heading_level: None,
            parent_heading: headings.last().cloned(),
            heading_path: headings.clone(),
            asset_ids: deduplicate(projection.asset_ids),
            attributes,
        });
    }
    output
}

fn normalize_block(
    block: &Block,
    format: DocumentFormat,
    asset_identity: &HashMap<usize, String>,
    headings: &mut Vec<String>,
    output: &mut Vec<NormalizedBlock>,
    depth: usize,
) {
    match block {
        Block::Heading { level, content, .. } => {
            let projection = project_inlines(content, asset_identity);
            let text = bounded_text(&projection.text, MAX_HEADING_CHARS);
            if text.is_empty() && projection.asset_ids.is_empty() {
                return;
            }
            let level = (*level).clamp(1, 64);
            let parent = headings.get(usize::from(level).saturating_sub(2)).cloned();
            headings.truncate(usize::from(level).saturating_sub(1));
            if !text.is_empty() {
                headings.push(text.clone());
            }
            let mut attributes = BTreeMap::new();
            attributes.insert("headingLevel".to_string(), level.to_string());
            add_external_image_count(&mut attributes, projection.external_image_count);
            output.push(NormalizedBlock {
                role: if output.is_empty() && level == 1 {
                    CanonicalBlockRole::Title
                } else {
                    CanonicalBlockRole::Heading
                },
                text,
                heading_level: Some(level),
                parent_heading: parent,
                heading_path: headings.clone(),
                asset_ids: deduplicate(projection.asset_ids),
                attributes,
            });
        }
        Block::Paragraph(inlines) => {
            let projection = project_inlines(inlines, asset_identity);
            if projection.text.trim().is_empty() && projection.asset_ids.is_empty() {
                return;
            }
            let role = if projection.text.trim().is_empty() && !projection.asset_ids.is_empty() {
                CanonicalBlockRole::Image
            } else {
                CanonicalBlockRole::Paragraph
            };
            let mut attributes = BTreeMap::new();
            add_external_image_count(&mut attributes, projection.external_image_count);
            output.push(NormalizedBlock {
                role,
                text: bounded_text(&projection.text, MAX_BLOCK_TEXT_CHARS),
                heading_level: None,
                parent_heading: headings.last().cloned(),
                heading_path: headings.clone(),
                asset_ids: deduplicate(projection.asset_ids),
                attributes,
            });
        }
        Block::List(list) => normalize_list(list, asset_identity, headings, output, depth),
        Block::Table(table) => {
            let mut projection = InlineProjection::default();
            let text = table_plain_text(table, asset_identity, &mut projection);
            let mut attributes = BTreeMap::new();
            attributes.insert("headerRows".to_string(), table.header_rows.to_string());
            attributes.insert("rowCount".to_string(), table.grid.len().to_string());
            attributes.insert(
                "tableKind".to_string(),
                match table.kind {
                    TableKind::Data => "data",
                    TableKind::Layout => "layout",
                }
                .to_string(),
            );
            add_external_image_count(&mut attributes, projection.external_image_count);
            output.push(NormalizedBlock {
                role: CanonicalBlockRole::Table,
                text: bounded_text(&text, MAX_BLOCK_TEXT_CHARS),
                heading_level: None,
                parent_heading: headings.last().cloned(),
                heading_path: headings.clone(),
                asset_ids: deduplicate(projection.asset_ids),
                attributes,
            });
        }
        Block::BlockQuote(blocks) => {
            let mut projection = InlineProjection::default();
            let text = blocks_plain_text(blocks, asset_identity, &mut projection);
            if text.is_empty() && projection.asset_ids.is_empty() {
                return;
            }
            output.push(NormalizedBlock {
                role: if matches!(
                    format,
                    DocumentFormat::Ppt | DocumentFormat::Pptx | DocumentFormat::Odp
                ) {
                    CanonicalBlockRole::SpeakerNote
                } else {
                    CanonicalBlockRole::Quote
                },
                text: bounded_text(&text, MAX_BLOCK_TEXT_CHARS),
                heading_level: None,
                parent_heading: headings.last().cloned(),
                heading_path: headings.clone(),
                asset_ids: deduplicate(projection.asset_ids),
                attributes: BTreeMap::new(),
            });
        }
        Block::CodeBlock { lang, text } => {
            let mut attributes = BTreeMap::new();
            if let Some(language) = lang {
                let language = bounded_text(language, MAX_ATTRIBUTE_VALUE_CHARS);
                if !language.is_empty() {
                    attributes.insert("language".to_string(), language);
                }
            }
            output.push(NormalizedBlock {
                role: CanonicalBlockRole::CodeBlock,
                text: bounded_text(text, MAX_BLOCK_TEXT_CHARS),
                heading_level: None,
                parent_heading: headings.last().cloned(),
                heading_path: headings.clone(),
                asset_ids: Vec::new(),
                attributes,
            });
        }
        Block::Math(tex) => output.push(NormalizedBlock {
            role: CanonicalBlockRole::Equation,
            text: bounded_text(tex, MAX_BLOCK_TEXT_CHARS),
            heading_level: None,
            parent_heading: headings.last().cloned(),
            heading_path: headings.clone(),
            asset_ids: Vec::new(),
            attributes: BTreeMap::new(),
        }),
        Block::Rule => {}
    }
}

fn normalize_list(
    list: &List,
    asset_identity: &HashMap<usize, String>,
    headings: &mut Vec<String>,
    output: &mut Vec<NormalizedBlock>,
    depth: usize,
) {
    for (index, item) in list.items.iter().enumerate() {
        let mut projection = InlineProjection::default();
        let text = blocks_plain_text(&item.blocks, asset_identity, &mut projection);
        if text.is_empty() && projection.asset_ids.is_empty() {
            continue;
        }
        let ordinal = list.start.saturating_add(index as u64);
        let marker = item
            .marker_label
            .clone()
            .unwrap_or_else(|| list.marker.label(ordinal));
        let mut attributes = BTreeMap::new();
        attributes.insert(
            "listMarker".to_string(),
            bounded_text(&marker, MAX_ATTRIBUTE_VALUE_CHARS),
        );
        attributes.insert("listDepth".to_string(), depth.to_string());
        attributes.insert("ordered".to_string(), list.ordered().to_string());
        add_external_image_count(&mut attributes, projection.external_image_count);
        output.push(NormalizedBlock {
            role: CanonicalBlockRole::ListItem,
            text: bounded_text(&text, MAX_BLOCK_TEXT_CHARS),
            heading_level: None,
            parent_heading: headings.last().cloned(),
            heading_path: headings.clone(),
            asset_ids: deduplicate(projection.asset_ids),
            attributes,
        });
        for nested in &item.blocks {
            if let Block::List(nested_list) = nested {
                normalize_list(
                    nested_list,
                    asset_identity,
                    headings,
                    output,
                    depth.saturating_add(1),
                );
            }
        }
    }
}

fn project_inlines(
    inlines: &[Inline],
    asset_identity: &HashMap<usize, String>,
) -> InlineProjection {
    let mut projection = InlineProjection::default();
    project_inline_slice(inlines, asset_identity, &mut projection);
    projection.text = bounded_text(&projection.text, MAX_BLOCK_TEXT_CHARS);
    projection
}

fn project_inline_slice(
    inlines: &[Inline],
    asset_identity: &HashMap<usize, String>,
    projection: &mut InlineProjection,
) {
    for inline in inlines {
        match inline {
            Inline::Text { text, .. } => projection.text.push_str(text),
            Inline::Link { content, .. } => {
                project_inline_slice(content, asset_identity, projection)
            }
            Inline::Image { alt, source } => {
                if !alt.trim().is_empty() {
                    projection.text.push_str(alt);
                }
                match source {
                    ImageSource::Asset(AssetId(index)) => {
                        if let Some(asset_id) = asset_identity.get(index) {
                            projection.asset_ids.push(asset_id.clone());
                        }
                    }
                    ImageSource::External(_) => {
                        projection.external_image_count =
                            projection.external_image_count.saturating_add(1);
                    }
                    ImageSource::Unavailable => {}
                }
            }
            Inline::Anchor(_) | Inline::NoteRef(_) => {}
            Inline::LineBreak => projection.text.push('\n'),
            Inline::Math(tex) => {
                projection.text.push('$');
                projection.text.push_str(tex);
                projection.text.push('$');
            }
            Inline::Checkbox(checked) => {
                projection
                    .text
                    .push_str(if *checked { "[x]" } else { "[ ]" })
            }
        }
    }
}

fn blocks_plain_text(
    blocks: &[Block],
    asset_identity: &HashMap<usize, String>,
    projection: &mut InlineProjection,
) -> String {
    let mut parts = Vec::new();
    for block in blocks {
        match block {
            Block::Heading { content, .. } | Block::Paragraph(content) => {
                let before = projection.text.len();
                project_inline_slice(content, asset_identity, projection);
                let projected = projection.text[before..].trim().to_string();
                projection.text.truncate(before);
                if !projected.is_empty() {
                    parts.push(projected);
                }
            }
            Block::List(list) => {
                for (index, item) in list.items.iter().enumerate() {
                    let item_text = blocks_plain_text(&item.blocks, asset_identity, projection);
                    if !item_text.is_empty() {
                        let marker = item.marker_label.clone().unwrap_or_else(|| {
                            list.marker.label(list.start.saturating_add(index as u64))
                        });
                        parts.push(format!("{marker} {item_text}"));
                    }
                }
            }
            Block::Table(table) => {
                let text = table_plain_text(table, asset_identity, projection);
                if !text.is_empty() {
                    parts.push(text);
                }
            }
            Block::BlockQuote(nested) => {
                let text = blocks_plain_text(nested, asset_identity, projection);
                if !text.is_empty() {
                    parts.push(text);
                }
            }
            Block::CodeBlock { text, .. } | Block::Math(text) => {
                if !text.trim().is_empty() {
                    parts.push(text.trim().to_string());
                }
            }
            Block::Rule => {}
        }
    }
    bounded_text(&parts.join("\n"), MAX_BLOCK_TEXT_CHARS)
}

fn table_plain_text(
    table: &Table,
    asset_identity: &HashMap<usize, String>,
    projection: &mut InlineProjection,
) -> String {
    let rows = table
        .grid
        .iter()
        .map(|row| {
            row.iter()
                .map(|slot| match slot {
                    CellSlot::Origin(cell) => {
                        blocks_plain_text(&cell.blocks, asset_identity, projection)
                    }
                    CellSlot::Covered { .. } => String::new(),
                })
                .collect::<Vec<_>>()
                .join("\t")
        })
        .collect::<Vec<_>>();
    bounded_text(&rows.join("\n"), MAX_BLOCK_TEXT_CHARS)
}

fn segment_units(format: DocumentFormat, blocks: Vec<NormalizedBlock>) -> Vec<UnitDraft> {
    let mut units = Vec::<UnitDraft>::new();
    let mut current = UnitDraft::default();
    for block in blocks {
        let heading_boundary = block.heading_level == Some(1) && !current.is_empty();
        let table_boundary = matches!(
            format,
            DocumentFormat::Excel | DocumentFormat::Ods | DocumentFormat::Csv
        ) && block.role == CanonicalBlockRole::Table
            && current
                .blocks
                .iter()
                .any(|existing| existing.role == CanonicalBlockRole::Table);
        let capacity_boundary = !current.is_empty()
            && (current.blocks.len() >= MAX_UNIT_BLOCKS
                || current
                    .text_chars
                    .saturating_add(block.text.chars().count())
                    > MAX_UNIT_TARGET_CHARS);
        if heading_boundary || table_boundary || capacity_boundary {
            units.push(current);
            current = UnitDraft::default();
        }
        current.push(block);
    }
    if !current.is_empty() {
        units.push(current);
    }
    units
}

fn unit_text_projection(unit: &UnitDraft, label: &str) -> String {
    let text = unit
        .blocks
        .iter()
        .map(|block| block.text.trim())
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    if text.is_empty() && unit.blocks.iter().any(|block| !block.asset_ids.is_empty()) {
        format!(
            "{label} contains embedded visual material with no extractable text. Use the attached source assets as evidence and do not infer unsupported details."
        )
    } else {
        bounded_text(&text, MAX_UNIT_TEXT_CHARS)
    }
}

fn build_semantic_chunks(
    unit_kind: SourceUnitKind,
    units: &[CanonicalSourceUnit],
    blocks: &[CanonicalBlock],
) -> Result<Vec<CanonicalSemanticChunk>, CommandError> {
    let mut chunks = Vec::new();
    for unit in units {
        let unit_blocks = blocks
            .iter()
            .filter(|block| block.source_unit_number == unit.unit_number)
            .collect::<Vec<_>>();
        let mut selected = Vec::<&CanonicalBlock>::new();
        let mut selected_chars = 0_usize;
        for block in unit_blocks {
            let heading_boundary =
                block.role == CanonicalBlockRole::Heading && !selected.is_empty();
            let capacity_boundary = !selected.is_empty()
                && selected_chars.saturating_add(block.text.chars().count())
                    > MAX_CHUNK_TARGET_CHARS;
            if heading_boundary || capacity_boundary {
                push_chunk(&mut chunks, unit_kind, unit, &selected)?;
                selected.clear();
                selected_chars = 0;
            }
            selected_chars = selected_chars.saturating_add(block.text.chars().count());
            selected.push(block);
        }
        if !selected.is_empty() {
            push_chunk(&mut chunks, unit_kind, unit, &selected)?;
        }
    }
    Ok(chunks)
}

fn push_chunk(
    chunks: &mut Vec<CanonicalSemanticChunk>,
    unit_kind: SourceUnitKind,
    unit: &CanonicalSourceUnit,
    blocks: &[&CanonicalBlock],
) -> Result<(), CommandError> {
    let asset_ids = blocks
        .iter()
        .flat_map(|block| block.asset_ids.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let element_types = blocks
        .iter()
        .map(|block| block.role.as_str().to_string())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut full_text = blocks
        .iter()
        .map(|block| block.text.trim())
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    if full_text.is_empty() && !asset_ids.is_empty() {
        full_text = format!(
            "{} contains embedded visual material with no extractable text. Use the attached source assets as evidence and do not infer unsupported details.",
            unit.label
        );
    }
    if full_text.is_empty() {
        return Ok(());
    }
    full_text = bounded_text(&full_text, MAX_CHUNK_TEXT_CHARS);
    let heading_context = blocks
        .iter()
        .rev()
        .find_map(|block| block.heading_path.last().cloned())
        .or_else(|| unit.title.clone());
    let locator = SourceLocator::with_evidence(
        unit_kind,
        vec![unit.unit_number],
        heading_context.clone().or_else(|| Some(unit.label.clone())),
        asset_ids,
        None,
    )?;
    let token_estimate = full_text.chars().count().div_ceil(4).max(1);
    chunks.push(CanonicalSemanticChunk::new(
        i64::try_from(chunks.len()).unwrap_or(i64::MAX),
        full_text.clone(),
        full_text,
        locator,
        element_types,
        heading_context,
        token_estimate,
        false,
    ));
    Ok(())
}

fn persist_image_assets(
    paths: &AppPaths,
    document_id: &str,
    document: &Document,
) -> Result<Vec<CanonicalAsset>, CommandError> {
    let directory = paths.document_assets_dir(document_id);
    if directory.exists() {
        fs::remove_dir_all(&directory).map_err(CommandError::io)?;
    }
    fs::create_dir_all(&directory).map_err(CommandError::io)?;
    let mut total_bytes = 0_usize;
    let mut assets = Vec::new();
    for asset in &document.assets {
        if !asset.media_type.starts_with("image/") || asset.bytes.is_empty() {
            continue;
        }
        if asset.bytes.len() > MAX_ANYDOC_ASSET_BYTES {
            return Err(CommandError::new(
                "document_asset_too_large",
                "An embedded document image exceeds oStudy's 32 MB asset limit.",
            ));
        }
        total_bytes = total_bytes.saturating_add(asset.bytes.len());
        if total_bytes > MAX_ANYDOC_RETAINED_ASSET_BYTES {
            return Err(CommandError::new(
                "document_assets_too_large",
                "The document's retained embedded images exceed oStudy's 128 MB limit.",
            ));
        }
        let ordinal = assets.len() + 1;
        let extension = image_extension(&asset.media_type);
        let relative_path = AppPaths::document_asset_relative_path(document_id, ordinal, extension);
        let destination = paths.resolve_relative(&relative_path);
        write_replacing_file(&destination, &asset.bytes)?;
        assets.push(CanonicalAsset {
            asset_id: format!("anydoc-asset:{ordinal:06}"),
            source_unit_number: None,
            block_index: None,
            role: CanonicalAssetRole::EmbeddedImage,
            media_type: bounded_text(&asset.media_type, 160),
            relative_path,
            byte_size: i64::try_from(asset.bytes.len()).unwrap_or(i64::MAX),
            sha256: sha256_hex(&asset.bytes),
            width: None,
            height: None,
            alt_text: None,
        });
    }
    Ok(assets)
}

fn read_bounded_source(path: &Path) -> Result<Vec<u8>, CommandError> {
    let metadata = fs::metadata(path).map_err(CommandError::io)?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > MAX_ANYDOC_SOURCE_BYTES {
        return Err(CommandError::new(
            "invalid_document_size",
            "The document is empty or exceeds oStudy's 512 MB source limit.",
        ));
    }
    fs::read(path).map_err(CommandError::io)
}

fn map_anydoc_error(error: ConvertError) -> CommandError {
    let detail = bounded_text(&error.to_string(), 500);
    match error {
        ConvertError::Unsupported(_) => CommandError::new(
            "unsupported_document_format",
            "AnyDoc does not support this document variant.",
        ),
        ConvertError::Malformed { .. } | ConvertError::MissingPart { .. } => CommandError::new(
            "document_malformed",
            format!("The document is malformed or missing required content: {detail}"),
        ),
        ConvertError::Encrypted => CommandError::new(
            "document_encrypted",
            "Encrypted or password-protected documents cannot be processed.",
        ),
        ConvertError::ResourceLimit { .. } => CommandError::new(
            "document_resource_limit",
            format!("The document exceeded a parser safety limit: {detail}"),
        ),
        ConvertError::Io(error) => CommandError::io(error),
        _ => CommandError::new(
            "document_parse_failed",
            format!("The document could not be parsed safely: {detail}"),
        ),
    }
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

fn media_type_for_format(format: DocumentFormat) -> &'static str {
    match format {
        DocumentFormat::Doc => "application/msword",
        DocumentFormat::Docx => {
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
        }
        DocumentFormat::Ppt => "application/vnd.ms-powerpoint",
        DocumentFormat::Pptx => {
            "application/vnd.openxmlformats-officedocument.presentationml.presentation"
        }
        DocumentFormat::Odt => "application/vnd.oasis.opendocument.text",
        DocumentFormat::Odp => "application/vnd.oasis.opendocument.presentation",
        DocumentFormat::Rtf => "application/rtf",
        DocumentFormat::Epub => "application/epub+zip",
        DocumentFormat::Excel => {
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
        }
        DocumentFormat::Ods => "application/vnd.oasis.opendocument.spreadsheet",
        DocumentFormat::Csv => "text/csv",
        _ => "application/octet-stream",
    }
}

fn image_extension(media_type: &str) -> &'static str {
    match media_type.to_ascii_lowercase().as_str() {
        "image/png" => "png",
        "image/jpeg" | "image/jpg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/svg+xml" => "svg",
        "image/bmp" => "bmp",
        "image/tiff" => "tif",
        _ => "bin",
    }
}

fn add_external_image_count(attributes: &mut BTreeMap<String, String>, count: usize) {
    if count > 0 {
        attributes.insert("externalImageReferenceCount".to_string(), count.to_string());
    }
}

fn deduplicate(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
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

fn capitalize(value: &str) -> String {
    let mut characters = value.chars();
    match characters.next() {
        Some(first) => first.to_uppercase().collect::<String>() + characters.as_str(),
        None => String::new(),
    }
}

fn hash_bytes(bytes: &[u8]) -> (i64, String) {
    (
        i64::try_from(bytes.len()).unwrap_or(i64::MAX),
        sha256_hex(bytes),
    )
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn write_replacing_file(destination: &Path, bytes: &[u8]) -> Result<(), CommandError> {
    let parent = destination.parent().ok_or_else(|| {
        CommandError::new(
            "invalid_document_asset_path",
            "The document asset path is invalid.",
        )
    })?;
    fs::create_dir_all(parent).map_err(CommandError::io)?;
    let temporary = destination.with_extension(format!("tmp-{}", std::process::id()));
    let _ = fs::remove_file(&temporary);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(CommandError::io)?;
    file.write_all(bytes).map_err(CommandError::io)?;
    file.sync_all().map_err(CommandError::io)?;
    if destination.exists() {
        fs::remove_file(destination).map_err(CommandError::io)?;
    }
    fs::rename(&temporary, destination).map_err(CommandError::io)?;
    File::open(destination)
        .and_then(|file| file.sync_all())
        .map_err(CommandError::io)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_mapping_never_routes_pdf_through_anydoc() {
        assert_eq!(
            map_anydoc_format(Format::Docx).unwrap(),
            DocumentFormat::Docx
        );
        assert_eq!(
            map_anydoc_format(Format::Excel).unwrap(),
            DocumentFormat::Excel
        );
        assert_eq!(
            expected_format(DocumentFormat::Pdf).unwrap_err().code,
            "document_adapter_boundary_violation"
        );
    }

    #[test]
    fn segmentation_uses_heading_and_capacity_boundaries() {
        let blocks = vec![
            NormalizedBlock {
                role: CanonicalBlockRole::Title,
                text: "First".to_string(),
                heading_level: Some(1),
                parent_heading: None,
                heading_path: vec!["First".to_string()],
                asset_ids: Vec::new(),
                attributes: BTreeMap::new(),
            },
            NormalizedBlock {
                role: CanonicalBlockRole::Paragraph,
                text: "Body".to_string(),
                heading_level: None,
                parent_heading: Some("First".to_string()),
                heading_path: vec!["First".to_string()],
                asset_ids: Vec::new(),
                attributes: BTreeMap::new(),
            },
            NormalizedBlock {
                role: CanonicalBlockRole::Heading,
                text: "Second".to_string(),
                heading_level: Some(1),
                parent_heading: None,
                heading_path: vec!["Second".to_string()],
                asset_ids: Vec::new(),
                attributes: BTreeMap::new(),
            },
        ];
        let units = segment_units(DocumentFormat::Docx, blocks);
        assert_eq!(units.len(), 2);
        assert_eq!(units[0].title.as_deref(), Some("First"));
        assert_eq!(units[1].title.as_deref(), Some("Second"));
    }

    #[test]
    fn csv_is_the_only_extension_authorized_format() {
        assert_eq!(
            detected_document_format(b"name,value\na,1\n", "table.csv").unwrap(),
            DocumentFormat::Csv
        );
        assert_eq!(
            detected_document_format(b"not an OOXML package", "renamed.docx")
                .unwrap_err()
                .code,
            "unsupported_document_format"
        );
        assert_eq!(
            detected_document_format(b"%PDF-1.7", "renamed.docx").unwrap(),
            DocumentFormat::Pdf
        );
    }
}
