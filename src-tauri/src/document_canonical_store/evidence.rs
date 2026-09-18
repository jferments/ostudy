//! Native immutable evidence payloads and independently owned source occurrences.
//! Callers own the transaction. No provider call, file access or background work.
use std::collections::{BTreeMap, BTreeSet, HashMap};

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::{
    document_canonical::{
        sha256_hex,
        structure::{BlockStructure, SourceBounds, TableStructure},
        CanonicalBlock, CanonicalBlockRole, NormalizedBoundingBox,
    },
    error::CommandError,
    util::generate_id,
};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Payload {
    pub version: u32,
    pub role: CanonicalBlockRole,
    // Table cell values have one authority. Flat text is a validated projection
    // in document_source_blocks, not a second editable value in this payload.
    pub text: Option<String>,
    pub table: Option<TableStructure>,
}

impl Payload {
    pub(crate) fn validate(&self) -> Result<(), CommandError> {
        if self.version != 1 {
            return Err(invalid());
        }
        match (&self.text, &self.table) {
            (Some(text), None)
                if text.chars().count() <= 250_000
                    && !text
                        .chars()
                        .any(|ch| ch.is_control() && !matches!(ch, '\n' | '\t')) =>
            {
                Ok(())
            }
            (None, Some(table)) if self.role == CanonicalBlockRole::Table => table.validate(),
            _ => Err(invalid()),
        }
    }

    pub(crate) fn text(&self) -> Result<String, CommandError> {
        self.validate()?;
        match &self.table {
            Some(table) => table.text_projection(),
            None => Ok(self.text.clone().unwrap_or_default()),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Location {
    version: u32,
    source_node: Option<u32>,
    extractor_bounds: Option<SourceBounds>,
    normalized_bounds: Option<NormalizedBoundingBox>,
    cell_bounds: Vec<Option<SourceBounds>>,
    headings: Vec<String>,
    assets: Vec<String>,
    attributes: BTreeMap<String, String>,
}

fn encode(block: &CanonicalBlock) -> Result<(String, String, String), CommandError> {
    if let Some(structure) = &block.structure {
        structure.validate(block.role)?;
    }
    let mut table = block.structure.as_ref().and_then(|s| s.table.clone());
    let cell_bounds = table.as_mut().map_or_else(Vec::new, |table| {
        table
            .cells
            .iter_mut()
            .map(|cell| cell.bounds.take())
            .collect()
    });
    let payload = Payload {
        version: 1,
        role: block.role,
        text: table.is_none().then(|| block.text.clone()),
        table,
    };
    if payload.text()? != block.text {
        return Err(invalid());
    }
    let serialized = serde_json::to_string(&payload).map_err(CommandError::serialization)?;
    let location = Location {
        version: 1,
        source_node: block.structure.as_ref().and_then(|s| s.node),
        extractor_bounds: block.structure.as_ref().and_then(|s| s.bounds.clone()),
        normalized_bounds: block.geometry,
        cell_bounds,
        headings: block.heading_path.clone(),
        assets: block.asset_ids.clone(),
        attributes: block
            .attributes
            .iter()
            .filter(|(key, _)| !key.starts_with("_ostudy"))
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect(),
    };
    Ok((
        sha256_hex(serialized.as_bytes()),
        serialized,
        serde_json::to_string(&location).map_err(CommandError::serialization)?,
    ))
}

struct Previous {
    id: String,
    unit: i64,
    payload: String,
    location: String,
}

/// Reconcile identities by exact content and provenance, never by new order.
/// Ambiguous identical occurrences remain distinct and explicitly marked.
pub(crate) fn publish(
    connection: &Connection,
    document: &str,
    blocks: &[CanonicalBlock],
) -> Result<usize, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT id, source_unit_number, payload_sha256, location_json
         FROM document_evidence_occurrences WHERE document_id = ?1 ORDER BY id",
        )
        .map_err(CommandError::database)?;
    let previous = statement
        .query_map([document], |row| {
            Ok(Previous {
                id: row.get(0)?,
                unit: row.get(1)?,
                payload: row.get(2)?,
                location: row.get(3)?,
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    drop(statement);
    let mut available = BTreeMap::<(i64, String, String), Vec<String>>::new();
    for old in previous {
        available
            .entry((old.unit, old.payload, old.location))
            .or_default()
            .push(old.id);
    }
    let mut ambiguous = available
        .iter()
        .filter(|(_, ids)| ids.len() > 1)
        .map(|(key, _)| key.clone())
        .collect::<BTreeSet<_>>();
    let mut incoming = BTreeSet::new();
    for block in blocks {
        let (hash, _, location) = encode(block)?;
        let key = (block.source_unit_number, hash, location);
        if !incoming.insert(key.clone()) {
            ambiguous.insert(key);
        }
    }
    let mut source_files = connection
        .prepare(
            "SELECT u.unit_number, f.sha256 FROM document_source_units u
         JOIN document_source_files f ON f.document_id = u.document_id
           AND f.file_ordinal = u.source_file_ordinal WHERE u.document_id = ?1",
        )
        .map_err(CommandError::database)?;
    let files = source_files
        .query_map([document], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(CommandError::database)?
        .collect::<Result<HashMap<_, _>, _>>()
        .map_err(CommandError::database)?;
    drop(source_files);
    connection
        .execute(
            "UPDATE document_evidence_occurrences SET block_index = NULL WHERE document_id = ?1",
            [document],
        )
        .map_err(CommandError::database)?;
    connection.execute("DELETE FROM document_evidence_links WHERE occurrence_id IN (SELECT id FROM document_evidence_occurrences WHERE document_id = ?1)", [document])
        .map_err(CommandError::database)?;
    let mut nodes = HashMap::<u32, Vec<String>>::new();
    let mut links = Vec::new();
    for block in blocks {
        let file = files.get(&block.source_unit_number).ok_or_else(invalid)?;
        let (hash, payload, location) = encode(block)?;
        connection.execute("INSERT INTO document_evidence_payloads (sha256, payload_json) VALUES (?1, ?2) ON CONFLICT DO NOTHING", params![hash, payload])
            .map_err(CommandError::database)?;
        let key = (block.source_unit_number, hash.clone(), location.clone());
        let candidates = available.remove(&key).unwrap_or_default();
        let (id, state) = if !ambiguous.contains(&key) && candidates.len() == 1 {
            (candidates[0].clone(), "exact")
        } else {
            (
                generate_id(connection)?,
                if ambiguous.contains(&key) {
                    "ambiguous"
                } else {
                    "exact"
                },
            )
        };
        connection.execute(
            "INSERT INTO document_evidence_occurrences
             (id, document_id, block_index, source_file_sha256, source_unit_number, payload_sha256, location_json, identity_state)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(id) DO UPDATE SET block_index=excluded.block_index,
               source_file_sha256=excluded.source_file_sha256, identity_state=excluded.identity_state",
            params![id, document, block.block_index, file, block.source_unit_number, hash, location, state],
        ).map_err(CommandError::database)?;
        if let Some(structure) = &block.structure {
            if let Some(node) = structure.node {
                nodes.entry(node).or_default().push(id.clone());
            }
            for link in &structure.links {
                links.push((id.clone(), link.clone()));
            }
        }
    }
    connection.execute("DELETE FROM document_evidence_occurrences WHERE document_id = ?1 AND block_index IS NULL", [document])
        .map_err(CommandError::database)?;
    for (source, link) in links {
        let target = nodes
            .get(&link.target_node)
            .filter(|ids| ids.len() == 1)
            .map(|ids| &ids[0]);
        let kind = serde_json::to_value(link.kind).map_err(CommandError::serialization)?;
        connection.execute(
            "INSERT INTO document_evidence_links (occurrence_id, target_node, kind, target_occurrence_id) VALUES (?1, ?2, ?3, ?4)",
            params![source, link.target_node, kind.as_str().ok_or_else(invalid)?, target],
        ).map_err(CommandError::database)?;
    }
    Ok(blocks.len())
}

/// Upgrade retained canonical data once. Unknown cell/relationship structure is
/// unavailable, not inferred from flattened text or sent to an interpretation model.
pub(crate) fn backfill(connection: &Connection) -> Result<usize, CommandError> {
    let mut documents = connection
        .prepare("SELECT DISTINCT document_id FROM document_source_blocks ORDER BY document_id")
        .map_err(CommandError::database)?;
    let ids = documents
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    drop(documents);
    let mut count = 0;
    for document in ids {
        let mut statement = connection.prepare(
            "SELECT block_index, block_ordinal, source_unit_number, role, text,
             parent_heading, heading_path_json, geometry_json, confidence, asset_ids_json, attributes_json
             FROM document_source_blocks WHERE document_id = ?1 ORDER BY block_index",
        ).map_err(CommandError::database)?;
        let mut rows = statement.query([&document]).map_err(CommandError::database)?;
        let mut blocks = Vec::new();
        while let Some(row) = rows.next().map_err(CommandError::database)? {
            let role: String = row.get(3).map_err(CommandError::database)?;
            let headings: String = row.get(6).map_err(CommandError::database)?;
            let geometry: Option<String> = row.get(7).map_err(CommandError::database)?;
            let assets: String = row.get(9).map_err(CommandError::database)?;
            let attributes: String = row.get(10).map_err(CommandError::database)?;
            blocks.push(CanonicalBlock::new(
                row.get(0).map_err(CommandError::database)?,
                row.get(1).map_err(CommandError::database)?,
                row.get(2).map_err(CommandError::database)?,
                CanonicalBlockRole::from_adapter(&role)?,
                row.get::<_, String>(4).map_err(CommandError::database)?,
                row.get(5).map_err(CommandError::database)?,
                serde_json::from_str(&headings).map_err(CommandError::serialization)?,
                geometry
                    .map(|value| serde_json::from_str(&value))
                    .transpose()
                    .map_err(CommandError::serialization)?,
                row.get(8).map_err(CommandError::database)?,
                serde_json::from_str(&assets).map_err(CommandError::serialization)?,
                serde_json::from_str(&attributes).map_err(CommandError::serialization)?,
            ));
            if blocks.len() > 1_000_000 {
                return Err(invalid());
            }
        }
        drop(rows);
        drop(statement);
        count += publish(connection, &document, &blocks)?;
    }
    Ok(count)
}

/// Read-only current-format admission. Payload hashes are identities, never
/// permissions: every payload used by a consumer must be joined through its owner.
pub(crate) fn validate(connection: &Connection) -> Result<(), CommandError> {
    let mut statement = connection
        .prepare("SELECT sha256, payload_json FROM document_evidence_payloads")
        .map_err(CommandError::database)?;
    let mut rows = statement.query([]).map_err(CommandError::database)?;
    while let Some(row) = rows.next().map_err(CommandError::database)? {
        let hash: String = row.get(0).map_err(CommandError::database)?;
        let json: String = row.get(1).map_err(CommandError::database)?;
        if json.len() > 8_000_000 || sha256_hex(json.as_bytes()) != hash {
            return Err(invalid());
        }
        let payload: Payload = serde_json::from_str(&json).map_err(|_| invalid())?;
        payload.validate()?;
    }
    let mut locations = connection.prepare(
        "SELECT o.location_json, o.source_unit_number, o.block_index, p.payload_json, b.text, b.role, b.source_unit_number
         FROM document_evidence_occurrences o JOIN document_evidence_payloads p ON p.sha256=o.payload_sha256
         LEFT JOIN document_source_blocks b ON b.document_id=o.document_id AND b.block_index=o.block_index",
    ).map_err(CommandError::database)?;
    let mut rows = locations.query([]).map_err(CommandError::database)?;
    while let Some(row) = rows.next().map_err(CommandError::database)? {
        let json: String = row.get(0).map_err(CommandError::database)?;
        if json.len() > 8_000_000 {
            return Err(invalid());
        }
        let location: Location = serde_json::from_str(&json).map_err(|_| invalid())?;
        let serialized: String = row.get(3).map_err(CommandError::database)?;
        let payload: Payload = serde_json::from_str(&serialized).map_err(|_| invalid())?;
        let text: Option<String> = row.get(4).map_err(CommandError::database)?;
        let role: Option<String> = row.get(5).map_err(CommandError::database)?;
        if location.version != 1
            || row.get::<_, i64>(1).map_err(CommandError::database)?
                != row.get::<_, i64>(6).map_err(CommandError::database)?
            || row
                .get::<_, Option<i64>>(2)
                .map_err(CommandError::database)?
                .is_none()
            || text.as_deref() != Some(payload.text()?.as_str())
            || role.as_deref() != Some(payload.role.as_str())
            || location.cell_bounds.len() != payload.table.as_ref().map_or(0, |t| t.cells.len())
        {
            return Err(invalid());
        }
        let structure = BlockStructure {
            version: 1,
            node: location.source_node,
            bounds: location.extractor_bounds,
            table: None,
            links: Vec::new(),
        };
        structure.validate(payload.role)?;
        for bounds in location.cell_bounds.iter().flatten() {
            bounds.validate()?;
        }
        if location.normalized_bounds.is_some_and(|bounds| {
            [bounds.x, bounds.y, bounds.width, bounds.height]
                .iter()
                .any(|value| !value.is_finite())
                || bounds.width < 0.0
                || bounds.height < 0.0
        }) {
            return Err(invalid());
        }
    }
    let cross_document: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM document_evidence_links l
         JOIN document_evidence_occurrences a ON a.id=l.occurrence_id
         JOIN document_evidence_occurrences b ON b.id=l.target_occurrence_id
         WHERE a.document_id != b.document_id)",
            [],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    if cross_document {
        return Err(invalid());
    }
    Ok(())
}

fn invalid() -> CommandError {
    CommandError::new(
        "invalid_canonical_evidence",
        "Stored canonical evidence is unsupported, corrupt, or inconsistent.",
    )
}

#[cfg(test)]
mod tests;
