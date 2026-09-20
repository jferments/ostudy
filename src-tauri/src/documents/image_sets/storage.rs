//! Current image settings and atomic publication of native processing receipts.

use std::collections::BTreeMap;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::{document_canonical::{sha256_hex, CanonicalIngestion}, error::CommandError};

use super::{filter::RegionOverride, manifest, receipt::{invalid, ImageReceipt, MAX_RECEIPT_BYTES, MAX_REGIONS}, repository};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FilterSettings {
    pub revision: i64,
    pub include_interface: bool,
    pub overrides: BTreeMap<String, RegionOverride>,
}

impl Default for FilterSettings {
    fn default() -> Self {
        Self { revision: 1, include_interface: false, overrides: BTreeMap::new() }
    }
}

pub(crate) fn settings(connection: &Connection, id: &str) -> Result<FilterSettings, CommandError> {
    let mut settings = connection.query_row(
        "SELECT revision, include_interface FROM document_image_settings WHERE document_id = ?1",
        [id], |row| Ok(FilterSettings { revision: row.get(0)?, include_interface: row.get(1)?, overrides: BTreeMap::new() }),
    ).optional().map_err(CommandError::database)?.unwrap_or_default();
    let mut statement = connection.prepare(
        "SELECT o.region_id, o.action, m.document_id FROM document_image_overrides o
         LEFT JOIN document_image_members m ON m.id = o.member_id WHERE o.document_id = ?1 ORDER BY o.region_id",
    ).map_err(CommandError::database)?;
    let mut rows = statement.query([id]).map_err(CommandError::database)?;
    while let Some(row) = rows.next().map_err(CommandError::database)? {
        let region: String = row.get(0).map_err(CommandError::database)?;
        let action: String = row.get(1).map_err(CommandError::database)?;
        let owner: Option<String> = row.get(2).map_err(CommandError::database)?;
        manifest::validate_hash(&region)?;
        if owner.as_deref() != Some(id) || settings.overrides.len() >= MAX_REGIONS {
            return Err(invalid("A filter override has no matching image source or exceeds its limit."));
        }
        let action = match action.as_str() {
            "include" => RegionOverride::Include,
            "exclude" => RegionOverride::Exclude,
            "metadata" => RegionOverride::Metadata,
            _ => return Err(invalid("An image override uses an unsupported decision.")),
        };
        settings.overrides.insert(region, action);
    }
    Ok(settings)
}

pub(crate) fn load(connection: &Connection, id: &str) -> Result<Option<ImageReceipt>, CommandError> {
    let row: Option<(String, String, String, i64, u32)> = connection.query_row(
        "SELECT payload_json, payload_sha256, manifest_sha256, filter_revision, version FROM document_image_receipts WHERE document_id = ?1", [id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
    ).optional().map_err(CommandError::database)?;
    row.map(|(json, hash, manifest_hash, revision, version)| {
        if json.len() > MAX_RECEIPT_BYTES || sha256_hex(json.as_bytes()) != hash {
            return Err(invalid("An image processing receipt failed its size or hash check."));
        }
        let receipt: ImageReceipt = serde_json::from_str(&json).map_err(CommandError::serialization)?;
        receipt.validate(&repository::load(connection, id)?)?;
        if receipt.manifest_sha256 != manifest_hash || receipt.filter_revision != revision || receipt.version != version {
            return Err(invalid("The image processing receipt and its ledger disagree."));
        }
        Ok(receipt)
    }).transpose()
}

pub(crate) fn ready(connection: &Connection, id: &str) -> Result<bool, CommandError> {
    let adapter = crate::document_ingestion::active_adapter_backend(crate::document_domain::DocumentFormat::ImageBundle)?;
    connection.query_row(
        "SELECT NOT EXISTS(SELECT 1 FROM document_image_sets s WHERE s.document_id = ?1
         AND (s.prepared_manifest_sha256 <> s.manifest_sha256
             OR (SELECT adapter_backend FROM documents WHERE id = s.document_id) <> ?2 OR NOT EXISTS (
             SELECT 1 FROM document_image_receipts r WHERE r.document_id = s.document_id
             AND r.manifest_sha256 = s.manifest_sha256 AND r.filter_revision = COALESCE(
                 (SELECT revision FROM document_image_settings WHERE document_id = s.document_id), 1))))",
        params![id, adapter], |row| row.get(0),
    ).map_err(CommandError::database)
}

pub(crate) fn begin_projection(connection: &Connection, receipt: &ImageReceipt) -> Result<(), CommandError> {
    receipt.validate(&repository::load(connection, &receipt.document_id)?)?;
    if settings(connection, &receipt.document_id)?.revision != receipt.filter_revision {
        return Err(CommandError::new("image_set_changed", "Content filters changed during image processing. Retry processing the saved source."));
    }
    connection.execute("INSERT INTO document_image_projection_writes (document_id) VALUES (?1)", [&receipt.document_id])
        .map_err(CommandError::database)?;
    Ok(())
}

pub(crate) fn publish(connection: &Connection, receipt: &ImageReceipt, canonical: &CanonicalIngestion) -> Result<(), CommandError> {
    receipt.validate_projection(canonical)?;
    let mut receipt = receipt.clone();
    preserve_unique_keys(connection, &mut receipt)?;
    let json = serde_json::to_string(&receipt).map_err(CommandError::serialization)?;
    if json.len() > MAX_RECEIPT_BYTES { return Err(invalid("The image source exceeds the processing-receipt budget.")); }
    connection.execute(
        "INSERT INTO document_image_receipts (document_id, manifest_sha256, filter_revision, version, payload_json, payload_sha256)
         VALUES (?1, ?2, ?3, 1, ?4, ?5) ON CONFLICT(document_id) DO UPDATE SET
         manifest_sha256 = excluded.manifest_sha256, filter_revision = excluded.filter_revision,
         version = excluded.version, payload_json = excluded.payload_json, payload_sha256 = excluded.payload_sha256",
        params![receipt.document_id, receipt.manifest_sha256, receipt.filter_revision, json, sha256_hex(json.as_bytes())],
    ).map_err(CommandError::database)?;
    connection.execute("DELETE FROM document_image_chunk_keys WHERE document_id = ?1", [&receipt.document_id]).map_err(CommandError::database)?;
    for (index, chunk) in receipt.chunks.iter().enumerate() {
        connection.execute(
            "INSERT INTO document_image_chunk_keys (document_id, chunk_index, stable_key, evidence_sha256) VALUES (?1, ?2, ?3, ?4)",
            params![receipt.document_id, index as i64, chunk.stable_key, chunk.evidence_sha256],
        ).map_err(CommandError::database)?;
    }
    connection.execute("UPDATE documents SET adapter_backend = ?1 WHERE id = ?2",
        params![canonical.document.extraction.adapter_backend, receipt.document_id]).map_err(CommandError::database)?;
    connection.execute("UPDATE document_image_sets SET prepared_manifest_sha256 = ?1 WHERE document_id = ?2",
        params![receipt.manifest_sha256, receipt.document_id]).map_err(CommandError::database)?;
    Ok(())
}

pub(crate) fn finish_projection(connection: &Connection, id: &str) -> Result<(), CommandError> {
    connection.execute("DELETE FROM document_image_projection_writes WHERE document_id = ?1", [id]).map_err(CommandError::database)?;
    Ok(())
}

pub(crate) fn invalidate(connection: &Connection, id: &str, now: i64) -> Result<(), CommandError> {
    connection.execute("UPDATE document_image_sets SET prepared_manifest_sha256 = '' WHERE document_id = ?1", [id]).map_err(CommandError::database)?;
    connection.execute(
        "UPDATE documents SET adapter_backend = 'pending', outline_state = 'pending',
         lifecycle_state = 'ready', safe_error_code = '', safe_error_message = '',
         updated_at = ?1, revision = revision + 1 WHERE id = ?2", params![now, id],
    ).map_err(CommandError::database)?;
    connection.execute("DELETE FROM knowledge_document_views WHERE document_id = ?1", [id]).map_err(CommandError::database)?;
    Ok(())
}

/// Read-only startup/restore admission; never repair a forged or partial receipt.
pub(crate) fn validate(connection: &Connection) -> Result<(), CommandError> {
    super::operations::validate(connection)?;
    let guards: i64 = connection.query_row("SELECT COUNT(*) FROM document_image_projection_writes", [], |row| row.get(0)).map_err(CommandError::database)?;
    if guards != 0 { return Err(invalid("An incomplete native image transaction cannot be admitted.")); }
    let mut statement = connection.prepare("SELECT document_id FROM document_image_sets").map_err(CommandError::database)?;
    let ids = statement.query_map([], |row| row.get::<_, String>(0)).map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>().map_err(CommandError::database)?;
    for id in ids {
        let settings = settings(connection, &id)?;
        if let Some(receipt) = load(connection, &id)? {
            if receipt.filter_revision > settings.revision { return Err(invalid("The image receipt claims a future filter revision.")); }
            let mut keys = connection.prepare("SELECT k.stable_key, k.evidence_sha256, c.content_sha256 FROM document_image_chunk_keys k
                JOIN document_semantic_chunk_sources c ON c.document_id = k.document_id AND c.chunk_index = k.chunk_index
                WHERE k.document_id = ?1 ORDER BY k.chunk_index").map_err(CommandError::database)?;
            let keys = keys.query_map([&id], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?)))
                .map_err(CommandError::database)?.collect::<Result<Vec<_>, _>>().map_err(CommandError::database)?;
            if keys != receipt.chunks.iter().map(|chunk| (chunk.stable_key.clone(), chunk.evidence_sha256.clone(), chunk.projection_sha256.clone())).collect::<Vec<_>>() {
                return Err(invalid("Image source identities disagree with their native processing receipt."));
            }
        }
        crate::knowledge::image_sources::validate_current(connection, &id)?;
    }
    Ok(())
}

/// A unique exact evidence proof may retain its earlier identity when its
/// representative occurrence changes. Ambiguous repeated proofs stay separate.
fn preserve_unique_keys(connection: &Connection, receipt: &mut ImageReceipt) -> Result<(), CommandError> {
    let mut statement = connection.prepare("SELECT stable_key, evidence_sha256 FROM document_image_chunk_keys WHERE document_id = ?1")
        .map_err(CommandError::database)?;
    let rows = statement.query_map([&receipt.document_id], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
        .map_err(CommandError::database)?.collect::<Result<Vec<_>, _>>().map_err(CommandError::database)?;
    let mut old = BTreeMap::<String, Vec<String>>::new();
    for (key, proof) in rows { old.entry(proof).or_default().push(key); }
    let mut counts = BTreeMap::<String, usize>::new();
    let mut used = receipt.chunks.iter().map(|chunk| chunk.stable_key.clone()).collect::<std::collections::HashSet<_>>();
    for chunk in &receipt.chunks { *counts.entry(chunk.evidence_sha256.clone()).or_default() += 1; }
    for chunk in &mut receipt.chunks {
        if counts.get(&chunk.evidence_sha256) != Some(&1) { continue; }
        let Some(keys) = old.get(&chunk.evidence_sha256) else { continue; };
        if keys.len() == 1 && !used.contains(&keys[0]) {
            used.remove(&chunk.stable_key);
            chunk.stable_key = keys[0].clone();
            used.insert(chunk.stable_key.clone());
        }
    }
    Ok(())
}
