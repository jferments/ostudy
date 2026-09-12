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
                OR knowledge_sources.content_hash <> excluded.content_hash
                OR knowledge_sources.record_json <> excluded.record_json",
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
             WHERE id = ?1 AND active = 1 AND content_hash = ?2 AND owner_revision = ?3)",
            params![source.id, source.content_hash, source.owner_revision],
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
             WHERE (u.source_id = ?1 OR EXISTS (
                 SELECT 1 FROM knowledge_unit_evidence e WHERE e.unit_id = u.id AND e.source_id = ?1))
               AND s.active = 1 AND u.source_hash = s.content_hash
               AND u.extraction_contract = s.extraction_contract
             ORDER BY u.id",
        )
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map([source_id], |row| row.get::<_, String>(0))
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    decode_current_units(connection, rows)
}

/// Saved interpretation belongs to a source revision, not a consuming model.
/// The producer's contract remains recorded for provenance and checkpoint repair.
pub(crate) fn extraction_available(
    connection: &Connection,
    source: &SourceRecord,
) -> Result<bool, CommandError> {
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM knowledge_sources WHERE id=?1 AND active=1
         AND content_hash=?2 AND owner_revision=?3 AND extracted_hash=?2
         AND length(extraction_contract)=64)",
            params![source.id, source.content_hash, source.owner_revision],
            |row| row.get(0),
        )
        .map_err(CommandError::database)
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
/// The caller admits individual units; window coverage is recorded separately.
pub(crate) fn publish_units(
    database: &Database,
    source: &SourceRecord,
    contract: &str,
    inputs: Vec<KnowledgeUnitInput>,
) -> Result<Vec<KnowledgeUnit>, CommandError> {
    database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        let units = publish_units_in(&transaction, source, contract, inputs, unix_time_millis()?)?;
        transaction.commit().map_err(CommandError::database)?;
        Ok(units)
    })
}

pub(crate) fn publish_units_in(
    transaction: &Connection,
    source: &SourceRecord,
    contract: &str,
    inputs: Vec<KnowledgeUnitInput>,
    now: i64,
) -> Result<Vec<KnowledgeUnit>, CommandError> {
    if !is_hash(contract) || inputs.len() > MAX_UNITS_PER_SOURCE {
        return Err(invalid(
            "The knowledge extraction contract or unit count is invalid.",
        ));
    }
    if !source_is_current(transaction, source)? {
        return Err(stale_source());
    }
    let mut units = Vec::with_capacity(inputs.len());
    let mut seen = HashSet::new();
    for content in inputs {
        content.validate(source)?;
        validate_grounding(transaction, &content)?;
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
    let ids = units.iter().map(|unit| &unit.id).collect::<Vec<_>>();
    transaction
        .execute(
            "DELETE FROM knowledge_units WHERE source_id = ?1
                 AND id NOT IN (SELECT value FROM json_each(?2))",
            params![
                source.id,
                serde_json::to_string(&ids).map_err(CommandError::serialization)?
            ],
        )
        .map_err(CommandError::database)?;
    for unit in &units {
        let serialized = serde_json::to_string(unit).map_err(CommandError::serialization)?;
        let content_hash =
            hash(&serde_json::to_string(&unit.content).map_err(CommandError::serialization)?);
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
        transaction
            .execute(
                "DELETE FROM knowledge_unit_evidence WHERE unit_id = ?1",
                [&unit.id],
            )
            .map_err(CommandError::database)?;
        if let Some(grounding) = &unit.content.grounding {
            for reference in &grounding.evidence {
                transaction.execute(
                        "INSERT INTO knowledge_unit_evidence (unit_id, evidence_id, source_id, source_hash) VALUES (?1, ?2, ?3, ?4)",
                        params![unit.id, reference.id, reference.source_id, reference.source_hash],
                    ).map_err(CommandError::database)?;
            }
        }
    }
    transaction
        .execute(
            "UPDATE knowledge_sources SET extraction_contract = ?1, extracted_hash = ?2,
                     updated_at = ?3 WHERE id = ?4 AND active = 1 AND content_hash = ?2",
            params![contract, source.content_hash, now, source.id],
        )
        .map_err(CommandError::database)?;
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
        return Err(invalid(
            "The semantic artifact association is invalid or stale.",
        ));
    }
    for id in unit_ids {
        let serialized: String = connection
            .query_row(
                "SELECT record_json FROM knowledge_units WHERE id = ?1",
                [id],
                |row| row.get(0),
            )
            .optional()
            .map_err(CommandError::database)?
            .ok_or_else(stale_source)?;
        let unit: KnowledgeUnit =
            serde_json::from_str(&serialized).map_err(CommandError::serialization)?;
        validate_grounding(connection, &unit.content)?;
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
        .query_map(params![artifact.id, artifact.content_hash], |row| {
            row.get::<_, String>(0)
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    decode_current_units(connection, rows)
}

pub(crate) fn validate_grounding(
    connection: &Connection,
    content: &KnowledgeUnitInput,
) -> Result<(), CommandError> {
    if let Some(grounding) = &content.grounding {
        for reference in &grounding.evidence {
            let source = load_source(connection, &reference.source_id)?.ok_or_else(stale_source)?;
            reference.validate(&source).map_err(|_| stale_source())?;
        }
    }
    Ok(())
}

fn decode_current_units(
    connection: &Connection,
    rows: Vec<String>,
) -> Result<Vec<KnowledgeUnit>, CommandError> {
    let mut result = Vec::new();
    for value in rows {
        let unit: KnowledgeUnit =
            serde_json::from_str(&value).map_err(CommandError::serialization)?;
        // Stale derived units are not usable evidence. Storage/decoding errors
        // remain errors rather than being mistaken for an empty library.
        let primary = load_source(connection, &unit.source_id)?;
        let checked = primary
            .filter(|source| {
                source.content_hash == unit.source_hash && source.origin == unit.origin
            })
            .ok_or_else(stale_source)
            .and_then(|source| {
                unit.content.validate(&source)?;
                validate_grounding(connection, &unit.content)
            });
        match checked {
            Ok(()) => result.push(unit),
            Err(error) if error.code == "semantic_source_changed" => {}
            Err(error) => return Err(error),
        }
    }
    Ok(result)
}

pub(crate) fn stale_source() -> CommandError {
    CommandError::new(
        "semantic_source_changed",
        "This material changed while AI processing was running. Prepare its current revision and retry.",
    )
}
