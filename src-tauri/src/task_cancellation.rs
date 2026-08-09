//! Task-scoped cancellation for long-running document AI work.
//!
//! Cancellation is intentionally process-local. Durable document/job state
//! remains in SQLite; this module binds one running document task to an in-memory
//! signal. Async provider traffic uses the Tokio task-local signal, while
//! blocking extraction/render/OCR workers can query the same registry by exact
//! document id.

use std::{
    collections::HashMap,
    future::Future,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, LazyLock, Mutex,
    },
};

use tokio::sync::Notify;

use crate::error::CommandError;

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
            // Register the waiter before re-checking the atomic flag so a
            // cancellation racing this call cannot be missed by notify_waiters().
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

#[derive(Default)]
struct ActiveDocumentTasks {
    tasks: Mutex<HashMap<String, CancellationSignal>>,
}

impl ActiveDocumentTasks {
    fn begin(&self, document_id: &str) -> Result<CancellationSignal, CommandError> {
        let mut tasks = self.tasks.lock().map_err(|_| registry_error())?;
        if tasks.contains_key(document_id) {
            return Err(CommandError::new(
                "document_generation_already_running",
                "Flashcard generation is already running for this PDF.",
            ));
        }
        let signal = CancellationSignal::new();
        tasks.insert(document_id.to_string(), signal.clone());
        Ok(signal)
    }

    fn finish(&self, document_id: &str) {
        if let Ok(mut tasks) = self.tasks.lock() {
            tasks.remove(document_id);
        }
    }

    fn cancel(&self, document_id: &str) -> Result<(), CommandError> {
        let tasks = self.tasks.lock().map_err(|_| registry_error())?;
        let signal = tasks.get(document_id).ok_or_else(|| {
            CommandError::new(
                "document_generation_not_running",
                "There is no active flashcard-generation request for this PDF.",
            )
        })?;
        signal.cancel();
        Ok(())
    }

    fn is_active(&self, document_id: &str) -> bool {
        self.tasks
            .lock()
            .map(|tasks| tasks.contains_key(document_id))
            .unwrap_or(false)
    }

    fn is_canceled(&self, document_id: &str) -> bool {
        self.tasks
            .lock()
            .map(|tasks| {
                tasks
                    .get(document_id)
                    .is_some_and(CancellationSignal::is_canceled)
            })
            .unwrap_or(false)
    }
}

static ACTIVE_DOCUMENT_TASKS: LazyLock<ActiveDocumentTasks> =
    LazyLock::new(ActiveDocumentTasks::default);

tokio::task_local! {
    static CURRENT_DOCUMENT_SIGNAL: CancellationSignal;
}

pub(crate) async fn run_document_task<T, F>(document_id: &str, future: F) -> Result<T, CommandError>
where
    F: Future<Output = Result<T, CommandError>>,
{
    let signal = ACTIVE_DOCUMENT_TASKS.begin(document_id)?;
    let result = CURRENT_DOCUMENT_SIGNAL.scope(signal, future).await;
    ACTIVE_DOCUMENT_TASKS.finish(document_id);
    result
}

pub(crate) fn current_signal() -> Option<CancellationSignal> {
    CURRENT_DOCUMENT_SIGNAL.try_with(Clone::clone).ok()
}

pub(crate) fn cancel_document_task(document_id: &str) -> Result<(), CommandError> {
    ACTIVE_DOCUMENT_TASKS.cancel(document_id)
}

pub(crate) fn is_document_task_active(document_id: &str) -> bool {
    ACTIVE_DOCUMENT_TASKS.is_active(document_id)
}

/// Blocking extraction/render/OCR workers do not inherit Tokio task-local
/// storage. They use this exact-document registry query to observe the same Stop
/// action without introducing a second cancellation authority.
pub(crate) fn is_document_task_canceled(document_id: &str) -> bool {
    ACTIVE_DOCUMENT_TASKS.is_canceled(document_id)
}

fn registry_error() -> CommandError {
    CommandError::new(
        "document_task_registry_unavailable",
        "The document task registry is unavailable. Restart ostudy and try again.",
    )
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
        first.canceled().await;
        assert!(first.is_canceled());
        assert!(is_document_task_canceled(first_id));
        assert!(!second.is_canceled());
        assert!(!is_document_task_canceled(second_id));

        ACTIVE_DOCUMENT_TASKS.finish(first_id);
        ACTIVE_DOCUMENT_TASKS.finish(second_id);
        assert!(!is_document_task_active(first_id));
        assert!(!is_document_task_canceled(first_id));
    }
}
