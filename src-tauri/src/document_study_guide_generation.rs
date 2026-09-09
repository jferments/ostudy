//! Resumable, deterministic study-guide generation over stable learning topics.
//!
//! Provider calls may complete concurrently for cloud profiles, but validated
//! checkpoints, progress, and durable guide sections are resolved in learning-plan
//! order. Local/LAN inference remains serial to avoid competing requests against a
//! constrained llama.cpp server.

use sha2::{Digest, Sha256};

use crate::{
    ai::{
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
        GroundedStudyGuideSection, StudyGuideOverview, StudyGuideSectionDraft,
        STUDY_GUIDE_ASSEMBLY_VERSION, STUDY_GUIDE_PROMPT_VERSION, STUDY_GUIDE_SCHEMA_VERSION,
    },
    error::CommandError,
    knowledge::authoring::{self, KnowledgeCatalog},
    paths::AppPaths,
    util::unix_time_millis,
};

const CLOUD_GUIDE_PARALLELISM: usize = 4;
const STUDY_GUIDE_CHECKPOINT_CONTRACT: &str = "document-study-guide-v2-knowledge-source";

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

        authoring::prepare_document(
            execution.database,
            execution.paths,
            execution.runtime,
            execution.task,
            execution.document_id,
            &indices,
        )
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
    let parallelism = if execution.task.network_scope == "cloud" {
        CLOUD_GUIDE_PARALLELISM
    } else {
        1
    }
    .clamp(1, execution.plan.topics.len());

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
        key_concepts: execution.plan.key_concepts.clone(),
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
    let mut cursor = 0_usize;
    let mut completed = cached;
    while cursor < missing.len() {
        let end = cursor.saturating_add(parallelism).min(missing.len());
        let batch = &missing[cursor..end];
        let i0 = batch.first().copied();
        let i1 = batch.get(1).copied();
        let i2 = batch.get(2).copied();
        let i3 = batch.get(3).copied();
        let (r0, r1, r2, r3) = tokio::join!(
            generate_slot(execution, catalogs, i0),
            generate_slot(execution, catalogs, i1),
            generate_slot(execution, catalogs, i2),
            generate_slot(execution, catalogs, i3),
        );

        let mut results = [r0, r1, r2, r3].into_iter().flatten().collect::<Vec<_>>();
        results.sort_by_key(|(index, _)| *index);
        for (index, result) in results {
            let section = result?;
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
            )?;
        }
        cursor = end;
    }
    Ok(())
}

async fn generate_slot(
    execution: &StudyGuideGenerationExecution<'_>,
    catalogs: &[KnowledgeCatalog],
    index: Option<usize>,
) -> Option<(usize, Result<GroundedStudyGuideSection, CommandError>)> {
    let index = index?;
    let topic = &execution.plan.topics[index];
    let evidence = &execution.evidence[index];
    let result = generate_study_guide_topic_section(
        execution.runtime,
        execution.task,
        execution.document_title,
        execution.plan,
        topic,
        &evidence.source_text,
        AuthoringEvidence {
            images: &evidence.images,
            knowledge: &catalogs[index],
        },
    )
    .await
    .map_err(|error| {
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
    Some((index, result))
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
            if !catalogs[*index].validates(&section.knowledge_support, 64)
                || catalogs[*index].fingerprint()? != section.catalog_fingerprint
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
    fn checkpoint_contract_name_is_versioned() {
        assert!(STUDY_GUIDE_CHECKPOINT_CONTRACT.contains("v2"));
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
}
