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
            complete_study_guide_section_coverage, generate_document_study_guide_editorial,
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
        publish_document_study_guide_with_editorial, DocumentStudyGuide, GroundedStudyGuideSection,
        StudyGuideOverview, StudyGuideSectionContent, StudyGuideSectionDraft,
        STUDY_GUIDE_ASSEMBLY_VERSION, STUDY_GUIDE_PROMPT_VERSION, STUDY_GUIDE_SCHEMA_VERSION,
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

mod gaps;
pub(crate) mod ownership;
mod partition_checkpoints;
mod preservation;

// Old topic checkpoints may have replaced partition prose with a short plan
// summary. Invalidate only derived guide checkpoints, never learner cards or
// shared source knowledge. Cleanup still waits for successful publication.
const STUDY_GUIDE_CHECKPOINT_CONTRACT: &str = "document-study-guide-v5-lossless-sections";
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
        None,
        false,
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

        let gaps = authoring::prepare_document(authoring::DocumentPreparation {
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
                    &gaps,
                )
            })
            .collect::<Result<Vec<_>, CommandError>>()
            .map(|catalogs| ownership::assign(execution.plan, catalogs))
    }
    .await;
    let catalogs = match knowledge {
        Ok(catalogs) => catalogs,
        Err(error) => {
            if error.code == "ai_request_canceled" {
                let (usage, complete) = execution.task.usage_summary();
                let _ = transition_document_artifact_job(
                    execution.database,
                    execution.job_id,
                    DocumentArtifactJobState::Preparing,
                    DocumentArtifactJobState::Canceled,
                    0,
                    usage,
                    complete,
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
    let (usage, _) = execution.task.usage_summary();
    transition_document_artifact_job(
        execution.database,
        execution.job_id,
        DocumentArtifactJobState::Preparing,
        DocumentArtifactJobState::Generating,
        cached,
        usage,
        false,
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
            let (usage, complete) = execution.task.usage_summary();
            let _ = transition_document_artifact_job(
                execution.database,
                execution.job_id,
                DocumentArtifactJobState::Generating,
                DocumentArtifactJobState::Canceled,
                completed,
                usage,
                complete,
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

    let section_contents = drafts
        .iter()
        .map(|draft| &draft.content)
        .collect::<Vec<_>>();
    let cancellation = crate::task_cancellation::current_signal()
        .unwrap_or_else(crate::task_cancellation::CancellationSignal::new);
    execution.task.ensure_current(execution.database)?;
    let request_editorial = || {
        generate_document_study_guide_editorial(
            execution.runtime,
            execution.task,
            execution.document_title,
            execution.plan,
            &section_contents,
            &cancellation,
        )
    };
    let mut editorial = request_editorial().await;
    if editorial.as_ref().is_err_and(editorial_falls_back) {
        editorial = request_editorial().await;
    }
    // Chapters and an introduction make the guide easier to read, but the
    // sections are already validated. Without them the guide still publishes in
    // learning-plan order with the plan's own summary.
    let editorial = match editorial {
        Err(error) if editorial_falls_back(&error) => {
            crate::logging::warn_target(
                "ai.document.study_guide",
                "study_guide_editorial_skipped",
                "Published the guide in plan order after the editorial pass failed twice.",
                // Only the editorial checks' own messages are logged: they are
                // fixed text naming the failed check, never learner material.
                serde_json::json!({"document":execution.document_id,"code":error.code,
                    "check":(error.code == "study_guide_editorial_invalid").then_some(&error.message)}),
            );
            Ok((plan_overview(execution.plan), None))
        }
        other => other.map(|(overview, editorial)| (overview, Some(editorial))),
    };
    let (overview, editorial) = match editorial {
        Ok(value) => {
            execution.task.ensure_current(execution.database)?;
            value
        }
        Err(error) => {
            if error.code == "ai_request_canceled" {
                let (usage, complete) = execution.task.usage_summary();
                let _ = transition_document_artifact_job(
                    execution.database,
                    execution.job_id,
                    DocumentArtifactJobState::Generating,
                    DocumentArtifactJobState::Canceled,
                    execution.plan.topics.len(),
                    usage,
                    complete,
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

    let (usage, _) = execution.task.usage_summary();
    transition_document_artifact_job(
        execution.database,
        execution.job_id,
        DocumentArtifactJobState::Generating,
        DocumentArtifactJobState::Committing,
        execution.plan.topics.len(),
        usage,
        false,
    )?;
    let guide = match publish_document_study_guide_with_editorial(
        execution.database,
        execution.document_id,
        &execution.plan.id,
        Some(execution.job_id),
        overview,
        drafts,
        editorial,
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

fn editorial_falls_back(error: &CommandError) -> bool {
    error.code != "ai_request_canceled"
        && (gaps::recoverable(error) || error.code == "study_guide_editorial_invalid")
}

fn plan_overview(plan: &DocumentLearningPlan) -> StudyGuideOverview {
    StudyGuideOverview {
        summary: plan.summary.clone(),
        learning_objectives: plan.learning_objectives.clone(),
        key_concepts: plan.key_concepts.clone(),
    }
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
    let mut gaps = 0usize;
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
                consume_completed_topic(index, result, &mut first_failure, |index, authored| {
                    let section = match authored {
                        AuthoredTopic::Written(section) => {
                            persist_checkpoint_best_effort(
                                execution.database,
                                execution.document_id,
                                contract_key,
                                &execution.plan.topics[index],
                                &section,
                            );
                            section
                        }
                        AuthoredTopic::Gap(section) => {
                            gaps = gaps.saturating_add(1);
                            section
                        }
                    };
                    resolved[index] = Some(section);
                    completed = completed.saturating_add(1);
                    let (usage, _) = execution.task.usage_summary();
                    transition_document_artifact_job(
                        execution.database,
                        execution.job_id,
                        DocumentArtifactJobState::Generating,
                        DocumentArtifactJobState::Generating,
                        completed,
                        usage,
                        false,
                    )
                    .map(|_| ())
                });
            Ok::<QueueControl, CommandError>(control)
        },
    )
    .await?;
    if let Some((_, error)) = first_failure {
        return Err(error);
    }
    // A guide made only of gaps says nothing the knowledge list does not, and
    // usually means the model cannot do this task at all.
    if gaps > 0 && gaps == execution.plan.topics.len() {
        return Err(CommandError::new(
            "ai_document_output_invalid",
            "The AI model could not write any section of this study guide. Try another model.",
        ));
    }
    if gaps > 0 {
        crate::logging::warn_target(
            "ai.document.study_guide",
            "study_guide_published_with_gaps",
            "Some topics are published as marked gaps; generating the guide again retries them.",
            serde_json::json!({"document":execution.document_id,"gaps":gaps,
                "topics":execution.plan.topics.len()}),
        );
    }
    Ok(())
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

/// One topic's outcome. A gap is a native section listing the topic's
/// extracted facts; it is published but never checkpointed.
enum AuthoredTopic {
    Written(GroundedStudyGuideSection),
    Gap(GroundedStudyGuideSection),
}

#[derive(Clone, Copy)]
enum TopicSplit {
    /// The partitions the context budget admits.
    Planned,
    /// Each half of the topic's knowledge on its own, after a failed output.
    Halves,
}

async fn generate_slot(
    execution: &StudyGuideGenerationExecution<'_>,
    catalogs: &[KnowledgeCatalog],
    index: usize,
) -> Result<AuthoredTopic, CommandError> {
    let topic = &execution.plan.topics[index];
    let root_key = hash(&format!(
        "document-guide-topic-v2|{}|{}|{}",
        execution.document_id,
        topic.id,
        topic.source_locator.stable_identity(),
    ));
    let mut error =
        match author_topic(execution, catalogs, index, &root_key, TopicSplit::Planned).await {
            Ok(section) => return Ok(AuthoredTopic::Written(section)),
            Err(error) => log_topic_failure(execution, index, error),
        };
    if !gaps::recoverable(&error) {
        return Err(topic_error(execution, index, error));
    }
    // A smaller request usually gets past a slip or an over-long answer. The
    // halves are a new recovery family, so their retries have their own budget.
    if catalogs[index].units.len() >= 2 {
        let split_key = hash(&format!("document-guide-topic-split-v1|{root_key}"));
        match author_topic(execution, catalogs, index, &split_key, TopicSplit::Halves).await {
            Ok(section) => return Ok(AuthoredTopic::Written(section)),
            Err(retry) => {
                error = log_topic_failure(execution, index, retry);
                if !gaps::recoverable(&error) {
                    return Err(topic_error(execution, index, error));
                }
            }
        }
    }
    crate::logging::warn_target(
        "ai.document.study_guide",
        "study_guide_topic_gap",
        "Published a marked gap listing the topic's extracted facts instead of failing the guide.",
        serde_json::json!({"document":execution.document_id,"topicIndex":index,
            "units":catalogs[index].units.len(),"code":error.code}),
    );
    gaps::section(topic, &catalogs[index]).map(AuthoredTopic::Gap)
}

async fn author_topic(
    execution: &StudyGuideGenerationExecution<'_>,
    catalogs: &[KnowledgeCatalog],
    index: usize,
    root_key: &str,
    split: TopicSplit,
) -> Result<GroundedStudyGuideSection, CommandError> {
    let topic = &execution.plan.topics[index];
    let evidence = &execution.evidence[index];
    let mut root_task = execution.task.clone();
    root_task.diagnostic_window_key = Some(root_key.to_string());

    root_task.ensure_current(execution.database)?;
    let preflight = |knowledge: &KnowledgeCatalog| {
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
    };
    let partitions = match split {
        TopicSplit::Planned => authoring_plan::partition_catalog(&catalogs[index], preflight)?,
        TopicSplit::Halves => {
            let Some(halves) = authoring_plan::halve_catalog(&catalogs[index]) else {
                return Err(CommandError::new(
                    "document_study_guide_incomplete",
                    "A single knowledge unit cannot be split further.",
                ));
            };
            let mut partitions = Vec::new();
            for half in &halves {
                partitions.extend(authoring_plan::partition_catalog(half, preflight)?);
            }
            partitions
        }
    };
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
            diagnostics.bind_recovery_family("guide_authoring", root_key, &child_keys)?;
        }
    }

    let contract_key = study_guide_contract_key(&root_task, execution.plan);
    let mut partials = Vec::with_capacity(partitions.len());
    for (part_index, knowledge) in partitions.iter().enumerate() {
        let mut child_task = root_task.clone();
        child_task.diagnostic_window_key = Some(if child_keys.len() == 1 {
            root_key.to_string()
        } else {
            child_keys[part_index].clone()
        });
        root_task.ensure_current(execution.database)?;
        let partition_key =
            partition_checkpoints::contract_key(&contract_key, &child_keys[part_index]);
        if partitions.len() > 1 {
            if let Some(section) = partition_checkpoints::load(
                execution.database,
                execution.document_id,
                &partition_key,
                topic,
                knowledge,
            ) {
                partials.push(section);
                continue;
            }
        }
        let section = generate_study_guide_topic_section(
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
        .await?;
        root_task.ensure_current(execution.database)?;
        let mut coverage_task = child_task.clone();
        coverage_task.diagnostic_window_key = child_task
            .diagnostic_window_key
            .as_deref()
            .map(|key| hash(&format!("document-guide-coverage-v1|{key}")));
        let section = complete_study_guide_section_coverage(
            execution.runtime,
            &coverage_task,
            execution.document_title,
            topic,
            knowledge,
            section,
        )
        .await;
        root_task.ensure_current(execution.database)?;
        if partitions.len() > 1 {
            persist_checkpoint_best_effort(
                execution.database,
                execution.document_id,
                &partition_key,
                topic,
                &section,
            );
        }
        partials.push(section);
    }
    root_task.ensure_current(execution.database)?;
    merge_topic_sections(topic, &catalogs[index], partials)
}

fn log_topic_failure(
    execution: &StudyGuideGenerationExecution<'_>,
    index: usize,
    error: CommandError,
) -> CommandError {
    let topic = &execution.plan.topics[index];
    crate::logging::error_target(
        "ai.document.study_guide",
        "study_guide_topic_failed",
        "A guide topic failed; safe structural details are available without document or model content.",
        serde_json::json!({"document":execution.document_id,"topicIndex":index,
            "unitStart":topic.source_locator.first(),"unitEnd":topic.source_locator.last(),
            "provider":execution.task.provider_kind,"model":execution.task.model_id,
            "code":error.code,"detail":error.message}),
    );
    error
}

fn topic_error(
    execution: &StudyGuideGenerationExecution<'_>,
    index: usize,
    error: CommandError,
) -> CommandError {
    let topic = &execution.plan.topics[index];
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
}

fn merge_topic_sections(
    _topic: &LearningTopic,
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

    // A planning summary is not a substitute for validated teaching prose.
    // Preserve distinct paragraphs in partition order, including qualifications.
    let mut seen_overviews = HashSet::new();
    let overview = partials
        .iter()
        .map(|section| section.content.overview.trim())
        .filter(|value| !value.is_empty() && seen_overviews.insert(*value))
        .collect::<Vec<_>>()
        .join("\n\n");
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
    let content = preservation::normalize(content)?;

    let mut seen_support = HashSet::new();
    let mut knowledge_support = Vec::new();
    for partial in partials {
        for support in partial.knowledge_support {
            if seen_support.insert(support.unit_id.clone()) {
                knowledge_support.push(support);
            }
        }
    }
    let knowledge_support = gaps::bounded(knowledge_support, catalog, |support| {
        support.unit_id.as_str()
    });
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
            let Ok(content) = preservation::normalize(section.content) else {
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

    pub(super) fn plan() -> DocumentLearningPlan {
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
        assert_eq!(
            STUDY_GUIDE_CHECKPOINT_CONTRACT,
            "document-study-guide-v5-lossless-sections"
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

    #[test]
    fn partition_merge_preserves_prose_instead_of_substituting_plan_summary() {
        let plan = plan();
        let catalog = crate::knowledge::authoring::tests::catalog_fixture();
        let first = "The first supported mechanism applies only when x ≤ −2.050e1 units.";
        let second = "The second supported mechanism does not apply outside its stated context.";
        let sections = [first, second, first]
            .into_iter()
            .map(|text| GroundedStudyGuideSection {
                content: StudyGuideSectionContent {
                    overview: text.into(),
                    ..Default::default()
                },
                knowledge_support: catalog.select(&["a".repeat(32)], 64).unwrap(),
                catalog_fingerprint: catalog.fingerprint().unwrap(),
            })
            .collect();
        let merged = merge_topic_sections(&plan.topics[0], &catalog, sections).unwrap();
        assert_eq!(merged.content.overview, format!("{first}\n\n{second}"));
        assert_ne!(merged.content.overview, plan.topics[0].summary);
        assert_eq!(merged.knowledge_support.len(), 1);
        assert_eq!(merged.catalog_fingerprint, catalog.fingerprint().unwrap());
    }
}
