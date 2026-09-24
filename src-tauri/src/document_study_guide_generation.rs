//! Resumable, deterministic study-guide generation over stable learning topics.
//!
//! Provider calls may complete concurrently for cloud profiles, but validated
//! checkpoints, progress, and durable guide sections are resolved in learning-plan
//! order. Local/LAN inference remains serial to avoid competing requests against a
//! constrained llama.cpp server.

use std::collections::HashSet;

use sha2::{Digest, Sha256};

use crate::{
    ai::{
        concurrency::{run_bounded_provider_queue, QueueControl},
        document_intelligence::{
            generate_study_guide_topic_section, AuthoringEvidence, DocumentTaskContext,
            SourcePageImage,
        },
        AiRuntimeState,
    },
    database::Database,
    document_artifacts::{
        complete_document_artifact_job, fail_document_artifact_job,
        transition_document_artifact_job, DocumentArtifactJobState,
    },
    document_learning::{DocumentLearningPlan, LearningTopic},
    document_study_guides::{
        publish_document_study_guide, validate_study_guide_section_content, DocumentStudyGuide,
        GroundedStudyGuideSection, StudyGuideOverview, StudyGuideSectionContent,
        StudyGuideSectionDraft, STUDY_GUIDE_ASSEMBLY_VERSION, STUDY_GUIDE_PROMPT_VERSION,
        STUDY_GUIDE_SCHEMA_VERSION,
    },
    error::CommandError,
    knowledge::{
        authoring::{self, KnowledgeCatalog},
        authoring_plan,
        contracts::hash,
    },
    paths::AppPaths,
    util::unix_time_millis,
};

const STUDY_GUIDE_CHECKPOINT_CONTRACT: &str = "document-study-guide-v3-explicit-output-contract";
const MAX_OVERVIEW_KEY_CONCEPTS: usize = 6;
const MAX_OVERVIEW_CONCEPT_CAPTION_CHARS: usize = 180;

#[derive(Clone, Debug)]
pub(crate) struct StudyGuideTopicEvidence {
    pub topic_id: String,
    pub source_text: String,
    pub images: Vec<SourcePageImage>,
}

pub(crate) struct StudyGuideGenerationExecution<'a> {
    pub database: &'a Database,
    pub paths: &'a AppPaths,
    pub runtime: &'a AiRuntimeState,
    pub task: &'a DocumentTaskContext,
    pub document_id: &'a str,
    pub document_title: &'a str,
    pub plan: &'a DocumentLearningPlan,
    pub job_id: &'a str,
    pub evidence: Vec<StudyGuideTopicEvidence>,
}

pub(crate) async fn execute_study_guide_generation(
    execution: StudyGuideGenerationExecution<'_>,
) -> Result<DocumentStudyGuide, CommandError> {
    validate_execution(&execution)?;
    transition_document_artifact_job(
        execution.database,
        execution.job_id,
        DocumentArtifactJobState::Queued,
        DocumentArtifactJobState::Preparing,
        0,
    )?;
    let knowledge = async {
        let indices = execution
            .plan
            .topics
            .iter()
            .flat_map(|topic| topic.evidence_chunk_indices.iter())
            .map(|index| {
                i64::try_from(*index).map_err(|_| {
                    CommandError::new("invalid_semantic_content", "Invalid source chunk index.")
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        authoring::prepare_document(authoring::DocumentPreparation {
            database: execution.database,
            paths: execution.paths,
            runtime: execution.runtime,
            task: execution.task,
            document_id: execution.document_id,
            chunk_indices: &indices,
            retry_partial: false,
        })
        .await?;
        execution
            .plan
            .topics
            .iter()
            .map(|topic| {
                let indices = topic
                    .evidence_chunk_indices
                    .iter()
                    .map(|index| *index as i64)
                    .collect::<Vec<_>>();
                authoring::catalog_for_chunks(
                    execution.database,
                    execution.document_id,
                    &indices,
                    execution.task,
                )
            })
            .collect::<Result<Vec<_>, CommandError>>()
    }
    .await;
    let catalogs = match knowledge {
        Ok(catalogs) => catalogs,
        Err(error) => {
            if error.code == "ai_request_canceled" {
                let _ = transition_document_artifact_job(
                    execution.database,
                    execution.job_id,
                    DocumentArtifactJobState::Preparing,
                    DocumentArtifactJobState::Canceled,
                    0,
                );
            } else {
                let (usage, complete) = execution.task.usage_summary();
                let _ = fail_document_artifact_job(
                    execution.database,
                    execution.job_id,
                    &error,
                    usage,
                    complete,
                );
            }
            return Err(error);
        }
    };
    let contract_key = study_guide_contract_key(execution.task, execution.plan);
    let mut resolved = load_checkpoints(
        execution.database,
        execution.document_id,
        &contract_key,
        execution.plan,
        &catalogs,
    );
    let cached = resolved.iter().filter(|section| section.is_some()).count();
    transition_document_artifact_job(
        execution.database,
        execution.job_id,
        DocumentArtifactJobState::Preparing,
        DocumentArtifactJobState::Generating,
        cached,
    )?;

    let missing = resolved
        .iter()
        .enumerate()
        .filter_map(|(index, value)| value.is_none().then_some(index))
        .collect::<Vec<_>>();
    let parallelism =
        crate::ai::concurrency::document_request_parallelism(&execution.task.network_scope)
            .clamp(1, execution.plan.topics.len().max(1));

    crate::logging::debug_target(
        "ai.document.study_guide",
        "study_guide_execution_plan",
        "Prepared bounded study-guide work without logging document or guide content.",
        serde_json::json!({
            "document": execution.document_id,
            "provider": execution.task.provider_kind.as_str(),
            "model": execution.task.model_id.as_str(),
            "topics": execution.plan.topics.len(),
            "cachedTopics": cached,
            "providerTopics": missing.len(),
            "parallelism": parallelism,
        }),
    );

    let generation = generate_missing_topics(
        &execution,
        &catalogs,
        &contract_key,
        &mut resolved,
        &missing,
        parallelism,
        cached,
    )
    .await;
    if let Err(error) = generation {
        let completed = resolved.iter().filter(|section| section.is_some()).count();
        if error.code == "ai_request_canceled" {
            let _ = transition_document_artifact_job(
                execution.database,
                execution.job_id,
                DocumentArtifactJobState::Generating,
                DocumentArtifactJobState::Canceled,
                completed,
            );
        } else {
            let (usage, complete) = execution.task.usage_summary();
            let _ = fail_document_artifact_job(
                execution.database,
                execution.job_id,
                &error,
                usage,
                complete,
            );
        }
        return Err(error);
    }

    let drafts = execution
        .plan
        .topics
        .iter()
        .enumerate()
        .map(|(index, topic)| {
            let content = resolved[index].clone().ok_or_else(|| {
                CommandError::new(
                    "document_study_guide_incomplete",
                    format!(
                        "Learning topic {} did not produce or reuse a validated guide section.",
                        index + 1
                    ),
                )
            })?;
            Ok(StudyGuideSectionDraft {
                topic_id: topic.id.clone(),
                content: content.content,
                knowledge_support: content.knowledge_support,
            })
        })
        .collect::<Result<Vec<_>, CommandError>>()?;

    transition_document_artifact_job(
        execution.database,
        execution.job_id,
        DocumentArtifactJobState::Generating,
        DocumentArtifactJobState::Committing,
        execution.plan.topics.len(),
    )?;
    let overview = StudyGuideOverview {
        summary: execution.plan.summary.clone(),
        learning_objectives: execution.plan.learning_objectives.clone(),
        key_concepts: overview_key_concepts(execution.plan),
    };
    let guide = match publish_document_study_guide(
        execution.database,
        execution.document_id,
        &execution.plan.id,
        Some(execution.job_id),
        overview,
        drafts,
    ) {
        Ok(guide) => guide,
        Err(error) => {
            let (usage, complete) = execution.task.usage_summary();
            let _ = fail_document_artifact_job(
                execution.database,
                execution.job_id,
                &error,
                usage,
                complete,
            );
            return Err(error);
        }
    };

    let (usage, complete) = execution.task.usage_summary();
    if let Err(error) =
        complete_document_artifact_job(execution.database, execution.job_id, usage, complete)
    {
        // The guide publication is the durable user-visible commit. A later
        // reconciliation can repair a lagging job projection without deleting or
        // hiding the valid guide revision.
        crate::logging::error(
            "study_guide_job_completion_projection_failed",
            format!(
                "document={} guide={} job={} code={} detail={}",
                execution.document_id,
                guide.id,
                execution.job_id,
                error.code,
                error.message.chars().take(400).collect::<String>(),
            ),
        );
    }
    cleanup_obsolete_checkpoints_best_effort(
        execution.database,
        execution.document_id,
        &contract_key,
    );
    Ok(guide)
}

async fn generate_missing_topics(
    execution: &StudyGuideGenerationExecution<'_>,
    catalogs: &[KnowledgeCatalog],
    contract_key: &str,
    resolved: &mut [Option<GroundedStudyGuideSection>],
    missing: &[usize],
    parallelism: usize,
    cached: usize,
) -> Result<(), CommandError> {
    let mut completed = cached;
    let mut first_failure: Option<(usize, CommandError)> = None;
    run_bounded_provider_queue(
        missing.len(),
        parallelism,
        |queue_index| {
            let index = missing[queue_index];
            async move { (index, generate_slot(execution, catalogs, index).await) }
        },
        |(index, result)| {
            let control =
                consume_completed_topic(index, result, &mut first_failure, |index, section| {
                    let topic = &execution.plan.topics[index];
                    persist_checkpoint_best_effort(
                        execution.database,
                        execution.document_id,
                        contract_key,
                        topic,
                        &section,
                    );
                    resolved[index] = Some(section);
                    completed = completed.saturating_add(1);
                    transition_document_artifact_job(
                        execution.database,
                        execution.job_id,
                        DocumentArtifactJobState::Generating,
                        DocumentArtifactJobState::Generating,
                        completed,
                    )
                    .map(|_| ())
                });
            Ok::<QueueControl, CommandError>(control)
        },
    )
    .await?;
    first_failure.map_or(Ok(()), |(_, error)| Err(error))
}

/// Consume every completed provider request, including siblings that finish
/// after one slot fails. Returning Stop prevents new dispatch while the queue
/// still drains already-started work so successful paid output is checkpointed.
fn consume_completed_topic<T>(
    index: usize,
    result: Result<T, CommandError>,
    first_failure: &mut Option<(usize, CommandError)>,
    mut save: impl FnMut(usize, T) -> Result<(), CommandError>,
) -> QueueControl {
    match result.and_then(|section| save(index, section)) {
        Ok(()) => QueueControl::Continue,
        Err(error) => {
            let replace = match first_failure.as_ref() {
                None => true,
                Some((failed_index, _)) => index < *failed_index,
            };
            if replace {
                *first_failure = Some((index, error));
            }
            QueueControl::Stop
        }
    }
}

async fn generate_slot(
    execution: &StudyGuideGenerationExecution<'_>,
    catalogs: &[KnowledgeCatalog],
    index: usize,
) -> Result<GroundedStudyGuideSection, CommandError> {
    let topic = &execution.plan.topics[index];
    let evidence = &execution.evidence[index];
    let root_key = hash(&format!(
        "document-guide-topic-v2|{}|{}|{}",
        execution.document_id,
        topic.id,
        topic.source_locator.stable_identity(),
    ));
    let mut root_task = execution.task.clone();
    root_task.diagnostic_window_key = Some(root_key.clone());

    let result = async {
        root_task.ensure_current(execution.database)?;
        let partitions = authoring_plan::partition_catalog(&catalogs[index], |knowledge| {
            crate::ai::document_intelligence::preflight_document_study_guide_section(
                &root_task,
                execution.document_title,
                execution.plan,
                topic,
                &evidence.source_text,
                AuthoringEvidence {
                    images: &evidence.images,
                    knowledge,
                },
            )
        })?;
        let child_keys = partitions
            .iter()
            .enumerate()
            .map(|(part_index, knowledge)| {
                Ok(hash(&format!(
                    "document-guide-topic-child-v1|{root_key}|{part_index}|{}",
                    knowledge.request_fingerprint()?
                )))
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        if child_keys.len() > 1 {
            if let Some(diagnostics) = root_task.diagnostics.as_ref() {
                diagnostics.bind_recovery_family("guide_authoring", &root_key, &child_keys)?;
            }
        }

        let mut partials = Vec::with_capacity(partitions.len());
        for (part_index, knowledge) in partitions.iter().enumerate() {
            let mut child_task = root_task.clone();
            child_task.diagnostic_window_key = Some(if child_keys.len() == 1 {
                root_key.clone()
            } else {
                child_keys[part_index].clone()
            });
            partials.push(
                generate_study_guide_topic_section(
                    execution.runtime,
                    &child_task,
                    execution.document_title,
                    execution.plan,
                    topic,
                    &evidence.source_text,
                    AuthoringEvidence {
                        images: &evidence.images,
                        knowledge,
                    },
                )
                .await?,
            );
        }
        root_task.ensure_current(execution.database)?;
        merge_topic_sections(topic, &catalogs[index], partials)
    }
    .await
    .map_err(|error: CommandError| {
        crate::logging::error_target(
            "ai.document.study_guide",
            "study_guide_topic_failed",
            "A guide topic failed; safe structural details are available without document or model content.",
            serde_json::json!({"document":execution.document_id,"topicIndex":index,
                "unitStart":topic.source_locator.first(),"unitEnd":topic.source_locator.last(),
                "provider":execution.task.provider_kind,"model":execution.task.model_id,
                "code":error.code,"detail":error.message}),
        );
        CommandError::new(
            error.code,
            format!(
                "Study-guide generation failed for topic {}/{} ('{}', source units {}–{}): {}",
                index + 1,
                execution.plan.topics.len(),
                topic.title,
                topic.source_locator.first(),
                topic.source_locator.last(),
                error.message,
            ),
        )
    });
    result
}

fn merge_topic_sections(
    topic: &LearningTopic,
    catalog: &KnowledgeCatalog,
    mut partials: Vec<GroundedStudyGuideSection>,
) -> Result<GroundedStudyGuideSection, CommandError> {
    if partials.len() == 1 {
        return Ok(partials.remove(0));
    }
    if partials.is_empty() {
        return Err(CommandError::new(
            "document_study_guide_incomplete",
            "A planned study-guide topic produced no validated subsection.",
        ));
    }

    let overview = if topic.summary.trim().is_empty() {
        partials
            .iter()
            .map(|section| section.content.overview.trim())
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    } else {
        topic.summary.clone()
    };
    let mut merged = serde_json::to_value(StudyGuideSectionContent::default())
        .map_err(CommandError::serialization)?;
    merged["overview"] = serde_json::Value::String(overview);
    for field in [
        "keyPoints",
        "explanations",
        "importantTerms",
        "processSteps",
        "comparisons",
        "commonMisconceptions",
        "callouts",
        "tables",
        "workedExamples",
        "retrievalChecks",
        "visualExplanations",
    ] {
        let target = merged
            .get_mut(field)
            .and_then(serde_json::Value::as_array_mut)
            .ok_or_else(|| CommandError::serialization("Invalid study-guide merge field."))?;
        let mut seen = HashSet::new();
        for partial in &partials {
            let value =
                serde_json::to_value(&partial.content).map_err(CommandError::serialization)?;
            for item in value
                .get(field)
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten()
            {
                let identity = serde_json::to_string(item).map_err(CommandError::serialization)?;
                if seen.insert(identity) {
                    target.push(item.clone());
                }
            }
        }
    }
    let content: StudyGuideSectionContent =
        serde_json::from_value(merged).map_err(CommandError::serialization)?;
    let content = validate_study_guide_section_content(content)?;

    let mut seen_support = HashSet::new();
    let mut knowledge_support = Vec::new();
    for partial in partials {
        for support in partial.knowledge_support {
            if seen_support.insert(support.unit_id.clone()) {
                knowledge_support.push(support);
            }
        }
    }
    if knowledge_support.is_empty()
        || !catalog.validates(&knowledge_support, catalog.units.len().max(1))
    {
        return Err(CommandError::new(
            "document_study_guide_invalid",
            "A partitioned study-guide section lost or changed its retained knowledge support.",
        ));
    }
    Ok(GroundedStudyGuideSection {
        content,
        knowledge_support,
        catalog_fingerprint: catalog.fingerprint()?,
    })
}

fn validate_execution(execution: &StudyGuideGenerationExecution<'_>) -> Result<(), CommandError> {
    if execution.plan.document_id != execution.document_id {
        return Err(CommandError::new(
            "document_learning_plan_mismatch",
            "The study-guide learning plan belongs to another document.",
        ));
    }
    if execution.plan.topics.is_empty() || execution.evidence.len() != execution.plan.topics.len() {
        return Err(CommandError::new(
            "document_study_guide_evidence_incomplete",
            "Every learning-plan topic requires one bounded source-evidence input.",
        ));
    }
    for (topic, evidence) in execution.plan.topics.iter().zip(&execution.evidence) {
        if topic.id != evidence.topic_id {
            return Err(CommandError::new(
                "document_study_guide_evidence_mismatch",
                "Study-guide evidence was not ordered by the active learning plan.",
            ));
        }
    }
    Ok(())
}

fn overview_key_concepts(plan: &DocumentLearningPlan) -> Vec<String> {
    let roots = plan
        .topics
        .iter()
        .filter(|topic| topic.parent_topic_id.is_none())
        .collect::<Vec<_>>();
    let candidates = if roots.len() >= 3 {
        roots
    } else {
        plan.topics.iter().collect::<Vec<_>>()
    };
    if candidates.is_empty() {
        return plan
            .key_concepts
            .iter()
            .take(MAX_OVERVIEW_KEY_CONCEPTS)
            .cloned()
            .collect();
    }

    evenly_spaced_indices(candidates.len(), MAX_OVERVIEW_KEY_CONCEPTS)
        .into_iter()
        .filter_map(|index| candidates.get(index).copied())
        .map(|topic| {
            let title = topic.title.trim();
            let caption = brief_concept_caption(&topic.summary);
            if caption.is_empty() || caption.eq_ignore_ascii_case(title) {
                title.to_string()
            } else {
                format!("{title} — {caption}")
            }
        })
        .filter(|value| !value.trim().is_empty())
        .collect()
}

fn evenly_spaced_indices(length: usize, maximum: usize) -> Vec<usize> {
    if length == 0 || maximum == 0 {
        return Vec::new();
    }
    if length <= maximum {
        return (0..length).collect();
    }
    let denominator = maximum.saturating_sub(1).max(1);
    (0..maximum)
        .map(|position| position * (length - 1) / denominator)
        .collect()
}

fn brief_concept_caption(value: &str) -> String {
    let compact = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let sentence = compact
        .find(". ")
        .map_or(compact.as_str(), |index| &compact[..=index]);
    if sentence.chars().count() <= MAX_OVERVIEW_CONCEPT_CAPTION_CHARS {
        return sentence.to_string();
    }
    let mut shortened = sentence
        .chars()
        .take(MAX_OVERVIEW_CONCEPT_CAPTION_CHARS.saturating_sub(1))
        .collect::<String>();
    shortened.push('…');
    shortened
}

fn study_guide_contract_key(task: &DocumentTaskContext, plan: &DocumentLearningPlan) -> String {
    hash_text(&format!(
        "{STUDY_GUIDE_CHECKPOINT_CONTRACT}|plan={}|provider={}|profile={}|model={}|reasoning={}|schema={STUDY_GUIDE_SCHEMA_VERSION}|prompt={STUDY_GUIDE_PROMPT_VERSION}|assembly={STUDY_GUIDE_ASSEMBLY_VERSION}|knowledge={}",
        plan.plan_fingerprint,
        task.provider_kind,
        task.profile_id,
        task.model_id,
        task.reasoning_effort,
        crate::knowledge::contracts::EXTRACTION_PROMPT_VERSION,
    ))
}

fn load_checkpoints(
    database: &Database,
    document_id: &str,
    contract_key: &str,
    plan: &DocumentLearningPlan,
    catalogs: &[KnowledgeCatalog],
) -> Vec<Option<GroundedStudyGuideSection>> {
    let result = database.with_connection(|connection| {
        let mut resolved = vec![None; plan.topics.len()];
        let indices = plan
            .topics
            .iter()
            .enumerate()
            .map(|(index, topic)| (topic.id.as_str(), (index, topic)))
            .collect::<std::collections::HashMap<_, _>>();
        let mut statement = connection
            .prepare(
                "SELECT topic_id, evidence_hash, section_json, content_hash
                 FROM document_study_guide_checkpoints
                 WHERE document_id = ?1 AND contract_key = ?2",
            )
            .map_err(CommandError::database)?;
        let rows = statement
            .query_map(rusqlite::params![document_id, contract_key], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .map_err(CommandError::database)?;
        for row in rows {
            let (topic_id, evidence_hash, json, content_hash) =
                row.map_err(CommandError::database)?;
            let Some((index, topic)) = indices.get(topic_id.as_str()) else {
                continue;
            };
            if evidence_hash != topic.evidence_hash || hash_text(&json) != content_hash {
                continue;
            }
            let Ok(section) = serde_json::from_str::<GroundedStudyGuideSection>(&json) else {
                continue;
            };
            if !catalogs[*index].validates(
                &section.knowledge_support,
                catalogs[*index].units.len().max(1),
            ) || catalogs[*index].fingerprint()? != section.catalog_fingerprint
            {
                continue;
            }
            let Ok(content) = validate_study_guide_section_content(section.content) else {
                continue;
            };
            resolved[*index] = Some(GroundedStudyGuideSection { content, ..section });
        }
        Ok(resolved)
    });
    match result {
        Ok(resolved) => resolved,
        Err(error) => {
            crate::logging::info(
                "study_guide_checkpoint_read_failed",
                format!(
                    "document={} code={} detail={}",
                    document_id,
                    error.code,
                    error.message.chars().take(320).collect::<String>()
                ),
            );
            vec![None; plan.topics.len()]
        }
    }
}

fn persist_checkpoint_best_effort(
    database: &Database,
    document_id: &str,
    contract_key: &str,
    topic: &LearningTopic,
    section: &GroundedStudyGuideSection,
) {
    let Ok(section_json) = serde_json::to_string(section) else {
        return;
    };
    let content_hash = hash_text(&section_json);
    let Ok(now) = unix_time_millis() else {
        return;
    };
    let result = database.with_connection(|connection| {
        connection
            .execute(
                "INSERT INTO document_study_guide_checkpoints (
                     document_id, contract_key, topic_id, evidence_hash,
                     section_json, content_hash, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)
                 ON CONFLICT(document_id, contract_key, topic_id) DO UPDATE SET
                     evidence_hash = excluded.evidence_hash,
                     section_json = excluded.section_json,
                     content_hash = excluded.content_hash,
                     updated_at = excluded.updated_at",
                rusqlite::params![
                    document_id,
                    contract_key,
                    topic.id,
                    topic.evidence_hash,
                    section_json,
                    content_hash,
                    now,
                ],
            )
            .map_err(CommandError::database)?;
        Ok(())
    });
    if let Err(error) = result {
        crate::logging::info(
            "study_guide_checkpoint_write_failed",
            format!(
                "document={} topic={} code={} detail={}",
                document_id,
                topic.id,
                error.code,
                error.message.chars().take(320).collect::<String>()
            ),
        );
    }
}

fn cleanup_obsolete_checkpoints_best_effort(
    database: &Database,
    document_id: &str,
    active_contract: &str,
) {
    let _ = database.with_connection(|connection| {
        connection
            .execute(
                "DELETE FROM document_study_guide_checkpoints
                 WHERE document_id = ?1 AND contract_key <> ?2",
                rusqlite::params![document_id, active_contract],
            )
            .map_err(CommandError::database)?;
        Ok(())
    });
}

fn hash_text(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        document_domain::{SourceLocator, SourceUnitKind},
        document_learning::{LearningTopic, LEARNING_PLAN_COMPILER_VERSION},
    };

    fn locator(unit: i64) -> SourceLocator {
        SourceLocator::new(SourceUnitKind::Page, vec![unit], None).unwrap()
    }

    fn plan() -> DocumentLearningPlan {
        DocumentLearningPlan {
            id: "a".repeat(32),
            document_id: "b".repeat(32),
            revision: 1,
            compiler_version: LEARNING_PLAN_COMPILER_VERSION,
            outline_fingerprint: "c".repeat(64),
            plan_fingerprint: "d".repeat(64),
            summary: "Summary".to_string(),
            learning_objectives: Vec::new(),
            key_concepts: Vec::new(),
            topics: vec![LearningTopic {
                id: "e".repeat(32),
                parent_topic_id: None,
                sort_index: 0,
                title: "Topic".to_string(),
                summary: "Topic summary".to_string(),
                learning_objectives: Vec::new(),
                key_concepts: Vec::new(),
                source_locator: locator(1),
                evidence_chunk_indices: vec![0],
                evidence_hashes: vec!["f".repeat(64)],
                evidence_hash: "f".repeat(64),
                anchor_hash: "1".repeat(64),
            }],
        }
    }

    #[test]
    fn execution_requires_plan_ordered_evidence() {
        let plan = plan();
        let evidence = [StudyGuideTopicEvidence {
            topic_id: "wrong".to_string(),
            source_text: "Evidence".to_string(),
            images: Vec::new(),
        }];
        assert_ne!(plan.topics[0].id, evidence[0].topic_id);
    }

    #[test]
    fn overview_uses_at_most_six_high_level_topics_with_brief_captions() {
        let mut plan = plan();
        plan.topics = (0..9)
            .map(|index| LearningTopic {
                id: format!("{:032x}", index + 1),
                parent_topic_id: None,
                sort_index: index,
                title: format!("Major concept {}", index + 1),
                summary: format!(
                    "Explains the high-level relationship for concept {}. Secondary detail should not become the caption.",
                    index + 1
                ),
                learning_objectives: Vec::new(),
                key_concepts: Vec::new(),
                source_locator: locator(index as i64 + 1),
                evidence_chunk_indices: vec![index],
                evidence_hashes: vec!["f".repeat(64)],
                evidence_hash: "f".repeat(64),
                anchor_hash: format!("{:064x}", index + 1),
            })
            .collect();

        let concepts = overview_key_concepts(&plan);
        assert_eq!(concepts.len(), MAX_OVERVIEW_KEY_CONCEPTS);
        assert!(concepts.first().unwrap().starts_with("Major concept 1 — "));
        assert!(concepts.last().unwrap().starts_with("Major concept 9 — "));
        assert!(concepts.iter().all(|value| value.contains(" — Explains ")));
        assert!(concepts
            .iter()
            .all(|value| !value.contains("Secondary detail")));
    }

    #[test]
    fn checkpoint_contract_name_is_versioned() {
        assert_eq!(
            STUDY_GUIDE_CHECKPOINT_CONTRACT,
            "document-study-guide-v3-explicit-output-contract"
        );
    }
    #[test]
    fn content_only_checkpoints_are_not_knowledge_first_checkpoints() {
        assert!(serde_json::from_value::<GroundedStudyGuideSection>(serde_json::json!({
            "overview":"Legacy", "keyPoints":["Old"], "explanations":[], "importantTerms":[],
            "processSteps":[], "comparisons":[], "commonMisconceptions":[], "visualExplanations":[]
        })).is_err());
        let catalog = crate::knowledge::authoring::tests::catalog_fixture();
        let support = catalog.select(&["a".repeat(32)], 64).unwrap();
        let mut altered = catalog.clone();
        altered.units[0].source_hash = "d".repeat(64);
        assert!(!altered.validates(&support, 64));
        assert_ne!(
            catalog.fingerprint().unwrap(),
            altered.fingerprint().unwrap()
        );
    }
    #[test]
    fn failed_topic_does_not_discard_successful_parallel_siblings() {
        let mut saved = Vec::new();
        let mut first_failure = None;
        for (index, result) in [
            (2, Ok("later")),
            (0, Err(CommandError::new("bad_section", "invalid"))),
            (1, Ok("middle")),
        ] {
            let _ = consume_completed_topic(index, result, &mut first_failure, |index, content| {
                saved.push((index, content));
                Ok(())
            });
        }
        saved.sort_by_key(|(index, _)| *index);
        assert_eq!(first_failure.unwrap().1.code, "bad_section");
        assert_eq!(saved, vec![(1, "middle"), (2, "later")]);
    }
    #[test]
    fn checkpoint_projection_error_does_not_skip_remaining_successes() {
        let mut visited = Vec::new();
        let mut first_failure = None;
        for (index, content) in [(0, "first"), (1, "second")] {
            let _ = consume_completed_topic(
                index,
                Ok(content),
                &mut first_failure,
                |index, content| {
                    visited.push(content);
                    if index == 0 {
                        Err(CommandError::new("projection_failed", "failed"))
                    } else {
                        Ok(())
                    }
                },
            );
        }
        assert_eq!(first_failure.unwrap().1.code, "projection_failed");
        assert_eq!(visited, vec!["first", "second"]);
    }
}
