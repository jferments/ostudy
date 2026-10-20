//! Knowledge-first authoring over the same canonical units used by the tutor.
//! Provider-selected IDs are resolved here; providers never supply provenance
//! hashes. Publication helpers join the caller's transaction, never commit it.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    ai::{
        concurrency::{run_bounded_provider_queue, QueueControl},
        document_intelligence::DocumentTaskContext,
        AiRuntimeState,
    },
    database::Database,
    error::CommandError,
    paths::AppPaths,
    task_cancellation::{current_signal, set_current_task_phase, CancellationSignal},
    util::validate_entity_id,
};

use super::{
    adapters::{materialize_source_groups, read_seed, SourceSeed},
    contracts::{hash, ContentOrigin, KnowledgeUnit, SourceKind},
    extraction::{batch_ranges, ensure_units_batch, extraction_contract, retry_units_batch},
    repository,
    runtime::check_canceled,
};

pub(crate) const AUTHORING_VERSION: i64 = 4;
pub(crate) const CARD_SUPPORT_KEY: &str = "_ostudyKnowledge";
const MAX_AUTHORING_SOURCES: usize = 100_000;

mod prompt;

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
    pub document_core_units: BTreeSet<String>,
    pub preparation: super::learning_context::PreparationSummary,
}

impl KnowledgeCatalog {
    pub(crate) fn unit_alias(index: usize) -> String {
        format!("K{index}")
    }

    fn resolve_unit_id<'a>(&'a self, id: &str) -> Option<&'a KnowledgeUnit> {
        if let Some(index) = id
            .strip_prefix('K')
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|index| Self::unit_alias(*index) == id)
        {
            return self.units.get(index);
        }
        self.units.iter().find(|unit| unit.id == id)
    }

    pub(crate) fn support_schema(&self, maximum: usize) -> Value {
        json!({"type":"array", "minItems":1, "maxItems":maximum,
            "items":{"type":"string","minLength":2,"maxLength":16,
                "pattern":"^K(?:0|[1-9][0-9]*)$"}})
    }

    pub(crate) fn prompt(&self) -> Result<String, CommandError> {
        let mut native_evidence = BTreeMap::<String, (String, String)>::new();
        for unit in &self.units {
            if let Some(grounding) = &unit.content.grounding {
                for reference in &grounding.evidence {
                    native_evidence
                        .entry(reference.id.clone())
                        .or_insert_with(|| {
                            (reference.source_id.clone(), reference.excerpt.clone())
                        });
                }
            } else {
                native_evidence
                    .entry(format!("legacy:{}", unit.id))
                    .or_insert_with(|| {
                        (
                            unit.source_id.clone(),
                            unit.content.supporting_excerpt.clone(),
                        )
                    });
            }
        }

        let source_ids = self
            .units
            .iter()
            .flat_map(|unit| {
                std::iter::once(unit.source_id.clone()).chain(
                    unit.content.grounding.iter().flat_map(|grounding| {
                        grounding
                            .evidence
                            .iter()
                            .map(|reference| reference.source_id.clone())
                    }),
                )
            })
            .collect::<BTreeSet<_>>();
        let source_aliases = source_ids
            .into_iter()
            .enumerate()
            .map(|(index, source_id)| (source_id, format!("S{index}")))
            .collect::<BTreeMap<_, _>>();
        let evidence_aliases = native_evidence
            .keys()
            .enumerate()
            .map(|(index, id)| (id.clone(), format!("E{index}")))
            .collect::<BTreeMap<_, _>>();

        let evidence = native_evidence
            .iter()
            .map(|(id, (source_id, text))| {
                json!({
                    "id": evidence_aliases[id],
                    "source": source_aliases[source_id],
                    "text": text,
                })
            })
            .collect::<Vec<_>>();
        let units = self
            .units
            .iter()
            .enumerate()
            .map(|(index, unit)| {
                let evidence_ids = if let Some(grounding) = &unit.content.grounding {
                    grounding
                        .evidence
                        .iter()
                        .map(|reference| evidence_aliases[&reference.id].clone())
                        .collect::<Vec<_>>()
                } else {
                    vec![evidence_aliases[&format!("legacy:{}", unit.id)].clone()]
                };
                let details = unit
                    .content
                    .details
                    .as_ref()
                    .map(|details| {
                        details.validate(&unit.content.statement, unit.content.kind)?;
                        details.prompt(&unit.content.statement)
                    })
                    .transpose()?;
                Ok(json!({
                    "details": details,
                    "id": Self::unit_alias(index),
                    "title": unit.content.title,
                    "kind": unit.content.kind,
                    "statement": unit.content.statement,
                    "qualifications": unit.content.qualifications,
                    "topic": unit.content.topic,
                    "origin": unit.origin,
                    "evidenceIds": evidence_ids,
                    "importance": unit.content.grounding.as_ref().map(|grounding| grounding.importance),
                    "documentCore": self.document_core_units.contains(&unit.id),
                }))
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        serde_json::to_string(&json!({
            "preparation": prompt::preparation(self, &source_aliases),
            "units": units,
            "evidence": evidence,
            "trust": "Knowledge statements and priorities are model interpretations. Evidence is literal source material. Request-local K/E/S aliases are not durable identity. Traceability does not establish truth or entailment; preserve conditions and conflicting source claims."
        }))
        .map_err(CommandError::serialization)
    }

    pub(crate) fn fingerprint(&self) -> Result<String, CommandError> {
        Ok(hash(&format!(
            "authoring-v{AUTHORING_VERSION}|{}",
            serde_json::to_string(&(&self.units, &self.document_core_units, &self.preparation))
                .map_err(CommandError::serialization)?
        )))
    }

    /// Identity of the evidence actually supplied to one authoring request.
    /// Collection-wide preparation counters are deliberately excluded so an
    /// unrelated source does not invalidate an otherwise identical window.
    pub(crate) fn request_fingerprint(&self) -> Result<String, CommandError> {
        Ok(hash(
            &serde_json::to_string(&prompt::request_identity(self))
                .map_err(CommandError::serialization)?,
        ))
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
        ids.iter()
            .map(|id| {
                self.resolve_unit_id(id)
                    .ok_or_else(|| {
                        invalid(
                            "The model selected a knowledge unit outside its supplied source context.",
                        )
                    })
                    .and_then(UnitSupport::from_unit)
            })
            .collect()
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

/// Bounded SQLite hydration is not a semantic boundary. The last unfinished
/// batch is carried into the next hydration group, preserving cross-page context.
pub(crate) struct DocumentPreparation<'a> {
    pub database: &'a Database,
    pub paths: &'a AppPaths,
    pub runtime: &'a AiRuntimeState,
    pub task: &'a DocumentTaskContext,
    pub document_id: &'a str,
    pub chunk_indices: &'a [i64],
    pub retry_partial: bool,
}

/// Source records whose extraction window failed on its output this run. They
/// stay unextracted (nothing durable records "no knowledge here"), so the next
/// preparation asks the model again; until then catalogs skip them.
#[derive(Clone, Debug, Default)]
pub(crate) struct PreparationGaps {
    pub sources: BTreeSet<String>,
}

pub(crate) async fn prepare_document(
    input: DocumentPreparation<'_>,
) -> Result<PreparationGaps, CommandError> {
    let DocumentPreparation {
        database,
        paths,
        runtime,
        task,
        document_id,
        chunk_indices,
        retry_partial,
    } = input;
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
    let mut pending = Vec::new();
    let mut gaps = PreparationGaps::default();
    let mut extracted_windows = 0usize;
    let mut first_gap: Option<CommandError> = None;
    let mut completed = 0;
    let mut remaining = std::collections::HashMap::<String, usize>::new();
    let mut source_parents = std::collections::HashMap::<String, String>::new();
    let parallelism =
        crate::ai::concurrency::document_request_parallelism(&task.network_scope).clamp(1, 4);
    crate::logging::debug_target(
        "ai.knowledge.extraction",
        "knowledge_extraction_execution_plan",
        "Independent source windows share one bounded provider queue and durable knowledge store.",
        json!({"document":document_id,"sourceChunks":seeds.len(),"parallelism":parallelism}),
    );
    for (batch_index, batch) in seeds.chunks(32).enumerate() {
        check_canceled(&cancellation)?;
        let groups = materialize_source_groups(database, batch)?;
        let mut sources = Vec::new();
        for (seed, group) in batch.iter().zip(groups) {
            if group.is_empty() {
                return Err(repository::stale_source());
            }
            for source in &group {
                if source_parents
                    .insert(source.id.clone(), seed.parent_key.clone())
                    .is_some_and(|previous| previous != seed.parent_key)
                {
                    return Err(repository::stale_source());
                }
                *remaining.entry(seed.parent_key.clone()).or_default() += 1;
            }
            sources.extend(group);
        }
        pending.extend(sources);
        let ranges = batch_ranges(&pending);
        let last_batch = (batch_index + 1) * 32 >= seeds.len();
        let process_count = if last_batch {
            ranges.len()
        } else {
            ranges.len().saturating_sub(1)
        };
        let process_ranges = &ranges[..process_count];
        let groups = process_ranges
            .iter()
            .map(|range| &pending[range.clone()])
            .collect::<Vec<_>>();
        if !groups.is_empty() {
            set_current_task_phase(
                "understanding material",
                None,
                completed,
                seeds.len(),
                groups
                    .first()
                    .and_then(|group| group.first())
                    .and_then(|source| source.locator.as_ref().map(|l| l.first())),
                groups
                    .last()
                    .and_then(|group| group.last())
                    .and_then(|source| source.locator.as_ref().map(|l| l.last())),
                None,
            );
        }
        let mut first_failure: Option<(usize, CommandError)> = None;
        run_bounded_provider_queue(
            groups.len(),
            parallelism,
            |index| {
                let cancellation = &cancellation;
                let group = groups[index];
                async move {
                    let outcome = if retry_partial {
                        retry_units_batch(database, paths, runtime, task, group, cancellation).await
                    } else {
                        ensure_units_batch(database, paths, runtime, task, group, cancellation)
                            .await
                    };
                    (index, outcome)
                }
            },
            |(index, outcome)| {
                let group = groups[index];
                // One window's bad output must not sink the document: keep its
                // sources as gaps and let the rest of the document finish.
                let outcome = match outcome {
                    // So must a window of pictures the model cannot see; a
                    // document with nothing else still reports it below.
                    Err(error)
                        if crate::ai::document_intelligence::is_output_failure(&error)
                            || error.code == super::extraction::IMAGES_REQUIRE_VISION =>
                    {
                        crate::logging::warn_target(
                            "ai.knowledge.extraction",
                            "knowledge_extraction_window_gap",
                            "Kept a failed extraction window as a gap; the next preparation retries it.",
                            json!({"document":document_id,"sources":group.len(),"code":error.code}),
                        );
                        gaps.sources.extend(group.iter().map(|source| source.id.clone()));
                        first_gap.get_or_insert(error);
                        Ok(())
                    }
                    Ok(_) => {
                        extracted_windows += 1;
                        Ok(())
                    }
                    Err(error) => Err(error),
                };
                let result = outcome.and_then(|_| {
                    for source in group {
                        let parent = source_parents
                            .get(&source.id)
                            .ok_or_else(repository::stale_source)?;
                        if let Some(left) = remaining.get_mut(parent) {
                            *left = left.saturating_sub(1);
                            if *left == 0 {
                                completed += 1;
                            }
                        }
                    }
                    Ok(())
                });
                let control = match result {
                    Ok(()) => {
                        set_current_task_phase(
                            "understanding material",
                            None,
                            completed,
                            seeds.len(),
                            None,
                            None,
                            None,
                        );
                        QueueControl::Continue
                    }
                    Err(error) => {
                        let replace = match first_failure.as_ref() {
                            None => true,
                            Some((failed_index, _)) => index < *failed_index,
                        };
                        if replace {
                            first_failure = Some((index, error));
                        }
                        QueueControl::Stop
                    }
                };
                Ok::<QueueControl, CommandError>(control)
            },
        )
        .await?;
        let consumed = process_ranges.last().map_or(0, |range| range.end);
        drop(groups);
        if let Some((_, error)) = first_failure {
            return Err(error);
        }
        pending.drain(..consumed);
    }
    check_canceled(&cancellation)?;
    task.ensure_current(database)?;
    if let Some(error) = first_gap {
        // Nothing extracted at all means the model cannot do this task.
        if extracted_windows == 0 {
            return Err(error);
        }
        crate::logging::warn_target(
            "ai.knowledge.extraction",
            "knowledge_extraction_finished_with_gaps",
            "Prepared the document with some windows left as gaps.",
            json!({"document":document_id,"gapSources":gaps.sources.len(),
                "extractedWindows":extracted_windows}),
        );
    }
    Ok(gaps)
}

pub(crate) fn catalog_for_chunks(
    database: &Database,
    document_id: &str,
    chunk_indices: &[i64],
    task: &DocumentTaskContext,
    gaps: &PreparationGaps,
) -> Result<KnowledgeCatalog, CommandError> {
    let contract = extraction_contract(task)?;
    task.ensure_current(database)?;
    database.with_connection(|connection| {
        let context = super::learning_context::for_document_chunks_with_gaps(
            connection,
            document_id,
            chunk_indices,
            &gaps.sources,
        )?;
        let document_core_units =
            super::document_view::core_ids(connection, document_id, &contract)?;
        Ok(KnowledgeCatalog {
            units: context.units,
            document_core_units,
            preparation: context.preparation,
        })
    })
}

/// Legacy artifact locators are projections of the actual selected evidence,
/// not the transport window that happened to contain a knowledge target.
pub(crate) fn support_locator(
    connection: &Connection,
    support: &[UnitSupport],
    fallback: &crate::document_domain::SourceLocator,
) -> Result<crate::document_domain::SourceLocator, CommandError> {
    let mut numbers = BTreeSet::new();
    let mut assets = BTreeSet::new();
    let mut time: Option<crate::document_domain::TimeRangeMs> = None;
    for item in support {
        let serialized: String = connection
            .query_row(
                "SELECT record_json FROM knowledge_units WHERE id = ?1",
                [&item.unit_id],
                |row| row.get(0),
            )
            .map_err(CommandError::database)?;
        let unit: KnowledgeUnit =
            serde_json::from_str(&serialized).map_err(CommandError::serialization)?;
        if UnitSupport::from_unit(&unit)? != *item {
            return Err(repository::stale_source());
        }
        repository::validate_grounding(connection, &unit.content)?;
        let locators = if let Some(grounding) = &unit.content.grounding {
            grounding
                .evidence
                .iter()
                .filter_map(|reference| reference.locator.clone())
                .collect::<Vec<_>>()
        } else {
            repository::load_source(connection, &unit.source_id)?
                .and_then(|source| source.locator)
                .into_iter()
                .collect()
        };
        for locator in locators {
            if locator.unit_kind != fallback.unit_kind {
                return Err(invalid(
                    "Knowledge evidence uses incompatible source locator kinds.",
                ));
            }
            numbers.extend(locator.unit_numbers);
            assets.extend(locator.asset_ids);
            if let Some(range) = locator.time_range_ms {
                time = Some(
                    time.map_or(range, |old| crate::document_domain::TimeRangeMs {
                        start: old.start.min(range.start),
                        end: old.end.max(range.end),
                    }),
                );
            }
        }
    }
    if numbers.is_empty() {
        return Ok(fallback.clone());
    }
    crate::document_domain::SourceLocator::with_evidence(
        fallback.unit_kind,
        numbers.into_iter().collect(),
        None,
        assets.into_iter().collect(),
        time,
    )
}

pub(crate) fn card_support(card: &Value) -> Result<Vec<UnitSupport>, CommandError> {
    let support: Vec<UnitSupport> = serde_json::from_value(
        card.get(CARD_SUPPORT_KEY)
            .cloned()
            .ok_or_else(|| invalid("A generated card is missing its knowledge targets."))?,
    )
    .map_err(|_| invalid("A generated card contains invalid knowledge support."))?;
    if let Some(record) = super::assessment::record_from_card(card)? {
        if record.support()? != support {
            return Err(invalid(
                "The card summary differs from its component evidence.",
            ));
        }
        return Ok(support);
    }
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
        object.remove(super::assessment::ASSESSMENT_KEY);
        object.remove(super::assessment::CARD_POLICY_KEY);
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
        let serialized: String = connection
            .query_row(
                "SELECT record_json FROM knowledge_units WHERE id = ?1",
                [&item.unit_id],
                |row| row.get(0),
            )
            .map_err(CommandError::database)?;
        let unit: KnowledgeUnit =
            serde_json::from_str(&serialized).map_err(CommandError::serialization)?;
        repository::validate_grounding(connection, &unit.content)?;
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

fn invalid(message: &str) -> CommandError {
    CommandError::new("ai_document_output_invalid", message)
}

#[cfg(test)]
pub(crate) mod tests;
