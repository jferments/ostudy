use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex},
};

use crate::{
    error::CommandError, task_activity::ActivityGuard, task_cancellation::CancellationSignal,
};

static ACTIVE: LazyLock<Mutex<HashMap<String, CancellationSignal>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub(crate) struct Guard {
    operation_id: String,
    signal: CancellationSignal,
    _activity: ActivityGuard,
}

impl Guard {
    pub(crate) fn begin(operation_id: &str) -> Result<Self, CommandError> {
        Self::with_activity(operation_id, ActivityGuard::begin()?)
    }

    fn with_activity(operation_id: &str, activity: ActivityGuard) -> Result<Self, CommandError> {
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
            _activity: activity,
        })
    }

    pub(crate) fn signal(&self) -> CancellationSignal {
        self.signal.clone()
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        self.signal.cancel();
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
    use crate::task_activity::TestGate;

    #[test]
    fn operations_cancel_independently() {
        let gate = TestGate::default();
        let first = Guard::with_activity("h02-first", gate.activity().unwrap()).unwrap();
        let second = Guard::with_activity("h02-second", gate.activity().unwrap()).unwrap();
        let first_signal = first.signal();
        let second_signal = second.signal();

        cancel("h02-first").unwrap();
        assert!(first_signal.is_canceled());
        assert!(!second_signal.is_canceled());
        assert!(gate.maintenance().is_err());
        assert!(
            gate.has_work(),
            "close protection lasts until actual completion"
        );

        drop(first);
        assert!(!is_active("h02-first"));
        assert!(is_active("h02-second"));
        assert!(gate.maintenance().is_err());
        drop(second);
        assert!(!gate.has_work());
        assert!(gate.maintenance().is_ok());
    }

    #[test]
    fn maintenance_first_refuses_registration_and_duplicate_start_releases_its_activity() {
        let gate = TestGate::default();
        let maintenance = gate.maintenance().unwrap();
        let begin = || Guard::with_activity("h02-duplicate", gate.activity()?);
        assert!(begin().is_err());
        assert!(!is_active("h02-duplicate"));
        drop(maintenance);
        let first = begin().unwrap();
        assert!(begin().is_err());
        assert!(is_active("h02-duplicate"));
        drop(first);
        assert!(!is_active("h02-duplicate"));
        assert!(!gate.has_work());
        assert!(gate.maintenance().is_ok());
    }
}
