//! Durable semantic records. Callers supply a transaction when several sources
//! must change together. No provider request or implicit commit occurs here.

use std::collections::HashSet;

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

use crate::{database::Database, error::CommandError, util::unix_time_millis};

use super::contracts::{
    hash, invalid, is_hash, KnowledgeUnit, KnowledgeUnitInput, SourceKind, SourceRecord,
    MAX_UNITS_PER_SOURCE,
};

pub(crate) fn publish_source(
    connection: &Connection,
    source: &SourceRecord,
    parent_key: &str,
    now: i64,
) -> Result<(), CommandError> {
    source.validate()?;
    super::contracts::bounded(parent_key, 160, "source parent", false)?;
    let serialized = serde_json::to_string(source).map_err(CommandError::serialization)?;
    connection
        .execute(
            "INSERT INTO knowledge_sources (
                 id, kind, owner_id, segment_key, parent_key, owner_revision,
                 content_hash, record_json, active, document_id, note_id, card_id,
                 guide_id, conversation_id, turn_id, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?15)
             ON CONFLICT(id) DO UPDATE SET
                 owner_revision = excluded.owner_revision,
                 content_hash = excluded.content_hash, record_json = excluded.record_json,
                 active = 1, document_id = excluded.document_id, note_id = excluded.note_id,
                 card_id = excluded.card_id, guide_id = excluded.guide_id,
                 conversation_id = excluded.conversation_id, turn_id = excluded.turn_id,
                 updated_at = excluded.updated_at
             WHERE knowledge_sources.active = 0
                OR knowledge_sources.owner_revision <> excluded.owner_revision
                OR knowledge_sources.content_hash <> excluded.content_hash",
            params![
                source.id,
                source.kind.as_str(),
                source.owner_id,
                source.segment_key,
                parent_key,
                source.owner_revision,
                source.content_hash,
                serialized,
                source.document_id,
                source.note_id,
                source.card_id,
                source.guide_id,
                source.conversation_id,
                source.turn_id,
                now,
            ],
        )
        .map_err(CommandError::database)?;
    Ok(())
}

pub(crate) fn retire_missing_segments(
    connection: &Connection,
    kind: SourceKind,
    owner_id: &str,
    parent_key: &str,
    retained_ids: &[String],
) -> Result<(), CommandError> {
    let retained = serde_json::to_string(retained_ids).map_err(CommandError::serialization)?;
    connection
        .execute(
            "DELETE FROM knowledge_sources
             WHERE kind = ?1 AND owner_id = ?2 AND parent_key = ?3
               AND id NOT IN (SELECT value FROM json_each(?4))",
            params![kind.as_str(), owner_id, parent_key, retained],
        )
        .map_err(CommandError::database)?;
    Ok(())
}

pub(crate) fn load_source(
    connection: &Connection,
    id: &str,
) -> Result<Option<SourceRecord>, CommandError> {
    let serialized = connection
        .query_row(
            "SELECT record_json FROM knowledge_sources WHERE id = ?1 AND active = 1",
            [id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(CommandError::database)?;
    serialized
        .map(|value| {
            let source: SourceRecord =
                serde_json::from_str(&value).map_err(CommandError::serialization)?;
            source.validate()?;
            Ok(source)
        })
        .transpose()
}

pub(crate) fn source_is_current(
    connection: &Connection,
    source: &SourceRecord,
) -> Result<bool, CommandError> {
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM knowledge_sources
             WHERE id = ?1 AND active = 1 AND content_hash = ?2)",
            params![source.id, source.content_hash],
            |row| row.get::<_, bool>(0),
        )
        .map_err(CommandError::database)
}

pub(crate) fn load_units(
    connection: &Connection,
    source_id: &str,
) -> Result<Vec<KnowledgeUnit>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT u.record_json FROM knowledge_units u
             JOIN knowledge_sources s ON s.id = u.source_id
             WHERE u.source_id = ?1 AND s.active = 1 AND u.source_hash = s.content_hash
             ORDER BY u.id",
        )
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map([source_id], |row| row.get::<_, String>(0))
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    rows.into_iter()
        .map(|value| serde_json::from_str(&value).map_err(CommandError::serialization))
        .collect()
}

pub(crate) fn extraction_is_current(
    connection: &Connection,
    source: &SourceRecord,
    contract: &str,
) -> Result<bool, CommandError> {
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM knowledge_sources
             WHERE id = ?1 AND active = 1 AND content_hash = ?2
               AND extracted_hash = ?2 AND extraction_contract = ?3)",
            params![source.id, source.content_hash, contract],
            |row| row.get::<_, bool>(0),
        )
        .map_err(CommandError::database)
}

/// Persist one validated extraction only if the exact source is still current.
/// Invalid siblings are not quietly accepted: extraction is a single contract.
pub(crate) fn publish_units(
    database: &Database,
    source: &SourceRecord,
    contract: &str,
    inputs: Vec<KnowledgeUnitInput>,
) -> Result<Vec<KnowledgeUnit>, CommandError> {
    if !is_hash(contract) || inputs.len() > MAX_UNITS_PER_SOURCE {
        return Err(invalid("The knowledge extraction contract or unit count is invalid."));
    }
    let mut units = Vec::with_capacity(inputs.len());
    let mut seen = HashSet::new();
    for content in inputs {
        content.validate(source)?;
        let content_json = serde_json::to_string(&content).map_err(CommandError::serialization)?;
        let id = hash(&format!("unit-v1|{}|{content_json}", source.id))[..32].to_string();
        if seen.insert(id.clone()) {
            units.push(KnowledgeUnit {
                id,
                source_id: source.id.clone(),
                source_hash: source.content_hash.clone(),
                origin: source.origin,
                content,
            });
        }
    }
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        if !source_is_current(&transaction, source)? {
            return Err(stale_source());
        }
        let ids = units.iter().map(|unit| &unit.id).collect::<Vec<_>>();
        transaction
            .execute(
                "DELETE FROM knowledge_units WHERE source_id = ?1
                 AND id NOT IN (SELECT value FROM json_each(?2))",
                params![source.id, serde_json::to_string(&ids).map_err(CommandError::serialization)?],
            )
            .map_err(CommandError::database)?;
        for unit in &units {
            let serialized = serde_json::to_string(unit).map_err(CommandError::serialization)?;
            let content_hash = hash(
                &serde_json::to_string(&unit.content).map_err(CommandError::serialization)?,
            );
            transaction
                .execute(
                    "INSERT INTO knowledge_units (
                         id, source_id, source_hash, content_hash, record_json,
                         extraction_contract, created_at, updated_at
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)
                     ON CONFLICT(id) DO UPDATE SET source_hash = excluded.source_hash,
                         content_hash = excluded.content_hash, record_json = excluded.record_json,
                         extraction_contract = excluded.extraction_contract, updated_at = excluded.updated_at",
                    params![unit.id, source.id, source.content_hash, content_hash, serialized, contract, now],
                )
                .map_err(CommandError::database)?;
        }
        transaction
            .execute(
                "UPDATE knowledge_sources SET extraction_contract = ?1, extracted_hash = ?2,
                     updated_at = ?3 WHERE id = ?4 AND active = 1 AND content_hash = ?2",
                params![contract, source.content_hash, now, source.id],
            )
            .map_err(CommandError::database)?;
        transaction.commit().map_err(CommandError::database)?;
        Ok(())
    })?;
    Ok(units)
}

/// Explicit validated artifact-to-unit links. A nearest-neighbour result by
/// itself is never permission to attach a document citation to an authored card.
pub(crate) fn link_artifact_units(
    connection: &Connection,
    artifact: &SourceRecord,
    unit_ids: &[String],
    relationship: &str,
    now: i64,
) -> Result<(), CommandError> {
    if !matches!(relationship, "assesses" | "explains" | "mentions")
        || unit_ids.len() > 64
        || !source_is_current(connection, artifact)?
    {
        return Err(invalid("The semantic artifact association is invalid or stale."));
    }
    for id in unit_ids {
        let changed = connection
            .execute(
                "INSERT INTO knowledge_artifact_links (source_id, unit_id, source_hash, relationship, created_at)
                 SELECT ?1, u.id, ?2, ?3, ?4 FROM knowledge_units u
                 JOIN knowledge_sources s ON s.id = u.source_id
                 WHERE u.id = ?5 AND s.active = 1 AND s.content_hash = u.source_hash
                 ON CONFLICT(source_id, unit_id) DO UPDATE SET
                     source_hash = excluded.source_hash, relationship = excluded.relationship",
                params![artifact.id, artifact.content_hash, relationship, now, id],
            )
            .map_err(CommandError::database)?;
        if changed != 1 {
            return Err(stale_source());
        }
    }
    Ok(())
}

pub(crate) fn linked_units(
    connection: &Connection,
    artifact: &SourceRecord,
) -> Result<Vec<KnowledgeUnit>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT u.record_json FROM knowledge_artifact_links l
             JOIN knowledge_units u ON u.id = l.unit_id
             JOIN knowledge_sources s ON s.id = u.source_id
             WHERE l.source_id = ?1 AND l.source_hash = ?2
               AND s.active = 1 AND s.content_hash = u.source_hash
             ORDER BY u.id LIMIT 64",
        )
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map(params![artifact.id, artifact.content_hash], |row| row.get::<_, String>(0))
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    rows.into_iter()
        .map(|value| serde_json::from_str(&value).map_err(CommandError::serialization))
        .collect()
}

pub(crate) fn stale_source() -> CommandError {
    CommandError::new(
        "semantic_source_changed",
        "This material changed while AI processing was running. Prepare its current revision and retry.",
    )
}
