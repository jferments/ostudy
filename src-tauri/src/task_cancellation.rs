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
    fn new() -> Self {
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
    pub current_window: Option<usize>,
    pub completed_windows: usize,
    pub total_windows: usize,
    pub current_page_start: Option<i64>,
    pub current_page_end: Option<i64>,
    pub current_partition: Option<usize>,
    pub reasoning_effort: String,
    pub provider_request_count: u64,
    pub retry_count: u64,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub reasoning_tokens: u64,
    pub total_tokens: u64,
    pub cached_prompt_tokens: u64,
    pub cache_write_tokens: u64,
    pub usage_reported_requests: u64,
    pub usage_unreported_requests: u64,
}

impl DocumentTaskProgress {
    fn new(started_at: i64) -> Self {
        Self {
            started_at,
            phase: "preparing".to_string(),
            current_window: None,
            completed_windows: 0,
            total_windows: 0,
            current_page_start: None,
            current_page_end: None,
            current_partition: None,
            reasoning_effort: "low".to_string(),
            provider_request_count: 0,
            retry_count: 0,
            prompt_tokens: 0,
            completion_tokens: 0,
            reasoning_tokens: 0,
            total_tokens: 0,
            cached_prompt_tokens: 0,
            cache_write_tokens: 0,
            usage_reported_requests: 0,
            usage_unreported_requests: 0,
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
    signal: CancellationSignal,
    progress: Arc<Mutex<DocumentTaskProgress>>,
}

impl ActiveDocumentTask {
    fn new(document_id: &str, started_at: i64) -> Self {
        Self {
            document_id: document_id.to_string(),
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

pub(crate) async fn run_document_task<T, F>(document_id: &str, future: F) -> Result<T, CommandError>
where
    F: Future<Output = Result<T, CommandError>>,
{
    let task = ACTIVE_DOCUMENT_TASKS.begin(document_id)?;
    let result = {
        let _foreground_execution = MobileForegroundExecution::begin(document_id)?;
        CURRENT_DOCUMENT_TASK.scope(task, future).await
    };
    ACTIVE_DOCUMENT_TASKS.finish(document_id);
    result
}

pub(crate) fn current_signal() -> Option<CancellationSignal> {
    CURRENT_DOCUMENT_TASK
        .try_with(|task| task.signal.clone())
        .ok()
}

pub(crate) fn current_document_id() -> Option<String> {
    CURRENT_DOCUMENT_TASK
        .try_with(|task| task.document_id.clone())
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

pub(crate) fn set_current_reasoning_effort(effort: &str) {
    let _ = CURRENT_DOCUMENT_TASK.try_with(|task| {
        if let Ok(mut progress) = task.progress.lock() {
            progress.reasoning_effort = effort.chars().take(16).collect();
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
    let _ = CURRENT_DOCUMENT_TASK.try_with(|task| {
        if let Ok(mut progress) = task.progress.lock() {
            progress.phase = phase.chars().take(80).collect();
            progress.current_window = current_window;
            progress.completed_windows = completed_windows;
            progress.total_windows = total_windows;
            progress.current_page_start = current_page_start;
            progress.current_page_end = current_page_end;
            progress.current_partition = current_partition;
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

pub(crate) fn record_current_provider_retry() {
    let _ = CURRENT_DOCUMENT_TASK.try_with(|task| {
        if let Ok(mut progress) = task.progress.lock() {
            progress.retry_count = progress.retry_count.saturating_add(1);
        }
    });
}

pub(crate) fn record_current_usage(delta: DocumentTaskUsageDelta) {
    let _ = CURRENT_DOCUMENT_TASK.try_with(|task| {
        if let Ok(mut progress) = task.progress.lock() {
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

pub(crate) fn cancel_document_task(document_id: &str) -> Result<(), CommandError> {
    ACTIVE_DOCUMENT_TASKS.cancel(document_id)
}

pub(crate) fn is_document_task_active(document_id: &str) -> bool {
    ACTIVE_DOCUMENT_TASKS.is_active(document_id)
}

pub(crate) fn has_active_document_tasks() -> bool {
    ACTIVE_DOCUMENT_TASKS.has_any()
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
                record_current_provider_retry();
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
        assert_eq!(progress.total_tokens, 1_200);
        assert_eq!(progress.reasoning_tokens, 80);
        assert_eq!(progress.cached_prompt_tokens, 600);
        ACTIVE_DOCUMENT_TASKS.finish(document_id);
    }
}
