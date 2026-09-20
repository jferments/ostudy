//! Image evidence identity and explicit location rebinding. Native pixel/text
//! proofs, not a model or UI claim, authorize reuse across source ordering edits.

use std::collections::{BTreeMap, HashSet};

use rusqlite::{params, Connection};
use serde_json::Value;

use crate::{documents::image_sets::storage, error::CommandError};

use super::{
    adapters::{self, SourceSeed},
    contracts::{hash, KnowledgeUnit, SourceKind, SourceRecord},
    evidence::EvidenceReference,
    repository,
};

pub(crate) fn available(connection: &Connection, source: &SourceRecord) -> Result<bool, CommandError> {
    if source.kind != SourceKind::Document {
        return Ok(true);
    }
    // Runtime callers receive the latest database. Historical read-only entry
    // validation explicitly uses load_retained_source instead.
    storage::ready(connection, &source.owner_id)
}

/// Called only inside the guarded canonical image publication transaction.
/// Publish all new locations before retiring anything, so a moved source cannot
/// be deleted by another source's old numeric parent position.
pub(crate) fn reconcile(connection: &Connection, document_id: &str, now: i64) -> Result<(), CommandError> {
    let mut statement = connection.prepare(
        "SELECT record_json FROM knowledge_sources WHERE kind = 'document' AND document_id = ?1",
    ).map_err(CommandError::database)?;
    let old = statement.query_map([document_id], |row| row.get::<_, String>(0))
        .map_err(CommandError::database)?.collect::<Result<Vec<_>, _>>().map_err(CommandError::database)?;
    let old = old.into_iter().map(|text| {
        let source: SourceRecord = serde_json::from_str(&text).map_err(CommandError::serialization)?;
        source.validate()?;
        Ok((source.id.clone(), source))
    }).collect::<Result<BTreeMap<_, _>, CommandError>>()?;
    let mut indexes = connection.prepare(
        "SELECT chunk_index FROM document_image_chunk_keys WHERE document_id = ?1 ORDER BY chunk_index",
    ).map_err(CommandError::database)?;
    let indexes = indexes.query_map([document_id], |row| row.get::<_, i64>(0))
        .map_err(CommandError::database)?.collect::<Result<Vec<_>, _>>().map_err(CommandError::database)?;
    let mut retained = Vec::new();
    let mut rebound = BTreeMap::new();
    for index in indexes {
        let parent_key = index.to_string();
        let seed = SourceSeed { kind: SourceKind::Document, owner_id: document_id.to_string(), parent_key: parent_key.clone() };
        for mut source in adapters::read_seed(connection, &seed)? {
            source.refresh_identity()?;
            if let Some(previous) = old.get(&source.id) {
                if previous.identity_version == 3 && source.identity_version == 3
                    && previous.content_hash == source.content_hash && previous.owner_revision == source.owner_revision
                    && previous.text == source.text && previous.images == source.images {
                    rebound.insert(source.id.clone(), (previous.clone(), source.clone()));
                }
            }
            repository::publish_source(connection, &source, &parent_key, now)?;
            connection.execute("UPDATE knowledge_sources SET parent_key = ?1 WHERE id = ?2 AND parent_key <> ?1",
                params![parent_key, source.id]).map_err(CommandError::database)?;
            retained.push(source.id);
        }
    }
    connection.execute(
        "DELETE FROM knowledge_sources WHERE kind = 'document' AND document_id = ?1
         AND id NOT IN (SELECT value FROM json_each(?2))",
        params![document_id, serde_json::to_string(&retained).map_err(CommandError::serialization)?],
    ).map_err(CommandError::database)?;
    rebind_dependents(connection, &rebound)?;
    Ok(())
}

fn rebind_dependents(connection: &Connection, sources: &BTreeMap<String, (SourceRecord, SourceRecord)>) -> Result<(), CommandError> {
    if sources.is_empty() { return Ok(()); }
    let ids = serde_json::to_string(&sources.keys().collect::<Vec<_>>()).map_err(CommandError::serialization)?;
    let mut statement = connection.prepare(
        "SELECT DISTINCT u.id, u.record_json FROM knowledge_units u
         LEFT JOIN knowledge_unit_evidence e ON e.unit_id = u.id
         WHERE u.source_id IN (SELECT value FROM json_each(?1)) OR e.source_id IN (SELECT value FROM json_each(?1))",
    ).map_err(CommandError::database)?;
    let units = statement.query_map([&ids], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
        .map_err(CommandError::database)?.collect::<Result<Vec<_>, _>>().map_err(CommandError::database)?;
    for (id, json) in units {
        let mut value: Value = serde_json::from_str(&json).map_err(CommandError::serialization)?;
        if rebind(&mut value, sources)? {
            let unit: KnowledgeUnit = serde_json::from_value(value).map_err(CommandError::serialization)?;
            let primary = repository::load_source(connection, &unit.source_id)?.ok_or_else(repository::stale_source)?;
            unit.content.validate(&primary)?;
            repository::validate_grounding(connection, &unit.content)?;
            connection.execute("UPDATE knowledge_units SET record_json = ?1, content_hash = ?2 WHERE id = ?3",
                params![serde_json::to_string(&unit).map_err(CommandError::serialization)?,
                hash(&serde_json::to_string(&unit.content).map_err(CommandError::serialization)?), id]).map_err(CommandError::database)?;
        }
    }
    let mut statement = connection.prepare(
        "SELECT DISTINCT w.id, w.report_json FROM knowledge_analysis_windows w
         JOIN knowledge_analysis_sources s ON s.window_id = w.id WHERE s.source_id IN (SELECT value FROM json_each(?1))",
    ).map_err(CommandError::database)?;
    let reports = statement.query_map([ids], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
        .map_err(CommandError::database)?.collect::<Result<Vec<_>, _>>().map_err(CommandError::database)?;
    for (id, json) in reports {
        let mut value: Value = serde_json::from_str(&json).map_err(CommandError::serialization)?;
        if rebind(&mut value, sources)? {
            let report: super::extraction::decode::AnalysisReport = serde_json::from_value(value).map_err(CommandError::serialization)?;
            connection.execute("UPDATE knowledge_analysis_windows SET report_json = ?1 WHERE id = ?2",
                params![serde_json::to_string(&report).map_err(CommandError::serialization)?, id]).map_err(CommandError::database)?;
        }
    }
    Ok(())
}

/// Only complete application-owned evidence references are changed. Arbitrary
/// JSON strings, source wording, qualifications, and generated artifacts are not.
fn rebind(value: &mut Value, sources: &BTreeMap<String, (SourceRecord, SourceRecord)>) -> Result<bool, CommandError> {
    if value.as_object().is_some_and(|object| object.contains_key("selector") && object.contains_key("sourceHash")) {
      if let Ok(mut reference) = serde_json::from_value::<EvidenceReference>(value.clone()) {
        if let Some((old, new)) = sources.get(&reference.source_id) {
            reference.validate(old)?;
            if reference.locator != new.locator {
                reference.locator = new.locator.clone();
                reference.validate(new)?;
                *value = serde_json::to_value(reference).map_err(CommandError::serialization)?;
                return Ok(true);
            }
        }
        return Ok(false);
      }
    }
    let mut changed = false;
    match value {
        Value::Object(object) => for child in object.values_mut() { changed |= rebind(child, sources)?; },
        Value::Array(array) => for child in array { changed |= rebind(child, sources)?; },
        _ => {}
    }
    Ok(changed)
}

pub(crate) fn validate_current(connection: &Connection, document_id: &str) -> Result<(), CommandError> {
    if !storage::ready(connection, document_id)? { return Ok(()); }
    let mut statement = connection.prepare(
        "SELECT parent_key, record_json FROM knowledge_sources WHERE kind = 'document' AND document_id = ?1 AND active = 1",
    ).map_err(CommandError::database)?;
    let rows = statement.query_map([document_id], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
        .map_err(CommandError::database)?.collect::<Result<Vec<_>, _>>().map_err(CommandError::database)?;
    let mut checked = HashSet::new();
    for (parent_key, json) in rows {
        let stored: SourceRecord = serde_json::from_str(&json).map_err(CommandError::serialization)?;
        if stored.identity_version != 3 || !checked.insert(stored.id.clone()) { return Err(repository::stale_source()); }
        let mut verified = stored.clone();
        verified.refresh_identity()?;
        if verified.id != stored.id || verified.content_hash != stored.content_hash { return Err(repository::stale_source()); }
        let seed = SourceSeed { kind: SourceKind::Document, owner_id: document_id.to_string(), parent_key };
        let mut found = false;
        for mut current in adapters::read_seed(connection, &seed)? {
            current.refresh_identity()?;
            if current.id == stored.id && current.content_hash == stored.content_hash
                && current.owner_revision == stored.owner_revision && current.locator == stored.locator { found = true; }
        }
        if !found { return Err(repository::stale_source()); }
    }
    Ok(())
}
