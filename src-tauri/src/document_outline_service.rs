//! Shared semantic analysis precedes every document learning artifact. Outlines
//! and display metadata are published atomically against all analyzed sources,
//! including explicit empty and partial dispositions.
use crate::{
    ai::{
        document_intelligence::{
            self, DocumentOutline, DocumentTaskContext, OUTLINE_PROMPT_VERSION,
            OUTLINE_SCHEMA_VERSION,
        },
        AiRuntimeState,
    },
    database::Database,
    document_domain::DocumentSourceDescriptor,
    document_scale::DocumentScalePlan,
    document_semantics::{
        build_partitioned_semantic_windows, validate_semantic_windows, SemanticChunk,
    },
    error::CommandError,
    knowledge::{authoring, document_view, snapshot::DocumentSnapshot},
    paths::AppPaths,
    task_cancellation::{current_signal, CancellationSignal},
    util::unix_time_millis,
};
use rusqlite::{params, TransactionBehavior};
#[path = "document_outline_service/artifact_metadata.rs"]
mod artifact_metadata;

pub(crate) struct EnsureDocumentOutlineInput<'a> {
    pub database: &'a Database,
    pub paths: &'a AppPaths,
    pub runtime: &'a AiRuntimeState,
    pub task: &'a DocumentTaskContext,
    pub document_id: &'a str,
    pub extraction_backend: &'a str,
    pub source: &'a DocumentSourceDescriptor,
    pub chunks: &'a [SemanticChunk],
    pub plan: DocumentScalePlan,
}

pub(crate) async fn ensure_document_outline(
    input: EnsureDocumentOutlineInput<'_>,
) -> Result<DocumentOutline, CommandError> {
    input.source.validate()?;
    let cancellation = current_signal().unwrap_or_else(CancellationSignal::new);
    let windows = build_partitioned_semantic_windows(
        input.chunks,
        input.plan,
        input.plan.outline_window_max_tokens,
        input.plan.outline_window_max_units,
        input.plan.outline_window_overlap_tokens,
    );
    validate_semantic_windows(&windows, input.source)?;
    update_outline_projection(
        input.database,
        input.document_id,
        "building",
        windows.len(),
        0,
    )?;
    let generated = async {
        let indices = input.chunks.iter().map(|chunk| chunk.chunk_index).collect::<Vec<_>>();
        authoring::prepare_document(authoring::DocumentPreparation {
            database: input.database, paths: input.paths, runtime: input.runtime,
            task: input.task, document_id: input.document_id, chunk_indices: &indices, retry_partial: true,
        }).await?;
        let snapshot = input.database.with_connection(|connection| DocumentSnapshot::capture(connection, input.document_id))?;
        let catalogs = windows.iter().map(|window| {
            authoring::catalog_for_chunks(input.database, input.document_id,
                &(window.first_chunk_index..=window.last_chunk_index).collect::<Vec<_>>(), input.task)
        }).collect::<Result<Vec<_>, _>>()?;
        let outline = document_view::ensure(document_view::ViewInput {
            database: input.database, runtime: input.runtime, task: input.task,
            document_id: input.document_id, windows: &windows, catalogs: &catalogs,
            snapshot: &snapshot,
        }).await?;
        input.task.ensure_current(input.database)?;
        input.database.with_connection(|connection| snapshot.ensure_current(connection))?;
        let metadata = match document_intelligence::generate_document_artifact_metadata(input.runtime, input.task, &outline).await {
            Ok(value) => Some(value),
            Err(error) if error.code == "ai_request_canceled" => return Err(error),
            Err(error) => {
                crate::logging::warn_target("ai.document.artifact_metadata", "document_artifact_metadata_native_fallback",
                    "Using native metadata from the shared learning view.", serde_json::json!({"code":error.code}));
                None
            }
        };
        input.task.ensure_current(input.database)?;
        crate::knowledge::runtime::check_canceled(&cancellation)?;
        let now = unix_time_millis()?;
        let outline_json = serde_json::to_string(&outline).map_err(CommandError::serialization)?;
        input.database.with_connection(|connection| {
            let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(CommandError::database)?;
            snapshot.ensure_current(&transaction)?;
            crate::knowledge::runtime::check_canceled(&cancellation)?;
            transaction.execute(
                "INSERT INTO document_outlines (
                    document_id, schema_version, prompt_version, profile_id, provider_kind,
                    model_id, profile_revision, credential_epoch, extraction_backend,
                    outline_json, summary_text, created_at, updated_at
                 ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?12)
                 ON CONFLICT(document_id) DO UPDATE SET schema_version=excluded.schema_version,
                    prompt_version=excluded.prompt_version, profile_id=excluded.profile_id,
                    provider_kind=excluded.provider_kind, model_id=excluded.model_id,
                    profile_revision=excluded.profile_revision, credential_epoch=excluded.credential_epoch,
                    extraction_backend=excluded.extraction_backend, outline_json=excluded.outline_json,
                    summary_text=excluded.summary_text, updated_at=excluded.updated_at",
                params![input.document_id, OUTLINE_SCHEMA_VERSION, OUTLINE_PROMPT_VERSION, input.task.profile_id,
                    input.task.provider_kind, input.task.model_id, input.task.profile_revision,
                    input.task.credential_epoch, input.extraction_backend, outline_json, outline.summary, now],
            ).map_err(CommandError::database)?;
            artifact_metadata::ensure_document_artifact_metadata(&transaction, input.document_id, &outline, metadata.as_ref(), now)?;
            transaction.execute(
                "UPDATE documents SET outline_state='ready', outline_window_count=?1,
                    outline_completed_window_count=?1, updated_at=?2, revision=revision+1 WHERE id=?3",
                params![to_i64(windows.len()), now, input.document_id],
            ).map_err(CommandError::database)?;
            transaction.commit().map_err(CommandError::database)
        })?;
        Ok(outline)
    }.await;
    if generated.is_err() {
        let _ = update_outline_projection(
            input.database,
            input.document_id,
            "failed",
            windows.len(),
            0,
        );
    }
    generated
}

fn update_outline_projection(
    database: &Database,
    document_id: &str,
    state: &str,
    total: usize,
    completed: usize,
) -> Result<(), CommandError> {
    if !matches!(state, "building" | "ready" | "failed") {
        return Err(CommandError::new(
            "invalid_document_outline_state",
            "The document outline state transition was invalid.",
        ));
    }
    database.with_connection(|connection| {
        let changed = connection
            .execute(
                "UPDATE documents SET outline_state=?1, outline_window_count=?2,
                outline_completed_window_count=?3, updated_at=?4, revision=revision+1 WHERE id=?5",
                params![
                    state,
                    to_i64(total),
                    to_i64(completed.min(total)),
                    unix_time_millis()?,
                    document_id
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
