//! Shared admission for provider/processing work and exclusive local maintenance.
//! Domain registries still own operation IDs, progress, and concurrency limits.
//! This gate only closes the check-then-start race across those registries; its
//! mutex is never held during IO, a database transaction, or an await.

use std::sync::{Arc, LazyLock, Mutex};

use crate::error::CommandError;

static GATE: LazyLock<Arc<ActivityGate>> = LazyLock::new(|| Arc::new(ActivityGate::default()));

/// Window-close policy includes both background activities and queued/running IPC.
/// A cancellation request does not release activity until the actual owner exits.
pub(crate) fn has_active_work() -> bool {
    GATE.has_work() || crate::library_session::has_active_requests()
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
    fn has_work(&self) -> bool {
        self.state.lock().map_or(true, |state| state.active != 0)
    }

    fn enter(self: &Arc<Self>, maintenance: bool) -> Result<ActivityGuard, CommandError> {
        let mut state = self.state.lock().map_err(|_| unavailable())?;
        if state.maintenance || (maintenance && state.active != 0) {
            return Err(CommandError::new(
                "learning_maintenance_busy",
                "Finish or cancel active document, collection, tutor, preparation, or device synchronization work before library maintenance; finish maintenance before starting another task.",
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
pub(crate) fn isolated_activity_for_test() -> (ActivityGuard, impl Fn() -> bool) {
    let gate = Arc::new(ActivityGate::default());
    let activity = gate.enter(false).unwrap();
    (activity, move || gate.enter(true).is_err())
}

#[cfg(test)]
#[derive(Default)]
pub(crate) struct TestGate(Arc<ActivityGate>);

#[cfg(test)]
impl TestGate {
    pub(crate) fn activity(&self) -> Result<ActivityGuard, CommandError> {
        self.0.enter(false)
    }

    pub(crate) fn maintenance(&self) -> Result<ActivityGuard, CommandError> {
        self.0.enter(true)
    }

    pub(crate) fn has_work(&self) -> bool {
        self.0.has_work()
    }
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

    #[test]
    fn simultaneous_work_and_maintenance_cannot_both_enter() {
        let gate = Arc::new(ActivityGate::default());
        let barrier = Arc::new(std::sync::Barrier::new(2));
        let worker_gate = gate.clone();
        let worker_barrier = barrier.clone();
        let worker = std::thread::spawn(move || {
            worker_barrier.wait();
            let lease = worker_gate.enter(false);
            worker_barrier.wait();
            lease
        });
        barrier.wait();
        let maintenance = gate.enter(true);
        barrier.wait();
        let activity = worker.join().unwrap();
        assert_ne!(activity.is_ok(), maintenance.is_ok());
        assert!(gate.has_work());
        drop(activity);
        drop(maintenance);
        assert!(!gate.has_work());
    }
}
