//! Source-grounded learning plans shared by study guides, flashcards, tests, and tutoring.
//!
//! Canonical document content remains authoritative. A learning plan is a durable,
//! provider-neutral projection over that content: it gives stable identities to
//! learnable topics while keeping each topic anchored to canonical source evidence.
//! Generated artifacts may change presentation or wording without changing topic
//! identity when the underlying source evidence is unchanged.

use std::collections::{HashMap, HashSet};

use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    database::Database,
    document_domain::SourceLocator,
    error::CommandError,
    util::{generate_id, unix_time_millis, validate_entity_id},
};

pub(crate) const LEARNING_PLAN_COMPILER_VERSION: i64 = 1;

const MAX_PLAN_TOPICS: usize = 512;
const MAX_EVIDENCE_CHUNKS: usize = 100_000;
const MAX_GLOBAL_ITEMS: usize = 256;
const MAX_TOPIC_ITEMS: usize = 64;
const MAX_TITLE_CHARS: usize = 500;
const MAX_SUMMARY_CHARS: usize = 100_000;
const MAX_ITEM_CHARS: usize = 2_000;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct DocumentLearningPlanInput {
    pub summary: String,
    #[serde(default)]
    pub learning_objectives: Vec<String>,
    #[serde(default)]
    pub key_concepts: Vec<String>,
    pub topics: Vec<LearningTopicInput>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct LearningTopicInput {
    pub parent_index: Option<usize>,
    pub title: String,
    pub summary: String,
    #[serde(default)]
    pub learning_objectives: Vec<String>,
    #[serde(default)]
    pub key_concepts: Vec<String>,
    pub source_locator: SourceLocator,
}

/// Minimal canonical evidence identity consumed by the plan compiler.
///
/// The semantic layer owns how this fingerprint is produced. The learning-plan
/// domain only requires a stable chunk index, canonical locator, and 64-character
/// SHA-256 identity; it never imports parser or provider types.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct LearningEvidenceChunk {
    pub chunk_index: usize,
    pub source_locator: SourceLocator,
    pub content_hash: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct DocumentLearningPlan {
    pub id: String,
    pub document_id: String,
    pub revision: i64,
    pub compiler_version: i64,
    pub outline_fingerprint: String,
    pub plan_fingerprint: String,
    pub summary: String,
    pub learning_objectives: Vec<String>,
    pub key_concepts: Vec<String>,
    pub topics: Vec<LearningTopic>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct LearningTopic {
    pub id: String,
    pub parent_topic_id: Option<String>,
    pub sort_index: usize,
    pub title: String,
    pub summary: String,
    pub learning_objectives: Vec<String>,
    pub key_concepts: Vec<String>,
    pub source_locator: SourceLocator,
    pub evidence_chunk_indices: Vec<usize>,
    pub evidence_hashes: Vec<String>,
    pub evidence_hash: String,
    pub anchor_hash: String,
}

#[derive(Clone, Debug)]
struct CompiledTopicDraft {
    parent_index: Option<usize>,
    sort_index: usize,
    title: String,
    summary: String,
    learning_objectives: Vec<String>,
    key_concepts: Vec<String>,
    source_locator: SourceLocator,
    evidence_chunk_indices: Vec<usize>,
    evidence_hashes: Vec<String>,
    evidence_hash: String,
    anchor_hash: String,
}

#[derive(Clone, Debug)]
struct CompiledPlanDraft {
    outline_fingerprint: String,
    plan_fingerprint: String,
    summary: String,
    learning_objectives: Vec<String>,
    key_concepts: Vec<String>,
    topics: Vec<CompiledTopicDraft>,
}

/// Compile and atomically publish the active learning plan for a document.
///
/// Repeating the same semantic contract is idempotent. A changed presentation
/// creates a new plan revision, while exact source/evidence anchors reuse stable
/// topic IDs so downstream learner associations remain intact.
pub(crate) fn ensure_document_learning_plan(
    database: &Database,
    document_id: &str,
    input: DocumentLearningPlanInput,
    evidence_chunks: &[LearningEvidenceChunk],
) -> Result<DocumentLearningPlan, CommandError> {
    validate_entity_id(document_id, "document")?;
    let draft = compile_plan(input, evidence_chunks)?;
    let now = unix_time_millis()?;

    let plan_id = database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;

        let document_exists = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM documents WHERE id = ?1)",
                [document_id],
                |row| row.get::<_, i64>(0),
            )
            .map(|value| value != 0)
            .map_err(CommandError::database)?;
        if !document_exists {
            return Err(CommandError::new(
                "document_not_found",
                "That document is no longer in the library.",
            ));
        }

        if let Some(existing_id) = transaction
            .query_row(
                "SELECT id FROM document_learning_plans
                 WHERE document_id = ?1 AND plan_fingerprint = ?2 AND state = 'active'",
                params![document_id, draft.plan_fingerprint],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(CommandError::database)?
        {
            transaction.commit().map_err(CommandError::database)?;
            return Ok(existing_id);
        }

        let revision = transaction
            .query_row(
                "SELECT COALESCE(MAX(revision), 0) + 1
                 FROM document_learning_plans WHERE document_id = ?1",
                [document_id],
                |row| row.get::<_, i64>(0),
            )
            .map_err(CommandError::database)?;

        let mut topic_ids = Vec::with_capacity(draft.topics.len());
        for topic in &draft.topics {
            let existing_topic_id = transaction
                .query_row(
                    "SELECT id FROM document_learning_topics
                     WHERE document_id = ?1 AND anchor_hash = ?2",
                    params![document_id, topic.anchor_hash],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(CommandError::database)?;
            let topic_id = match existing_topic_id {
                Some(topic_id) => topic_id,
                None => generate_id(&transaction)?,
            };
            transaction
                .execute(
                    "INSERT OR IGNORE INTO document_learning_topics (
                         id, document_id, anchor_hash, created_at
                     ) VALUES (?1, ?2, ?3, ?4)",
                    params![topic_id, document_id, topic.anchor_hash, now],
                )
                .map_err(CommandError::database)?;
            let persisted_id = transaction
                .query_row(
                    "SELECT id FROM document_learning_topics
                     WHERE document_id = ?1 AND anchor_hash = ?2",
                    params![document_id, topic.anchor_hash],
                    |row| row.get::<_, String>(0),
                )
                .map_err(CommandError::database)?;
            topic_ids.push(persisted_id);
        }

        transaction
            .execute(
                "UPDATE document_learning_plans
                 SET state = 'superseded', updated_at = ?1
                 WHERE document_id = ?2 AND state = 'active'",
                params![now, document_id],
            )
            .map_err(CommandError::database)?;

        let plan_id = generate_id(&transaction)?;
        transaction
            .execute(
                "INSERT INTO document_learning_plans (
                     id, document_id, revision, compiler_version,
                     outline_fingerprint, plan_fingerprint, summary_text,
                     learning_objectives_json, key_concepts_json,
                     state, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'active', ?10, ?10)",
                params![
                    plan_id,
                    document_id,
                    revision,
                    LEARNING_PLAN_COMPILER_VERSION,
                    draft.outline_fingerprint,
                    draft.plan_fingerprint,
                    draft.summary,
                    serialize_json(&draft.learning_objectives)?,
                    serialize_json(&draft.key_concepts)?,
                    now,
                ],
            )
            .map_err(CommandError::database)?;

        for (index, topic) in draft.topics.iter().enumerate() {
            let parent_topic_id = topic.parent_index.map(|parent_index| {
                topic_ids
                    .get(parent_index)
                    .cloned()
                    .expect("validated learning-plan parent index")
            });
            transaction
                .execute(
                    "INSERT INTO document_learning_plan_topics (
                         plan_id, topic_id, parent_topic_id, sort_index,
                         title, summary_text, learning_objectives_json,
                         key_concepts_json, source_locator_json,
                         evidence_chunk_indices_json, evidence_hashes_json,
                         evidence_hash, created_at
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                    params![
                        plan_id,
                        topic_ids[index],
                        parent_topic_id,
                        to_i64(topic.sort_index),
                        topic.title,
                        topic.summary,
                        serialize_json(&topic.learning_objectives)?,
                        serialize_json(&topic.key_concepts)?,
                        serialize_json(&topic.source_locator)?,
                        serialize_json(&topic.evidence_chunk_indices)?,
                        serialize_json(&topic.evidence_hashes)?,
                        topic.evidence_hash,
                        now,
                    ],
                )
                .map_err(CommandError::database)?;
        }

        transaction.commit().map_err(CommandError::database)?;
        Ok(plan_id)
    })?;

    load_document_learning_plan(database, &plan_id)
}

pub(crate) fn load_active_document_learning_plan(
    database: &Database,
    document_id: &str,
) -> Result<Option<DocumentLearningPlan>, CommandError> {
    validate_entity_id(document_id, "document")?;
    let plan_id = database.with_connection(|connection| {
        connection
            .query_row(
                "SELECT id FROM document_learning_plans
                 WHERE document_id = ?1 AND state = 'active'",
                [document_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(CommandError::database)
    })?;
    plan_id
        .map(|plan_id| load_document_learning_plan(database, &plan_id))
        .transpose()
}

pub(crate) fn load_document_learning_plan(
    database: &Database,
    plan_id: &str,
) -> Result<DocumentLearningPlan, CommandError> {
    validate_entity_id(plan_id, "learning plan")?;
    database.with_connection(|connection| {
        let header = connection
            .query_row(
                "SELECT document_id, revision, compiler_version,
                        outline_fingerprint, plan_fingerprint, summary_text,
                        learning_objectives_json, key_concepts_json
                 FROM document_learning_plans WHERE id = ?1",
                [plan_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, String>(7)?,
                    ))
                },
            )
            .optional()
            .map_err(CommandError::database)?
            .ok_or_else(|| {
                CommandError::new(
                    "document_learning_plan_not_found",
                    "That document learning plan is no longer available.",
                )
            })?;

        let mut statement = connection
            .prepare(
                "SELECT topic.id, plan_topic.parent_topic_id,
                        plan_topic.sort_index, plan_topic.title,
                        plan_topic.summary_text,
                        plan_topic.learning_objectives_json,
                        plan_topic.key_concepts_json,
                        plan_topic.source_locator_json,
                        plan_topic.evidence_chunk_indices_json,
                        plan_topic.evidence_hashes_json,
                        plan_topic.evidence_hash, topic.anchor_hash
                 FROM document_learning_plan_topics plan_topic
                 JOIN document_learning_topics topic ON topic.id = plan_topic.topic_id
                 WHERE plan_topic.plan_id = ?1
                 ORDER BY plan_topic.sort_index, topic.id",
            )
            .map_err(CommandError::database)?;
        let topics = statement
            .query_map([plan_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, String>(9)?,
                    row.get::<_, String>(10)?,
                    row.get::<_, String>(11)?,
                ))
            })
            .map_err(CommandError::database)?
            .map(|row| {
                let (
                    id,
                    parent_topic_id,
                    sort_index,
                    title,
                    summary,
                    objectives_json,
                    concepts_json,
                    locator_json,
                    chunk_indices_json,
                    evidence_hashes_json,
                    evidence_hash,
                    anchor_hash,
                ) = row.map_err(CommandError::database)?;
                Ok(LearningTopic {
                    id,
                    parent_topic_id,
                    sort_index: to_usize(sort_index)?,
                    title,
                    summary,
                    learning_objectives: deserialize_json(&objectives_json)?,
                    key_concepts: deserialize_json(&concepts_json)?,
                    source_locator: deserialize_json(&locator_json)?,
                    evidence_chunk_indices: deserialize_json(&chunk_indices_json)?,
                    evidence_hashes: deserialize_json(&evidence_hashes_json)?,
                    evidence_hash,
                    anchor_hash,
                })
            })
            .collect::<Result<Vec<_>, CommandError>>()?;

        Ok(DocumentLearningPlan {
            id: plan_id.to_string(),
            document_id: header.0,
            revision: header.1,
            compiler_version: header.2,
            outline_fingerprint: header.3,
            plan_fingerprint: header.4,
            summary: header.5,
            learning_objectives: deserialize_json(&header.6)?,
            key_concepts: deserialize_json(&header.7)?,
            topics,
        })
    })
}

/// Select source-overlapping topics in deterministic specificity order.
///
/// The narrowest topic is first and is the natural primary association for a
/// generated note. Wider ancestors or neighboring overlapping topics may be
/// retained as supporting associations.
pub(crate) fn topics_for_source_locator<'a>(
    plan: &'a DocumentLearningPlan,
    locator: &SourceLocator,
) -> Vec<&'a LearningTopic> {
    let mut topics = plan
        .topics
        .iter()
        .filter(|topic| locators_overlap(&topic.source_locator, locator))
        .collect::<Vec<_>>();
    topics.sort_by_key(|topic| {
        (
            topic.source_locator.unit_numbers.len(),
            topic.sort_index,
            topic.id.as_str(),
        )
    });
    topics
}

fn compile_plan(
    input: DocumentLearningPlanInput,
    evidence_chunks: &[LearningEvidenceChunk],
) -> Result<CompiledPlanDraft, CommandError> {
    if input.topics.is_empty() || input.topics.len() > MAX_PLAN_TOPICS {
        return Err(CommandError::new(
            "document_learning_plan_invalid",
            format!(
                "A document learning plan must contain between 1 and {MAX_PLAN_TOPICS} source-grounded topics."
            ),
        ));
    }
    if evidence_chunks.is_empty() || evidence_chunks.len() > MAX_EVIDENCE_CHUNKS {
        return Err(CommandError::new(
            "document_learning_plan_evidence_invalid",
            "The document learning plan requires a bounded set of canonical semantic evidence chunks.",
        ));
    }

    let mut evidence_by_index = HashMap::new();
    for chunk in evidence_chunks {
        if !is_sha256(&chunk.content_hash)
            || evidence_by_index.insert(chunk.chunk_index, chunk).is_some()
        {
            return Err(CommandError::new(
                "document_learning_plan_evidence_invalid",
                "Canonical learning evidence contained an invalid hash or duplicate chunk index.",
            ));
        }
    }

    let normalized = DocumentLearningPlanInput {
        summary: normalize_text(
            &input.summary,
            MAX_SUMMARY_CHARS,
            "Structured learning material from the source document.",
        ),
        learning_objectives: normalize_items(
            input.learning_objectives,
            MAX_GLOBAL_ITEMS,
            MAX_ITEM_CHARS,
        ),
        key_concepts: normalize_items(input.key_concepts, MAX_GLOBAL_ITEMS, MAX_ITEM_CHARS),
        topics: input
            .topics
            .into_iter()
            .enumerate()
            .map(|(index, topic)| normalize_topic_input(index, topic))
            .collect::<Result<Vec<_>, _>>()?,
    };
    let outline_fingerprint = hash_serialized(&normalized)?;

    let mut compiled_topics = Vec::with_capacity(normalized.topics.len());
    for (sort_index, topic) in normalized.topics.iter().enumerate() {
        let mut matching = evidence_chunks
            .iter()
            .filter(|chunk| locators_overlap(&topic.source_locator, &chunk.source_locator))
            .collect::<Vec<_>>();
        matching.sort_by_key(|chunk| chunk.chunk_index);
        if matching.is_empty() {
            return Err(CommandError::new(
                "document_learning_topic_without_evidence",
                format!(
                    "Learning topic {} does not overlap canonical semantic evidence.",
                    sort_index + 1
                ),
            ));
        }

        let mut asset_ids = topic.source_locator.asset_ids.clone();
        asset_ids.extend(
            matching
                .iter()
                .flat_map(|chunk| chunk.source_locator.asset_ids.iter().cloned()),
        );
        asset_ids.sort();
        asset_ids.dedup();
        let source_locator = SourceLocator::with_evidence(
            topic.source_locator.unit_kind,
            topic.source_locator.unit_numbers.clone(),
            topic.source_locator.section_label.clone(),
            asset_ids,
            topic.source_locator.time_range_ms,
        )?;

        let evidence_chunk_indices = matching
            .iter()
            .map(|chunk| chunk.chunk_index)
            .collect::<Vec<_>>();
        let evidence_hashes = matching
            .iter()
            .map(|chunk| chunk.content_hash.clone())
            .collect::<Vec<_>>();
        let evidence_hash = hash_text(&format!(
            "evidence-v1|{}",
            matching
                .iter()
                .map(|chunk| format!("{}:{}", chunk.chunk_index, chunk.content_hash))
                .collect::<Vec<_>>()
                .join("|")
        ));
        let mut stable_evidence = matching
            .iter()
            .map(|chunk| {
                format!(
                    "{}:{}",
                    chunk.source_locator.stable_identity(),
                    chunk.content_hash
                )
            })
            .collect::<Vec<_>>();
        stable_evidence.sort();
        stable_evidence.dedup();
        let stable_evidence_hash = hash_text(&format!(
            "topic-evidence-anchor-v1|{}",
            stable_evidence.join("|")
        ));
        let anchor_hash = hash_text(&format!(
            "topic-anchor-v2|locator={}|evidence={stable_evidence_hash}",
            source_locator.stable_identity(),
        ));
        compiled_topics.push(CompiledTopicDraft {
            parent_index: topic.parent_index,
            sort_index,
            title: topic.title.clone(),
            summary: topic.summary.clone(),
            learning_objectives: topic.learning_objectives.clone(),
            key_concepts: topic.key_concepts.clone(),
            source_locator,
            evidence_chunk_indices,
            evidence_hashes,
            evidence_hash,
            anchor_hash,
        });
    }

    let plan_fingerprint = hash_serialized(&serde_json::json!({
        "compilerVersion": LEARNING_PLAN_COMPILER_VERSION,
        "outlineFingerprint": outline_fingerprint,
        "summary": normalized.summary,
        "learningObjectives": normalized.learning_objectives,
        "keyConcepts": normalized.key_concepts,
        "topics": compiled_topics.iter().map(|topic| serde_json::json!({
            "parentIndex": topic.parent_index,
            "sortIndex": topic.sort_index,
            "title": topic.title,
            "summary": topic.summary,
            "learningObjectives": topic.learning_objectives,
            "keyConcepts": topic.key_concepts,
            "sourceLocator": topic.source_locator,
            "evidenceHash": topic.evidence_hash,
            "anchorHash": topic.anchor_hash,
        })).collect::<Vec<_>>(),
    }))?;

    Ok(CompiledPlanDraft {
        outline_fingerprint,
        plan_fingerprint,
        summary: normalized.summary,
        learning_objectives: normalized.learning_objectives,
        key_concepts: normalized.key_concepts,
        topics: compiled_topics,
    })
}

fn normalize_topic_input(
    index: usize,
    topic: LearningTopicInput,
) -> Result<LearningTopicInput, CommandError> {
    if topic.parent_index.is_some_and(|parent| parent >= index) {
        return Err(CommandError::new(
            "document_learning_plan_invalid",
            format!(
                "Learning topic {} must reference an earlier parent topic.",
                index + 1
            ),
        ));
    }
    Ok(LearningTopicInput {
        parent_index: topic.parent_index,
        title: normalize_text(
            &topic.title,
            MAX_TITLE_CHARS,
            &format!("Learning topic {}", index + 1),
        ),
        summary: normalize_text(
            &topic.summary,
            MAX_SUMMARY_CHARS,
            "Source-grounded learning material.",
        ),
        learning_objectives: normalize_items(
            topic.learning_objectives,
            MAX_TOPIC_ITEMS,
            MAX_ITEM_CHARS,
        ),
        key_concepts: normalize_items(topic.key_concepts, MAX_TOPIC_ITEMS, MAX_ITEM_CHARS),
        source_locator: topic.source_locator,
    })
}

fn locators_overlap(left: &SourceLocator, right: &SourceLocator) -> bool {
    left.unit_kind == right.unit_kind
        && left
            .unit_numbers
            .iter()
            .any(|number| right.unit_numbers.contains(number))
}

fn normalize_items(values: Vec<String>, maximum: usize, maximum_chars: usize) -> Vec<String> {
    let mut seen = HashSet::new();
    values
        .into_iter()
        .filter_map(|value| {
            let normalized = normalize_text(&value, maximum_chars, "");
            if normalized.is_empty() {
                return None;
            }
            seen.insert(normalized.to_ascii_lowercase())
                .then_some(normalized)
        })
        .take(maximum)
        .collect()
}

fn normalize_text(value: &str, maximum_chars: usize, fallback: &str) -> String {
    let trimmed = value.trim();
    let selected = if trimmed.is_empty() {
        fallback.trim()
    } else {
        trimmed
    };
    selected.chars().take(maximum_chars).collect()
}

fn hash_serialized<T: Serialize>(value: &T) -> Result<String, CommandError> {
    let bytes = serde_json::to_vec(value).map_err(CommandError::serialization)?;
    Ok(hash_bytes(&bytes))
}

fn hash_text(value: &str) -> String {
    hash_bytes(value.as_bytes())
}

fn hash_bytes(value: &[u8]) -> String {
    format!("{:x}", Sha256::digest(value))
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn serialize_json<T: Serialize>(value: &T) -> Result<String, CommandError> {
    serde_json::to_string(value).map_err(CommandError::serialization)
}

fn deserialize_json<T: for<'de> Deserialize<'de>>(value: &str) -> Result<T, CommandError> {
    serde_json::from_str(value).map_err(CommandError::serialization)
}

fn to_i64(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn to_usize(value: i64) -> Result<usize, CommandError> {
    usize::try_from(value).map_err(|_| {
        CommandError::new(
            "document_learning_plan_invalid",
            "A persisted learning-plan index was invalid.",
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document_domain::{SourceLocator, SourceUnitKind};

    const DOCUMENT_ID: &str = "0123456789abcdef0123456789abcdef";

    fn database() -> Database {
        let database = Database::open_in_memory().unwrap();
        database
            .with_connection(|connection| {
                connection
                    .execute(
                        "INSERT INTO documents (
                             id, title, original_filename, mime_type, byte_size, sha256,
                             source_relative_path, lifecycle_state, page_count,
                             source_format, source_unit_kind, created_at, updated_at
                         ) VALUES (?1, 'Physiology', 'physiology.pdf', 'application/pdf', 10,
                                   ?2, 'documents/physiology.pdf', 'ready', 4,
                                   'pdf', 'page', 1, 1)",
                        params![DOCUMENT_ID, "a".repeat(64)],
                    )
                    .map_err(CommandError::database)?;
                Ok(())
            })
            .unwrap();
        database
    }

    fn locator(start: i64, end: i64) -> SourceLocator {
        SourceLocator::new(SourceUnitKind::Page, (start..=end).collect(), None).unwrap()
    }

    fn input(title: &str) -> DocumentLearningPlanInput {
        DocumentLearningPlanInput {
            summary: "Cardiovascular physiology".to_string(),
            learning_objectives: vec!["Explain ventricular filling".to_string()],
            key_concepts: vec!["Preload".to_string()],
            topics: vec![LearningTopicInput {
                parent_index: None,
                title: title.to_string(),
                summary: "How ventricular filling affects contraction.".to_string(),
                learning_objectives: vec!["Relate preload to stroke volume".to_string()],
                key_concepts: vec!["Frank-Starling mechanism".to_string()],
                source_locator: locator(1, 2),
            }],
        }
    }

    fn evidence(hash: char) -> Vec<LearningEvidenceChunk> {
        vec![
            LearningEvidenceChunk {
                chunk_index: 0,
                source_locator: locator(1, 1),
                content_hash: hash.to_string().repeat(64),
            },
            LearningEvidenceChunk {
                chunk_index: 1,
                source_locator: locator(2, 2),
                content_hash: "b".repeat(64),
            },
        ]
    }

    #[test]
    fn presentation_revision_reuses_stable_topic_identity() {
        let database = database();
        let first = ensure_document_learning_plan(
            &database,
            DOCUMENT_ID,
            input("Ventricular preload"),
            &evidence('a'),
        )
        .unwrap();
        let second = ensure_document_learning_plan(
            &database,
            DOCUMENT_ID,
            input("Preload and ventricular filling"),
            &evidence('a'),
        )
        .unwrap();

        assert_eq!(first.revision, 1);
        assert_eq!(second.revision, 2);
        assert_eq!(first.topics[0].id, second.topics[0].id);
        assert_ne!(first.plan_fingerprint, second.plan_fingerprint);
    }

    #[test]
    fn changed_source_evidence_creates_a_new_topic_identity() {
        let database = database();
        let first = ensure_document_learning_plan(
            &database,
            DOCUMENT_ID,
            input("Ventricular preload"),
            &evidence('a'),
        )
        .unwrap();
        let second = ensure_document_learning_plan(
            &database,
            DOCUMENT_ID,
            input("Ventricular preload"),
            &evidence('c'),
        )
        .unwrap();

        assert_ne!(first.topics[0].id, second.topics[0].id);
        assert_ne!(first.topics[0].anchor_hash, second.topics[0].anchor_hash);
    }

    #[test]
    fn semantic_chunk_reindexing_preserves_topic_identity() {
        let database = database();
        let first = ensure_document_learning_plan(
            &database,
            DOCUMENT_ID,
            input("Ventricular preload"),
            &evidence('a'),
        )
        .unwrap();
        let mut reindexed = evidence('a');
        reindexed[0].chunk_index = 20;
        reindexed[1].chunk_index = 21;
        let second = ensure_document_learning_plan(
            &database,
            DOCUMENT_ID,
            input("Ventricular preload"),
            &reindexed,
        )
        .unwrap();

        assert_eq!(first.topics[0].id, second.topics[0].id);
        assert_ne!(
            first.topics[0].evidence_hash,
            second.topics[0].evidence_hash
        );
    }

    #[test]
    fn identical_contract_is_idempotent() {
        let database = database();
        let first = ensure_document_learning_plan(
            &database,
            DOCUMENT_ID,
            input("Ventricular preload"),
            &evidence('a'),
        )
        .unwrap();
        let second = ensure_document_learning_plan(
            &database,
            DOCUMENT_ID,
            input("Ventricular preload"),
            &evidence('a'),
        )
        .unwrap();

        assert_eq!(first.id, second.id);
        assert_eq!(first.revision, second.revision);
    }

    #[test]
    fn source_selection_prefers_the_narrowest_overlapping_topic() {
        let plan = DocumentLearningPlan {
            id: "1".repeat(32),
            document_id: DOCUMENT_ID.to_string(),
            revision: 1,
            compiler_version: LEARNING_PLAN_COMPILER_VERSION,
            outline_fingerprint: "a".repeat(64),
            plan_fingerprint: "b".repeat(64),
            summary: String::new(),
            learning_objectives: Vec::new(),
            key_concepts: Vec::new(),
            topics: vec![
                LearningTopic {
                    id: "2".repeat(32),
                    parent_topic_id: None,
                    sort_index: 0,
                    title: "Whole chapter".to_string(),
                    summary: String::new(),
                    learning_objectives: Vec::new(),
                    key_concepts: Vec::new(),
                    source_locator: locator(1, 4),
                    evidence_chunk_indices: vec![0],
                    evidence_hashes: vec!["a".repeat(64)],
                    evidence_hash: "a".repeat(64),
                    anchor_hash: "b".repeat(64),
                },
                LearningTopic {
                    id: "3".repeat(32),
                    parent_topic_id: Some("2".repeat(32)),
                    sort_index: 1,
                    title: "Preload".to_string(),
                    summary: String::new(),
                    learning_objectives: Vec::new(),
                    key_concepts: Vec::new(),
                    source_locator: locator(2, 2),
                    evidence_chunk_indices: vec![1],
                    evidence_hashes: vec!["c".repeat(64)],
                    evidence_hash: "c".repeat(64),
                    anchor_hash: "d".repeat(64),
                },
            ],
        };

        let selected = topics_for_source_locator(&plan, &locator(2, 2));
        assert_eq!(selected[0].title, "Preload");
        assert_eq!(selected[1].title, "Whole chapter");
    }

    #[test]
    fn parent_topic_must_precede_its_child() {
        let mut invalid = input("Topic");
        invalid.topics[0].parent_index = Some(0);
        let error = compile_plan(invalid, &evidence('a')).unwrap_err();
        assert_eq!(error.code, "document_learning_plan_invalid");
    }
}
