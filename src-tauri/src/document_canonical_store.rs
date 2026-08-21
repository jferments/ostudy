//! Atomic persistence for the app-owned canonical document representation.
//!
//! Adapters never write arbitrary database state. The ingestion orchestrator
//! validates a complete `CanonicalIngestion`, opens one transaction, and asks
//! this module to replace only the canonical source ledger and semantic-source
//! projections. The current PDF compatibility rows are written by the same
//! orchestrator in that transaction until the later generic UI/IPC stage.

use rusqlite::{params, Transaction};

use crate::{
    document_canonical::{CanonicalIngestion, CANONICAL_DOCUMENT_SCHEMA_VERSION},
    document_domain::SourceLocator,
    error::CommandError,
};

pub(crate) fn clear_canonical_derivatives(
    transaction: &Transaction<'_>,
    document_id: &str,
) -> Result<(), CommandError> {
    // Order is explicit rather than relying on cascading side effects. Source
    // files are referenced with RESTRICT so they are always cleared last.
    for table in [
        "document_semantic_chunk_sources",
        "document_source_assets",
        "document_source_blocks",
        "document_source_units",
        "document_source_files",
    ] {
        transaction
            .execute(
                &format!("DELETE FROM {table} WHERE document_id = ?1"),
                [document_id],
            )
            .map_err(|error| {
                CommandError::database_at(
                    "canonical_document_clear_failed",
                    format!("document={document_id} table={table}"),
                    error,
                )
            })?;
    }
    Ok(())
}

pub(crate) fn persist_canonical_ingestion(
    transaction: &Transaction<'_>,
    document_id: &str,
    ingestion: &CanonicalIngestion,
    now: i64,
) -> Result<(), CommandError> {
    ingestion.validate()?;

    for source_file in &ingestion.document.source_files {
        transaction
            .execute(
                "INSERT INTO document_source_files (
                     document_id, file_ordinal, original_filename, media_type,
                     relative_path, byte_size, sha256, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
                params![
                    document_id,
                    source_file.file_ordinal,
                    source_file.original_filename,
                    source_file.media_type,
                    source_file.relative_path,
                    source_file.byte_size,
                    source_file.sha256,
                    now,
                ],
            )
            .map_err(|error| {
                canonical_db_error(
                    "canonical_source_file_insert_failed",
                    document_id,
                    format!("file={}", source_file.file_ordinal),
                    error,
                )
            })?;
    }

    for unit in &ingestion.document.units {
        let asset_ids = ingestion
            .document
            .assets
            .iter()
            .filter(|asset| asset.source_unit_number == Some(unit.unit_number))
            .map(|asset| asset.asset_id.clone())
            .collect::<Vec<_>>();
        let locator = SourceLocator::with_evidence(
            unit.unit_kind,
            vec![unit.unit_number],
            unit.title.clone().or_else(|| Some(unit.label.clone())),
            asset_ids,
            None,
        )?;
        transaction
            .execute(
                "INSERT INTO document_source_units (
                     document_id, unit_number, unit_kind, label, title,
                     plain_text, text_sha256, source_file_ordinal,
                     locator_json, extraction_confidence, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, NULL, ?10, ?10)",
                params![
                    document_id,
                    unit.unit_number,
                    unit.unit_kind.as_str(),
                    unit.label,
                    unit.title,
                    unit.plain_text,
                    unit.text_sha256,
                    unit.source_file_ordinal,
                    serde_json::to_string(&locator).map_err(CommandError::serialization)?,
                    now,
                ],
            )
            .map_err(|error| {
                canonical_db_error(
                    "canonical_source_unit_insert_failed",
                    document_id,
                    format!("unit={}", unit.unit_number),
                    error,
                )
            })?;
    }

    for block in &ingestion.document.blocks {
        let geometry_json = block
            .geometry
            .map(|geometry| serde_json::to_string(&geometry))
            .transpose()
            .map_err(CommandError::serialization)?;
        transaction
            .execute(
                "INSERT INTO document_source_blocks (
                     document_id, block_index, block_ordinal,
                     source_unit_number, role, text, parent_heading,
                     heading_path_json, geometry_json, confidence,
                     asset_ids_json, attributes_json, content_sha256, created_at
                 ) VALUES (
                     ?1, ?2, ?3, ?4, ?5, ?6, ?7,
                     ?8, ?9, ?10, ?11, ?12, ?13, ?14
                 )",
                params![
                    document_id,
                    block.block_index,
                    block.block_ordinal,
                    block.source_unit_number,
                    block.role.as_str(),
                    block.text,
                    block.parent_heading,
                    serde_json::to_string(&block.heading_path)
                        .map_err(CommandError::serialization)?,
                    geometry_json,
                    block.confidence,
                    serde_json::to_string(&block.asset_ids)
                        .map_err(CommandError::serialization)?,
                    serde_json::to_string(&block.attributes)
                        .map_err(CommandError::serialization)?,
                    block.content_sha256,
                    now,
                ],
            )
            .map_err(|error| {
                canonical_db_error(
                    "canonical_source_block_insert_failed",
                    document_id,
                    format!("block={}", block.block_index),
                    error,
                )
            })?;
    }

    for asset in &ingestion.document.assets {
        transaction
            .execute(
                "INSERT INTO document_source_assets (
                     document_id, asset_id, source_unit_number, block_index,
                     role, media_type, relative_path, byte_size, sha256,
                     width, height, alt_text, created_at, updated_at
                 ) VALUES (
                     ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9,
                     ?10, ?11, ?12, ?13, ?13
                 )",
                params![
                    document_id,
                    asset.asset_id,
                    asset.source_unit_number,
                    asset.block_index,
                    asset.role.as_str(),
                    asset.media_type,
                    asset.relative_path,
                    asset.byte_size,
                    asset.sha256,
                    asset.width,
                    asset.height,
                    asset.alt_text,
                    now,
                ],
            )
            .map_err(|error| {
                canonical_db_error(
                    "canonical_source_asset_insert_failed",
                    document_id,
                    format!("asset={}", asset.asset_id),
                    error,
                )
            })?;
    }

    for chunk in &ingestion.semantic_chunks {
        transaction
            .execute(
                "INSERT INTO document_semantic_chunk_sources (
                     document_id, chunk_index, source_locator_json,
                     visual_asset_ids_json, content_sha256,
                     canonical_schema_version, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
                params![
                    document_id,
                    chunk.chunk_index,
                    serde_json::to_string(&chunk.source_locator)
                        .map_err(CommandError::serialization)?,
                    serde_json::to_string(&chunk.source_locator.asset_ids)
                        .map_err(CommandError::serialization)?,
                    chunk.content_sha256,
                    CANONICAL_DOCUMENT_SCHEMA_VERSION,
                    now,
                ],
            )
            .map_err(|error| {
                canonical_db_error(
                    "canonical_semantic_source_insert_failed",
                    document_id,
                    format!("chunk={}", chunk.chunk_index),
                    error,
                )
            })?;
    }

    Ok(())
}

fn canonical_db_error(
    event: &str,
    document_id: &str,
    detail: String,
    error: impl std::fmt::Display,
) -> CommandError {
    CommandError::database_at(event, format!("document={document_id} {detail}"), error)
}
