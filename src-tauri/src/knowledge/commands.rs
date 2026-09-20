//! Narrow IPC operations for explicit semantic preparation and source retrieval.
//! The UI never receives vectors, credentials, raw provider envelopes or a SQL
//! interface.

use serde::Deserialize;
use tauri::{AppHandle, Manager, State};

use crate::{
    ai::{document_intelligence, AiRuntimeState},
    database::Database,
    error::CommandError,
    paths::AppPaths,
    util::validate_entity_id,
};

use super::{
    adapters,
    collection_view::{self, CollectionViewState},
    contracts::{ScopeChoice, SourceCitation},
    preparation::{self, PreparationPreview, PreparationScope, PrepareMaterialInput},
    preparation_store::{self, PreparationStatus},
    retrieval::{self, RetrievalPreview},
    runtime::{self, TaskGuard, TaskProgress},
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SemanticSearchInput {
    pub operation_id: String,
    pub selection: PreparationScope,
    pub query: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CitationInput {
    pub source_id: String,
    pub content_hash: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PrepareCollectionSynthesisInput {
    pub operation_id: String,
    pub collection_id: String,
}

#[tauri::command]
pub fn list_semantic_scope_choices(
    database: State<'_, Database>,
) -> Result<Vec<ScopeChoice>, CommandError> {
    adapters::list_scope_choices(database.inner())
}

#[tauri::command]
pub async fn preview_semantic_preparation(
    app: AppHandle,
    input: PreparationScope,
) -> Result<PreparationPreview, CommandError> {
    tauri::async_runtime::spawn_blocking(move || {
        preparation::preview(app.state::<Database>().inner(), input)
    })
    .await
    .map_err(|_| {
        CommandError::new(
            "semantic_preview_interrupted",
            "The material preview was interrupted before any provider request was made.",
        )
    })?
}

#[tauri::command]
pub async fn prepare_semantic_material(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    runtime: State<'_, AiRuntimeState>,
    input: PrepareMaterialInput,
) -> Result<PreparationStatus, CommandError> {
    preparation::prepare(database.inner(), paths.inner(), runtime.inner(), input).await
}

#[tauri::command]
pub fn get_semantic_preparation(
    database: State<'_, Database>,
    input: PreparationScope,
) -> Result<Option<PreparationStatus>, CommandError> {
    preparation_store::get_status(database.inner(), &input.key()?)
}

#[tauri::command]
pub fn get_learning_task_progress(
    operation_id: String,
) -> Result<Option<TaskProgress>, CommandError> {
    validate_entity_id(&operation_id, "learning operation")?;
    runtime::progress(&operation_id)
}

#[tauri::command]
pub fn cancel_learning_task(operation_id: String) -> Result<(), CommandError> {
    validate_entity_id(&operation_id, "learning operation")?;
    runtime::cancel(&operation_id)
}

#[tauri::command]
pub async fn search_semantic_material(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    runtime: State<'_, AiRuntimeState>,
    input: SemanticSearchInput,
) -> Result<RetrievalPreview, CommandError> {
    validate_entity_id(&input.operation_id, "search operation")?;
    let guard = TaskGuard::begin(
        &input.operation_id,
        &format!("search:{}", input.operation_id),
        false,
    )?;
    let result = retrieval::retrieve(
        database.inner(),
        paths.inner(),
        runtime.inner(),
        &input.selection.scope,
        input.selection.include_conversations,
        &input.query,
        &guard.signal,
    )
    .await?;
    Ok(RetrievalPreview {
        sources: result
            .hits
            .iter()
            .take(12)
            .map(|hit| SourceCitation::from(&hit.source))
            .collect(),
        eligible_entries: result.eligible_entries,
        embedding_space: result.space_id,
    })
}

#[tauri::command]
pub fn get_semantic_source_citation(
    database: State<'_, Database>,
    input: CitationInput,
) -> Result<SourceCitation, CommandError> {
    retrieval::resolve_citation(database.inner(), &input.source_id, &input.content_hash)
}

#[tauri::command]
pub fn clear_semantic_index(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
) -> Result<(), CommandError> {
    preparation::clear_index(database.inner(), paths.inner())
}

#[tauri::command]
pub fn get_collection_synthesis(
    database: State<'_, Database>,
    collection_id: String,
) -> Result<Option<CollectionViewState>, CommandError> {
    collection_view::load(database.inner(), &collection_id)
}

#[tauri::command]
pub async fn refresh_collection_synthesis(
    app: AppHandle,
    collection_id: String,
) -> Result<CollectionViewState, CommandError> {
    validate_entity_id(&collection_id, "document collection")?;
    tauri::async_runtime::spawn_blocking(move || {
        collection_view::refresh_native(app.state::<Database>().inner(), &collection_id)
    })
    .await
    .map_err(|_| {
        CommandError::new(
            "collection_synthesis_interrupted",
            "The local collection synthesis was interrupted before publication.",
        )
    })?
}

/// Explicit provider-authorized preparation. Browsing/get/refresh remain local.
#[tauri::command]
pub async fn prepare_collection_synthesis(
    database: State<'_, Database>,
    runtime: State<'_, AiRuntimeState>,
    input: PrepareCollectionSynthesisInput,
) -> Result<CollectionViewState, CommandError> {
    validate_entity_id(&input.operation_id, "collection synthesis operation")?;
    validate_entity_id(&input.collection_id, "document collection")?;
    let guard = TaskGuard::begin(
        &input.operation_id,
        &format!("collection-synthesis:{}", input.collection_id),
        false,
    )?;
    guard.progress("organizing collection", 0, 1);
    let task = document_intelligence::load_document_task_context(database.inner())?;
    let state = collection_view::ensure_reduced(
        database.inner(),
        runtime.inner(),
        &task,
        &input.collection_id,
        &guard.signal,
    )
    .await?;
    guard.progress("ready", 1, 1);
    Ok(state)
}
