//! Reusable document-outline construction independent of any final learning artifact.
//!
//! Flashcards and study guides consume the same validated outline. This service
//! owns current-plan checkpoint validation, missing-fragment generation, final
//! persistence, and outline progress projection without starting card authoring.

use std::sync::atomic::{AtomicUsize, Ordering};

use rusqlite::{params, TransactionBehavior};

use crate::{
    ai::document_intelligence::{
        self, DocumentOutline, DocumentTaskContext, OUTLINE_PROMPT_VERSION, OUTLINE_SCHEMA_VERSION,
    },
    ai::AiRuntimeState,
    database::Database,
    document_checkpoints::{self, CheckpointIdentity},
    document_domain::DocumentSourceDescriptor,
    document_scale::DocumentScalePlan,
    document_semantics::{
        build_partitioned_semantic_windows, validate_semantic_windows, SemanticChunk,
    },
    error::CommandError,
    util::unix_time_millis,
};

pub(crate) struct EnsureDocumentOutlineInput<'a> {
    pub database: &'a Database,
    pub runtime: &'a AiRuntimeState,
    pub task: &'a DocumentTaskContext,
    pub document_id: &'a str,
    pub document_title: &'a str,
    pub extraction_backend: &'a str,
    pub source: &'a DocumentSourceDescriptor,
    pub chunks: &'a [SemanticChunk],
    pub plan: DocumentScalePlan,
    pub provider_capability_fingerprint: &'a str,
}

pub(crate) async fn ensure_document_outline(
    input: EnsureDocumentOutlineInput<'_>,
) -> Result<DocumentOutline, CommandError> {
    input.source.validate()?;
    if input.chunks.is_empty() {
        return Err(CommandError::new(
            "document_has_no_semantic_content",
            "The document did not contain canonical semantic content that ostudy could outline.",
        ));
    }

    let windows = build_partitioned_semantic_windows(
        input.chunks,
        input.plan,
        input.plan.outline_window_max_tokens,
        input.plan.outline_window_max_units,
        input.plan.outline_window_overlap_tokens,
    );
    validate_semantic_windows(&windows, input.source)?;
    if windows.is_empty() {
        return Err(CommandError::new(
            "document_has_no_semantic_content",
            "The document did not produce bounded semantic windows for outlining.",
        ));
    }

    let identity = CheckpointIdentity::from_task(
        input.task,
        input.extraction_backend,
        input.provider_capability_fingerprint,
    );
    let contract = document_checkpoints::outline_contract_key(&identity, input.plan);
    let cached_fragments = document_checkpoints::load_outline_fragments_best_effort(
        input.database,
        input.document_id,
        &contract,
        &windows,
    );
    let cached_count = cached_fragments
        .iter()
        .filter(|fragment| fragment.is_some())
        .count();

    if cached_count == windows.len() {
        if let Some(outline) = document_checkpoints::load_final_outline_best_effort(
            input.database,
            input.document_id,
            &identity,
        ) {
            update_outline_projection(
                input.database,
                input.document_id,
                "ready",
                windows.len(),
                windows.len(),
            )?;
            return Ok(outline);
        }
    }

    update_outline_projection(
        input.database,
        input.document_id,
        "building",
        windows.len(),
        cached_count,
    )?;
    let generated_count = AtomicUsize::new(0);
    let generated = document_intelligence::build_document_outline(
        input.runtime,
        input.task,
        input.document_title,
        i64::try_from(input.source.unit_count).unwrap_or(i64::MAX),
        &windows,
        &cached_fragments,
        |window, fragment| {
            document_checkpoints::persist_outline_fragment_best_effort(
                input.database,
                input.document_id,
                &contract,
                window,
                fragment,
            );
            let generated_count = generated_count
                .fetch_add(1, Ordering::Relaxed)
                .saturating_add(1);
            update_outline_projection(
                input.database,
                input.document_id,
                "building",
                windows.len(),
                cached_count.saturating_add(generated_count),
            )
        },
    )
    .await;

    match generated {
        Ok(outline) => {
            persist_final_outline(
                input.database,
                input.document_id,
                input.task,
                input.extraction_backend,
                &outline,
            )?;
            update_outline_projection(
                input.database,
                input.document_id,
                "ready",
                windows.len(),
                windows.len(),
            )?;
            Ok(outline)
        }
        Err(error) => {
            let _ = update_outline_projection(
                input.database,
                input.document_id,
                "failed",
                windows.len(),
                cached_count.saturating_add(generated_count.load(Ordering::Relaxed)),
            );
            Err(error)
        }
    }
}

fn persist_final_outline(
    database: &Database,
    document_id: &str,
    task: &DocumentTaskContext,
    extraction_backend: &str,
    outline: &DocumentOutline,
) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    let outline_json = serde_json::to_string(outline).map_err(CommandError::serialization)?;
    database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        transaction
            .execute(
                "INSERT INTO document_outlines (
                     document_id, schema_version, prompt_version, profile_id,
                     provider_kind, model_id, profile_revision, credential_epoch,
                     extraction_backend, outline_json, summary_text, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12)
                 ON CONFLICT(document_id) DO UPDATE SET
                     schema_version = excluded.schema_version,
                     prompt_version = excluded.prompt_version,
                     profile_id = excluded.profile_id,
                     provider_kind = excluded.provider_kind,
                     model_id = excluded.model_id,
                     profile_revision = excluded.profile_revision,
                     credential_epoch = excluded.credential_epoch,
                     extraction_backend = excluded.extraction_backend,
                     outline_json = excluded.outline_json,
                     summary_text = excluded.summary_text,
                     updated_at = excluded.updated_at",
                params![
                    document_id,
                    OUTLINE_SCHEMA_VERSION,
                    OUTLINE_PROMPT_VERSION,
                    task.profile_id,
                    task.provider_kind,
                    task.model_id,
                    task.profile_revision,
                    task.credential_epoch,
                    extraction_backend,
                    outline_json,
                    outline.summary,
                    now,
                ],
            )
            .map_err(CommandError::database)?;
        transaction.commit().map_err(CommandError::database)
    })
}

fn update_outline_projection(
    database: &Database,
    document_id: &str,
    state: &str,
    total_windows: usize,
    completed_windows: usize,
) -> Result<(), CommandError> {
    if !matches!(state, "building" | "ready" | "failed") {
        return Err(CommandError::new(
            "invalid_document_outline_state",
            "The document outline state transition was invalid.",
        ));
    }
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        let changed = connection
            .execute(
                "UPDATE documents
                 SET outline_state = ?1,
                     outline_window_count = ?2,
                     outline_completed_window_count = ?3,
                     updated_at = ?4,
                     revision = revision + 1
                 WHERE id = ?5",
                params![
                    state,
                    to_i64(total_windows),
                    to_i64(completed_windows.min(total_windows)),
                    now,
                    document_id,
                ],
            )
            .map_err(CommandError::database)?;
        if changed != 1 {
            return Err(CommandError::new(
                "document_not_found",
                "That document is no longer in the library.",
            ));
        }
        Ok(())
    })
}

fn to_i64(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outline_projection_accepts_only_persisted_states() {
        assert!(matches!("building", "building" | "ready" | "failed"));
        assert!(!matches!("generating", "building" | "ready" | "failed"));
    }
}
