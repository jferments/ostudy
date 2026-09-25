use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex},
};

use crate::{error::CommandError, task_cancellation::CancellationSignal};

static ACTIVE: LazyLock<Mutex<HashMap<String, CancellationSignal>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub(crate) struct Guard {
    operation_id: String,
    signal: CancellationSignal,
}

impl Guard {
    pub(crate) fn begin(operation_id: &str) -> Result<Self, CommandError> {
        let signal = CancellationSignal::new();
        let mut active = ACTIVE.lock().map_err(|_| registry_error())?;
        if active.contains_key(operation_id) {
            return Err(CommandError::new(
                "study_target_generation_already_running",
                "That collection study-material operation is already running.",
            ));
        }
        active.insert(operation_id.to_string(), signal.clone());
        Ok(Self {
            operation_id: operation_id.to_string(),
            signal,
        })
    }

    pub(crate) fn signal(&self) -> CancellationSignal {
        self.signal.clone()
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        if let Ok(mut active) = ACTIVE.lock() {
            active.remove(&self.operation_id);
        }
    }
}

pub(crate) fn cancel(operation_id: &str) -> Result<(), CommandError> {
    let active = ACTIVE.lock().map_err(|_| registry_error())?;
    let signal = active.get(operation_id).ok_or_else(|| {
        CommandError::new(
            "study_target_generation_not_running",
            "That collection study-material operation is no longer running.",
        )
    })?;
    signal.cancel();
    Ok(())
}

#[cfg(test)]
pub(crate) fn is_active(operation_id: &str) -> bool {
    ACTIVE
        .lock()
        .map(|active| active.contains_key(operation_id))
        .unwrap_or(false)
}

fn registry_error() -> CommandError {
    CommandError::new(
        "study_target_generation_registry_unavailable",
        "The collection generation registry is unavailable. Restart oStudy and try again.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operations_cancel_independently() {
        let first = Guard::begin("a").unwrap();
        let second = Guard::begin("b").unwrap();
        let first_signal = first.signal();
        let second_signal = second.signal();

        cancel("a").unwrap();
        assert!(first_signal.is_canceled());
        assert!(!second_signal.is_canceled());

        drop(first);
        assert!(!is_active("a"));
        assert!(is_active("b"));
        drop(second);
    }
}
