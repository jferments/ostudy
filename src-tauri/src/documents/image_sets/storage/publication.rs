//! Transactional publication of an image projection and its stable evidence keys.

use std::collections::BTreeMap;

use rusqlite::{params, Connection};

use crate::{
    document_canonical::{sha256_hex, CanonicalIngestion},
    error::CommandError,
};

use super::{invalid, repository, settings, ImageReceipt, MAX_RECEIPT_BYTES};

pub(crate) fn begin_projection(
    connection: &Connection,
    receipt: &ImageReceipt,
) -> Result<(), CommandError> {
    receipt.validate(&repository::load(connection, &receipt.document_id)?)?;
    if settings(connection, &receipt.document_id)?.revision != receipt.filter_revision {
        return Err(CommandError::new(
            "image_set_changed",
            "Content filters changed during image processing. Retry processing the saved source.",
        ));
    }
    connection
        .execute(
            "INSERT INTO document_image_projection_writes (document_id) VALUES (?1)",
            [&receipt.document_id],
        )
        .map_err(CommandError::database)?;
    Ok(())
}

pub(crate) fn publish(
    connection: &Connection,
    receipt: &ImageReceipt,
    canonical: &CanonicalIngestion,
) -> Result<(), CommandError> {
    receipt.validate_projection(canonical)?;
    let mut receipt = receipt.clone();
    preserve_unique_keys(connection, &mut receipt)?;
    let json = serde_json::to_string(&receipt).map_err(CommandError::serialization)?;
    if json.len() > MAX_RECEIPT_BYTES {
        return Err(invalid(
            "The image source exceeds the processing-receipt budget.",
        ));
    }
    connection.execute(
        "INSERT INTO document_image_receipts (document_id, manifest_sha256, filter_revision, version, payload_json, payload_sha256)
         VALUES (?1, ?2, ?3, 1, ?4, ?5) ON CONFLICT(document_id) DO UPDATE SET
         manifest_sha256 = excluded.manifest_sha256, filter_revision = excluded.filter_revision,
         version = excluded.version, payload_json = excluded.payload_json, payload_sha256 = excluded.payload_sha256",
        params![receipt.document_id, receipt.manifest_sha256, receipt.filter_revision, json, sha256_hex(json.as_bytes())],
    ).map_err(CommandError::database)?;
    connection
        .execute(
            "DELETE FROM document_image_chunk_keys WHERE document_id = ?1",
            [&receipt.document_id],
        )
        .map_err(CommandError::database)?;
    for (index, chunk) in receipt.chunks.iter().enumerate() {
        connection.execute(
            "INSERT INTO document_image_chunk_keys (document_id, chunk_index, stable_key, evidence_sha256) VALUES (?1, ?2, ?3, ?4)",
            params![receipt.document_id, index as i64, chunk.stable_key, chunk.evidence_sha256],
        ).map_err(CommandError::database)?;
    }
    connection
        .execute(
            "UPDATE documents SET adapter_backend = ?1 WHERE id = ?2",
            params![
                canonical.document.extraction.adapter_backend,
                receipt.document_id
            ],
        )
        .map_err(CommandError::database)?;
    connection
        .execute(
            "UPDATE document_image_sets SET prepared_manifest_sha256 = ?1 WHERE document_id = ?2",
            params![receipt.manifest_sha256, receipt.document_id],
        )
        .map_err(CommandError::database)?;
    Ok(())
}

pub(crate) fn finish_projection(connection: &Connection, id: &str) -> Result<(), CommandError> {
    connection
        .execute(
            "DELETE FROM document_image_projection_writes WHERE document_id = ?1",
            [id],
        )
        .map_err(CommandError::database)?;
    Ok(())
}

pub(crate) fn invalidate(connection: &Connection, id: &str, now: i64) -> Result<(), CommandError> {
    connection
        .execute(
            "UPDATE document_image_sets SET prepared_manifest_sha256 = '' WHERE document_id = ?1",
            [id],
        )
        .map_err(CommandError::database)?;
    connection
        .execute(
            "UPDATE documents SET adapter_backend = 'pending', outline_state = 'pending',
         lifecycle_state = 'ready', safe_error_code = '', safe_error_message = '',
         updated_at = ?1, revision = revision + 1 WHERE id = ?2",
            params![now, id],
        )
        .map_err(CommandError::database)?;
    connection
        .execute(
            "DELETE FROM knowledge_document_views WHERE document_id = ?1",
            [id],
        )
        .map_err(CommandError::database)?;
    Ok(())
}

/// A unique exact evidence proof may retain its earlier identity when its
/// representative occurrence changes. Ambiguous repeated proofs stay separate.
fn preserve_unique_keys(
    connection: &Connection,
    receipt: &mut ImageReceipt,
) -> Result<(), CommandError> {
    let mut statement = connection
        .prepare("SELECT stable_key, evidence_sha256 FROM document_image_chunk_keys WHERE document_id = ?1")
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map([&receipt.document_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    let mut old = BTreeMap::<String, Vec<String>>::new();
    for (key, proof) in rows {
        old.entry(proof).or_default().push(key);
    }
    let mut counts = BTreeMap::<String, usize>::new();
    let mut used = receipt
        .chunks
        .iter()
        .map(|chunk| chunk.stable_key.clone())
        .collect::<std::collections::HashSet<_>>();
    for chunk in &receipt.chunks {
        *counts.entry(chunk.evidence_sha256.clone()).or_default() += 1;
    }
    for chunk in &mut receipt.chunks {
        if counts.get(&chunk.evidence_sha256) != Some(&1) {
            continue;
        }
        let Some(keys) = old.get(&chunk.evidence_sha256) else {
            continue;
        };
        if keys.len() == 1 && !used.contains(&keys[0]) {
            used.remove(&chunk.stable_key);
            chunk.stable_key = keys[0].clone();
            used.insert(chunk.stable_key.clone());
        }
    }
    Ok(())
}
