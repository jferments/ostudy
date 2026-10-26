//! Headless study-guide evaluation runner (cargo feature `guide-eval`).
//!
//! Builds an isolated library, configures one provider through the same
//! profile, credential, probe and task-default services Settings uses, imports
//! one source file through the document staging and import path, and creates
//! its study guide with the reader's own generation path. The caller receives
//! the published guide, the learning plan and the canonical source chunks so a
//! separate scorer can judge the guide against its source. Nothing here is
//! registered as a Tauri command or compiled into the application.

use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::{json, Value};

use crate::{
    ai::{
        probes::{self, AiProfileReferenceInput},
        profiles::{
            self, CreateAiProviderProfileInput, SetAiProviderCredentialInput, SetAiTaskDefaultInput,
        },
        AiRuntimeState,
    },
    database::Database,
    document_import::import_staged_document,
    document_initial_generation::{
        ensure_document_derivatives, process_and_generate_initial_flashcards, FigureSearch,
    },
    document_learning::load_active_document_learning_plan,
    document_store,
    document_study_guide_commands::generate_document_study_guide_for_document,
    error::CommandError,
    external_file_streaming::stage_local_import,
    external_files::ExternalImportPurpose,
    knowledge::assessment::policy::instructions::GenerationInstructions,
    paths::AppPaths,
    pdf_render_backend::PdfRenderRuntime,
    task_cancellation::run_document_task,
    util::generate_id,
};

/// One provider connection, as a learner would enter it in Settings.
#[derive(Clone, Debug)]
pub struct GuideEvalProvider {
    /// `llama_cpp`, `deepseek` or `openrouter`.
    pub kind: String,
    pub base_url: String,
    pub model_id: String,
    pub reasoning_effort: String,
    /// Kept for this process only (session persistence); never written to disk.
    pub credential: Option<String>,
    pub allow_insecure_http: bool,
    pub request_timeout_ms: i64,
}

#[derive(Clone, Debug)]
pub struct GuideEvalRequest {
    pub source: PathBuf,
    /// An empty or new directory that becomes the isolated library.
    pub library_dir: PathBuf,
    pub provider: GuideEvalProvider,
    /// Where the provisioned PDFium library lives (`OSTUDY_ROOT_DIR` otherwise).
    pub resource_dir: Option<PathBuf>,
    /// Run the document's whole first generation (flashcards and study guide
    /// together, as the Documents page does) instead of the guide alone.
    pub cards: bool,
    /// Where to write the guide laid out as the reader shows it, in Sage, once
    /// the PDF's figure search is done; none skips the layout.
    pub pdf: Option<PathBuf>,
}

/// What a run produced. `guide` is `None` when generation failed; `error` then
/// says why, and the stage that failed.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuideEvalOutcome {
    pub document_id: Option<String>,
    pub probe: Option<Value>,
    pub source: Option<Value>,
    pub learning_plan: Option<Value>,
    pub guide: Option<Value>,
    /// Saved deck counts when `cards` was requested.
    pub cards: Option<Value>,
    pub error: Option<GuideEvalError>,
    pub timings_ms: GuideEvalTimings,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuideEvalError {
    pub stage: &'static str,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuideEvalTimings {
    pub configure: u128,
    pub ingest: u128,
    pub generate: u128,
}

pub fn run(request: GuideEvalRequest) -> Result<GuideEvalOutcome, String> {
    std::fs::create_dir_all(&request.library_dir).map_err(|error| error.to_string())?;
    let paths = AppPaths::new(request.library_dir.clone()).map_err(|error| error.to_string())?;
    crate::logging::initialize(paths.app_data_dir());
    let database = Database::open(paths.database_path())?;
    let runtime =
        AiRuntimeState::new(std::sync::Arc::new(crate::platform::UnavailableSecretStore))?;
    let render_runtime =
        PdfRenderRuntime::new(request.resource_dir.clone()).map_err(|error| error.to_string())?;
    Ok(tauri::async_runtime::block_on(evaluate(
        &database,
        &paths,
        &runtime,
        render_runtime,
        &request,
    )))
}

async fn evaluate(
    database: &Database,
    paths: &AppPaths,
    runtime: &AiRuntimeState,
    render_runtime: PdfRenderRuntime,
    request: &GuideEvalRequest,
) -> GuideEvalOutcome {
    let mut outcome = GuideEvalOutcome {
        document_id: None,
        probe: None,
        source: None,
        learning_plan: None,
        guide: None,
        cards: None,
        error: None,
        timings_ms: GuideEvalTimings::default(),
    };
    let fail = |stage: &'static str, error: CommandError| GuideEvalError {
        stage,
        code: error.code,
        message: error.message,
    };

    let started = std::time::Instant::now();
    match configure_provider(database, runtime, &request.provider).await {
        Ok(probe) => outcome.probe = Some(probe),
        Err(error) => {
            outcome.error = Some(fail("configure", error));
            return outcome;
        }
    }
    outcome.timings_ms.configure = started.elapsed().as_millis();

    let started = std::time::Instant::now();
    let (document_id, figure_search) =
        match import_and_ingest(database, paths, render_runtime.clone(), &request.source).await {
            Ok(imported) => imported,
            Err(error) => {
                outcome.error = Some(fail("ingest", error));
                return outcome;
            }
        };
    outcome.timings_ms.ingest = started.elapsed().as_millis();
    let render_runtime_for_cards = render_runtime.clone();
    outcome.document_id = Some(document_id.clone());
    match source_snapshot(database, &document_id) {
        Ok(source) => outcome.source = Some(source),
        Err(error) => {
            outcome.error = Some(fail("ingest", error));
            return outcome;
        }
    }

    let started = std::time::Instant::now();
    if request.cards {
        let instructions = GenerationInstructions::default();
        let generated = run_document_task(
            &document_id,
            process_and_generate_initial_flashcards(
                database,
                paths,
                runtime,
                render_runtime_for_cards,
                &document_id,
                &instructions,
                None,
            ),
        )
        .await;
        outcome.timings_ms.generate = started.elapsed().as_millis();
        match generated {
            Ok(result) => {
                outcome.cards = Some(json!({
                    "notes": result.created_note_count,
                    "cards": result.created_card_count,
                    "skippedDuplicates": result.skipped_duplicate_count,
                }))
            }
            Err(error) => outcome.error = Some(fail("generate", error)),
        }
        return outcome;
    }
    let generated = run_document_task(
        &document_id,
        generate_document_study_guide_for_document(database, paths, runtime, &document_id),
    )
    .await;
    outcome.timings_ms.generate = started.elapsed().as_millis();
    if let Ok(Some(plan)) = load_active_document_learning_plan(database, &document_id) {
        outcome.learning_plan = serde_json::to_value(plan).ok();
    }
    match generated
        .and_then(|guide| serde_json::to_value(guide).map_err(CommandError::serialization))
    {
        Ok(guide) => outcome.guide = Some(guide),
        Err(error) => {
            outcome.error = Some(fail("generate", error));
            return outcome;
        }
    }
    if let Some(destination) = &request.pdf {
        // The reader lays a guide out after its figures are found.
        if let Some(search) = figure_search {
            let _ = search.await;
        }
        if let Err(error) = crate::document_study_guide_export::render_document_guide_pdf(
            database,
            paths,
            &render_runtime,
            &document_id,
            destination,
        ) {
            outcome.error = Some(fail("layout", error));
        }
    }
    outcome
}

async fn configure_provider(
    database: &Database,
    runtime: &AiRuntimeState,
    provider: &GuideEvalProvider,
) -> Result<Value, CommandError> {
    let operation = || database.with_connection(|connection| generate_id(connection));
    let auth_mode = if provider.credential.is_some() {
        "bearer"
    } else {
        "none"
    };
    let created = profiles::create_profile(
        database,
        runtime,
        input::<CreateAiProviderProfileInput>(json!({
            "operationId": operation()?,
            "displayName": "Guide evaluation",
            "providerKind": provider.kind,
            "baseUrl": provider.base_url,
            "authMode": auth_mode,
            "enabled": true,
            "allowInsecureHttp": provider.allow_insecure_http,
            "requestTimeoutMs": provider.request_timeout_ms,
        }))?,
    )?;
    let mut revision = created.revision;
    if let Some(credential) = &provider.credential {
        revision = profiles::set_credential(
            database,
            runtime,
            input::<SetAiProviderCredentialInput>(json!({
                "operationId": operation()?,
                "profileId": created.id,
                "revision": revision,
                "credential": credential,
                "persistence": "session",
            }))?,
        )?
        .revision;
    }
    profiles::set_task_default(
        database,
        runtime,
        input::<SetAiTaskDefaultInput>(json!({
            "operationId": operation()?,
            "taskKind": "flashcard_generation",
            "profileId": created.id,
            "modelId": provider.model_id,
            "reasoningEffort": provider.reasoning_effort,
        }))?,
    )?;
    let probe = probes::test_profile(
        database,
        runtime,
        input::<AiProfileReferenceInput>(json!({
            "profileId": created.id,
            "revision": revision,
            "modelId": provider.model_id,
            "reasoningEffort": provider.reasoning_effort,
        }))?,
    )
    .await?;
    serde_json::to_value(probe).map_err(CommandError::serialization)
}

fn input<T: serde::de::DeserializeOwned>(value: Value) -> Result<T, CommandError> {
    serde_json::from_value(value).map_err(|error| {
        CommandError::new(
            "guide_eval_invalid_input",
            format!("The evaluation runner built an invalid settings input: {error}"),
        )
    })
}

async fn import_and_ingest(
    database: &Database,
    paths: &AppPaths,
    render_runtime: PdfRenderRuntime,
    source: &Path,
) -> Result<(String, Option<FigureSearch>), CommandError> {
    let operation_id = database.with_connection(|connection| generate_id(connection))?;
    let staged = stage_local_import(
        database,
        paths,
        operation_id,
        ExternalImportPurpose::Document,
        source,
    )?;
    let imported = import_staged_document(database, paths, &staged.id, None)?;
    let (_, figure_search) =
        ensure_document_derivatives(database, paths, render_runtime, &imported.document_id).await?;
    // Generation reads text and page renders; it does not wait for figures.
    Ok((imported.document_id, figure_search))
}

/// Lays out again the guide of the one document in an earlier run's library,
/// so a layout change can be seen without generating the guide again.
pub fn relayout(
    library_dir: PathBuf,
    resource_dir: Option<PathBuf>,
    destination: &Path,
) -> Result<(), String> {
    let paths = AppPaths::new(library_dir).map_err(|error| error.to_string())?;
    let database = Database::open(paths.database_path())?;
    let render_runtime = PdfRenderRuntime::new(resource_dir).map_err(|error| error.to_string())?;
    let document_id = database
        .with_connection(|connection| {
            connection
                .query_row(
                    "SELECT id FROM documents ORDER BY created_at LIMIT 1",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .map_err(CommandError::database)
        })
        .map_err(|error| error.message)?;
    // The figures are found again, so a change to finding them shows too.
    let source = database
        .with_connection(|connection| {
            crate::document_annotations::pdf_source_path(connection, &document_id)
        })
        .map_err(|error| error.message)?;
    tauri::async_runtime::block_on(crate::document_figures::find_after_ingestion(
        &database,
        render_runtime.clone(),
        &document_id,
        paths.resolve_relative(&source),
    ));
    crate::document_study_guide_export::render_document_guide_pdf(
        &database,
        &paths,
        &render_runtime,
        &document_id,
        destination,
    )
    .map_err(|error| error.message)
}

/// Canonical chunks with their locators: the text a judge checks guide claims
/// against, in the same units the guide's own source locators name.
fn source_snapshot(database: &Database, document_id: &str) -> Result<Value, CommandError> {
    let chunks = document_store::load_semantic_chunks(database, document_id)?;
    Ok(json!({
        "chunks": chunks
            .iter()
            .map(|chunk| {
                json!({
                    "chunkIndex": chunk.chunk_index,
                    "sourceLocator": chunk.source_locator,
                    "headingContext": chunk.heading_context,
                    "elementTypes": chunk.element_types,
                    "tokenEstimate": chunk.token_estimate,
                    "text": chunk.full_text,
                })
            })
            .collect::<Vec<_>>(),
    }))
}
