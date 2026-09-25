//! Shared semantic analysis precedes every document learning artifact. The
//! grounded outline and deterministic display metadata publish atomically;
//! optional provider metadata and semantic indexing run after the artifact path.
use crate::{
    ai::{
        document_intelligence::{
            self, DocumentOutline, DocumentTaskContext, ARTIFACT_METADATA_GENERATOR_VERSION,
            ARTIFACT_METADATA_PROMPT_VERSION, ARTIFACT_METADATA_SCHEMA_VERSION,
            OUTLINE_PROMPT_VERSION, OUTLINE_SCHEMA_VERSION,
        },
        document_provider, generation, AiRuntimeState,
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
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use tauri::{AppHandle, Manager};
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
        let extraction_contract = crate::knowledge::extraction::extraction_contract(input.task)?;
        let snapshot = input.database.with_connection(|connection| DocumentSnapshot::prepared(
            connection, input.document_id, input.source.unit_count, input.chunks, &extraction_contract,
        ))?;
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
            artifact_metadata::ensure_document_artifact_metadata(
                &transaction,
                input.document_id,
                &outline,
                None,
                now,
            )?;
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

#[derive(Clone, Debug)]
struct MetadataEnrichmentSnapshot {
    source_fingerprint_sha256: String,
    outline_json: String,
    profile_id: Option<String>,
    provider_kind: String,
    model_id: String,
    profile_revision: i64,
    credential_epoch: i64,
    metadata_origin: Option<String>,
    metadata_source_fingerprint_sha256: Option<String>,
    metadata_generator_version: Option<i64>,
    metadata_schema_version: Option<i64>,
    metadata_prompt_version: Option<i64>,
}

impl MetadataEnrichmentSnapshot {
    fn has_current_provider_metadata(&self) -> bool {
        self.metadata_origin.as_deref() == Some("provider")
            && self.metadata_source_fingerprint_sha256.as_deref()
                == Some(self.source_fingerprint_sha256.as_str())
            && self.metadata_generator_version == Some(ARTIFACT_METADATA_GENERATOR_VERSION)
            && self.metadata_schema_version == Some(ARTIFACT_METADATA_SCHEMA_VERSION)
            && self.metadata_prompt_version == Some(ARTIFACT_METADATA_PROMPT_VERSION)
    }
}

pub(crate) fn schedule_document_postprocessing(app: AppHandle, document_id: String) {
    tauri::async_runtime::spawn(async move {
        let database = app.state::<Database>();
        let paths = app.state::<AppPaths>();
        let runtime = app.state::<AiRuntimeState>();
        let mut operation = match generation::Operation::begin(
            database.inner(),
            None,
            Some(&document_id),
            None,
            "document_postprocessing",
        ) {
            Ok(operation) => operation,
            Err(error) => {
                crate::logging::debug_target(
                    "ai.document.postprocessing",
                    "document_postprocessing_not_started",
                    "Optional document post-processing did not start; saved learning material remains usable.",
                    serde_json::json!({"document":document_id,"code":error.code}),
                );
                return;
            }
        };
        let diagnostics = operation.context();
        let signal = CancellationSignal::new();
        let metadata_result = enrich_document_artifact_metadata(
            database.inner(),
            runtime.inner(),
            &document_id,
            Some(diagnostics.clone()),
            &signal,
        )
        .await;
        if let Err(error) = &metadata_result {
            crate::logging::warn_target(
                "ai.document.artifact_metadata",
                "document_artifact_metadata_deferred",
                "Deterministic metadata remains active; optional provider metadata can be refreshed later.",
                serde_json::json!({"document":document_id,"code":error.code}),
            );
        }

        let indexing_result = crate::knowledge::indexing::index_document(
            database.inner(),
            paths.inner(),
            runtime.inner(),
            &document_id,
            &signal,
            Some(diagnostics),
        )
        .await;
        if let Err(error) = &indexing_result {
            crate::logging::warn_target(
                "ai.knowledge.index",
                "knowledge_index_deferred",
                "Knowledge is saved. Search indexing will catch up on use without repeating interpretation.",
                serde_json::json!({"document":document_id,"code":error.code}),
            );
        }
        let postprocessing_result = metadata_result.and(indexing_result);
        operation.finish_result(&postprocessing_result);
    });
}

async fn enrich_document_artifact_metadata(
    database: &Database,
    runtime: &AiRuntimeState,
    document_id: &str,
    diagnostics: Option<generation::Context>,
    cancellation: &CancellationSignal,
) -> Result<(), CommandError> {
    let snapshot = load_metadata_enrichment_snapshot(database, document_id)?;
    let outline: DocumentOutline =
        serde_json::from_str(&snapshot.outline_json).map_err(CommandError::serialization)?;

    // Always reapply already-stored desired metadata locally. This is important
    // after initial card publication, when the generated deck first exists.
    database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        ensure_metadata_snapshot_current(&transaction, document_id, &snapshot)?;
        artifact_metadata::ensure_document_artifact_metadata(
            &transaction,
            document_id,
            &outline,
            None,
            unix_time_millis()?,
        )?;
        transaction.commit().map_err(CommandError::database)
    })?;
    if snapshot.has_current_provider_metadata() {
        if let Some(context) = diagnostics.as_ref() {
            context.reused("artifact_metadata", &snapshot.source_fingerprint_sha256);
        }
        return Ok(());
    }

    let Some(profile_id) = snapshot.profile_id.as_deref() else {
        return Ok(());
    };
    let mut task = DocumentTaskContext::for_profile(database, profile_id)?;
    if task.provider_kind != snapshot.provider_kind
        || task.model_id != snapshot.model_id
        || task.profile_revision != snapshot.profile_revision
        || task.credential_epoch != snapshot.credential_epoch
    {
        return Err(CommandError::new(
            "ai_profile_changed",
            "The document provider changed before optional display metadata could be refreshed.",
        ));
    }
    let capabilities = document_provider::load_document_provider_capabilities(
        database,
        &task.profile_id,
        &task.provider_kind,
        &task.model_id,
    )?;
    task.apply_provider_capabilities(&capabilities);
    task.diagnostics = diagnostics;
    let candidate = document_intelligence::generate_document_artifact_metadata(
        runtime,
        &task,
        &outline,
        cancellation,
    )
    .await?;
    task.ensure_current(database)?;

    database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        ensure_metadata_snapshot_current(&transaction, document_id, &snapshot)?;
        artifact_metadata::ensure_document_artifact_metadata(
            &transaction,
            document_id,
            &outline,
            Some(&candidate),
            unix_time_millis()?,
        )?;
        transaction.commit().map_err(CommandError::database)
    })
}

fn load_metadata_enrichment_snapshot(
    database: &Database,
    document_id: &str,
) -> Result<MetadataEnrichmentSnapshot, CommandError> {
    database.with_connection(|connection| {
        connection
            .query_row(
                "SELECT document.sha256, outline.outline_json, outline.profile_id,
                        outline.provider_kind, outline.model_id, outline.profile_revision,
                        outline.credential_epoch, metadata.metadata_origin,
                        metadata.source_fingerprint_sha256, metadata.generator_version,
                        metadata.schema_version, metadata.prompt_version
                 FROM documents AS document
                 JOIN document_outlines AS outline ON outline.document_id = document.id
                 LEFT JOIN document_artifact_metadata AS metadata
                   ON metadata.document_id = document.id
                 WHERE document.id = ?1",
                [document_id],
                |row| {
                    Ok(MetadataEnrichmentSnapshot {
                        source_fingerprint_sha256: row.get(0)?,
                        outline_json: row.get(1)?,
                        profile_id: row.get(2)?,
                        provider_kind: row.get(3)?,
                        model_id: row.get(4)?,
                        profile_revision: row.get(5)?,
                        credential_epoch: row.get(6)?,
                        metadata_origin: row.get(7)?,
                        metadata_source_fingerprint_sha256: row.get(8)?,
                        metadata_generator_version: row.get(9)?,
                        metadata_schema_version: row.get(10)?,
                        metadata_prompt_version: row.get(11)?,
                    })
                },
            )
            .optional()
            .map_err(CommandError::database)?
            .ok_or_else(|| {
                CommandError::new(
                    "document_outline_missing",
                    "The saved document outline is no longer available for post-processing.",
                )
            })
    })
}

fn ensure_metadata_snapshot_current(
    transaction: &rusqlite::Transaction<'_>,
    document_id: &str,
    snapshot: &MetadataEnrichmentSnapshot,
) -> Result<(), CommandError> {
    let current = transaction
        .query_row(
            "SELECT document.sha256, outline.outline_json, outline.profile_id,
                    outline.provider_kind, outline.model_id, outline.profile_revision,
                    outline.credential_epoch
             FROM documents AS document
             JOIN document_outlines AS outline ON outline.document_id = document.id
             WHERE document.id = ?1",
            [document_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                ))
            },
        )
        .optional()
        .map_err(CommandError::database)?;
    let Some(current) = current else {
        return Err(CommandError::new(
            "document_outline_missing",
            "The saved document outline is no longer available for post-processing.",
        ));
    };
    if current.0 != snapshot.source_fingerprint_sha256
        || current.1 != snapshot.outline_json
        || current.2 != snapshot.profile_id
        || current.3 != snapshot.provider_kind
        || current.4 != snapshot.model_id
        || current.5 != snapshot.profile_revision
        || current.6 != snapshot.credential_epoch
    {
        return Err(CommandError::new(
            "document_outline_changed",
            "The document outline changed before optional metadata could be applied.",
        ));
    }
    Ok(())
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

#[cfg(test)]
mod postprocessing_tests {
    use super::*;

    fn snapshot() -> MetadataEnrichmentSnapshot {
        MetadataEnrichmentSnapshot {
            source_fingerprint_sha256: "a".repeat(64),
            outline_json: "{}".to_string(),
            profile_id: Some("b".repeat(32)),
            provider_kind: "openrouter".to_string(),
            model_id: "model".to_string(),
            profile_revision: 1,
            credential_epoch: 0,
            metadata_origin: Some("provider".to_string()),
            metadata_source_fingerprint_sha256: Some("a".repeat(64)),
            metadata_generator_version: Some(ARTIFACT_METADATA_GENERATOR_VERSION),
            metadata_schema_version: Some(ARTIFACT_METADATA_SCHEMA_VERSION),
            metadata_prompt_version: Some(ARTIFACT_METADATA_PROMPT_VERSION),
        }
    }

    #[test]
    fn provider_metadata_reuse_requires_exact_source_and_contract() {
        let current = snapshot();
        assert!(current.has_current_provider_metadata());

        let mut changed_source = current.clone();
        changed_source.metadata_source_fingerprint_sha256 = Some("c".repeat(64));
        assert!(!changed_source.has_current_provider_metadata());

        let mut changed_prompt = current;
        changed_prompt.metadata_prompt_version =
            Some(ARTIFACT_METADATA_PROMPT_VERSION.saturating_add(1));
        assert!(!changed_prompt.has_current_provider_metadata());
    }
}
