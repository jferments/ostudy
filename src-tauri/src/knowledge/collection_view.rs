//! Compact collection understanding over retained source knowledge.
//!
//! A collection view is a derived projection: it references current source/unit
//! identities and coverage receipts but never copies canonical source facts or
//! embeddings into collection ownership. Native synthesis remains useful without
//! a provider; an optional bounded reducer may add validated relationships later.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};

use crate::{
    collections::{self, SourceSelectionInput, SourceSetSnapshotDto},
    database::Database,
    error::CommandError,
    util::{unix_time_millis, validate_entity_id},
};

use super::{
    collection_scope,
    contracts::{hash, invalid, KnowledgeUnit, ScopeKind, StudyScope},
    evidence::Importance,
    learning_context::{
        self, ContextBudget, ContextPolicy, LearningContext, LearningContextRequest,
        PreparationSummary,
    },
    repository,
};

const VIEW_VERSION: i64 = 1;
const MAX_TOPICS: usize = 1_024;
const MAX_COVERAGE_SOURCES: usize = 2_048;
const MAX_COVERAGE_UNITS: usize = 16_384;
const MAX_SYNTHESIS_BYTES: usize = 2 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CollectionRelationshipKind {
    Complementary,
    Contradictory,
    Prerequisite,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CollectionRelationship {
    pub id: String,
    pub kind: CollectionRelationshipKind,
    pub topic_ids: Vec<String>,
    pub support_unit_ids: Vec<String>,
    pub note: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CollectionTopic {
    pub id: String,
    pub title: String,
    pub unit_ids: Vec<String>,
    pub support_source_ids: Vec<String>,
    pub support_document_ids: Vec<String>,
    /// Instructional emphasis is advisory and is never counted as factual support.
    pub emphasis_source_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CollectionGap {
    pub kind: String,
    pub count: usize,
    pub document_id: Option<String>,
    pub source_id: Option<String>,
    pub unit_ids: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CollectionSynthesis {
    pub version: i64,
    pub collection_id: String,
    pub snapshot_id: String,
    pub summary: String,
    pub learning_objectives: Vec<String>,
    pub topics: Vec<CollectionTopic>,
    pub relationships: Vec<CollectionRelationship>,
    pub gaps: Vec<CollectionGap>,
    pub reduction_status: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CollectionViewState {
    pub synthesis: CollectionSynthesis,
    pub stale: bool,
    pub reduced: bool,
    pub analysis_hash: String,
}

struct CoveragePlan {
    snapshot: SourceSetSnapshotDto,
    context: LearningContext,
    source_documents: HashMap<String, String>,
    omitted_dependency_units: Vec<String>,
    analysis_hash: String,
}

#[derive(Clone)]
struct Dependency {
    kind: &'static str,
    id: String,
    hash: String,
}

#[derive(Default)]
struct TopicBuilder {
    title: String,
    unit_ids: BTreeSet<String>,
    support_source_ids: BTreeSet<String>,
    support_document_ids: BTreeSet<String>,
    emphasis_source_ids: BTreeSet<String>,
}

pub(crate) fn refresh_native(
    database: &Database,
    collection_id: &str,
) -> Result<CollectionViewState, CommandError> {
    validate_entity_id(collection_id, "document collection")?;
    let plan = database.with_connection(|connection| coverage_plan(connection, collection_id))?;
    let synthesis = native_synthesis(collection_id, &plan)?;
    publish(database, &plan, &synthesis, false)?;
    database.with_connection(|connection| {
        load_in(connection, collection_id)?.ok_or_else(|| {
            CommandError::new(
                "collection_synthesis_missing",
                "The collection synthesis could not be reloaded after publication.",
            )
        })
    })
}

pub(crate) fn load(
    database: &Database,
    collection_id: &str,
) -> Result<Option<CollectionViewState>, CommandError> {
    validate_entity_id(collection_id, "document collection")?;
    database.with_connection(|connection| load_in(connection, collection_id))
}

#[cfg(test)]
pub(crate) fn load_in_for_preservation(
    connection: &Connection,
    collection_id: &str,
) -> Result<Option<CollectionViewState>, CommandError> {
    load_in(connection, collection_id)
}

fn load_in(
    connection: &Connection,
    collection_id: &str,
) -> Result<Option<CollectionViewState>, CommandError> {
    let row = connection
        .query_row(
            "SELECT snapshot_id,analysis_hash,synthesis_json,reduced,stale
             FROM knowledge_collection_views WHERE collection_id=?1",
            [collection_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            },
        )
        .optional()
        .map_err(CommandError::database)?;
    let Some((snapshot_id, analysis_hash, serialized, reduced, stored_stale)) = row else {
        return Ok(None);
    };
    let synthesis: CollectionSynthesis =
        serde_json::from_str(&serialized).map_err(CommandError::serialization)?;
    validate_synthesis(&synthesis)?;
    let current_snapshot = collections::resolve_selection(
        connection,
        SourceSelectionInput {
            collection_ids: vec![collection_id.to_string()],
            documents: Vec::new(),
            exclusions: Vec::new(),
        },
    );
    let stale = stored_stale != 0
        || current_snapshot
            .map(|current| current.snapshot_id != snapshot_id)
            .unwrap_or(true);
    Ok(Some(CollectionViewState {
        synthesis,
        stale,
        reduced: reduced != 0,
        analysis_hash,
    }))
}

fn coverage_plan(
    connection: &Connection,
    collection_id: &str,
) -> Result<CoveragePlan, CommandError> {
    let scope = StudyScope::single(ScopeKind::Collection, Some(collection_id.to_string()));
    let snapshot = collection_scope::snapshot(connection, &scope)?
        .ok_or_else(|| invalid("The collection source scope is empty or unavailable."))?;
    let entries = collection_scope::eligible_entries(connection, &scope)?;
    let mut source_ids = BTreeSet::new();
    let mut target_units = BTreeSet::new();
    for entry in entries {
        if let Some(id) = entry.id.strip_prefix("source:") {
            source_ids.insert(id.to_string());
        } else if let Some(id) = entry.id.strip_prefix("unit:") {
            target_units.insert(id.to_string());
        }
    }
    if source_ids.len() > MAX_COVERAGE_SOURCES || target_units.len() > MAX_COVERAGE_UNITS {
        return Err(CommandError::new(
            "collection_context_partition_required",
            "The selected collection exceeds one complete material-context partition. Split the target into smaller explicit ranges instead of silently truncating it.",
        ));
    }
    let mut candidates = Vec::with_capacity(source_ids.len());
    for id in &source_ids {
        candidates
            .push(repository::load_source(connection, id)?.ok_or_else(repository::stale_source)?);
    }
    let mut all_units = BTreeSet::new();
    for source in &candidates {
        for unit in repository::load_units(connection, &source.id)? {
            all_units.insert(unit.id);
        }
    }
    let omitted_dependency_units = all_units
        .difference(&target_units)
        .cloned()
        .collect::<Vec<_>>();
    let context = learning_context::build(
        connection,
        LearningContextRequest {
            scope: &scope,
            include_conversations: false,
            candidates: &candidates,
            policy: ContextPolicy::Coverage,
            target_units: Some(&target_units),
            budget: ContextBudget::authoring(),
        },
    )?;
    let source_documents = context
        .sources
        .iter()
        .filter_map(|source| {
            source
                .document_id
                .as_ref()
                .map(|document| (source.id.clone(), document.clone()))
        })
        .collect::<HashMap<_, _>>();
    let unit_revisions = context
        .units
        .iter()
        .map(|unit| {
            Ok((
                &unit.id,
                &unit.source_id,
                &unit.source_hash,
                hash(&serde_json::to_string(&unit.content).map_err(CommandError::serialization)?),
            ))
        })
        .collect::<Result<Vec<_>, CommandError>>()?;
    let analysis_hash = hash(
        &serde_json::to_string(&(
            "collection-view-v1",
            &snapshot.snapshot_id,
            context.stamp.fingerprint(),
            &unit_revisions,
            &omitted_dependency_units,
        ))
        .map_err(CommandError::serialization)?,
    );
    Ok(CoveragePlan {
        snapshot,
        context,
        source_documents,
        omitted_dependency_units,
        analysis_hash,
    })
}

fn native_synthesis(
    collection_id: &str,
    plan: &CoveragePlan,
) -> Result<CollectionSynthesis, CommandError> {
    let mut topics = BTreeMap::<String, TopicBuilder>::new();
    let sources = plan
        .context
        .sources
        .iter()
        .map(|source| (source.id.as_str(), source))
        .collect::<HashMap<_, _>>();
    for unit in &plan.context.units {
        let title = if unit.content.topic.trim().is_empty() {
            unit.content.title.trim()
        } else {
            unit.content.topic.trim()
        };
        let key = normalize_topic(title);
        if key.is_empty() {
            continue;
        }
        let topic = topics.entry(key.clone()).or_default();
        if topic.title.is_empty() {
            topic.title = title.to_string();
        }
        topic.unit_ids.insert(unit.id.clone());
        let support = unit
            .content
            .grounding
            .as_ref()
            .map(|grounding| {
                grounding
                    .evidence
                    .iter()
                    .map(|reference| reference.source_id.clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| vec![unit.source_id.clone()]);
        for source_id in support {
            topic.support_source_ids.insert(source_id.clone());
            if let Some(document_id) = plan.source_documents.get(&source_id) {
                topic.support_document_ids.insert(document_id.clone());
            }
        }
        if unit
            .content
            .grounding
            .as_ref()
            .is_some_and(|grounding| grounding.importance == Importance::Core)
        {
            topic.emphasis_source_ids.insert(unit.source_id.clone());
        }
        if !sources.contains_key(unit.source_id.as_str()) {
            return Err(repository::stale_source());
        }
    }
    if topics.len() > MAX_TOPICS {
        return Err(CommandError::new(
            "collection_synthesis_partition_required",
            "The selected collection has too many distinct interpreted topics for one compact synthesis. Split the material target explicitly.",
        ));
    }
    let topics = topics
        .into_iter()
        .map(|(key, value)| CollectionTopic {
            id: hash(&format!("collection-topic-v1|{key}"))[..32].to_string(),
            title: value.title,
            unit_ids: value.unit_ids.into_iter().collect(),
            support_source_ids: value.support_source_ids.into_iter().collect(),
            support_document_ids: value.support_document_ids.into_iter().collect(),
            emphasis_source_ids: value.emphasis_source_ids.into_iter().collect(),
        })
        .collect::<Vec<_>>();

    let mut gaps = snapshot_gaps(&plan.snapshot);
    preparation_gaps(&plan.context.preparation, &mut gaps);
    if !plan.omitted_dependency_units.is_empty() {
        gaps.push(CollectionGap {
            kind: "dependency_outside_scope".to_string(),
            count: plan.omitted_dependency_units.len(),
            document_id: None,
            source_id: None,
            unit_ids: plan.omitted_dependency_units.clone(),
            detail: "Saved interpretations that require evidence outside the authorized collection ranges were not admitted.".to_string(),
        });
    }
    let supported_documents = topics
        .iter()
        .flat_map(|topic| topic.support_document_ids.iter().cloned())
        .collect::<HashSet<_>>();
    for member in &plan.snapshot.members {
        if !supported_documents.contains(&member.document_id) {
            gaps.push(CollectionGap {
                kind: "no_interpreted_units".to_string(),
                count: 1,
                document_id: Some(member.document_id.clone()),
                source_id: None,
                unit_ids: Vec::new(),
                detail: "This selected source currently contributes no admitted interpreted units."
                    .to_string(),
            });
        }
    }

    let objective_limit = topics.len().min(96);
    let learning_objectives = topics
        .iter()
        .take(objective_limit)
        .map(|topic| format!("Review {}", topic.title))
        .collect::<Vec<_>>();
    if objective_limit < topics.len() {
        gaps.push(CollectionGap {
            kind: "objective_compaction".to_string(),
            count: topics.len() - objective_limit,
            document_id: None,
            source_id: None,
            unit_ids: Vec::new(),
            detail: "All topics remain in the synthesis; the compact objective list intentionally does not repeat every topic.".to_string(),
        });
    }
    let summary = format!(
        "This collection currently contains {} interpreted topic{} across {} selected document{}; retained coverage gaps remain explicit and no topic count is a claim of exhaustive conceptual coverage.",
        topics.len(),
        if topics.len() == 1 { "" } else { "s" },
        plan.snapshot.members.len(),
        if plan.snapshot.members.len() == 1 { "" } else { "s" },
    );
    let synthesis = CollectionSynthesis {
        version: VIEW_VERSION,
        collection_id: collection_id.to_string(),
        snapshot_id: plan.snapshot.snapshot_id.clone(),
        summary,
        learning_objectives,
        topics,
        relationships: Vec::new(),
        gaps,
        reduction_status: "native".to_string(),
    };
    validate_synthesis(&synthesis)?;
    Ok(synthesis)
}

fn snapshot_gaps(snapshot: &SourceSetSnapshotDto) -> Vec<CollectionGap> {
    snapshot
        .dispositions
        .iter()
        .map(|disposition| CollectionGap {
            kind: "source_disposition".to_string(),
            count: 1,
            document_id: disposition.document_id.clone(),
            source_id: None,
            unit_ids: Vec::new(),
            detail: disposition.reason.clone(),
        })
        .collect()
}

fn preparation_gaps(preparation: &PreparationSummary, gaps: &mut Vec<CollectionGap>) {
    for (kind, count, detail) in [
        (
            "unprepared_source",
            preparation.unprepared_sources,
            "Selected source material has not completed compatible semantic preparation.",
        ),
        (
            "unresolved_region",
            preparation.unresolved_regions,
            "Retained evidence regions remain unresolved.",
        ),
        (
            "unreadable_region",
            preparation.unreadable_regions,
            "Retained evidence regions were explicitly recorded as unreadable.",
        ),
        (
            "rejected_proposal",
            preparation.rejected_proposals,
            "Rejected interpretation proposals are retained as coverage gaps, not silently repaired into facts.",
        ),
        (
            "legacy_coverage",
            preparation.legacy_coverage_sources,
            "Prepared knowledge exists without the current native coverage receipt.",
        ),
    ] {
        if count > 0 {
            gaps.push(CollectionGap {
                kind: kind.to_string(),
                count,
                document_id: None,
                source_id: None,
                unit_ids: Vec::new(),
                detail: detail.to_string(),
            });
        }
    }
}

fn publish(
    database: &Database,
    plan: &CoveragePlan,
    synthesis: &CollectionSynthesis,
    reduced: bool,
) -> Result<(), CommandError> {
    let serialized = serde_json::to_string(synthesis).map_err(CommandError::serialization)?;
    if serialized.len() > MAX_SYNTHESIS_BYTES {
        return Err(CommandError::new(
            "collection_synthesis_too_large",
            "The compact collection synthesis exceeds its retained-storage limit. Split the material target explicitly.",
        ));
    }
    let dependencies = dependencies(plan)?;
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        collections::ensure_snapshot_current(&transaction, &plan.snapshot)?;
        plan.context.stamp.ensure_current(&transaction)?;
        transaction
            .execute(
                "INSERT INTO knowledge_collection_views(
                     collection_id,snapshot_id,analysis_hash,view_version,synthesis_json,
                     reduced,stale,created_at,updated_at
                 ) VALUES(?1,?2,?3,1,?4,?5,0,?6,?6)
                 ON CONFLICT(collection_id) DO UPDATE SET
                     snapshot_id=excluded.snapshot_id,
                     analysis_hash=excluded.analysis_hash,
                     view_version=excluded.view_version,
                     synthesis_json=excluded.synthesis_json,
                     reduced=excluded.reduced,
                     stale=0,
                     updated_at=excluded.updated_at",
                params![
                    synthesis.collection_id,
                    plan.snapshot.snapshot_id,
                    plan.analysis_hash,
                    serialized,
                    i64::from(reduced),
                    now,
                ],
            )
            .map_err(CommandError::database)?;
        transaction
            .execute(
                "DELETE FROM knowledge_collection_view_dependencies WHERE collection_id=?1",
                [&synthesis.collection_id],
            )
            .map_err(CommandError::database)?;
        for dependency in dependencies {
            transaction
                .execute(
                    "INSERT INTO knowledge_collection_view_dependencies(
                         collection_id,dependency_kind,dependency_id,dependency_hash
                     ) VALUES(?1,?2,?3,?4)",
                    params![
                        synthesis.collection_id,
                        dependency.kind,
                        dependency.id,
                        dependency.hash
                    ],
                )
                .map_err(CommandError::database)?;
        }
        transaction.commit().map_err(CommandError::database)
    })
}

fn dependencies(plan: &CoveragePlan) -> Result<Vec<Dependency>, CommandError> {
    let mut result = Vec::new();
    for member in &plan.snapshot.members {
        result.push(Dependency {
            kind: "document",
            id: member.document_id.clone(),
            hash: hash(&serde_json::to_string(member).map_err(CommandError::serialization)?),
        });
    }
    for source in &plan.context.sources {
        result.push(Dependency {
            kind: "source",
            id: source.id.clone(),
            hash: source.content_hash.clone(),
        });
    }
    for unit in &plan.context.units {
        result.push(Dependency {
            kind: "unit",
            id: unit.id.clone(),
            hash: unit_hash(unit)?,
        });
    }
    result.push(Dependency {
        kind: "coverage",
        id: "context-v1".to_string(),
        hash: plan.context.stamp.fingerprint().to_string(),
    });
    for disposition in &plan.snapshot.dispositions {
        let encoded = serde_json::to_string(disposition).map_err(CommandError::serialization)?;
        result.push(Dependency {
            kind: "disposition",
            id: hash(&encoded)[..32].to_string(),
            hash: hash(&encoded),
        });
    }
    result
        .sort_by(|left, right| (left.kind, left.id.as_str()).cmp(&(right.kind, right.id.as_str())));
    result.dedup_by(|left, right| left.kind == right.kind && left.id == right.id);
    Ok(result)
}

fn unit_hash(unit: &KnowledgeUnit) -> Result<String, CommandError> {
    Ok(hash(
        &serde_json::to_string(&unit.content).map_err(CommandError::serialization)?,
    ))
}

fn normalize_topic(value: &str) -> String {
    let mut output = String::new();
    let mut space = false;
    for character in value.trim().chars().flat_map(char::to_lowercase) {
        if character.is_alphanumeric() {
            if space && !output.is_empty() {
                output.push(' ');
            }
            output.push(character);
            space = false;
        } else {
            space = true;
        }
    }
    output
}

fn validate_synthesis(synthesis: &CollectionSynthesis) -> Result<(), CommandError> {
    if synthesis.version != VIEW_VERSION
        || synthesis.topics.len() > MAX_TOPICS
        || synthesis.collection_id.len() != 32
        || synthesis.snapshot_id.len() != 64
        || synthesis.summary.chars().count() > 8_000
        || synthesis.learning_objectives.len() > 96
        || synthesis.relationships.len() > 256
    {
        return Err(invalid(
            "The collection synthesis exceeds its supported shape.",
        ));
    }
    let topic_ids = synthesis
        .topics
        .iter()
        .map(|topic| topic.id.as_str())
        .collect::<HashSet<_>>();
    if topic_ids.len() != synthesis.topics.len() {
        return Err(invalid(
            "The collection synthesis repeats a topic identity.",
        ));
    }
    for topic in &synthesis.topics {
        validate_entity_id(&topic.id, "collection synthesis topic")?;
        if topic.title.trim().is_empty()
            || topic.title.chars().count() > 500
            || topic.unit_ids.len() > MAX_COVERAGE_UNITS
        {
            return Err(invalid("A collection synthesis topic is invalid."));
        }
    }
    for relationship in &synthesis.relationships {
        validate_entity_id(&relationship.id, "collection relationship")?;
        if relationship.topic_ids.len() < 2
            || relationship.topic_ids.len() > 4
            || relationship
                .topic_ids
                .iter()
                .any(|id| !topic_ids.contains(id.as_str()))
            || relationship.support_unit_ids.is_empty()
            || relationship.support_unit_ids.len() > 32
            || relationship.note.chars().count() > 1_200
        {
            return Err(invalid("A collection relationship is invalid."));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use rusqlite::params;

    use super::*;
    use crate::{
        document_domain::{SourceLocator, SourceUnitKind},
        knowledge::{
            contracts::{
                ContentOrigin, KnowledgeKind, KnowledgeUnitInput, SourceKind, SourceRecord,
            },
            repository,
        },
    };

    fn id(value: char) -> String {
        value.to_string().repeat(32)
    }

    fn seed_document(
        database: &Database,
        collection: &str,
        membership: char,
        document: char,
        title: &str,
        topic: Option<&str>,
    ) -> Result<(String, String), CommandError> {
        let document_id = id(document);
        let sha = document.to_string().repeat(64);
        let locator = SourceLocator::new(SourceUnitKind::Page, vec![1], None)?;
        let locator_json = serde_json::to_string(&locator).map_err(CommandError::serialization)?;
        let source_id = database.with_connection(|connection| {
            connection
                .execute(
                    "INSERT INTO documents(
                         id,title,original_filename,mime_type,byte_size,sha256,source_relative_path,
                         lifecycle_state,created_at,updated_at,source_format,source_unit_kind,
                         source_unit_count,source_media_type
                     ) VALUES(?1,?2,'fixture.pdf','application/pdf',100,?3,?4,
                         'ready',1,1,'pdf','page',1,'application/pdf')",
                    params![
                        document_id,
                        title,
                        sha,
                        format!("documents/{document_id}/source.pdf")
                    ],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "INSERT INTO document_source_files(
                         document_id,file_ordinal,original_filename,media_type,relative_path,
                         byte_size,sha256,created_at,updated_at
                     ) VALUES(?1,1,'fixture.pdf','application/pdf',?2,100,?3,1,1)",
                    params![
                        document_id,
                        format!("documents/{document_id}/source.pdf"),
                        sha
                    ],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "INSERT INTO document_source_units(
                         document_id,unit_number,unit_kind,label,title,plain_text,text_sha256,
                         source_file_ordinal,locator_json,created_at,updated_at
                     ) VALUES(?1,1,'page','Page 1',NULL,?2,?3,1,?4,1,1)",
                    params![document_id, title, hash(title), locator_json],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "INSERT INTO document_collection_memberships(
                         id,collection_id,document_id,sort_order,created_at,updated_at,revision
                     ) VALUES(?1,?2,?3,1000,1,1,1)",
                    params![id(membership), collection, document_id],
                )
                .map_err(CommandError::database)?;
            let mut record = SourceRecord {
                identity_version: 2,
                id: String::new(),
                kind: SourceKind::Document,
                owner_id: document_id.clone(),
                segment_key: "0".into(),
                owner_revision: "fixture-v1".into(),
                content_hash: String::new(),
                title: title.into(),
                text: title.into(),
                origin: ContentOrigin::SourceDocument,
                document_id: Some(document_id.clone()),
                note_id: None,
                card_id: None,
                guide_id: None,
                conversation_id: None,
                turn_id: None,
                locator: Some(locator),
                images: Vec::new(),
            };
            record.refresh_identity()?;
            repository::publish_source(connection, &record, "0", 1)?;
            Ok(record.id)
        })?;
        if let Some(topic) = topic {
            let source = database.with_connection(|connection| {
                repository::load_source(connection, &source_id)?
                    .ok_or_else(repository::stale_source)
            })?;
            repository::publish_units(
                database,
                &source,
                &hash("fixture-contract"),
                vec![KnowledgeUnitInput {
                    title: title.into(),
                    kind: KnowledgeKind::Fact,
                    statement: title.into(),
                    qualifications: Vec::new(),
                    topic: topic.into(),
                    supporting_excerpt: title.into(),
                    grounding: None,
                    details: None,
                }],
            )?;
        }
        Ok((document_id, source_id))
    }

    fn fixture() -> Result<(Database, String, String, String, String), CommandError> {
        let database =
            Database::open_in_memory().map_err(|message| CommandError::new("fixture", message))?;
        let collection = id('c');
        database.with_connection(|connection| {
            connection
                .execute(
                    "INSERT INTO document_collections(
                         id,parent_id,name,sort_order,archived_at,created_at,updated_at,revision
                     ) VALUES(?1,NULL,'Week 1',1000,NULL,1,1,1)",
                    [&collection],
                )
                .map_err(CommandError::database)?;
            Ok(())
        })?;
        let (first, _) = seed_document(
            &database,
            &collection,
            'm',
            'a',
            "Cardiac output depends on heart rate.",
            Some("Hemodynamics"),
        )?;
        let (second, second_source) = seed_document(
            &database,
            &collection,
            'n',
            'b',
            "A rare qualifier remains important.",
            Some("Rare qualifier"),
        )?;
        Ok((database, collection, first, second, second_source))
    }

    #[test]
    fn native_view_keeps_minority_topics_and_does_not_rewrite_source_knowledge() {
        let (database, collection, _, _, _) = fixture().unwrap();
        let before = database
            .with_connection(|connection| {
                let rows: i64 = connection
                    .query_row("SELECT COUNT(*) FROM knowledge_units", [], |row| row.get(0))
                    .map_err(CommandError::database)?;
                let payload: String = connection
                    .query_row(
                        "SELECT group_concat(id || ':' || content_hash,'|')
                         FROM (SELECT id,content_hash FROM knowledge_units ORDER BY id)",
                        [],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                Ok((rows, payload))
            })
            .unwrap();
        let state = refresh_native(&database, &collection).unwrap();
        assert!(!state.stale);
        assert_eq!(state.synthesis.topics.len(), 2);
        assert!(state
            .synthesis
            .topics
            .iter()
            .any(|topic| topic.title == "Rare qualifier"));
        let after = database
            .with_connection(|connection| {
                let rows: i64 = connection
                    .query_row("SELECT COUNT(*) FROM knowledge_units", [], |row| row.get(0))
                    .map_err(CommandError::database)?;
                let payload: String = connection
                    .query_row(
                        "SELECT group_concat(id || ':' || content_hash,'|')
                         FROM (SELECT id,content_hash FROM knowledge_units ORDER BY id)",
                        [],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                Ok((rows, payload))
            })
            .unwrap();
        assert_eq!(before, after);
    }

    #[test]
    fn source_without_units_is_an_explicit_gap_and_new_knowledge_resolves_it() {
        let (database, collection, _, second, source_id) = fixture().unwrap();
        database
            .with_connection(|connection| {
                connection
                    .execute(
                        "DELETE FROM knowledge_units WHERE source_id=?1",
                        [&source_id],
                    )
                    .map_err(CommandError::database)?;
                connection
                    .execute(
                        "UPDATE knowledge_sources
                         SET extracted_hash='',extraction_contract='' WHERE id=?1",
                        [&source_id],
                    )
                    .map_err(CommandError::database)?;
                Ok(())
            })
            .unwrap();
        let first = refresh_native(&database, &collection).unwrap();
        assert!(first.synthesis.gaps.iter().any(|gap| {
            gap.kind == "no_interpreted_units"
                && gap.document_id.as_deref() == Some(second.as_str())
        }));
        let source = database
            .with_connection(|connection| {
                repository::load_source(connection, &source_id)?
                    .ok_or_else(repository::stale_source)
            })
            .unwrap();
        repository::publish_units(
            &database,
            &source,
            &hash("fixture-contract-2"),
            vec![KnowledgeUnitInput {
                title: "Restored qualifier".into(),
                kind: KnowledgeKind::Fact,
                statement: "A rare qualifier remains important.".into(),
                qualifications: Vec::new(),
                topic: "Rare qualifier".into(),
                supporting_excerpt: "A rare qualifier remains important.".into(),
                grounding: None,
                details: None,
            }],
        )
        .unwrap();
        let second_view = refresh_native(&database, &collection).unwrap();
        assert!(!second_view.synthesis.gaps.iter().any(|gap| {
            gap.kind == "no_interpreted_units"
                && gap.document_id.as_deref() == Some(second.as_str())
        }));
    }

    #[test]
    fn membership_removal_marks_previous_synthesis_stale_without_deleting_it() {
        let (database, collection, _, _, _) = fixture().unwrap();
        let original = refresh_native(&database, &collection).unwrap();
        assert!(!original.stale);
        database
            .with_connection(|connection| {
                connection
                    .execute(
                        "DELETE FROM document_collection_memberships WHERE id=?1",
                        [id('n')],
                    )
                    .map_err(CommandError::database)?;
                connection
                    .execute(
                        "UPDATE document_collection_organization
                         SET revision=revision+1,updated_at=2 WHERE singleton_id=1",
                        [],
                    )
                    .map_err(CommandError::database)?;
                Ok(())
            })
            .unwrap();
        let stale = load(&database, &collection).unwrap().unwrap();
        assert!(stale.stale);
        assert_eq!(stale.synthesis.snapshot_id, original.synthesis.snapshot_id);
        assert!(stale
            .synthesis
            .topics
            .iter()
            .any(|topic| topic.title == "Rare qualifier"));
    }
}
