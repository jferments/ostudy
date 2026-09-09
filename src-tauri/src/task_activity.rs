//! Shared admission for provider/processing work and exclusive local maintenance.
//! Domain registries still own operation IDs, progress, and concurrency limits.
//! This gate only closes the check-then-start race across those registries; its
//! mutex is never held during IO, a database transaction, or an await.

use std::sync::{Arc, LazyLock, Mutex};

use crate::error::CommandError;

static GATE: LazyLock<Arc<ActivityGate>> = LazyLock::new(|| Arc::new(ActivityGate::default()));

pub(crate) fn maintenance_active() -> bool {
    GATE.state.lock().map_or(true, |state| state.maintenance)
}

#[derive(Default)]
struct ActivityState {
    active: usize,
    maintenance: bool,
}

#[derive(Default)]
struct ActivityGate {
    state: Mutex<ActivityState>,
}

#[must_use]
pub(crate) struct ActivityGuard {
    gate: Arc<ActivityGate>,
    maintenance: bool,
}

impl ActivityGuard {
    pub(crate) fn begin() -> Result<Self, CommandError> {
        GATE.enter(false)
    }
}

#[must_use]
pub(crate) struct MaintenanceGuard {
    _lease: ActivityGuard,
}

impl MaintenanceGuard {
    pub(crate) fn begin() -> Result<Self, CommandError> {
        Ok(Self {
            _lease: GATE.enter(true)?,
        })
    }
}

impl ActivityGate {
    fn enter(self: &Arc<Self>, maintenance: bool) -> Result<ActivityGuard, CommandError> {
        let mut state = self.state.lock().map_err(|_| unavailable())?;
        if state.maintenance || (maintenance && state.active != 0) {
            return Err(CommandError::new(
                "learning_maintenance_busy",
                "Finish or cancel active document, tutor, or preparation work before library maintenance; finish maintenance before starting another task.",
            ));
        }
        state.active = state.active.checked_add(1).ok_or_else(unavailable)?;
        state.maintenance = maintenance;
        Ok(ActivityGuard {
            gate: self.clone(),
            maintenance,
        })
    }
}

impl Drop for ActivityGuard {
    fn drop(&mut self) {
        if let Ok(mut state) = self.gate.state.lock() {
            state.active = state.active.saturating_sub(1);
            if self.maintenance {
                state.maintenance = false;
            }
        }
    }
}

fn unavailable() -> CommandError {
    CommandError::new(
        "learning_task_registry_unavailable",
        "Task admission is unavailable. Restart ostudy before trying again.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maintenance_and_shared_work_exclude_each_other_without_serializing_shared_work() {
        let gate = Arc::new(ActivityGate::default());
        let document = gate.enter(false).unwrap();
        let tutor = gate.enter(false).unwrap();
        assert!(gate.enter(true).is_err());
        drop(document);
        assert!(gate.enter(true).is_err());
        drop(tutor);
        let maintenance = gate.enter(true).unwrap();
        assert!(gate.enter(false).is_err());
        assert!(gate.enter(true).is_err());
        drop(maintenance);
        assert!(gate.enter(false).is_ok());
    }

    #[test]
    fn failed_admission_does_not_leak_a_reservation() {
        let gate = Arc::new(ActivityGate::default());
        let maintenance = gate.enter(true).unwrap();
        for _ in 0..3 {
            assert!(gate.enter(false).is_err());
        }
        drop(maintenance);
        assert!(gate.enter(true).is_ok());
    }
}
