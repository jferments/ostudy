//! Task-scoped cancellation and live progress for long-running document AI work.
//!
//! Cancellation and high-frequency progress are intentionally process-local.
//! Durable document/job state remains in SQLite; this module binds one running
//! document task to an in-memory signal plus a bounded progress snapshot. Async
//! provider traffic uses the Tokio task-local context, while blocking
//! extraction/render/OCR workers can query the same registry by exact document
//! id. The application close guard also reads this registry so it can warn
//! before an in-flight document task is interrupted.

use std::{
    collections::HashMap,
    future::Future,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, LazyLock, Mutex,
    },
};

use serde::Serialize;
use tokio::sync::Notify;

use crate::{error::CommandError, util::unix_time_millis};

#[cfg(mobile)]
use tauri::AppHandle;
#[cfg(mobile)]
use tauri_plugin_ostudy_platform::{BeginBackgroundExecutionRequest, OstudyPlatformExt};

#[derive(Clone, Debug)]
pub(crate) struct CancellationSignal {
    canceled: Arc<AtomicBool>,
    notify: Arc<Notify>,
}

impl CancellationSignal {
    pub(crate) fn new() -> Self {
        Self {
            canceled: Arc::new(AtomicBool::new(false)),
            notify: Arc::new(Notify::new()),
        }
    }

    pub(crate) fn cancel(&self) {
        if !self.canceled.swap(true, Ordering::SeqCst) {
            self.notify.notify_waiters();
        }
    }

    pub(crate) fn is_canceled(&self) -> bool {
        self.canceled.load(Ordering::SeqCst)
    }

    pub(crate) async fn canceled(&self) {
        loop {
            let notified = self.notify.notified();
            if self.is_canceled() {
                return;
            }
            notified.await;
            if self.is_canceled() {
                return;
            }
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DocumentTaskProgress {
    pub started_at: i64,
    pub phase: String,
    pub phase_started_at: i64,
    pub current_window: Option<usize>,
    pub current_window_end: Option<usize>,
    pub completed_windows: usize,
    pub total_windows: usize,
    pub current_page_start: Option<i64>,
    pub current_page_end: Option<i64>,
    pub current_partition: Option<usize>,
    pub provider_kind: Option<String>,
    pub model_id: Option<String>,
    pub requested_reasoning_effort: Option<String>,
    pub reasoning_effort: String,
    pub current_request_started_at: Option<i64>,
    pub current_request_stage: Option<String>,
    pub current_request_bytes: Option<usize>,
    pub current_request_text_bytes: Option<usize>,
    pub current_request_image_count: Option<usize>,
    pub current_request_evidence_bytes: Option<usize>,
    pub current_request_max_output_tokens: Option<usize>,
    pub provider_request_count: u64,
    pub retry_count: u64,
    pub transient_retry_count: u64,
    pub compatibility_retry_count: u64,
    pub contract_repair_retry_count: u64,
    pub output_compaction_retry_count: u64,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub reasoning_tokens: u64,
    pub total_tokens: u64,
    pub cached_prompt_tokens: u64,
    pub cache_write_tokens: u64,
    pub usage_reported_requests: u64,
    pub usage_unreported_requests: u64,
    pub provider_activity: String,
    pub last_provider_activity_at: Option<i64>,
    pub last_model_activity_at: Option<i64>,
}

impl DocumentTaskProgress {
    fn new(started_at: i64) -> Self {
        Self {
            started_at,
            phase: "preparing".to_string(),
            phase_started_at: started_at,
            current_window: None,
            current_window_end: None,
            completed_windows: 0,
            total_windows: 0,
            current_page_start: None,
            current_page_end: None,
            current_partition: None,
            provider_kind: None,
            model_id: None,
            requested_reasoning_effort: None,
            reasoning_effort: "none".to_string(),
            current_request_started_at: None,
            current_request_stage: None,
            current_request_bytes: None,
            current_request_text_bytes: None,
            current_request_image_count: None,
            current_request_evidence_bytes: None,
            current_request_max_output_tokens: None,
            provider_request_count: 0,
            retry_count: 0,
            transient_retry_count: 0,
            compatibility_retry_count: 0,
            contract_repair_retry_count: 0,
            output_compaction_retry_count: 0,
            prompt_tokens: 0,
            completion_tokens: 0,
            reasoning_tokens: 0,
            total_tokens: 0,
            cached_prompt_tokens: 0,
            cache_write_tokens: 0,
            usage_reported_requests: 0,
            usage_unreported_requests: 0,
            provider_activity: "preparing".to_string(),
            last_provider_activity_at: None,
            last_model_activity_at: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct DocumentTaskUsageDelta {
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
    pub reasoning_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
    pub cached_prompt_tokens: Option<u64>,
    pub cache_write_tokens: Option<u64>,
}

#[derive(Clone, Debug)]
struct ActiveDocumentTask {
    document_id: String,
    diagnostics: Arc<Mutex<Option<crate::ai::generation::Operation>>>,
    signal: CancellationSignal,
    progress: Arc<Mutex<DocumentTaskProgress>>,
}

impl ActiveDocumentTask {
    fn new(document_id: &str, started_at: i64) -> Self {
        Self {
            document_id: document_id.to_string(),
            diagnostics: Arc::new(Mutex::new(None)),
            signal: CancellationSignal::new(),
            progress: Arc::new(Mutex::new(DocumentTaskProgress::new(started_at))),
        }
    }
}

#[derive(Default)]
struct ActiveDocumentTasks {
    tasks: Mutex<HashMap<String, ActiveDocumentTask>>,
}

impl ActiveDocumentTasks {
    fn begin(&self, document_id: &str) -> Result<ActiveDocumentTask, CommandError> {
        let mut tasks = self.tasks.lock().map_err(|_| registry_error())?;
        if tasks.contains_key(document_id) {
            return Err(CommandError::new(
                "document_generation_already_running",
                "Flashcard generation is already running for this PDF.",
            ));
        }
        let task = ActiveDocumentTask::new(document_id, unix_time_millis()?);
        tasks.insert(document_id.to_string(), task.clone());
        Ok(task)
    }

    fn finish(&self, document_id: &str) {
        if let Ok(mut tasks) = self.tasks.lock() {
            tasks.remove(document_id);
        }
    }

    fn cancel(&self, document_id: &str) -> Result<(), CommandError> {
        let tasks = self.tasks.lock().map_err(|_| registry_error())?;
        let task = tasks.get(document_id).ok_or_else(|| {
            CommandError::new(
                "document_generation_not_running",
                "There is no active flashcard-generation request for this PDF.",
            )
        })?;
        task.signal.cancel();
        Ok(())
    }

    fn is_active(&self, document_id: &str) -> bool {
        self.tasks
            .lock()
            .map(|tasks| tasks.contains_key(document_id))
            .unwrap_or(false)
    }

    fn has_any(&self) -> bool {
        self.tasks
            .lock()
            .map(|tasks| !tasks.is_empty())
            .unwrap_or(false)
    }

    fn count(&self) -> usize {
        self.tasks.lock().map(|tasks| tasks.len()).unwrap_or(1)
    }

    fn is_canceled(&self, document_id: &str) -> bool {
        self.tasks
            .lock()
            .map(|tasks| {
                tasks
                    .get(document_id)
                    .is_some_and(|task| task.signal.is_canceled())
            })
            .unwrap_or(false)
    }

    fn progress(&self, document_id: &str) -> Option<DocumentTaskProgress> {
        let tasks = self.tasks.lock().ok()?;
        let task = tasks.get(document_id)?;
        task.progress.lock().ok().map(|progress| progress.clone())
    }
}

static ACTIVE_DOCUMENT_TASKS: LazyLock<ActiveDocumentTasks> =
    LazyLock::new(ActiveDocumentTasks::default);

tokio::task_local! {
    static CURRENT_DOCUMENT_TASK: ActiveDocumentTask;
}

/// A registered document is retired on success, failure, early foreground-service
/// rejection, or future cancellation. Progress state must not remain stuck in
/// the process registry until an application restart.
struct DocumentRegistration {
    document_id: String,
    signal: CancellationSignal,
}

impl Drop for DocumentRegistration {
    fn drop(&mut self) {
        self.signal.cancel();
        ACTIVE_DOCUMENT_TASKS.finish(&self.document_id);
    }
}

/// A task reservation is created synchronously before a background command
/// acknowledges Start. Stop and duplicate-start therefore see the exact native
/// task identity even when the executor has not polled its future yet.
pub(crate) struct ReservedDocumentTask {
    task: ActiveDocumentTask,
    _registration: DocumentRegistration,
    _activity: crate::task_activity::ActivityGuard,
    _foreground_execution: MobileForegroundExecution,
}

pub(crate) fn reserve_document_task(
    document_id: &str,
) -> Result<ReservedDocumentTask, CommandError> {
    let activity = crate::task_activity::ActivityGuard::begin()?;
    let task = ACTIVE_DOCUMENT_TASKS.begin(document_id)?;
    let registration = DocumentRegistration {
        document_id: document_id.to_string(),
        signal: task.signal.clone(),
    };
    let foreground_execution = MobileForegroundExecution::begin(document_id)?;
    Ok(ReservedDocumentTask {
        task,
        _registration: registration,
        _activity: activity,
        _foreground_execution: foreground_execution,
    })
}

type BoxedDocumentFuture<'a, T> =
    std::pin::Pin<Box<dyn Future<Output = Result<T, CommandError>> + Send + 'a>>;

fn run_reserved_document_task_boxed<'a, T>(
    reservation: ReservedDocumentTask,
    future: BoxedDocumentFuture<'a, T>,
) -> BoxedDocumentFuture<'a, T>
where
    T: Send + 'a,
{
    Box::pin(async move {
        let task = reservation.task.clone();
        let result = CURRENT_DOCUMENT_TASK.scope(task.clone(), future).await;
        if let Ok(mut diagnostic) = task.diagnostics.lock() {
            if let Some(operation) = diagnostic.as_mut() {
                operation.finish_result(&result);
            }
        }
        drop(reservation);
        result
    })
}

/// Heap-box the caller's document future before it enters the task-local wrapper.
///
/// Document generation composes several large async state machines. Keeping those
/// state machines inline through the cancellation/Tauri wrapper chain can make
/// debug poll frames consume most of Tokio's worker-thread stack before the
/// document pipeline itself is polled. This boundary keeps the child state on
/// the heap while preserving the same cancellation identity and lifetime.
pub(crate) fn run_reserved_document_task<'a, T, F>(
    reservation: ReservedDocumentTask,
    future: F,
) -> BoxedDocumentFuture<'a, T>
where
    T: Send + 'a,
    F: Future<Output = Result<T, CommandError>> + Send + 'a,
{
    run_reserved_document_task_boxed(reservation, Box::pin(future))
}

pub(crate) fn run_document_task<'a, T, F>(
    document_id: &str,
    future: F,
) -> BoxedDocumentFuture<'a, T>
where
    T: Send + 'a,
    F: Future<Output = Result<T, CommandError>> + Send + 'a,
{
    let document_id = document_id.to_string();
    let future: BoxedDocumentFuture<'a, T> = Box::pin(future);
    Box::pin(async move {
        let reservation = reserve_document_task(&document_id)?;
        run_reserved_document_task_boxed(reservation, future).await
    })
}

/// Bind once to the real document task so all phases and cloned model contexts
/// share one durable operation, including failures and cancellation.
pub(crate) fn generation_context(
    database: &crate::database::Database,
) -> Result<Option<crate::ai::generation::Context>, CommandError> {
    CURRENT_DOCUMENT_TASK
        .try_with(|task| {
            let mut diagnostic = task.diagnostics.lock().map_err(|_| registry_error())?;
            if diagnostic.is_none() {
                *diagnostic = Some(crate::ai::generation::Operation::begin(
                    database,
                    None,
                    Some(&task.document_id),
                    None,
                    "document_generation",
                )?);
            }
            Ok(diagnostic.as_ref().map(|operation| operation.context()))
        })
        .unwrap_or(Ok(None))
}

pub(crate) fn record_current_generation_reuse(
    database: &crate::database::Database,
    stage: &str,
    cache_key: &str,
) {
    if let Ok(Some(context)) = generation_context(database) {
        context.reused(stage, cache_key);
    }
}

pub(crate) fn current_signal() -> Option<CancellationSignal> {
    CURRENT_DOCUMENT_TASK
        .try_with(|task| task.signal.clone())
        .ok()
}

pub(crate) fn current_reasoning_effort() -> Option<String> {
    CURRENT_DOCUMENT_TASK
        .try_with(|task| {
            task.progress
                .lock()
                .ok()
                .map(|progress| progress.reasoning_effort.clone())
        })
        .ok()
        .flatten()
}

pub(crate) fn document_task_progress(document_id: &str) -> Option<DocumentTaskProgress> {
    ACTIVE_DOCUMENT_TASKS.progress(document_id)
}

pub(crate) fn set_current_reasoning_context(
    provider: &str,
    model: &str,
    requested_effort: &str,
    effective_effort: &str,
) {
    let _ = CURRENT_DOCUMENT_TASK.try_with(|task| {
        if let Ok(mut progress) = task.progress.lock() {
            progress.provider_kind = Some(provider.chars().take(40).collect());
            progress.model_id = Some(model.chars().take(160).collect());
            progress.requested_reasoning_effort = Some(requested_effort.chars().take(16).collect());
            progress.reasoning_effort = effective_effort.chars().take(16).collect();
        }
    });
}

pub(crate) fn set_current_task_phase(
    phase: &str,
    current_window: Option<usize>,
    completed_windows: usize,
    total_windows: usize,
    current_page_start: Option<i64>,
    current_page_end: Option<i64>,
    current_partition: Option<usize>,
) {
    set_current_task_phase_range(
        phase,
        (current_window, current_window),
        completed_windows,
        total_windows,
        current_page_start,
        current_page_end,
        current_partition,
    );
}

pub(crate) fn set_current_task_phase_range(
    phase: &str,
    current_windows: (Option<usize>, Option<usize>),
    completed_windows: usize,
    total_windows: usize,
    current_page_start: Option<i64>,
    current_page_end: Option<i64>,
    current_partition: Option<usize>,
) {
    let (current_window, current_window_end) = current_windows;
    let _ = CURRENT_DOCUMENT_TASK.try_with(|task| {
        if let Ok(mut progress) = task.progress.lock() {
            let next_phase = phase.chars().take(80).collect::<String>();
            if progress.phase != next_phase {
                progress.phase = next_phase;
                progress.phase_started_at = unix_time_millis().unwrap_or(progress.phase_started_at);
            }
            progress.current_window = current_window;
            progress.current_window_end = current_window_end;
            progress.completed_windows = completed_windows;
            progress.total_windows = total_windows;
            progress.current_page_start = current_page_start;
            progress.current_page_end = current_page_end;
            progress.current_partition = current_partition;
            if matches!(phase, "outline" | "generating" | "curating") {
                progress.provider_activity = "connecting".to_string();
                progress.last_provider_activity_at = None;
                progress.last_model_activity_at = None;
            }
        }
    });
}

pub(crate) fn record_current_provider_activity(model_progress: bool) {
    let now = unix_time_millis().unwrap_or_default();
    let _ = CURRENT_DOCUMENT_TASK.try_with(|task| {
        if let Ok(mut progress) = task.progress.lock() {
            progress.last_provider_activity_at = Some(now);
            if model_progress {
                progress.provider_activity = "streaming".to_string();
                progress.last_model_activity_at = Some(now);
            } else if progress.provider_activity != "streaming" {
                progress.provider_activity = "responsive".to_string();
            }
        }
    });
}

pub(crate) fn record_current_provider_request() {
    let _ = CURRENT_DOCUMENT_TASK.try_with(|task| {
        if let Ok(mut progress) = task.progress.lock() {
            progress.provider_request_count = progress.provider_request_count.saturating_add(1);
        }
    });
}

pub(crate) struct CurrentRequestDiagnostics<'a> {
    pub(crate) stage: &'a str,
    pub(crate) provider: &'a str,
    pub(crate) model: &'a str,
    pub(crate) request_bytes: usize,
    pub(crate) text_bytes: usize,
    pub(crate) image_count: usize,
    pub(crate) evidence_bytes: usize,
    pub(crate) max_output_tokens: usize,
}

pub(crate) fn record_current_request_diagnostics(diagnostics: CurrentRequestDiagnostics<'_>) {
    let now = unix_time_millis().unwrap_or_default();
    let _ = CURRENT_DOCUMENT_TASK.try_with(|task| {
        if let Ok(mut progress) = task.progress.lock() {
            progress.current_request_started_at = Some(now);
            progress.current_request_stage = Some(diagnostics.stage.chars().take(80).collect());
            progress.provider_kind = Some(diagnostics.provider.chars().take(40).collect());
            progress.model_id = Some(diagnostics.model.chars().take(160).collect());
            progress.current_request_bytes = Some(diagnostics.request_bytes);
            progress.current_request_text_bytes = Some(diagnostics.text_bytes);
            progress.current_request_image_count = Some(diagnostics.image_count);
            progress.current_request_evidence_bytes = Some(diagnostics.evidence_bytes);
            progress.current_request_max_output_tokens = Some(diagnostics.max_output_tokens);
        }
    });
}

fn clear_current_request_diagnostics(progress: &mut DocumentTaskProgress) {
    progress.current_request_started_at = None;
    progress.current_request_stage = None;
    progress.current_request_bytes = None;
    progress.current_request_text_bytes = None;
    progress.current_request_image_count = None;
    progress.current_request_evidence_bytes = None;
    progress.current_request_max_output_tokens = None;
}

pub(crate) fn record_current_provider_retry(reason: &str) {
    let _ = CURRENT_DOCUMENT_TASK.try_with(|task| {
        if let Ok(mut progress) = task.progress.lock() {
            progress.retry_count = progress.retry_count.saturating_add(1);
            match reason {
                "transient_transport" => {
                    progress.transient_retry_count =
                        progress.transient_retry_count.saturating_add(1);
                }
                "provider_compatibility" => {
                    progress.compatibility_retry_count =
                        progress.compatibility_retry_count.saturating_add(1);
                }
                "contract_repair" => {
                    progress.contract_repair_retry_count =
                        progress.contract_repair_retry_count.saturating_add(1);
                }
                "output_compaction" => {
                    progress.output_compaction_retry_count =
                        progress.output_compaction_retry_count.saturating_add(1);
                }
                _ => {}
            }
        }
    });
}

pub(crate) fn record_current_usage(delta: DocumentTaskUsageDelta) {
    let _ = CURRENT_DOCUMENT_TASK.try_with(|task| {
        if let Ok(mut progress) = task.progress.lock() {
            clear_current_request_diagnostics(&mut progress);
            let reported = delta.prompt_tokens.is_some()
                || delta.completion_tokens.is_some()
                || delta.reasoning_tokens.is_some()
                || delta.total_tokens.is_some()
                || delta.cached_prompt_tokens.is_some()
                || delta.cache_write_tokens.is_some();
            if reported {
                progress.usage_reported_requests =
                    progress.usage_reported_requests.saturating_add(1);
            } else {
                progress.usage_unreported_requests =
                    progress.usage_unreported_requests.saturating_add(1);
                return;
            }
            progress.prompt_tokens = progress
                .prompt_tokens
                .saturating_add(delta.prompt_tokens.unwrap_or(0));
            progress.completion_tokens = progress
                .completion_tokens
                .saturating_add(delta.completion_tokens.unwrap_or(0));
            progress.reasoning_tokens = progress
                .reasoning_tokens
                .saturating_add(delta.reasoning_tokens.unwrap_or(0));
            progress.cached_prompt_tokens = progress
                .cached_prompt_tokens
                .saturating_add(delta.cached_prompt_tokens.unwrap_or(0));
            progress.cache_write_tokens = progress
                .cache_write_tokens
                .saturating_add(delta.cache_write_tokens.unwrap_or(0));
            let total = delta.total_tokens.unwrap_or_else(|| {
                delta
                    .prompt_tokens
                    .unwrap_or(0)
                    .saturating_add(delta.completion_tokens.unwrap_or(0))
            });
            progress.total_tokens = progress.total_tokens.saturating_add(total);
        }
    });
}

pub(crate) fn current_usage_summary() -> Option<(Option<i64>, bool)> {
    CURRENT_DOCUMENT_TASK
        .try_with(|task| {
            task.progress.lock().ok().map(|progress| {
                let total = (progress.usage_reported_requests > 0)
                    .then_some(i64::try_from(progress.total_tokens).unwrap_or(i64::MAX));
                let complete =
                    progress.usage_reported_requests > 0 && progress.usage_unreported_requests == 0;
                (total, complete)
            })
        })
        .ok()
        .flatten()
}

pub(crate) fn cancel_document_task(document_id: &str) -> Result<(), CommandError> {
    ACTIVE_DOCUMENT_TASKS.cancel(document_id)
}

pub(crate) fn is_document_task_active(document_id: &str) -> bool {
    ACTIVE_DOCUMENT_TASKS.is_active(document_id)
}

pub(crate) fn has_active_document_tasks() -> bool {
    ACTIVE_DOCUMENT_TASKS.has_any()
}

pub(crate) fn active_document_task_count() -> usize {
    ACTIVE_DOCUMENT_TASKS.count()
}

pub(crate) fn is_document_task_canceled(document_id: &str) -> bool {
    ACTIVE_DOCUMENT_TASKS.is_canceled(document_id)
}

fn registry_error() -> CommandError {
    CommandError::new(
        "document_task_registry_unavailable",
        "The document task registry is unavailable. Restart ostudy and try again.",
    )
}

#[cfg(mobile)]
struct MobileForegroundExecution {
    app: AppHandle,
    document_id: String,
}

#[cfg(mobile)]
impl MobileForegroundExecution {
    fn begin(document_id: &str) -> Result<Self, CommandError> {
        let app = crate::pdf_render_backend::android_app_handle().map_err(|error| {
            CommandError::new(
                "mobile_background_execution_unavailable",
                format!("Mobile document execution is not initialized: {error}"),
            )
        })?;
        app.ostudy_platform()
            .begin_background_execution(BeginBackgroundExecutionRequest {
                job_id: document_id.to_string(),
                title: "Processing study document".to_string(),
                detail: "Extracting text, rendering pages, and generating study material."
                    .to_string(),
            })
            .map_err(|error| {
                CommandError::new(
                    "mobile_background_execution_failed",
                    format!("Mobile foreground document execution could not start: {error}"),
                )
            })?;
        Ok(Self {
            app,
            document_id: document_id.to_string(),
        })
    }
}

#[cfg(mobile)]
impl Drop for MobileForegroundExecution {
    fn drop(&mut self) {
        let _ = self
            .app
            .ostudy_platform()
            .end_background_execution(self.document_id.clone());
    }
}

#[cfg(not(mobile))]
struct MobileForegroundExecution;

#[cfg(not(mobile))]
impl MobileForegroundExecution {
    fn begin(_document_id: &str) -> Result<Self, CommandError> {
        Ok(Self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn document_generation_local_reuse_receipt_completes_without_provider_requests() {
        let database = crate::database::Database::open_in_memory().unwrap();
        let document_id = "a".repeat(32);
        database
            .with_connection(|connection| {
                connection
                    .execute(
                        "INSERT INTO documents (
                             id,title,original_filename,mime_type,byte_size,sha256,
                             source_relative_path,lifecycle_state,created_at,updated_at
                         ) VALUES (?1,'Fixture','fixture.pdf','application/pdf',1,?2,
                                   'documents/fixture/fixture.pdf','ready',1,1)",
                        rusqlite::params![document_id, "b".repeat(64)],
                    )
                    .map_err(CommandError::database)?;
                Ok(())
            })
            .unwrap();
        let cache_key = crate::ai::generation::hash(b"published artifact receipt");
        let scoped = database.clone();
        run_document_task(&document_id, async move {
            record_current_generation_reuse(&scoped, "guide_publication", &cache_key);
            Ok::<(), CommandError>(())
        })
        .await
        .unwrap();

        database
            .with_connection(|connection| {
                let operation: (String, i64) = connection
                    .query_row(
                        "SELECT state,
                                (SELECT COUNT(*) FROM generation_requests
                                 WHERE operation_id = generation_operations.id)
                         FROM generation_operations",
                        [],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(operation, ("complete".to_string(), 0));
                let reuse: (String, String, i64) = connection
                    .query_row(
                        "SELECT stage,cache_key,COUNT(*) FROM generation_reuse_events
                         GROUP BY stage,cache_key",
                        [],
                        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(reuse.0, "guide_publication");
                assert_eq!(reuse.1.len(), 64);
                assert_eq!(reuse.2, 1);
                Ok(())
            })
            .unwrap();
    }

    #[tokio::test]
    async fn live_diagnostics_keep_requested_and_effective_reasoning_and_request_shape_separate() {
        let document_id = "d".repeat(32);
        let reservation = reserve_document_task(&document_id).unwrap();
        let observed = run_reserved_document_task(reservation, async {
            set_current_reasoning_context(
                "openrouter",
                "deepseek/deepseek-v4.1-flash",
                "automatic",
                "none",
            );
            set_current_task_phase("generating", Some(1), 0, 2, Some(4), Some(7), None);
            record_current_request_diagnostics(CurrentRequestDiagnostics {
                stage: "flashcard_authoring",
                provider: "openrouter",
                model: "deepseek/deepseek-v4.1-flash",
                request_bytes: 12_345,
                text_bytes: 4_321,
                image_count: 4,
                evidence_bytes: 2_222,
                max_output_tokens: 6_144,
            });
            let progress = document_task_progress(&document_id).unwrap();
            assert_eq!(progress.provider_kind.as_deref(), Some("openrouter"));
            assert_eq!(
                progress.model_id.as_deref(),
                Some("deepseek/deepseek-v4.1-flash")
            );
            assert_eq!(
                progress.requested_reasoning_effort.as_deref(),
                Some("automatic")
            );
            assert_eq!(progress.reasoning_effort, "none");
            assert_eq!(
                progress.current_request_stage.as_deref(),
                Some("flashcard_authoring")
            );
            assert_eq!(progress.current_request_bytes, Some(12_345));
            assert_eq!(progress.current_request_text_bytes, Some(4_321));
            assert_eq!(progress.current_request_image_count, Some(4));
            assert_eq!(progress.current_request_evidence_bytes, Some(2_222));
            assert_eq!(progress.current_request_max_output_tokens, Some(6_144));
            assert!(progress.phase_started_at >= progress.started_at);
            record_current_usage(DocumentTaskUsageDelta {
                total_tokens: Some(0),
                ..DocumentTaskUsageDelta::default()
            });
            let after = document_task_progress(&document_id).unwrap();
            assert!(after.current_request_started_at.is_none());
            assert!(after.current_request_stage.is_none());
            Ok::<(), CommandError>(())
        })
        .await;
        observed.unwrap();
    }

    #[test]
    fn document_task_wrapper_boxes_large_child_future_before_polling() {
        const LARGE_CHILD_BYTES: usize = 256 * 1024;
        let payload = [0x5a_u8; LARGE_CHILD_BYTES];
        let child = async move {
            std::hint::black_box(payload);
            Ok::<(), CommandError>(())
        };
        assert!(
            std::mem::size_of_val(&child) >= LARGE_CHILD_BYTES,
            "fixture must remain a genuinely large inline async state"
        );

        let wrapped = run_document_task("boxed-large-child", child);
        assert!(
            std::mem::size_of_val(&wrapped) <= 2 * std::mem::size_of::<usize>(),
            "document task boundary must retain only the boxed-future pointer"
        );
        drop(wrapped);
        assert!(!is_document_task_active("boxed-large-child"));
    }

    #[test]
    fn reservation_is_visible_to_stop_and_duplicate_start_before_polling() {
        let document_id = "reserved-before-poll";
        let reservation = reserve_document_task(document_id).unwrap();
        assert!(is_document_task_active(document_id));
        assert!(reserve_document_task(document_id).is_err());
        cancel_document_task(document_id).unwrap();
        assert!(is_document_task_canceled(document_id));
        drop(reservation);
        assert!(!is_document_task_active(document_id));
    }

    #[tokio::test]
    async fn cancellation_is_scoped_to_one_document_task() {
        let first_id = "task-cancel-a";
        let second_id = "task-cancel-b";
        let first = ACTIVE_DOCUMENT_TASKS
            .begin(first_id)
            .expect("first task should register");
        let second = ACTIVE_DOCUMENT_TASKS
            .begin(second_id)
            .expect("second task should register");

        cancel_document_task(first_id).expect("first task should cancel");
        first.signal.canceled().await;
        assert!(first.signal.is_canceled());
        assert!(is_document_task_canceled(first_id));
        assert!(!second.signal.is_canceled());
        assert!(!is_document_task_canceled(second_id));

        ACTIVE_DOCUMENT_TASKS.finish(first_id);
        ACTIVE_DOCUMENT_TASKS.finish(second_id);
        assert!(!is_document_task_active(first_id));
        assert!(!is_document_task_canceled(first_id));
    }

    #[test]
    fn active_document_registry_drives_close_protection() {
        let registry = ActiveDocumentTasks::default();
        let document_id = "task-close-guard";
        assert!(!registry.has_any());
        registry.begin(document_id).expect("task should register");
        assert!(registry.has_any());
        registry.finish(document_id);
        assert!(!registry.has_any());
    }

    #[tokio::test]
    async fn task_local_progress_accumulates_usage_retries_and_reasoning_policy() {
        let document_id = "task-progress";
        let task = ACTIVE_DOCUMENT_TASKS
            .begin(document_id)
            .expect("task should register");
        CURRENT_DOCUMENT_TASK
            .scope(task, async {
                set_current_reasoning_effort("minimal");
                set_current_task_phase("outline", Some(3), 2, 10, Some(5), Some(7), Some(1));
                record_current_provider_request();
                record_current_provider_retry("contract_repair");
                record_current_usage(DocumentTaskUsageDelta {
                    prompt_tokens: Some(1_000),
                    completion_tokens: Some(200),
                    reasoning_tokens: Some(80),
                    total_tokens: Some(1_200),
                    cached_prompt_tokens: Some(600),
                    cache_write_tokens: Some(100),
                });
                assert_eq!(current_reasoning_effort().as_deref(), Some("minimal"));
            })
            .await;
        let progress = document_task_progress(document_id).expect("progress should be visible");
        assert_eq!(progress.phase, "outline");
        assert_eq!(progress.current_window, Some(3));
        assert_eq!(progress.current_page_start, Some(5));
        assert_eq!(progress.reasoning_effort, "minimal");
        assert_eq!(progress.provider_request_count, 1);
        assert_eq!(progress.retry_count, 1);
        assert_eq!(progress.contract_repair_retry_count, 1);
        assert_eq!(progress.transient_retry_count, 0);
        assert_eq!(progress.total_tokens, 1_200);
        assert_eq!(progress.reasoning_tokens, 80);
        assert_eq!(progress.cached_prompt_tokens, 600);
        ACTIVE_DOCUMENT_TASKS.finish(document_id);
    }
    #[test]
    fn registration_is_released_after_an_early_error_or_abandoned_future() {
        let document_id = "interrupted-document-registration";
        let signal;
        {
            let task = ACTIVE_DOCUMENT_TASKS.begin(document_id).unwrap();
            signal = task.signal.clone();
            let _registration = DocumentRegistration {
                document_id: document_id.to_string(),
                signal: task.signal,
            };
            assert!(is_document_task_active(document_id));
        }
        assert!(!is_document_task_active(document_id));
        assert!(signal.is_canceled());
        ACTIVE_DOCUMENT_TASKS
            .begin(document_id)
            .expect("released document should register again");
        ACTIVE_DOCUMENT_TASKS.finish(document_id);
    }
}
