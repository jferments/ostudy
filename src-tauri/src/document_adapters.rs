//! Format-adapter registry and the canonical PDF reference adapter.
//!
//! The existing PDF engine remains responsible for native extraction, bounded
//! rendering, OCR recovery, cancellation, and resource planning. This module is
//! the public ingestion boundary used by the document workflow: it selects an
//! adapter from a verified source format, converts parser-specific output into
//! the app-owned canonical representation, and commits canonical plus temporary
//! PDF compatibility projections in one transaction.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::Read,
    path::Path,
};

use rusqlite::{params, OptionalExtension, TransactionBehavior};
use sha2::{Digest, Sha256};

use crate::{
    database::Database,
    document_canonical::{
        unit_render_asset_id, CanonicalAsset, CanonicalAssetRole, CanonicalBlock,
        CanonicalBlockRole, CanonicalDocument, CanonicalExtractionReport, CanonicalIngestion,
        CanonicalSemanticChunk, CanonicalSourceFile, CanonicalSourceUnit, DocumentMetadata,
        NormalizedBoundingBox, CANONICAL_DOCUMENT_SCHEMA_VERSION,
    },
    document_canonical_store,
    document_domain::{DocumentFormat, SourceLocator},
    error::CommandError,
    paths::AppPaths,
    pdf_document_ingestion::{self, IngestedDocument},
    pdf_render_backend::PdfRenderRuntime,
    util::unix_time_millis,
};

pub(crate) use crate::pdf_document_ingestion::IngestionProgress;

const PDF_ADAPTER_BACKEND: &str = "pdf-canonical-adapter-v1";
const HASH_BUFFER_BYTES: usize = 128 * 1024;
const MAX_CANONICAL_FILE_NAME_CHARS: usize = 255;
const MAX_CANONICAL_TITLE_CHARS: usize = 1_000;
const MAX_CANONICAL_UNIT_TEXT_CHARS: usize = 250_000;
const MAX_CANONICAL_BLOCK_TEXT_CHARS: usize = 250_000;
const MAX_CANONICAL_HEADING_CHARS: usize = 500;
const MAX_CANONICAL_HEADING_ITEMS: usize = 64;
const MAX_CANONICAL_ATTRIBUTE_VALUE_CHARS: usize = 2_000;
const MAX_CANONICAL_CHUNK_TEXT_CHARS: usize = 1_000_000;
const MAX_CANONICAL_CHUNK_FULL_TEXT_CHARS: usize = 1_100_000;
const MAX_CANONICAL_HEADING_CONTEXT_CHARS: usize = 2_000;

#[derive(Debug)]
pub(crate) struct AdapterExtraction {
    canonical: CanonicalIngestion,
    legacy_pdf: IngestedDocument,
}

#[derive(Clone, Copy)]
struct AdapterRequest<'a> {
    paths: &'a AppPaths,
    render_runtime: &'a PdfRenderRuntime,
    document_id: &'a str,
    source_path: &'a Path,
    maximum_source_units: usize,
}

trait DocumentAdapter: Sync {
    fn format(&self) -> DocumentFormat;
    fn backend_id(&self) -> &'static str;
    fn extraction_backend(&self) -> &'static str;
    fn render_backend(&self) -> Option<&'static str>;
    fn probe(&self, source_path: &Path) -> Result<bool, CommandError>;
    fn extract(
        &self,
        request: AdapterRequest<'_>,
        report_progress: &mut dyn FnMut(IngestionProgress) -> Result<(), CommandError>,
    ) -> Result<AdapterExtraction, CommandError>;
}

struct PdfDocumentAdapter;

static PDF_ADAPTER: PdfDocumentAdapter = PdfDocumentAdapter;

impl DocumentAdapter for PdfDocumentAdapter {
    fn format(&self) -> DocumentFormat {
        DocumentFormat::Pdf
    }

    fn backend_id(&self) -> &'static str {
        PDF_ADAPTER_BACKEND
    }

    fn extraction_backend(&self) -> &'static str {
        pdf_document_ingestion::active_pdf_extraction_backend()
    }

    fn render_backend(&self) -> Option<&'static str> {
        Some(pdf_document_ingestion::active_pdf_render_backend())
    }

    fn probe(&self, source_path: &Path) -> Result<bool, CommandError> {
        let mut file = File::open(source_path).map_err(CommandError::io)?;
        let mut signature = [0_u8; 5];
        let read = file.read(&mut signature).map_err(CommandError::io)?;
        Ok(read == signature.len() && signature == *b"%PDF-")
    }

    fn extract(
        &self,
        request: AdapterRequest<'_>,
        report_progress: &mut dyn FnMut(IngestionProgress) -> Result<(), CommandError>,
    ) -> Result<AdapterExtraction, CommandError> {
        let legacy_pdf = pdf_document_ingestion::ingest_pdf(
            request.paths,
            request.render_runtime,
            request.document_id,
            request.source_path,
            request.maximum_source_units,
            report_progress,
        )?;
        let canonical =
            canonicalize_pdf(request.document_id, request.source_path, self, &legacy_pdf)?;
        Ok(AdapterExtraction {
            canonical,
            legacy_pdf,
        })
    }
}

struct DocumentAdapterRegistry;

impl DocumentAdapterRegistry {
    fn select(
        format: DocumentFormat,
        source_path: &Path,
    ) -> Result<&'static dyn DocumentAdapter, CommandError> {
        let adapter: &'static dyn DocumentAdapter = match format {
            DocumentFormat::Pdf => &PDF_ADAPTER,
            _ => {
                return Err(CommandError::new(
                    "document_ingestion_adapter_unavailable",
                    "This ostudy build does not yet include an ingestion adapter for the saved document format.",
                ));
            }
        };
        if adapter.format() != format || !adapter.probe(source_path)? {
            return Err(CommandError::new(
                "document_source_format_mismatch",
                "The managed source bytes do not match the document format recorded by ostudy.",
            ));
        }
        Ok(adapter)
    }
}

pub(crate) fn active_pdf_extraction_backend() -> &'static str {
    PDF_ADAPTER.extraction_backend()
}

pub(crate) fn active_pdf_render_backend() -> &'static str {
    PDF_ADAPTER
        .render_backend()
        .expect("the PDF reference adapter always has a render backend")
}

pub(crate) fn active_adapter_backend(format: DocumentFormat) -> Result<&'static str, CommandError> {
    match format {
        DocumentFormat::Pdf => Ok(PDF_ADAPTER.backend_id()),
        _ => Err(CommandError::new(
            "document_ingestion_adapter_unavailable",
            "This ostudy build does not yet include an ingestion adapter for the saved document format.",
        )),
    }
}

pub(crate) fn ingest_document<F>(
    format: DocumentFormat,
    paths: &AppPaths,
    render_runtime: &PdfRenderRuntime,
    document_id: &str,
    source_path: &Path,
    maximum_source_units: usize,
    mut report_progress: F,
) -> Result<AdapterExtraction, CommandError>
where
    F: FnMut(IngestionProgress) -> Result<(), CommandError>,
{
    let adapter = DocumentAdapterRegistry::select(format, source_path)?;
    adapter.extract(
        AdapterRequest {
            paths,
            render_runtime,
            document_id,
            source_path,
            maximum_source_units,
        },
        &mut report_progress,
    )
}

/// Compatibility entry point retained until the later generic import/IPC stage.
pub(crate) fn ingest_pdf<F>(
    paths: &AppPaths,
    render_runtime: &PdfRenderRuntime,
    document_id: &str,
    source_path: &Path,
    maximum_source_units: usize,
    report_progress: F,
) -> Result<AdapterExtraction, CommandError>
where
    F: FnMut(IngestionProgress) -> Result<(), CommandError>,
{
    ingest_document(
        DocumentFormat::Pdf,
        paths,
        render_runtime,
        document_id,
        source_path,
        maximum_source_units,
        report_progress,
    )
}

pub(crate) fn update_ingestion_progress(
    database: &Database,
    document_id: &str,
    progress: IngestionProgress,
) -> Result<(), CommandError> {
    pdf_document_ingestion::update_ingestion_progress(database, document_id, progress)?;
    let IngestionProgress::Rendering {
        completed_units,
        total_units,
    } = progress
    else {
        return Ok(());
    };
    let total = to_i64(total_units);
    let completed = to_i64(completed_units).min(total);
    database.with_connection(|connection| {
        let changed = connection
            .execute(
                "UPDATE documents
                 SET source_unit_count = ?1,
                     extracted_source_unit_count = ?1,
                     visualized_source_unit_count = ?2
                 WHERE id = ?3 AND lifecycle_state = 'processing'",
                params![total, completed, document_id],
            )
            .map_err(|error| {
                CommandError::database_at(
                    "canonical_document_progress_failed",
                    format!("document={document_id} completed={completed} total={total}"),
                    error,
                )
            })?;
        require_one(
            changed,
            "document_changed",
            "The document changed while canonical ingestion progress was being recorded.",
        )
    })
}

pub(crate) fn persist_ingested_document(
    database: &Database,
    document_id: &str,
    processed: &AdapterExtraction,
) -> Result<(), CommandError> {
    let mut canonical = processed.canonical.clone();
    let expected_adapter = active_adapter_backend(canonical.document.source.format)?;
    if canonical.document.extraction.adapter_backend != expected_adapter {
        return Err(CommandError::new(
            "document_adapter_identity_changed",
            "The selected document adapter changed before its output could be committed.",
        ));
    }
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| {
                CommandError::database_at(
                    "document_ingestion_persist_begin_failed",
                    format!("document={document_id}"),
                    error,
                )
            })?;

        let stored_source = transaction
            .query_row(
                "SELECT title, original_filename, mime_type, byte_size, sha256,
                        source_relative_path
                 FROM documents WHERE id = ?1",
                [document_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                    ))
                },
            )
            .optional()
            .map_err(|error| {
                CommandError::database_at(
                    "canonical_source_metadata_read_failed",
                    format!("document={document_id}"),
                    error,
                )
            })?
            .ok_or_else(|| {
                CommandError::new(
                    "document_not_found",
                    "That document is no longer in the library.",
                )
            })?;
        if canonical.document.source_files.len() != 1 {
            return Err(CommandError::new(
                "invalid_canonical_document",
                "The PDF adapter must return exactly one immutable source file.",
            ));
        }
        let source_file = canonical
            .document
            .source_files
            .first_mut()
            .expect("the source-file count was checked above");
        if source_file.byte_size != stored_source.3 || source_file.sha256 != stored_source.4 {
            return Err(CommandError::new(
                "document_source_changed",
                "The managed source changed while ostudy was extracting it. Import the document again.",
            ));
        }
        let normalized_title = normalize_semantic_text(&stored_source.0, MAX_CANONICAL_TITLE_CHARS);
        canonical.document.metadata.title = (!normalized_title.is_empty()).then_some(normalized_title);
        let normalized_filename =
            normalize_semantic_text(&stored_source.1, MAX_CANONICAL_FILE_NAME_CHARS);
        source_file.original_filename = if normalized_filename.is_empty() {
            "source.pdf".to_string()
        } else {
            normalized_filename
        };
        source_file.media_type = stored_source.2;
        source_file.relative_path = stored_source.5;
        canonical.validate()?;
        let full_text_sha256 = canonical.document.full_text_sha256();
        crate::logging::debug_target(
            "document.canonical",
            "canonical_document_validated",
            "Validated a canonical document without logging source content.",
            serde_json::json!({
                "document": document_id,
                "format": canonical.document.source.format.as_str(),
                "adapter": canonical.document.extraction.adapter_backend.as_str(),
                "sourceFiles": canonical.document.source_files.len(),
                "sourceUnits": canonical.document.units.len(),
                "blocks": canonical.document.blocks.len(),
                "assets": canonical.document.assets.len(),
                "semanticChunks": canonical.semantic_chunks.len(),
                "fullTextSha256": full_text_sha256,
            }),
        );

        document_canonical_store::clear_canonical_derivatives(&transaction, document_id)?;
        clear_pdf_compatibility_derivatives(&transaction, document_id)?;
        persist_pdf_compatibility_projection(
            &transaction,
            document_id,
            &processed.legacy_pdf,
            now,
        )?;
        document_canonical_store::persist_canonical_ingestion(
            &transaction,
            document_id,
            &canonical,
            now,
        )?;

        let unit_count = to_i64(canonical.document.source.unit_count);
        let ocr_page_count = to_i64(processed.legacy_pdf.ocr.page_count);
        let visualized_units = canonical
            .document
            .assets
            .iter()
            .filter(|asset| asset.role == CanonicalAssetRole::UnitRender)
            .filter_map(|asset| asset.source_unit_number)
            .collect::<BTreeSet<_>>()
            .len();
        let changed = transaction
            .execute(
                "UPDATE documents
                 SET page_count = ?1, extracted_page_count = ?1,
                     rendered_page_count = ?1,
                     source_format = ?2, source_unit_kind = ?3,
                     extraction_backend = ?4, render_backend = ?5,
                     extraction_mode = ?6, ocr_page_count = ?7,
                     ocr_completed_page_count = ?7, ocr_mean_confidence = ?8,
                     ocr_engine = ?9,
                     semantic_element_count = ?10, semantic_chunk_count = ?11,
                     canonical_schema_version = ?12, adapter_backend = ?13,
                     source_file_count = ?14, source_unit_count = ?1,
                     extracted_source_unit_count = ?15,
                     visualized_source_unit_count = ?16,
                     source_asset_count = ?17,
                     scale_mode = 'standard', estimated_source_tokens = 0,
                     outline_window_count = 0, outline_completed_window_count = 0,
                     generation_window_count = 0, generation_completed_window_count = 0,
                     outline_state = 'pending', safe_error_code = '', safe_error_message = '',
                     updated_at = ?18, revision = revision + 1
                 WHERE id = ?19 AND lifecycle_state = 'processing'",
                params![
                    unit_count,
                    canonical.document.source.format.as_str(),
                    canonical.document.source.unit_kind.as_str(),
                    processed.legacy_pdf.extraction_backend,
                    processed.legacy_pdf.render_backend,
                    processed.legacy_pdf.extraction_mode.as_str(),
                    ocr_page_count,
                    processed.legacy_pdf.ocr.mean_confidence,
                    processed.legacy_pdf.ocr.engine,
                    to_i64(canonical.document.blocks.len()),
                    to_i64(canonical.semantic_chunks.len()),
                    canonical.document.schema_version,
                    canonical.document.extraction.adapter_backend,
                    to_i64(canonical.document.source_files.len()),
                    to_i64(canonical.document.units.len()),
                    to_i64(visualized_units),
                    to_i64(canonical.document.assets.len()),
                    now,
                    document_id,
                ],
            )
            .map_err(|error| {
                CommandError::database_at(
                    "document_ingestion_finalize_failed",
                    format!("document={document_id}"),
                    error,
                )
            })?;
        require_one(
            changed,
            "document_changed",
            "The document changed before extracted derivatives could be committed.",
        )?;
        transaction.commit().map_err(|error| {
            CommandError::database_at(
                "document_ingestion_commit_failed",
                format!("document={document_id}"),
                error,
            )
        })
    })
}

fn canonicalize_pdf(
    document_id: &str,
    source_path: &Path,
    adapter: &dyn DocumentAdapter,
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
                adapter_backend: adapter.backend_id().to_string(),
                extraction_backend: adapter.extraction_backend().to_string(),
                render_backend: adapter.render_backend().map(str::to_string),
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

fn clear_pdf_compatibility_derivatives(
    transaction: &rusqlite::Transaction<'_>,
    document_id: &str,
) -> Result<(), CommandError> {
    for table in [
        "document_pages",
        "document_semantic_elements",
        "document_semantic_chunks",
        "document_outlines",
        "document_outline_checkpoints",
        "document_generation_checkpoints",
    ] {
        transaction
            .execute(
                &format!("DELETE FROM {table} WHERE document_id = ?1"),
                [document_id],
            )
            .map_err(|error| {
                CommandError::database_at(
                    "document_ingestion_clear_derivatives_failed",
                    format!("document={document_id} table={table}"),
                    error,
                )
            })?;
    }
    Ok(())
}

fn persist_pdf_compatibility_projection(
    transaction: &rusqlite::Transaction<'_>,
    document_id: &str,
    processed: &IngestedDocument,
    now: i64,
) -> Result<(), CommandError> {
    for page in &processed.pages {
        transaction
            .execute(
                "INSERT INTO document_pages (
                     document_id, page_number, plain_text, text_sha256,
                     render_relative_path, render_mime_type, render_byte_size,
                     render_sha256, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, 'image/png', ?6, ?7, ?8, ?8)",
                params![
                    document_id,
                    page.unit_number,
                    page.plain_text,
                    page.text_sha256,
                    page.render_relative_path,
                    page.render_byte_size,
                    page.render_sha256,
                    now,
                ],
            )
            .map_err(|error| {
                CommandError::database_at(
                    "document_ingestion_page_insert_failed",
                    format!("document={document_id} unit={}", page.unit_number),
                    error,
                )
            })?;
    }

    for element in &processed.semantic.elements {
        let text = normalize_semantic_text(&element.text, 250_000);
        transaction
            .execute(
                "INSERT INTO document_semantic_elements (
                     document_id, element_index, page_number, element_type,
                     text, parent_heading, heading_path_json,
                     bbox_x, bbox_y, bbox_width, bbox_height, confidence,
                     font_name, font_size, is_bold, is_italic,
                     text_sha256, created_at
                 ) VALUES (
                     ?1, ?2, ?3, ?4, ?5, ?6, ?7,
                     ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18
                 )",
                params![
                    document_id,
                    to_i64(element.element_index),
                    to_i64(element.page_number),
                    normalize_element_type(&element.element_type)?,
                    text,
                    element.parent_heading,
                    serde_json::to_string(&element.heading_path)
                        .map_err(CommandError::serialization)?,
                    element.bbox.x,
                    element.bbox.y,
                    element.bbox.width.max(0.0),
                    element.bbox.height.max(0.0),
                    element.confidence.clamp(0.0, 1.0),
                    element.font_name,
                    element.font_size,
                    i64::from(element.is_bold),
                    i64::from(element.is_italic),
                    sha256_hex(text.as_bytes()),
                    now,
                ],
            )
            .map_err(|error| {
                CommandError::database_at(
                    "document_ingestion_element_insert_failed",
                    format!("document={document_id} element={}", element.element_index),
                    error,
                )
            })?;
    }

    for chunk in &processed.semantic.chunks {
        let text = normalize_semantic_text(&chunk.text, 1_000_000);
        let full_text = normalize_semantic_text(&chunk.full_text, 1_100_000);
        let unit_numbers = chunk
            .page_numbers
            .iter()
            .map(|unit| to_i64(*unit))
            .collect::<Vec<_>>();
        transaction
            .execute(
                "INSERT INTO document_semantic_chunks (
                     document_id, chunk_index, text, full_text,
                     page_numbers_json, bounding_boxes_json, element_types_json,
                     heading_context, token_estimate, is_oversized,
                     text_sha256, created_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![
                    document_id,
                    to_i64(chunk.chunk_index),
                    text,
                    full_text,
                    serde_json::to_string(&unit_numbers).map_err(CommandError::serialization)?,
                    serde_json::to_string(&chunk.bounding_boxes)
                        .map_err(CommandError::serialization)?,
                    serde_json::to_string(&chunk.element_types)
                        .map_err(CommandError::serialization)?,
                    chunk.heading_context,
                    to_i64(chunk.token_estimate),
                    i64::from(chunk.is_oversized),
                    sha256_hex(text.as_bytes()),
                    now,
                ],
            )
            .map_err(|error| {
                CommandError::database_at(
                    "document_ingestion_chunk_insert_failed",
                    format!("document={document_id} chunk={}", chunk.chunk_index),
                    error,
                )
            })?;
    }
    Ok(())
}

fn chunk_needs_visual_grounding(element_types: &[String], full_text: &str) -> bool {
    element_types.iter().any(|element_type| {
        matches!(
            element_type.as_str(),
            "image" | "table" | "key_value" | "equation" | "chart" | "diagram"
        )
    }) || full_text.contains("has no extractable structured text")
}

fn normalize_element_type(value: &str) -> Result<&'static str, CommandError> {
    match value.to_ascii_lowercase().replace('-', "_").as_str() {
        "title" => Ok("title"),
        "paragraph" => Ok("paragraph"),
        "table" => Ok("table"),
        "header" => Ok("header"),
        "footer" => Ok("footer"),
        "listitem" | "list_item" => Ok("list_item"),
        "image" => Ok("image"),
        "codeblock" | "code_block" => Ok("code_block"),
        "keyvalue" | "key_value" => Ok("key_value"),
        _ => Err(CommandError::new(
            "document_semantic_element_unsupported",
            "The extraction adapter returned an unsupported semantic element type.",
        )),
    }
}

fn normalize_optional_semantic_text(value: Option<&str>, maximum: usize) -> Option<String> {
    value
        .map(|value| normalize_semantic_text(value, maximum))
        .filter(|value| !value.is_empty())
}

fn normalize_semantic_text(value: &str, maximum: usize) -> String {
    value
        .chars()
        .filter(|character| !character.is_control() || matches!(character, '\n' | '\t'))
        .take(maximum)
        .collect::<String>()
        .trim()
        .to_string()
}

fn format_source_units(unit_numbers: &[i64]) -> String {
    match unit_numbers {
        [] => "unit".to_string(),
        [unit] => format!("unit {unit}"),
        units => format!(
            "units {}–{}",
            units.first().copied().unwrap_or(1),
            units.last().copied().unwrap_or(1)
        ),
    }
}

fn hash_file(path: &Path) -> Result<(i64, String), CommandError> {
    let metadata = fs::metadata(path).map_err(CommandError::io)?;
    let byte_size = i64::try_from(metadata.len()).unwrap_or(i64::MAX);
    if byte_size <= 0 {
        return Err(CommandError::new(
            "document_source_empty",
            "The managed source document is empty.",
        ));
    }
    let mut file = File::open(path).map_err(CommandError::io)?;
    let mut digest = Sha256::new();
    let mut buffer = vec![0_u8; HASH_BUFFER_BYTES];
    loop {
        let read = file.read(&mut buffer).map_err(CommandError::io)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok((byte_size, format!("{:x}", digest.finalize())))
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn to_i64(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn require_one(changed: usize, code: &str, message: &str) -> Result<(), CommandError> {
    if changed == 1 {
        Ok(())
    } else {
        Err(CommandError::new(code, message))
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;
    use crate::{
        document_domain::DocumentSourceDescriptor,
        pdf_backend::{
            ExtractedDocument, ExtractedSemanticChunk, ExtractedSemanticElement, ExtractedTextPage,
            SemanticBBox,
        },
        pdf_document_ingestion::{ExtractionMode, IngestedPage, OcrIngestionSummary},
    };

    #[test]
    fn registry_rejects_formats_without_an_enabled_adapter() {
        let error =
            match DocumentAdapterRegistry::select(DocumentFormat::Docx, Path::new("unused.docx")) {
                Ok(_) => panic!("DOCX must not be enabled before its adapter ships"),
                Err(error) => error,
            };
        assert_eq!(error.code, "document_ingestion_adapter_unavailable");
    }

    #[test]
    fn pdf_canonicalization_normalizes_parser_text_and_preserves_visual_evidence() {
        let root = std::env::temp_dir().join(format!(
            "ostudy-pdf-adapter-{}-{}",
            std::process::id(),
            unix_time_millis().unwrap_or(0)
        ));
        let paths = AppPaths::new(root.clone()).expect("test paths should initialize");
        let source_path = paths.resolve_relative(&AppPaths::document_source_relative_path(
            "0123456789abcdef0123456789abcdef",
        ));
        fs::create_dir_all(source_path.parent().unwrap()).unwrap();
        let mut source = File::create(&source_path).unwrap();
        source.write_all(b"%PDF-1.7 fixture").unwrap();
        source.sync_all().unwrap();

        let render_id = unit_render_asset_id(1);
        let long_heading = format!("{}\u{000c}", "H".repeat(600));
        let legacy = IngestedDocument {
            source: DocumentSourceDescriptor::pdf(1),
            pages: vec![IngestedPage {
                unit_number: 1,
                plain_text: "Cardiac output is heart rate times stroke volume.".to_string(),
                text_sha256: sha256_hex(b"Cardiac output is heart rate times stroke volume."),
                render_relative_path: "documents/fixture/pages/page-000001.png".to_string(),
                render_byte_size: 10,
                render_sha256: "a".repeat(64),
            }],
            semantic: ExtractedDocument {
                page_count: 1,
                pages: vec![ExtractedTextPage {
                    page_number: 1,
                    text: "Cardiac output is heart rate times stroke volume.".to_string(),
                }],
                elements: vec![
                    ExtractedSemanticElement {
                        element_index: 7,
                        page_number: 1,
                        element_type: "table".to_string(),
                        text: "CO = HR\u{000c} × SV".to_string(),
                        parent_heading: Some(long_heading),
                        heading_path: vec!["Hemodynamics\u{000b}".to_string()],
                        bbox: SemanticBBox {
                            x: 0.1,
                            y: 0.2,
                            width: 0.5,
                            height: 0.1,
                        },
                        confidence: 0.99,
                        font_name: Some("Example\u{0007} Sans".to_string()),
                        font_size: Some(12.0),
                        is_bold: true,
                        is_italic: false,
                    },
                    ExtractedSemanticElement {
                        element_index: 8,
                        page_number: 1,
                        element_type: "image".to_string(),
                        text: String::new(),
                        parent_heading: None,
                        heading_path: Vec::new(),
                        bbox: SemanticBBox {
                            x: 0.2,
                            y: 0.4,
                            width: 0.4,
                            height: 0.3,
                        },
                        confidence: 1.0,
                        font_name: None,
                        font_size: None,
                        is_bold: false,
                        is_italic: false,
                    },
                ],
                chunks: vec![
                    ExtractedSemanticChunk {
                        chunk_index: 4,
                        text: "Cardiac\u{000c} output table".to_string(),
                        full_text: "Cardiac output is heart rate\u{000b} times stroke volume."
                            .to_string(),
                        page_numbers: vec![1],
                        bounding_boxes: Vec::new(),
                        element_types: vec!["table".to_string()],
                        heading_context: Some("Hemodynamics\u{000c}".to_string()),
                        token_estimate: 16,
                        is_oversized: false,
                    },
                    ExtractedSemanticChunk {
                        chunk_index: 5,
                        text: "\u{000c}".to_string(),
                        full_text: "\u{000b}".to_string(),
                        page_numbers: vec![1],
                        bounding_boxes: Vec::new(),
                        element_types: vec!["image".to_string()],
                        heading_context: None,
                        token_estimate: 1,
                        is_oversized: false,
                    },
                ],
                native_text_failed_pages: Vec::new(),
                semantic_analysis_degraded: false,
            },
            extraction_backend: "fixture-extractor",
            render_backend: "fixture-renderer",
            extraction_mode: ExtractionMode::Native,
            ocr: OcrIngestionSummary {
                page_count: 0,
                mean_confidence: None,
                engine: String::new(),
            },
        };

        let canonical = canonicalize_pdf(
            "0123456789abcdef0123456789abcdef",
            &source_path,
            &PDF_ADAPTER,
            &legacy,
        )
        .expect("PDF parser text should normalize into canonical records");
        canonical
            .validate()
            .expect("normalized canonical output should validate");
        assert_eq!(
            canonical.document.extraction.adapter_backend,
            PDF_ADAPTER_BACKEND
        );
        assert_eq!(canonical.document.blocks[0].text, "CO = HR × SV");
        assert!(canonical.document.blocks[0]
            .parent_heading
            .as_deref()
            .is_some_and(|heading| heading.chars().count() == MAX_CANONICAL_HEADING_CHARS));
        assert!(canonical.document.blocks[0]
            .heading_path
            .iter()
            .all(|heading| !heading.chars().any(char::is_control)));
        assert_eq!(
            canonical.document.blocks[0].asset_ids,
            vec![render_id.clone()]
        );
        assert!(canonical.document.blocks[1].text.is_empty());
        assert_eq!(
            canonical.document.blocks[1].asset_ids,
            vec![render_id.clone()]
        );
        assert_eq!(canonical.semantic_chunks.len(), 2);
        assert_eq!(canonical.semantic_chunks[0].text, "Cardiac output table");
        assert!(!canonical.semantic_chunks[0]
            .full_text
            .chars()
            .any(char::is_control));
        assert_eq!(
            canonical.semantic_chunks[0].source_locator.asset_ids,
            vec![render_id.clone()]
        );
        assert!(canonical.semantic_chunks[1]
            .full_text
            .contains("visual or non-text content"));
        assert_eq!(
            canonical.semantic_chunks[1].source_locator.asset_ids,
            vec![render_id]
        );
        assert_eq!(canonical.semantic_chunks[0].content_sha256.len(), 64);
        assert_eq!(canonical.document.full_text_sha256().len(), 64);

        std::fs::remove_dir_all(root).expect("test paths should be removed");
    }
}
