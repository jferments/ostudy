use rusqlite::{params, OptionalExtension};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tauri::State;

use crate::{
    ai::{document_intelligence, document_provider, AiRuntimeState},
    database::Database,
    document_artifacts::{
        create_document_artifact_job, load_document_artifact_job, DocumentArtifactJob,
        DocumentArtifactKind, DocumentArtifactProviderSnapshot,
    },
    document_cost,
    document_domain::{DocumentFormat, DocumentSourceDescriptor, SourceUnitKind},
    document_learning::{
        ensure_document_learning_plan, load_active_document_learning_plan,
        DocumentLearningPlan, LearningEvidenceChunk,
    },
    document_learning_plan::learning_plan_input_from_outline,
    document_outline_service::{ensure_document_outline, EnsureDocumentOutlineInput},
    document_scale::DocumentScalePlan,
    document_semantics::SemanticChunk,
    document_store,
    document_study_guide_generation::{
        execute_study_guide_generation, StudyGuideGenerationExecution,
        StudyGuideTopicEvidence,
    },
    document_study_guides::{
        discard_study_guide_overlay, load_active_document_study_guide,
        save_study_guide_overlay, DocumentStudyGuide, SaveStudyGuideOverlayInput,
        StudyGuideOverlay,
    },
    document_topic_links::{load_topic_card_ids, load_topic_card_projection, TopicCardProjection},
    error::CommandError,
    util::validate_entity_id,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct GenerateDocumentStudyGuideInput {
    pub document_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct GetDocumentStudyGuideInput {
    pub document_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct DiscardStudyGuideOverlayInput {
    pub overlay_id: String,
    pub revision: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct GetTopicCardsInput {
    pub document_id: String,
    pub topic_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct GetTopicCardProjectionInput {
    pub document_id: String,
    pub topic_id: String,
}

#[derive(Clone, Debug)]
struct StoredGuideDocument {
    title: String,
    extraction_backend: String,
    source: DocumentSourceDescriptor,
    archived: bool,
}

#[tauri::command]
pub(crate) async fn generate_document_study_guide(
    database: State<'_, Database>,
    runtime: State<'_, AiRuntimeState>,
    input: GenerateDocumentStudyGuideInput,
) -> Result<DocumentStudyGuide, CommandError> {
    validate_entity_id(&input.document_id, "document")?;
    let stored = load_guide_document(database.inner(), &input.document_id)?;
    if stored.archived {
        return Err(CommandError::new(
            "document_archived",
            "Restore this document before creating or regenerating its study guide.",
        ));
    }
    stored.source.validate()?;
    let chunks = document_store::load_semantic_chunks(database.inner(), &input.document_id)?;
    if chunks.is_empty() {
        return Err(CommandError::new(
            "document_has_no_semantic_content",
            "The document did not contain canonical semantic chunks that ostudy could teach from.",
        ));
    }

    let mut task = document_intelligence::load_document_task_context(database.inner())?;
    let capabilities = document_provider::load_document_provider_capabilities(
        database.inner(),
        &task.profile_id,
        &task.provider_kind,
        &task.model_id,
    )?;
    task.apply_provider_capabilities(&capabilities);
    let calibration = document_cost::load_cost_calibration(
        database.inner(),
        &task.provider_kind,
        &task.model_id,
    )?;
    let scale_plan = DocumentScalePlan::for_capabilities(
        stored.source.unit_count,
        &capabilities,
        &task.reasoning_effort,
        calibration,
    )?;
    let capability_fingerprint = capabilities.planning_fingerprint();
    let outline = ensure_document_outline(EnsureDocumentOutlineInput {
        database: database.inner(),
        runtime: runtime.inner(),
        task: &task,
        document_id: &input.document_id,
        document_title: &stored.title,
        extraction_backend: &stored.extraction_backend,
        source: &stored.source,
        chunks: &chunks,
        plan: scale_plan,
        provider_capability_fingerprint: &capability_fingerprint,
    })
    .await?;

    let plan_input = learning_plan_input_from_outline(
        &outline,
        stored.source.unit_kind.clone(),
        stored.source.unit_count,
    )?;
    let evidence_chunks = chunks
        .iter()
        .map(learning_evidence_chunk)
        .collect::<Result<Vec<_>, _>>()?;
    let learning_plan = ensure_document_learning_plan(
        database.inner(),
        &input.document_id,
        plan_input,
        &evidence_chunks,
    )?;
    let topic_evidence = build_topic_evidence(&learning_plan, &chunks)?;

    let provider = DocumentArtifactProviderSnapshot {
        profile_id: Some(task.profile_id.clone()),
        provider_kind: task.provider_kind.clone(),
        model_id: task.model_id.clone(),
        profile_revision: task.profile_revision,
        credential_epoch: task.credential_epoch,
        network_scope: task.network_scope.clone(),
    };
    let job = create_document_artifact_job(
        database.inner(),
        &input.document_id,
        Some(&learning_plan.id),
        DocumentArtifactKind::StudyGuide,
        &provider,
        learning_plan.topics.len(),
    )?;

    execute_study_guide_generation(StudyGuideGenerationExecution {
        database: database.inner(),
        runtime: runtime.inner(),
        task: &task,
        document_id: &input.document_id,
        document_title: &stored.title,
        plan: &learning_plan,
        job_id: &job.id,
        evidence: topic_evidence,
    })
    .await
}

#[tauri::command]
pub(crate) fn get_document_study_guide(
    database: State<'_, Database>,
    input: GetDocumentStudyGuideInput,
) -> Result<Option<DocumentStudyGuide>, CommandError> {
    load_active_document_study_guide(database.inner(), &input.document_id)
}

#[tauri::command]
pub(crate) fn get_document_learning_plan(
    database: State<'_, Database>,
    input: GetDocumentStudyGuideInput,
) -> Result<Option<DocumentLearningPlan>, CommandError> {
    load_active_document_learning_plan(database.inner(), &input.document_id)
}

#[tauri::command]
pub(crate) fn get_document_artifact_job(
    database: State<'_, Database>,
    job_id: String,
) -> Result<DocumentArtifactJob, CommandError> {
    load_document_artifact_job(database.inner(), &job_id)
}

#[tauri::command]
pub(crate) fn save_document_study_guide_overlay(
    database: State<'_, Database>,
    input: SaveStudyGuideOverlayInput,
) -> Result<StudyGuideOverlay, CommandError> {
    save_study_guide_overlay(database.inner(), input)
}

#[tauri::command]
pub(crate) fn discard_document_study_guide_overlay(
    database: State<'_, Database>,
    input: DiscardStudyGuideOverlayInput,
) -> Result<(), CommandError> {
    discard_study_guide_overlay(database.inner(), &input.overlay_id, input.revision)
}

#[tauri::command]
pub(crate) fn get_document_topic_card_projection(
    database: State<'_, Database>,
    input: GetTopicCardProjectionInput,
) -> Result<TopicCardProjection, CommandError> {
    load_topic_card_projection(database.inner(), &input.document_id, &input.topic_id)
}

#[tauri::command]
pub(crate) fn get_document_topic_card_ids(
    database: State<'_, Database>,
    input: GetTopicCardsInput,
) -> Result<Vec<String>, CommandError> {
    load_topic_card_ids(database.inner(), &input.document_id, &input.topic_ids)
}

fn load_guide_document(
    database: &Database,
    document_id: &str,
) -> Result<StoredGuideDocument, CommandError> {
    database.with_connection(|connection| {
        connection
            .query_row(
                "SELECT title, extraction_backend, source_format,
                        source_unit_kind, page_count, archived_at
                 FROM documents WHERE id = ?1",
                [document_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, Option<i64>>(5)?,
                    ))
                },
            )
            .optional()
            .map_err(CommandError::database)?
            .ok_or_else(|| {
                CommandError::new(
                    "document_not_found",
                    "That document is no longer in the library.",
                )
            })
            .and_then(|row| {
                let unit_count = usize::try_from(row.4).map_err(|_| {
                    CommandError::new(
                        "invalid_document_source",
                        "The saved document source-unit count is invalid.",
                    )
                })?;
                Ok(StoredGuideDocument {
                    title: row.0,
                    extraction_backend: row.1,
                    source: DocumentSourceDescriptor {
                        format: DocumentFormat::from_db(&row.2)?,
                        unit_kind: SourceUnitKind::from_db(&row.3)?,
                        unit_count,
                    },
                    archived: row.5.is_some(),
                })
            })
    })
}

fn learning_evidence_chunk(chunk: &SemanticChunk) -> Result<LearningEvidenceChunk, CommandError> {
    let locator = chunk.source_locator.clone();
    let content_hash = hash_text(&format!(
        "learning-evidence-v1|locator={}|text={}",
        locator.stable_identity(),
        chunk.full_text,
    ));
    Ok(LearningEvidenceChunk {
        chunk_index: chunk.chunk_index,
        source_locator: locator,
        content_hash,
    })
}

fn build_topic_evidence(
    plan: &DocumentLearningPlan,
    chunks: &[SemanticChunk],
) -> Result<Vec<StudyGuideTopicEvidence>, CommandError> {
    plan.topics
        .iter()
        .map(|topic| {
            let mut selected = chunks
                .iter()
                .filter(|chunk| {
                    chunk.source_locator.first() <= topic.source_locator.last()
                        && topic.source_locator.first() <= chunk.source_locator.last()
                })
                .collect::<Vec<_>>();
            selected.sort_by_key(|chunk| chunk.chunk_index);
            if selected.is_empty() {
                return Err(CommandError::new(
                    "document_learning_topic_without_evidence",
                    format!(
                        "Learning topic '{}' no longer overlaps canonical semantic evidence.",
                        topic.title
                    ),
                ));
            }
            let source_text = selected
                .into_iter()
                .map(|chunk| chunk.full_text.as_str())
                .collect::<Vec<_>>()
                .join("\n\n");
            Ok(StudyGuideTopicEvidence {
                topic_id: topic.id.clone(),
                source_text,
                // Visual evidence selection is added by the shared canonical
                // asset selector before review readiness. Keeping this field in
                // the contract prevents text-only orchestration from becoming a
                // second architecture.
                images: Vec::new(),
            })
        })
        .collect()
}

fn hash_text(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn study_guide_input_rejects_unknown_fields() {
        let value = serde_json::json!({
            "documentId": "a".repeat(32),
            "unexpected": true
        });
        assert!(serde_json::from_value::<GenerateDocumentStudyGuideInput>(value).is_err());
    }
}
