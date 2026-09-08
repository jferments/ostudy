//! Process-local admission and cancellation for semantic preparation and tutor
//! turns. Durable operation state belongs to their repositories, not this map.

use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex},
};

use serde::Serialize;

use crate::{error::CommandError, task_cancellation::CancellationSignal};

static TASKS: LazyLock<TaskRegistry> = LazyLock::new(TaskRegistry::default);
const MAX_INTERACTIVE_TASKS: usize = 3;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskProgress {
    pub operation_id: String,
    pub phase: String,
    pub completed: usize,
    pub total: usize,
    pub canceled: bool,
}

#[derive(Clone)]
struct ActiveTask {
    key: String,
    preparation: bool,
    signal: CancellationSignal,
    progress: TaskProgress,
}

#[derive(Default)]
struct TaskRegistry {
    active: Mutex<HashMap<String, ActiveTask>>,
}

pub(crate) struct TaskGuard {
    operation_id: String,
    pub signal: CancellationSignal,
}

impl TaskGuard {
    pub(crate) fn begin(
        operation_id: &str,
        key: &str,
        preparation: bool,
    ) -> Result<Self, CommandError> {
        let signal = TASKS.begin(operation_id, key, preparation)?;
        Ok(Self {
            operation_id: operation_id.to_string(),
            signal,
        })
    }

    pub(crate) fn progress(&self, phase: &str, completed: usize, total: usize) {
        if let Ok(mut tasks) = TASKS.active.lock() {
            if let Some(task) = tasks.get_mut(&self.operation_id) {
                task.progress.phase = phase.chars().take(80).collect();
                task.progress.completed = completed.min(total);
                task.progress.total = total;
            }
        }
    }

    pub(crate) fn check(&self) -> Result<(), CommandError> {
        check_canceled(&self.signal)
    }
}

impl Drop for TaskGuard {
    fn drop(&mut self) {
        self.signal.cancel();
        if let Ok(mut tasks) = TASKS.active.lock() {
            tasks.remove(&self.operation_id);
        }
    }
}

impl TaskRegistry {
    fn begin(
        &self,
        operation_id: &str,
        key: &str,
        preparation: bool,
    ) -> Result<CancellationSignal, CommandError> {
        let mut tasks = self.active.lock().map_err(|_| unavailable())?;
        if tasks.contains_key(operation_id) || tasks.values().any(|task| task.key == key) {
            return Err(CommandError::new(
                "learning_task_running",
                "This learning operation is already running. Its existing progress remains available.",
            ));
        }
        let preparation_running = tasks.values().any(|task| task.preparation);
        let interactive_count = tasks.values().filter(|task| !task.preparation).count();
        if (preparation && preparation_running)
            || (!preparation && interactive_count >= MAX_INTERACTIVE_TASKS)
        {
            return Err(CommandError::new(
                "learning_task_capacity",
                "Another learning task is using the available capacity. Finish or cancel it before starting this task.",
            ));
        }
        let signal = CancellationSignal::new();
        tasks.insert(
            operation_id.to_string(),
            ActiveTask {
                key: key.to_string(),
                preparation,
                signal: signal.clone(),
                progress: TaskProgress {
                    operation_id: operation_id.to_string(),
                    phase: "preparing".to_string(),
                    completed: 0,
                    total: 0,
                    canceled: false,
                },
            },
        );
        Ok(signal)
    }
}

pub(crate) fn cancel(operation_id: &str) -> Result<(), CommandError> {
    let mut tasks = TASKS.active.lock().map_err(|_| unavailable())?;
    if let Some(task) = tasks.get_mut(operation_id) {
        task.signal.cancel();
        task.progress.canceled = true;
    }
    // Canceling an operation that just completed is intentionally idempotent.
    Ok(())
}

pub(crate) fn progress(operation_id: &str) -> Result<Option<TaskProgress>, CommandError> {
    Ok(TASKS
        .active
        .lock()
        .map_err(|_| unavailable())?
        .get(operation_id)
        .map(|task| task.progress.clone()))
}

pub(crate) fn has_active_tasks() -> bool {
    TASKS.active.lock().map_or(true, |tasks| !tasks.is_empty())
}

pub(crate) fn check_canceled(signal: &CancellationSignal) -> Result<(), CommandError> {
    if signal.is_canceled() {
        Err(CommandError::new(
            "ai_request_canceled",
            "This learning request was canceled. Previously saved material is unchanged.",
        ))
    } else {
        Ok(())
    }
}

fn unavailable() -> CommandError {
    CommandError::new(
        "learning_task_registry_unavailable",
        "Learning task state is unavailable. Restart ostudy before trying again.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admission_separates_conversations_and_bounds_preparation() {
        let registry = TaskRegistry::default();
        let first = registry.begin("a", "conversation:a", false).unwrap();
        assert!(registry.begin("b", "conversation:a", false).is_err());
        let second = registry.begin("b", "conversation:b", false).unwrap();
        registry.begin("c", "prepare:deck", true).unwrap();
        assert!(registry.begin("d", "prepare:other", true).is_err());
        first.cancel();
        assert!(first.is_canceled());
        assert!(!second.is_canceled());
    }
}
