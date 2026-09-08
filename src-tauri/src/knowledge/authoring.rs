//! Knowledge-first authoring over the same canonical units used by the tutor.
//! Provider-selected IDs are resolved here; providers never supply provenance
//! hashes. Publication helpers join the caller's transaction, never commit it.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    ai::{document_intelligence::DocumentTaskContext, AiRuntimeState},
    database::Database,
    error::CommandError,
    paths::AppPaths,
    task_cancellation::{current_signal, set_current_task_phase, CancellationSignal},
    util::validate_entity_id,
};

use super::{
    adapters::{materialize_sources, read_seed, SourceSeed},
    contracts::{hash, ContentOrigin, KnowledgeUnit, SourceKind},
    extraction::{batch_ranges, ensure_units_batch, extraction_contract},
    repository,
    runtime::check_canceled,
};

pub(crate) const AUTHORING_VERSION: i64 = 1;
pub(crate) const CARD_SUPPORT_KEY: &str = "_ostudyKnowledge";
const MAX_AUTHORING_SOURCES: usize = 100_000;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct UnitSupport {
    pub unit_id: String,
    pub source_id: String,
    pub source_hash: String,
    pub content_hash: String,
}

impl UnitSupport {
    fn from_unit(unit: &KnowledgeUnit) -> Result<Self, CommandError> {
        Ok(Self {
            unit_id: unit.id.clone(),
            source_id: unit.source_id.clone(),
            source_hash: unit.source_hash.clone(),
            content_hash: hash(
                &serde_json::to_string(&unit.content).map_err(CommandError::serialization)?,
            ),
        })
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct KnowledgeCatalog {
    pub units: Vec<KnowledgeUnit>,
}

impl KnowledgeCatalog {
    pub(crate) fn prompt(&self) -> Result<String, CommandError> {
        // The exact supporting excerpts are original evidence, not a second
        // model's summary. Full canonical passages remain in the library.
        let units = self.units.iter().map(|unit| json!({
            "id": unit.id, "title": unit.content.title, "kind": unit.content.kind,
            "statement": unit.content.statement, "qualifications": unit.content.qualifications,
            "topic": unit.content.topic, "sourceId": unit.source_id,
            "supportingExcerpt": unit.content.supporting_excerpt,
        })).collect::<Vec<_>>();
        serde_json::to_string(&units).map_err(CommandError::serialization)
    }

    pub(crate) fn fingerprint(&self) -> Result<String, CommandError> {
        Ok(hash(&format!(
            "authoring-v{AUTHORING_VERSION}|{}",
            serde_json::to_string(&self.units).map_err(CommandError::serialization)?
        )))
    }

    pub(crate) fn select(
        &self,
        ids: &[String],
        maximum: usize,
    ) -> Result<Vec<UnitSupport>, CommandError> {
        if ids.is_empty()
            || ids.len() > maximum
            || ids.iter().collect::<HashSet<_>>().len() != ids.len()
        {
            return Err(invalid(
                "Select distinct knowledge targets within this artifact's supported limit.",
            ));
        }
        ids.iter().map(|id| {
            self.units.iter().find(|unit| &unit.id == id)
                .ok_or_else(|| invalid("The model selected a knowledge unit outside its supplied source context."))
                .and_then(UnitSupport::from_unit)
        }).collect()
    }

    pub(crate) fn validates(&self, support: &[UnitSupport], maximum: usize) -> bool {
        let ids = support
            .iter()
            .map(|item| item.unit_id.clone())
            .collect::<Vec<_>>();
        self.select(&ids, maximum)
            .is_ok_and(|expected| expected == support)
    }
}

/// Prepare the explicitly selected document chunks before either generator
/// writes material. No embedding profile is required to author cards or guides.
/// Serial extraction avoids competing interpretations of overlapping windows;
/// generated artifacts may still be authored concurrently afterward.
pub(crate) async fn prepare_document(
    database: &Database,
    paths: &AppPaths,
    runtime: &AiRuntimeState,
    task: &DocumentTaskContext,
    document_id: &str,
    chunk_indices: &[i64],
) -> Result<(), CommandError> {
    validate_entity_id(document_id, "knowledge document")?;
    let cancellation = current_signal().unwrap_or_else(CancellationSignal::new);
    let indices = chunk_indices.iter().copied().collect::<BTreeSet<_>>();
    if indices.is_empty()
        || indices.len() > MAX_AUTHORING_SOURCES
        || indices.iter().any(|index| *index < 0)
    {
        return Err(invalid(
            "The authoring source selection is empty or invalid.",
        ));
    }
    let seeds = indices
        .into_iter()
        .map(|index| SourceSeed {
            kind: SourceKind::Document,
            owner_id: document_id.to_string(),
            parent_key: index.to_string(),
        })
        .collect::<Vec<_>>();
    let mut completed = 0;
    for batch in seeds.chunks(32) {
        check_canceled(&cancellation)?;
        let sources = materialize_sources(database, batch)?;
        // A deleted source must not turn into an apparently successful empty
        // extraction and then leave old generated artifacts reusable.
        for seed in batch {
            if !sources.iter().any(|source| {
                source
                    .segment_key
                    .starts_with(&format!("{}:", seed.parent_key))
            }) {
                return Err(repository::stale_source());
            }
        }
        for range in batch_ranges(&sources) {
            ensure_units_batch(
                database,
                paths,
                runtime,
                task,
                &sources[range],
                &cancellation,
            )
            .await?;
        }
        completed += batch.len();
        set_current_task_phase(
            "understanding material",
            None,
            completed,
            seeds.len(),
            None,
            None,
            None,
        );
    }
    check_canceled(&cancellation)?;
    task.ensure_current(database)
}

pub(crate) fn catalog_for_chunks(
    database: &Database,
    document_id: &str,
    chunk_indices: &[i64],
    task: &DocumentTaskContext,
) -> Result<KnowledgeCatalog, CommandError> {
    let wanted = serde_json::to_string(chunk_indices).map_err(CommandError::serialization)?;
    let contract = extraction_contract(task)?;
    database.with_connection(|connection| {
        let mut statement = connection
            .prepare(
                "SELECT id FROM knowledge_sources WHERE kind = 'document' AND document_id = ?1
             AND active = 1 AND parent_key IN (SELECT CAST(value AS TEXT) FROM json_each(?2))
             ORDER BY CAST(parent_key AS INTEGER), segment_key",
            )
            .map_err(CommandError::database)?;
        let ids = statement
            .query_map(params![document_id, wanted], |row| row.get::<_, String>(0))
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        if ids.is_empty() {
            return Err(repository::stale_source());
        }
        let mut units = BTreeMap::new();
        for id in ids {
            let source =
                repository::load_source(connection, &id)?.ok_or_else(repository::stale_source)?;
            if !repository::extraction_is_current(connection, &source, &contract)? {
                return Err(repository::stale_source());
            }
            for unit in repository::load_units(connection, &id)? {
                units.insert(unit.id.clone(), unit);
            }
        }
        Ok(KnowledgeCatalog {
            units: units.into_values().collect(),
        })
    })
}

pub(crate) fn card_support(card: &Value) -> Result<Vec<UnitSupport>, CommandError> {
    let support: Vec<UnitSupport> = serde_json::from_value(
        card.get(CARD_SUPPORT_KEY)
            .cloned()
            .ok_or_else(|| invalid("A generated card is missing its knowledge targets."))?,
    )
    .map_err(|_| invalid("A generated card contains invalid knowledge support."))?;
    if support.is_empty() || support.len() > 2 {
        return Err(invalid(
            "A card requires one target, or two for a necessary comparison.",
        ));
    }
    Ok(support)
}

pub(crate) fn without_card_support(mut card: Value) -> Value {
    if let Some(object) = card.as_object_mut() {
        object.remove(CARD_SUPPORT_KEY);
    }
    card
}

pub(crate) fn validate_current_support(
    connection: &Connection,
    document_id: &str,
    support: &[UnitSupport],
) -> Result<(), CommandError> {
    let mut seen = HashSet::new();
    for item in support {
        if !seen.insert(&item.unit_id) {
            return Err(invalid("An artifact repeats a knowledge target."));
        }
        let current: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM knowledge_units u JOIN knowledge_sources s ON s.id = u.source_id
             WHERE u.id = ?1 AND u.source_id = ?2 AND u.source_hash = ?3 AND u.content_hash = ?4
               AND s.content_hash = ?3 AND s.active = 1 AND s.kind = 'document' AND s.document_id = ?5)",
            params![item.unit_id, item.source_id, item.source_hash, item.content_hash, document_id],
            |row| row.get(0),
        ).map_err(CommandError::database)?;
        if !current {
            return Err(repository::stale_source());
        }
    }
    Ok(())
}

pub(crate) fn publish_artifact_links(
    connection: &Connection,
    document_id: &str,
    seed: &SourceSeed,
    support: &[UnitSupport],
    relationship: &str,
    now: i64,
) -> Result<(), CommandError> {
    validate_current_support(connection, document_id, support)?;
    let ids = support
        .iter()
        .map(|item| item.unit_id.clone())
        .collect::<Vec<_>>();
    let mut sources = read_seed(connection, seed)?;
    if sources.is_empty() {
        return Err(repository::stale_source());
    }
    for source in &mut sources {
        source.refresh_identity()?;
        repository::publish_source(connection, source, &seed.parent_key, now)?;
        // Carried learner overrides/annotations are not certified by links that
        // the model selected for the underlying generated section.
        if source.origin == ContentOrigin::Generated {
            repository::link_artifact_units(connection, source, &ids, relationship, now)?;
        }
    }
    Ok(())
}

pub(crate) fn support_schema(maximum: usize) -> Value {
    json!({"type":"array", "minItems":1, "maxItems":maximum,
        "items":{"type":"string", "minLength":32, "maxLength":32}})
}

fn invalid(message: &str) -> CommandError {
    CommandError::new("ai_document_output_invalid", message)
}

#[cfg(test)]
pub(crate) mod tests;
