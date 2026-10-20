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
    /// How far the whole update is, in thousandths, once it is known.
    permille: Option<u16>,
}

#[derive(Default)]
struct ProgressState(Mutex<Option<Snapshot>>);

impl ProgressState {
    fn publish(&self, message: &str, completed: bool) -> Option<Snapshot> {
        let mut current = self.0.lock().ok()?;
        let sequence = current
            .as_ref()
            .map_or(1, |value| value.sequence.saturating_add(1));
        // A new phase keeps the overall progress reached so far.
        let permille = current.as_ref().and_then(|value| value.permille);
        let next = Snapshot {
            sequence,
            message: message.into(),
            completed,
            permille: if completed { Some(1000) } else { permille },
        };
        *current = Some(next.clone());
        Some(next)
    }

    /// Moves the current phase's overall progress; nothing before a phase.
    fn advance(&self, permille: u16) -> Option<Snapshot> {
        let mut current = self.0.lock().ok()?;
        let previous = current.as_ref()?;
        let next = Snapshot {
            sequence: previous.sequence.saturating_add(1),
            permille: Some(permille.min(1000)),
            ..previous.clone()
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

pub(super) fn advance(app: &tauri::AppHandle, permille: u16) {
    if let Some(snapshot) = app
        .try_state::<ProgressState>()
        .and_then(|state| state.advance(permille))
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
    fn progress_moves_within_a_phase_and_carries_into_the_next() {
        let state = ProgressState::default();
        assert!(state.advance(100).is_none());
        state
            .publish("Creating a recovery checkpoint…", false)
            .unwrap();
        let moved = state.advance(250).unwrap();
        assert_eq!((moved.sequence, moved.permille), (2, Some(250)));
        assert_eq!(moved.message, "Creating a recovery checkpoint…");
        let next = state
            .publish("Updating a private library copy…", false)
            .unwrap();
        assert_eq!(next.permille, Some(250));
        assert_eq!(state.advance(4000).unwrap().permille, Some(1000));
        let done = state.publish("Library updated.", true).unwrap();
        assert_eq!(done.permille, Some(1000));
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
