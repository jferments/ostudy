//! Bounded, FIFO execution of the generated native command dispatcher.
//!
//! A single ordinary worker preserves dispatch order and avoids creating a pile
//! of threads waiting for the application's single SQLite connection. A separate
//! control worker keeps cancellation, session observation and platform Back away
//! from that queue. Neither admission path waits on the native event thread.
//! Async command adapters still own their future's original-generation lease;
//! this boundary owns synchronous work and the interval before async dispatch.

use std::{
    panic::{catch_unwind, AssertUnwindSafe},
    sync::{
        mpsc::{sync_channel, SyncSender, TrySendError},
        Arc, LazyLock,
    },
    thread,
    time::Instant,
};

use crate::error::CommandError;

use super::Lease;

const ORDINARY_PENDING: usize = 64;
const CONTROL_PENDING: usize = 16;

static ORDINARY: LazyLock<Result<Executor, CommandError>> =
    LazyLock::new(|| Executor::new("ostudy-native-commands", ORDINARY_PENDING));
static CONTROL: LazyLock<Result<Executor, CommandError>> =
    LazyLock::new(|| Executor::new("ostudy-native-controls", CONTROL_PENDING));

struct Job {
    operation: Box<dyn FnOnce() + Send>,
    on_panic: Box<dyn FnOnce() + Send>,
    queued_at: Instant,
}

struct Executor {
    sender: SyncSender<Job>,
}

impl Executor {
    fn new(name: &'static str, pending: usize) -> Result<Self, CommandError> {
        let (sender, receiver) = sync_channel::<Job>(pending);
        thread::Builder::new()
            .name(name.to_string())
            .spawn(move || {
                for job in receiver {
                    let started = Instant::now();
                    let queue_wait = started.duration_since(job.queued_at);
                    let failed = catch_unwind(AssertUnwindSafe(job.operation)).is_err();
                    if failed {
                        // The installed panic hook records the failure. Preserve
                        // database/session poison; this does not reopen either.
                        // A reply failure must not kill the queue's sole worker.
                        let _ = catch_unwind(AssertUnwindSafe(job.on_panic));
                    }
                    crate::logging::debug_target(
                        "ipc.execution",
                        "native_dispatch_completed",
                        "A native command dispatch worker completed a job.",
                        serde_json::json!({
                            "lane": name,
                            "queueWaitMicros": micros(queue_wait),
                            "executionMicros": micros(started.elapsed()),
                            "panicked": failed,
                        }),
                    );
                }
            })
            .map_err(|_| unavailable())?;
        Ok(Self { sender })
    }

    fn submit(
        &self,
        operation: impl FnOnce() + Send + 'static,
        on_panic: impl FnOnce() + Send + 'static,
    ) -> Result<(), CommandError> {
        // Captures (including leases and payloads) are dropped on rejection.
        // Accepted jobs remain owned by the queue/worker, not the caller's reply.
        match self.sender.try_send(Job {
            operation: Box::new(operation),
            on_panic: Box::new(on_panic),
            queued_at: Instant::now(),
        }) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => Err(CommandError::new(
                "native_command_queue_full",
                "Native command capacity is full. This request was not executed; wait for current work to finish before trying again.",
            )),
            Err(TrySendError::Disconnected(_)) => Err(unavailable()),
        }
    }
}

fn micros(duration: std::time::Duration) -> u64 {
    u64::try_from(duration.as_micros()).unwrap_or(u64::MAX)
}

fn unavailable() -> CommandError {
    CommandError::new(
        "native_command_executor_unavailable",
        "Native command execution is unavailable. This request was not executed. Restart oStudy.",
    )
}

fn is_control(command: &str) -> bool {
    matches!(
        command,
        "get_library_session"
            | "prepare_library_restore_async"
            | "get_platform_capabilities"
            | "set_platform_back_interception"
            | "cancel_device_sync_session"
            | "cancel_learning_task"
            | "cancel_study_target_bundle"
            | "cancel_document_flashcard_generation"
            | "cancel_document_study_guide_generation"
            | "cancel_subject_course_writing"
    )
}

pub(super) fn dispatch<R, F>(
    dispatch: Arc<F>,
    invoke: tauri::ipc::Invoke<R>,
    lease: Option<Lease>,
) -> bool
where
    R: tauri::Runtime,
    F: Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static,
{
    let executor = if is_control(invoke.message.command()) {
        &*CONTROL
    } else {
        &*ORDINARY
    };
    let executor = match executor {
        Ok(executor) => executor,
        Err(_) => {
            invoke.resolver.reject(unavailable());
            return true;
        }
    };
    let rejected = invoke.resolver.clone();
    let unknown = rejected.clone();
    let panicked = rejected.clone();
    let submitted = executor.submit(
        move || {
            // Keep this capture alive through deserialization, SQL/filesystem
            // work, serialization and reply. Losing a reply never cancels it.
            let _lease = lease;
            if !dispatch(invoke) {
                unknown.reject(CommandError::new(
                    "unknown_native_command",
                    "The requested native command is not registered.",
                ));
            }
        },
        move || {
            panicked.reject(CommandError::new(
                "native_command_panicked",
                "Native command execution failed unexpectedly. Its outcome may be unknown; do not repeat a mutation with a new operation identity. Restart oStudy.",
            ));
        },
    );
    if let Err(error) = submitted {
        rejected.reject(error);
    }
    // Deferred dispatch also owns rejection of unregistered commands; it cannot
    // return the generated dispatcher's eventual bool to the event loop.
    true
}

#[cfg(test)]
#[path = "execution_tests.rs"]
mod tests;
