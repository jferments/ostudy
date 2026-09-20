//! Shared read-only admission of saved learning material. Provider work, vector
//! ranking, task authorization and conversation policy stay with their owners.
//! Neither reading a context nor reporting preparation needs starts analysis.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use rusqlite::{params, Connection};
use serde::Serialize;
use serde_json::{json, Value};

use super::{
    contracts::{hash, invalid, KnowledgeUnit, SourceKind, SourceRecord, StudyScope},
    repository, retrieval,
};
use crate::error::CommandError;

mod receipt;
#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ContextPolicy {
    /// Admit the whole selected window or report an explicit budget error.
    Coverage,
    /// Keep ranking order; omissions are explicit and never imply coverage.
    Retrieval,
}

#[derive(Clone, Copy)]
pub(crate) struct ContextBudget {
    pub sources: usize,
    pub units: usize,
    pub bytes: usize,
}

impl ContextBudget {
    pub(crate) fn authoring() -> Self {
        Self {
            sources: 2048,
            units: 16384,
            bytes: 16 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PreparationSummary {
    pub unprepared_sources: usize,
    pub unresolved_regions: usize,
    pub unreadable_regions: usize,
    pub rejected_proposals: usize,
    pub legacy_coverage_sources: usize,
    pub omitted_sources: usize,
    pub omitted_units: usize,
    pub evidence_structures: Vec<Value>,
}

impl PreparationSummary {
    pub(crate) fn notice(&self) -> String {
        if self.unprepared_sources
            + self.unresolved_regions
            + self.unreadable_regions
            + self.rejected_proposals
            + self.legacy_coverage_sources
            > 0
        {
            format!("Saved material is usable but exhaustive analysis is not established: {} unprepared sources, {} unresolved regions, {} unreadable regions, {} rejected proposals, and {} sources with older coverage receipts. Do not claim complete source coverage.",
                self.unprepared_sources, self.unresolved_regions, self.unreadable_regions,
                self.rejected_proposals, self.legacy_coverage_sources)
        } else {
            "Processing dispositions are accounted for; this is not proof of exhaustive conceptual coverage or factual entailment.".into()
        }
    }
}

/// This request is assembled natively after scope/focus resolution. Candidate
/// records are still reloaded and authorized here; possession of an ID is not
/// permission, and consumer-model selection is not an interpretation revision.
pub(crate) struct LearningContextRequest<'a> {
    pub scope: &'a StudyScope,
    pub include_conversations: bool,
    pub candidates: &'a [SourceRecord],
    pub policy: ContextPolicy,
    pub target_units: Option<&'a BTreeSet<String>>,
    pub budget: ContextBudget,
}

pub(crate) struct LearningContext {
    pub sources: Vec<SourceRecord>,
    pub units: Vec<KnowledgeUnit>,
    pub preparation: PreparationSummary,
    pub stamp: ContextStamp,
}

#[derive(Clone, Debug)]
pub(crate) struct ContextStamp {
    scope: StudyScope,
    include_conversations: bool,
    sources: Vec<SourceRecord>,
    fingerprint: String,
}

impl ContextStamp {
    pub(crate) fn restrict_to(
        mut self,
        connection: &Connection,
        sources: &[SourceRecord],
    ) -> Result<Self, CommandError> {
        self.ensure_current(connection)?;
        if sources
            .iter()
            .any(|source| !self.sources.iter().any(|old| old.id == source.id))
        {
            return Err(repository::stale_source());
        }
        self.sources
            .retain(|old| sources.iter().any(|source| source.id == old.id));
        self.fingerprint = receipt::fingerprint(connection, &self.sources)?;
        Ok(self)
    }

    /// Recheck permission, every interpretation and empty/partial receipt in the
    /// caller's publication transaction, not just the citations a model chose.
    pub(crate) fn ensure_current(&self, connection: &Connection) -> Result<(), CommandError> {
        let allowed = permitted(connection, &self.scope, self.include_conversations)?;
        if self
            .sources
            .iter()
            .any(|source| !allowed.contains(&source.id))
        {
            return Err(repository::stale_source());
        }
        for source in &self.sources {
            if !repository::source_is_current(connection, source)? {
                return Err(repository::stale_source());
            }
        }
        if receipt::fingerprint(connection, &self.sources)? != self.fingerprint {
            return Err(repository::stale_source());
        }
        Ok(())
    }
}

fn permitted(
    connection: &Connection,
    scope: &StudyScope,
    include_conversations: bool,
) -> Result<HashSet<String>, CommandError> {
    Ok(
        retrieval::eligible_entries_in(connection, scope, include_conversations)?
            .into_iter()
            .filter_map(|entry| entry.id.strip_prefix("source:").map(str::to_string))
            .collect(),
    )
}

pub(crate) fn build(
    connection: &Connection,
    request: LearningContextRequest<'_>,
) -> Result<LearningContext, CommandError> {
    request.scope.validate()?;
    let ContextBudget {
        sources: source_limit,
        units: unit_limit,
        bytes: byte_limit,
    } = request.budget;
    if source_limit == 0
        || source_limit > 2048
        || unit_limit == 0
        || unit_limit > 16384
        || byte_limit == 0
        || byte_limit > 16 * 1024 * 1024
        || request.candidates.len() > 4096
    {
        return Err(invalid("The learning-context budget is invalid."));
    }
    let allowed = permitted(connection, request.scope, request.include_conversations)?;
    let mut sources = Vec::new();
    let mut seen = HashSet::new();
    let mut omitted_sources = 0;
    let mut bytes = 0_usize;
    for candidate in request.candidates {
        if !seen.insert(candidate.id.clone()) {
            continue;
        }
        if !allowed.contains(&candidate.id) {
            omitted_sources += 1;
            continue;
        }
        if !repository::source_is_current(connection, candidate)? {
            return Err(repository::stale_source());
        }
        let source = repository::load_source(connection, &candidate.id)?
            .ok_or_else(repository::stale_source)?;
        let size = serde_json::to_vec(&source)
            .map_err(CommandError::serialization)?
            .len();
        if sources.len() == source_limit || size > byte_limit.saturating_sub(bytes) {
            omitted_sources += 1;
            continue;
        }
        bytes += size;
        sources.push(source);
    }
    if request.policy == ContextPolicy::Coverage && omitted_sources > 0 {
        return Err(invalid(
            "The coverage window is not fully permitted or exceeds its source budget.",
        ));
    }
    let mut units = Vec::new();
    let mut seen_units = HashSet::new();
    let mut omitted_units = 0;
    for source in &sources {
        for unit in repository::load_units(connection, &source.id)? {
            if request
                .target_units
                .is_some_and(|ids| !ids.contains(&unit.id))
                || !seen_units.insert(unit.id.clone())
            {
                continue;
            }
            if !unit_visible(&unit, &sources) {
                omitted_units += 1;
                continue;
            }
            let size = serde_json::to_vec(&unit)
                .map_err(CommandError::serialization)?
                .len();
            if units.len() == unit_limit || size > byte_limit.saturating_sub(bytes) {
                omitted_units += 1;
                continue;
            }
            bytes += size;
            units.push(unit);
        }
    }
    if request.policy == ContextPolicy::Coverage && omitted_units > 0 {
        return Err(invalid(
            "The coverage window lacks required evidence or exceeds its knowledge budget.",
        ));
    }
    let mut preparation = receipt::summarize(connection, &sources)?;
    preparation.omitted_sources = omitted_sources;
    preparation.omitted_units = omitted_units;
    // Structures are optional exact evidence, never permission to read another
    // region. A complete block must occur literally within an admitted source.
    preparation.evidence_structures =
        receipt::structures(connection, &sources, byte_limit.saturating_sub(bytes))?;
    let stamp = ContextStamp {
        scope: request.scope.clone(),
        include_conversations: request.include_conversations,
        fingerprint: receipt::fingerprint(connection, &sources)?,
        sources: sources.clone(),
    };
    Ok(LearningContext {
        sources,
        units,
        preparation,
        stamp,
    })
}

pub(crate) fn unit_visible(unit: &KnowledgeUnit, sources: &[SourceRecord]) -> bool {
    sources
        .iter()
        .any(|source| source.id == unit.source_id && source.content_hash == unit.source_hash)
        && unit
            .content
            .grounding
            .as_ref()
            .is_none_or(|grounding| grounding.is_visible_in(sources))
}

/// Native source-window selection for cards and guides. Dependent pages may be
/// brought into the same request only within this explicitly selected document.
/// No collection priorities or consumer model participate in saved-unit identity.
pub(crate) fn for_document_chunks(
    connection: &Connection,
    document_id: &str,
    chunk_indices: &[i64],
) -> Result<LearningContext, CommandError> {
    crate::util::validate_entity_id(document_id, "knowledge document")?;
    if chunk_indices.is_empty()
        || chunk_indices.len() > 2048
        || chunk_indices.iter().any(|i| *i < 0)
    {
        return Err(invalid("The authoring context source window is invalid."));
    }
    let scope = StudyScope {
        kind: super::contracts::ScopeKind::Document,
        id: Some(document_id.to_string()),
        members: Vec::new(),
    };
    let wanted = serde_json::to_string(chunk_indices).map_err(CommandError::serialization)?;
    let mut statement = connection
        .prepare(
            "SELECT id,parent_key FROM knowledge_sources WHERE kind='document' AND document_id=?1
         AND active=1 AND parent_key IN(SELECT CAST(value AS TEXT) FROM json_each(?2))
         ORDER BY CAST(parent_key AS INTEGER),segment_key LIMIT 2049",
        )
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map(params![document_id, wanted], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    let present = rows
        .iter()
        .map(|(_, key)| key.as_str())
        .collect::<HashSet<_>>();
    if rows.len() > 2048
        || chunk_indices
            .iter()
            .any(|index| !present.contains(index.to_string().as_str()))
    {
        return Err(repository::stale_source());
    }
    let mut sources = BTreeMap::new();
    let mut dependencies = BTreeSet::new();
    let mut target_units = BTreeSet::new();
    for (id, _) in rows {
        let source =
            repository::load_source(connection, &id)?.ok_or_else(repository::stale_source)?;
        if !repository::extraction_available(connection, &source)? {
            return Err(repository::stale_source());
        }
        for unit in repository::load_units(connection, &id)? {
            target_units.insert(unit.id.clone());
            dependencies.insert(unit.source_id.clone());
            if let Some(grounding) = unit.content.grounding {
                dependencies.extend(
                    grounding
                        .evidence
                        .into_iter()
                        .map(|reference| reference.source_id),
                );
            }
        }
        sources.insert(source.id.clone(), source);
    }
    for id in dependencies {
        if sources.contains_key(&id) {
            continue;
        }
        let source =
            repository::load_source(connection, &id)?.ok_or_else(repository::stale_source)?;
        if source.kind != SourceKind::Document || source.document_id.as_deref() != Some(document_id)
        {
            return Err(repository::stale_source());
        }
        sources.insert(id, source);
    }
    build(
        connection,
        LearningContextRequest {
            scope: &scope,
            include_conversations: false,
            candidates: &sources.into_values().collect::<Vec<_>>(),
            policy: ContextPolicy::Coverage,
            target_units: Some(&target_units),
            budget: ContextBudget::authoring(),
        },
    )
}

/// One deduplicated prompt representation, also used after Tutor budget eviction.
/// Source data and interpretation remain distinct; excluded dependencies remove
/// the whole claim. This function performs no reads or provider requests.
pub(crate) fn material_prompt(
    sources: &[SourceRecord],
    units: &[KnowledgeUnit],
) -> Result<Value, CommandError> {
    let admitted = units
        .iter()
        .filter(|unit| unit_visible(unit, sources))
        .collect::<Vec<_>>();
    let evidence = admitted
        .iter()
        .flat_map(|unit| unit.content.grounding.iter())
        .flat_map(|grounding| &grounding.evidence)
        .map(|reference| (reference.id.clone(), reference))
        .collect::<BTreeMap<_, _>>();
    let projected = admitted.iter().map(|unit| {
        let details = unit.content.details.as_ref().map(|details| details.prompt(&unit.content.statement)).transpose()?;
        Ok(json!({
            "id":unit.id,"sourceId":unit.source_id,"origin":unit.origin,
            "title":unit.content.title,"kind":unit.content.kind,"statement":unit.content.statement,
            "qualifications":unit.content.qualifications,"topic":unit.content.topic,"details":details,
            "legacyExcerpt":if unit.content.grounding.is_none(){Some(&unit.content.supporting_excerpt)}else{None},
            "evidenceIds":unit.content.grounding.as_ref().map(|grounding|grounding.evidence.iter().map(|reference| &reference.id).collect::<Vec<_>>())
        }))
    }).collect::<Result<Vec<_>, CommandError>>()?;
    Ok(json!({"sources":sources.iter().map(|source| json!({
        "sourceId":source.id,"contentHash":source.content_hash,"kind":source.kind,
        "origin":source.origin,"title":source.title,"text":source.text,"locator":source.locator
    })).collect::<Vec<_>>(),
        "units":projected,"evidence":evidence.into_values().collect::<Vec<_>>()}))
}
