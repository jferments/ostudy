//! Advisory startup progress. Subscribe-then-request replays the latest bounded
//! snapshot for a webview which loaded after migration began. Neither event can
//! open the library, authorize recovery, or advance durable startup readiness.

use std::sync::Mutex;

use serde::Serialize;
use tauri::{Emitter, Listener, Manager};

const PROGRESS: &str = "ostudy-library-upgrade-progress";
const REQUEST: &str = "ostudy-library-upgrade-progress-request";

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct Snapshot {
    sequence: u32,
    message: String,
    completed: bool,
}

#[derive(Default)]
struct ProgressState(Mutex<Option<Snapshot>>);

impl ProgressState {
    fn publish(&self, message: &str, completed: bool) -> Option<Snapshot> {
        let mut current = self.0.lock().ok()?;
        let sequence = current
            .as_ref()
            .map_or(1, |value| value.sequence.saturating_add(1));
        let next = Snapshot {
            sequence,
            message: message.into(),
            completed,
        };
        *current = Some(next.clone());
        Some(next)
    }

    fn snapshot(&self) -> Option<Snapshot> {
        self.0.lock().ok()?.clone()
    }
}

pub(super) fn install(app: &tauri::App) {
    if app.manage(ProgressState::default()) {
        let handle = app.handle().clone();
        app.listen(REQUEST, move |_| {
            if let Some(snapshot) = handle
                .try_state::<ProgressState>()
                .and_then(|state| state.snapshot())
            {
                let _ = handle.emit(PROGRESS, snapshot);
            }
        });
    }
}

pub(super) fn publish(app: &tauri::AppHandle, message: &str, completed: bool) {
    if let Some(snapshot) = app
        .try_state::<ProgressState>()
        .and_then(|state| state.publish(message, completed))
    {
        let _ = app.emit(PROGRESS, snapshot);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_late_view_replays_the_current_phase_without_advancing_it() {
        let state = ProgressState::default();
        assert!(state.snapshot().is_none());
        let first = state
            .publish("Creating a recovery checkpoint…", false)
            .unwrap();
        assert_eq!(state.snapshot(), Some(first.clone()));
        assert_eq!(state.snapshot(), Some(first));
        let next = state
            .publish("Updating a private library copy…", false)
            .unwrap();
        assert_eq!(next.sequence, 2);
        assert!(!next.completed);
        assert_eq!(state.snapshot(), Some(next));
    }

    #[test]
    fn completion_is_an_explicit_non_authorizing_snapshot() {
        let state = ProgressState::default();
        let final_state = state
            .publish("Recovery checkpoint retained.", true)
            .unwrap();
        assert!(final_state.completed);
        let json = serde_json::to_value(final_state).unwrap();
        assert_eq!(json["sequence"], 1);
        assert_eq!(json["completed"], true);
        assert!(json.get("path").is_none());
        assert!(json.get("ready").is_none());
    }
}
