use std::collections::BTreeSet;

use rusqlite::{params, OptionalExtension, TransactionBehavior};

use crate::{
    database::Database,
    document_canonical::CanonicalAssetRole,
    document_canonical_store,
    error::CommandError,
    util::unix_time_millis,
};

use super::{
    contracts::{AdapterExtraction, CompatibilityProjection, DocumentAdapterIdentity},
    pdf::compatibility::{
        clear_pdf_compatibility_derivatives, persist_pdf_compatibility_projection,
    },
    registry::active_identity,
    support::{normalize_semantic_text, require_one, to_i64},
};

const MAX_CANONICAL_FILE_NAME_CHARS: usize = 255;
const MAX_CANONICAL_TITLE_CHARS: usize = 1_000;

pub(crate) fn persist_ingested_document(
    database: &Database,
    document_id: &str,
    processed: &AdapterExtraction,
) -> Result<(), CommandError> {
    let mut canonical = processed.canonical.clone();
    let format = canonical.document.source.format;
    let expected = active_identity(format)?;
    validate_backend_identity(processed, expected)?;

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
                "SELECT title, original_filename, source_media_type, byte_size, sha256,
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
                "A single-file document adapter must return exactly one immutable source file.",
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
                "The managed source changed while oStudy was extracting it. Import the document again.",
            ));
        }
        let normalized_title = normalize_semantic_text(&stored_source.0, MAX_CANONICAL_TITLE_CHARS);
        canonical.document.metadata.title = (!normalized_title.is_empty()).then_some(normalized_title);
        let normalized_filename =
            normalize_semantic_text(&stored_source.1, MAX_CANONICAL_FILE_NAME_CHARS);
        source_file.original_filename = if normalized_filename.is_empty() {
            format!(
                "source.{}",
                canonical.document.source.format.default_extension()
            )
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
                "formatLabel": canonical.document.source.format.display_label(),
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
        if let Some(CompatibilityProjection::Pdf(pdf)) = &processed.compatibility {
            persist_pdf_compatibility_projection(&transaction, document_id, pdf, now)?;
        }
        document_canonical_store::persist_canonical_ingestion(
            &transaction,
            document_id,
            &canonical,
            now,
        )?;

        let unit_count = to_i64(canonical.document.source.unit_count);
        let extracted_units = to_i64(canonical.document.units.len());
        let source_asset_count = to_i64(canonical.document.assets.len());
        let visualized_units = canonical
            .document
            .assets
            .iter()
            .filter_map(|asset| asset.source_unit_number)
            .collect::<BTreeSet<_>>()
            .len();
        let rendered_pages = if matches!(
            &processed.compatibility,
            Some(CompatibilityProjection::Pdf(_))
        ) {
            canonical
                .document
                .assets
                .iter()
                .filter(|asset| asset.role == CanonicalAssetRole::UnitRender)
                .filter_map(|asset| asset.source_unit_number)
                .collect::<BTreeSet<_>>()
                .len()
        } else {
            0
        };
        let extraction = &canonical.document.extraction;
        let changed = transaction
            .execute(
                "UPDATE documents
                 SET page_count = ?1, extracted_page_count = ?2,
                     rendered_page_count = ?3,
                     source_format = ?4, source_unit_kind = ?5,
                     extraction_backend = ?6, render_backend = ?7,
                     extraction_mode = ?8, ocr_page_count = ?9,
                     ocr_completed_page_count = ?9, ocr_mean_confidence = ?10,
                     ocr_engine = ?11,
                     semantic_element_count = ?12, semantic_chunk_count = ?13,
                     canonical_schema_version = ?14, adapter_backend = ?15,
                     source_file_count = ?16, source_unit_count = ?1,
                     extracted_source_unit_count = ?2,
                     visualized_source_unit_count = ?17,
                     source_asset_count = ?18,
                     scale_mode = 'standard', estimated_source_tokens = 0,
                     outline_window_count = 0, outline_completed_window_count = 0,
                     generation_window_count = 0, generation_completed_window_count = 0,
                     outline_state = 'pending', safe_error_code = '', safe_error_message = '',
                     updated_at = ?19, revision = revision + 1
                 WHERE id = ?20 AND lifecycle_state = 'processing'",
                params![
                    unit_count,
                    extracted_units,
                    to_i64(rendered_pages),
                    canonical.document.source.format.as_str(),
                    canonical.document.source.unit_kind.as_str(),
                    extraction.extraction_backend,
                    extraction.render_backend.as_deref().unwrap_or("none"),
                    extraction.extraction_mode,
                    to_i64(extraction.ocr_unit_count),
                    extraction.ocr_mean_confidence,
                    extraction.ocr_engine.as_deref().unwrap_or(""),
                    to_i64(canonical.document.blocks.len()),
                    to_i64(canonical.semantic_chunks.len()),
                    canonical.document.schema_version,
                    extraction.adapter_backend,
                    to_i64(canonical.document.source_files.len()),
                    to_i64(visualized_units),
                    source_asset_count,
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

fn validate_backend_identity(
    processed: &AdapterExtraction,
    expected: DocumentAdapterIdentity,
) -> Result<(), CommandError> {
    let extraction = &processed.canonical.document.extraction;
    if processed.identity.adapter_backend != expected.adapter_backend
        || extraction.adapter_backend != expected.adapter_backend
    {
        return Err(CommandError::new(
            "document_adapter_identity_changed",
            "The selected document adapter changed before its output could be committed.",
        ));
    }
    if processed.identity.extraction_backend != expected.extraction_backend
        || extraction.extraction_backend != expected.extraction_backend
    {
        return Err(CommandError::new(
            "document_extraction_backend_changed",
            "The selected document extraction backend changed before its output could be committed.",
        ));
    }
    if processed.identity.render_backend_or_none() != expected.render_backend_or_none()
        || extraction.render_backend.as_deref().unwrap_or("none")
            != expected.render_backend_or_none()
    {
        return Err(CommandError::new(
            "document_render_backend_changed",
            "The selected document rendering backend changed before its output could be committed.",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        document_canonical::{
            CanonicalDocument, CanonicalExtractionReport, CanonicalIngestion,
            CANONICAL_DOCUMENT_SCHEMA_VERSION,
        },
        document_domain::DocumentSourceDescriptor,
    };

    fn extraction(identity: DocumentAdapterIdentity) -> AdapterExtraction {
        AdapterExtraction {
            identity,
            canonical: CanonicalIngestion {
                document: CanonicalDocument {
                    schema_version: CANONICAL_DOCUMENT_SCHEMA_VERSION,
                    source: DocumentSourceDescriptor::pdf(0),
                    source_files: Vec::new(),
                    metadata: Default::default(),
                    units: Vec::new(),
                    blocks: Vec::new(),
                    assets: Vec::new(),
                    extraction: CanonicalExtractionReport {
                        adapter_backend: identity.adapter_backend.to_string(),
                        extraction_backend: identity.extraction_backend.to_string(),
                        render_backend: identity.render_backend.map(str::to_string),
                        extraction_mode: "native".to_string(),
                        semantic_analysis_degraded: false,
                        ocr_unit_count: 0,
                        ocr_mean_confidence: None,
                        ocr_engine: None,
                    },
                },
                semantic_chunks: Vec::new(),
            },
            compatibility: None,
        }
    }

    #[test]
    fn persistence_rejects_backend_identity_drift() {
        let expected = DocumentAdapterIdentity {
            adapter_backend: "adapter-v1",
            extraction_backend: "extract-v1",
            render_backend: None,
        };
        let changed = DocumentAdapterIdentity {
            adapter_backend: "adapter-v2",
            ..expected
        };
        assert_eq!(
            validate_backend_identity(&extraction(changed), expected)
                .unwrap_err()
                .code,
            "document_adapter_identity_changed"
        );
    }
}
