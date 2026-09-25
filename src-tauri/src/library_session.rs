//! Admission for one live learner-library generation.
//!
//! Synchronous IPC holds a lease through dispatch. Async forwarding adapters
//! hold a second lease through the entire future, using the ORIGINAL request's
//! generation. A queued old future cannot start against a replacement library.
//! Restore closes admission before draining leases; no database/file IO occurs
//! under this mutex. The separate activity gate owns detached background work.

use std::sync::{Arc, Condvar, LazyLock, Mutex};

use serde::Serialize;
use tauri::{ipc::InvokeBody, Manager};

use crate::{database::Database, error::CommandError};

pub(crate) mod commands;
#[cfg(test)]
#[path = "library_session_tests.rs"]
mod current_format_tests;

static SESSION: LazyLock<Arc<Session>> = LazyLock::new(|| Arc::new(Session::default()));

#[derive(Default)]
struct State {
    generation: u64,
    readers: usize,
    replacing: bool,
    failed: bool,
}

#[derive(Default)]
struct Session {
    state: Mutex<State>,
    drained: Condvar,
}

#[must_use]
pub(crate) struct Lease {
    session: Arc<Session>,
}

#[must_use]
pub(crate) struct Replacement {
    session: Arc<Session>,
    switching: bool,
    finished: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SessionStatus {
    pub generation: u64,
    pub ready: bool,
    pub restoring: bool,
    pub recovery_required: bool,
    pub last_restore: Option<crate::library_backup::live::LiveRestoreResult>,
}

impl Session {
    fn enter(self: &Arc<Self>, generation: Option<u64>) -> Result<Lease, CommandError> {
        let mut state = self.state.lock().map_err(|_| unavailable())?;
        check_generation(&state, generation)?;
        if state.failed {
            return Err(unavailable());
        }
        if state.replacing {
            return Err(busy());
        }
        state.readers = state.readers.checked_add(1).ok_or_else(unavailable)?;
        Ok(Lease {
            session: Arc::clone(self),
        })
    }

    fn replace(self: &Arc<Self>, generation: Option<u64>) -> Result<Replacement, CommandError> {
        let mut state = self.state.lock().map_err(|_| unavailable())?;
        check_generation(&state, generation)?;
        if state.failed {
            return Err(unavailable());
        }
        if state.replacing {
            return Err(busy());
        }
        state.replacing = true;
        while state.readers != 0 {
            state = self.drained.wait(state).map_err(|_| unavailable())?;
        }
        Ok(Replacement {
            session: Arc::clone(self),
            switching: false,
            finished: false,
        })
    }

    fn status(&self) -> Result<SessionStatus, CommandError> {
        let state = self.state.lock().map_err(|_| unavailable())?;
        Ok(SessionStatus {
            generation: state.generation,
            ready: !state.replacing && !state.failed,
            restoring: state.replacing,
            recovery_required: state.failed,
            last_restore: None,
        })
    }
}

fn check_generation(state: &State, generation: Option<u64>) -> Result<(), CommandError> {
    // Legacy native tests and the first bundled page can address generation 0.
    // After a replacement, an unstamped request is stale, never current.
    if generation.unwrap_or(0) != state.generation {
        return Err(CommandError::new(
            "library_generation_changed",
            "The active library changed. Refresh this view; the old request was not executed.",
        ));
    }
    Ok(())
}

impl Drop for Lease {
    fn drop(&mut self) {
        if let Ok(mut state) = self.session.state.lock() {
            state.readers = state.readers.saturating_sub(1);
            if state.readers == 0 {
                self.session.drained.notify_all();
            }
        }
    }
}

impl Replacement {
    /// After the old connection closes, a panic must leave admission closed.
    pub(crate) fn switching(&mut self) {
        self.switching = true;
    }

    /// Reopen admission only after a coherent old/new connection is installed.
    pub(crate) fn finish(mut self, changed: bool) -> Result<u64, CommandError> {
        let mut state = self.session.state.lock().map_err(|_| unavailable())?;
        if changed {
            state.generation = state.generation.checked_add(1).ok_or_else(unavailable)?;
        }
        state.failed = false;
        state.replacing = false;
        self.finished = true;
        Ok(state.generation)
    }
}

impl Drop for Replacement {
    fn drop(&mut self) {
        if !self.finished {
            if let Ok(mut state) = self.session.state.lock() {
                state.replacing = false;
                state.failed |= self.switching;
            }
        }
    }
}

pub(crate) fn enter(generation: Option<u64>) -> Result<Lease, CommandError> {
    SESSION.enter(generation)
}

pub(crate) fn enter_current() -> Result<Lease, CommandError> {
    let generation = SESSION.status()?.generation;
    SESSION.enter(Some(generation))
}

pub(crate) fn begin_restore(generation: Option<u64>) -> Result<Replacement, CommandError> {
    SESSION.replace(generation)
}

#[tauri::command]
pub(crate) fn get_library_session(app: tauri::AppHandle) -> Result<SessionStatus, CommandError> {
    let mut status = SESSION.status()?;
    status.ready &= app.try_state::<Database>().is_some();
    if let Some(paths) = app.try_state::<crate::paths::AppPaths>() {
        status.last_restore = crate::library_backup::live::last_result(&paths)?;
    }
    Ok(status)
}

pub(crate) fn handler<R, F>(dispatch: F) -> impl Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync
where
    R: tauri::Runtime,
    F: Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static,
{
    move |invoke| {
        match invoke.message.command() {
            "get_library_session" | "prepare_library_restore_async" => return dispatch(invoke),
            _ => {}
        }
        let generation = match invoke.message.payload() {
            InvokeBody::Json(value) => match value.get("libraryGeneration") {
                None | Some(serde_json::Value::Null) => None,
                Some(value) => match value.as_u64() {
                    Some(value) => Some(value),
                    None => {
                        invoke.resolver.reject(CommandError::new(
                            "invalid_library_generation",
                            "The library generation must be a nonnegative integer.",
                        ));
                        return true;
                    }
                },
            },
            InvokeBody::Raw(_) => None,
        };
        let _lease = match enter(generation) {
            Ok(lease) => lease,
            Err(error) => {
                invoke.resolver.reject(error);
                return true;
            }
        };
        dispatch(invoke)
    }
}

fn busy() -> CommandError {
    CommandError::new(
        "library_restore_in_progress",
        "The library is being restored. Wait for restoration to finish before making changes.",
    )
}

fn unavailable() -> CommandError {
    CommandError::new(
        "library_recovery_required",
        "Library activation could not finish safely. Preserve the backup and recovery files and reopen oStudy for recovery. No new library commands are admitted.",
    )
}


pub(crate) fn asset_response<R: tauri::Runtime>(
    context: tauri::UriSchemeContext<'_, R>,
    request: tauri::http::Request<Vec<u8>>,
) -> tauri::http::Response<Vec<u8>> {
    use tauri::http::{header, HeaderValue, Response, StatusCode};
    let lease = enter_current();
    if lease.is_err() || context.app_handle().try_state::<Database>().is_none() {
        return Response::builder().status(StatusCode::SERVICE_UNAVAILABLE)
            .header(header::CACHE_CONTROL, "no-store")
            .body(Vec::new()).unwrap_or_else(|_| Response::new(Vec::new()));
    }
    let _lease = lease;
    let mut response = crate::asset_protocol::respond(context, request);
    response.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}
