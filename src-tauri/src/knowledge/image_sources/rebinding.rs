//! Rebind native image occurrence locations without changing learned statements.

use std::collections::BTreeMap;

use rusqlite::{params, Connection};
use serde_json::Value;

use crate::error::CommandError;

use super::super::{
    contracts::{hash, KnowledgeUnit, SourceRecord},
    evidence::EvidenceReference,
    repository,
};

pub(super) fn rebind_dependents(
    connection: &Connection,
    sources: &BTreeMap<String, (SourceRecord, SourceRecord)>,
) -> Result<(), CommandError> {
    if sources.is_empty() {
        return Ok(());
    }
    let ids = serde_json::to_string(&sources.keys().collect::<Vec<_>>())
        .map_err(CommandError::serialization)?;
    let mut statement = connection.prepare(
        "SELECT DISTINCT u.id, u.record_json FROM knowledge_units u
         LEFT JOIN knowledge_unit_evidence e ON e.unit_id = u.id
         WHERE u.source_id IN (SELECT value FROM json_each(?1)) OR e.source_id IN (SELECT value FROM json_each(?1))",
    ).map_err(CommandError::database)?;
    let units = statement
        .query_map([&ids], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    for (id, json) in units {
        let mut value: Value = serde_json::from_str(&json).map_err(CommandError::serialization)?;
        if rebind(&mut value, sources)? {
            let unit: KnowledgeUnit =
                serde_json::from_value(value).map_err(CommandError::serialization)?;
            let primary = repository::load_source(connection, &unit.source_id)?
                .ok_or_else(repository::stale_source)?;
            unit.content.validate(&primary)?;
            repository::validate_grounding(connection, &unit.content)?;
            connection
                .execute(
                    "UPDATE knowledge_units SET record_json = ?1, content_hash = ?2 WHERE id = ?3",
                    params![
                        serde_json::to_string(&unit).map_err(CommandError::serialization)?,
                        hash(&serde_json::to_string(&unit.content).map_err(CommandError::serialization)?),
                        id
                    ],
                )
                .map_err(CommandError::database)?;
        }
    }
    let mut statement = connection.prepare(
        "SELECT DISTINCT w.id, w.report_json FROM knowledge_analysis_windows w
         JOIN knowledge_analysis_sources s ON s.window_id = w.id WHERE s.source_id IN (SELECT value FROM json_each(?1))",
    ).map_err(CommandError::database)?;
    let reports = statement
        .query_map([ids], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    for (id, json) in reports {
        let mut value: Value = serde_json::from_str(&json).map_err(CommandError::serialization)?;
        if rebind(&mut value, sources)? {
            let report: super::super::extraction::decode::AnalysisReport =
                serde_json::from_value(value).map_err(CommandError::serialization)?;
            connection
                .execute(
                    "UPDATE knowledge_analysis_windows SET report_json = ?1 WHERE id = ?2",
                    params![serde_json::to_string(&report).map_err(CommandError::serialization)?, id],
                )
                .map_err(CommandError::database)?;
        }
    }
    Ok(())
}

/// Only complete application-owned evidence references are changed. Arbitrary
/// JSON strings, source wording, qualifications, and generated artifacts are not.
fn rebind(
    value: &mut Value,
    sources: &BTreeMap<String, (SourceRecord, SourceRecord)>,
) -> Result<bool, CommandError> {
    if value.as_object().is_some_and(|object| {
        object.contains_key("selector") && object.contains_key("sourceHash")
    }) {
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
        Value::Object(object) => {
            for child in object.values_mut() {
                changed |= rebind(child, sources)?;
            }
        }
        Value::Array(array) => {
            for child in array {
                changed |= rebind(child, sources)?;
            }
        }
        _ => {}
    }
    Ok(changed)
}
