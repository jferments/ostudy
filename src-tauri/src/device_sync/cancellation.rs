use std::{
    collections::HashMap,
    net::{Shutdown, TcpStream},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, LazyLock, Mutex,
    },
    time::Duration,
};

use rusqlite::{params, Connection, OptionalExtension};

use crate::{error::CommandError, paths::AppPaths, util::unix_time_millis};

const ACTIVE_STATES_SQL: &str = "'waiting', 'connecting', 'transferring', 'merging'";
const CANCELLABLE_STATES_SQL: &str = "'waiting', 'connecting', 'transferring'";

#[derive(Clone, Debug)]
pub(crate) struct SyncCancellationSignal {
    inner: Arc<SignalInner>,
}

#[derive(Debug)]
struct SignalInner {
    canceled: AtomicBool,
    stream: Mutex<Option<TcpStream>>,
}

impl SyncCancellationSignal {
    fn new() -> Self {
        Self {
            inner: Arc::new(SignalInner {
                canceled: AtomicBool::new(false),
                stream: Mutex::new(None),
            }),
        }
    }

    pub(crate) fn is_canceled(&self) -> bool {
        self.inner.canceled.load(Ordering::SeqCst)
    }

    pub(crate) fn check(&self) -> Result<(), CommandError> {
        if self.is_canceled() {
            Err(canceled_error())
        } else {
            Ok(())
        }
    }

    pub(crate) fn attach_stream(&self, stream: &TcpStream) -> Result<(), CommandError> {
        let clone = stream.try_clone().map_err(CommandError::io)?;
        let mut active = self.inner.stream.lock().map_err(|_| registry_error())?;
        if self.is_canceled() {
            let _ = clone.shutdown(Shutdown::Both);
            return Err(canceled_error());
        }
        *active = Some(clone);
        Ok(())
    }

    fn cancel(&self) {
        self.inner.canceled.store(true, Ordering::SeqCst);
        if let Ok(active) = self.inner.stream.lock() {
            if let Some(stream) = active.as_ref() {
                let _ = stream.shutdown(Shutdown::Both);
            }
        }
    }
}

#[derive(Default)]
struct ActiveSyncSessions {
    sessions: Mutex<HashMap<String, SyncCancellationSignal>>,
}

impl ActiveSyncSessions {
    fn begin(&self, session_id: &str) -> Result<SyncCancellationSignal, CommandError> {
        let mut sessions = self.sessions.lock().map_err(|_| registry_error())?;
        if sessions.contains_key(session_id) {
            return Err(CommandError::new(
                "device_sync_session_already_active",
                "That synchronization session is already running.",
            ));
        }
        let signal = SyncCancellationSignal::new();
        sessions.insert(session_id.to_string(), signal.clone());
        Ok(signal)
    }

    fn finish(&self, session_id: &str) {
        if let Ok(mut sessions) = self.sessions.lock() {
            sessions.remove(session_id);
        }
    }

    fn cancel_if_active(&self, session_id: &str) {
        if let Ok(sessions) = self.sessions.lock() {
            if let Some(signal) = sessions.get(session_id) {
                signal.cancel();
            }
        }
    }
}

static ACTIVE_SYNC_SESSIONS: LazyLock<ActiveSyncSessions> =
    LazyLock::new(ActiveSyncSessions::default);

pub(crate) fn begin(session_id: &str) -> Result<SyncCancellationSignal, CommandError> {
    ACTIVE_SYNC_SESSIONS.begin(session_id)
}

pub(crate) fn finish(session_id: &str) {
    ACTIVE_SYNC_SESSIONS.finish(session_id);
}

pub(crate) fn normalize_result<T>(
    signal: &SyncCancellationSignal,
    result: Result<T, CommandError>,
) -> Result<T, CommandError> {
    if signal.is_canceled() {
        Err(canceled_error())
    } else {
        result
    }
}

pub(crate) fn cancel_session(paths: &AppPaths, session_id: &str) -> Result<(), CommandError> {
    crate::util::validate_entity_id(session_id, "sync session")?;
    let connection = open_store(paths, "cancel_session")?;
    let now = unix_time_millis()?;
    let changed = connection
        .execute(
            &format!(
                "UPDATE sessions
                 SET state = 'canceled', completed_at = ?1,
                     error_code = 'device_sync_canceled',
                     error_message = 'Synchronization was canceled by the user.'
                 WHERE id = ?2 AND state IN ({CANCELLABLE_STATES_SQL})"
            ),
            params![now, session_id],
        )
        .map_err(|error| {
            CommandError::database_at(
                "device_sync_database_error",
                format!("operation=cancel_session session_id={session_id}"),
                error,
            )
        })?;
    if changed != 1 {
        let state: Option<String> = connection
            .query_row(
                "SELECT state FROM sessions WHERE id = ?1",
                [session_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| {
                CommandError::database_at(
                    "device_sync_database_error",
                    format!("operation=cancel_session_lookup session_id={session_id}"),
                    error,
                )
            })?;
        if state.as_deref() == Some("merging") {
            return Err(CommandError::new(
                "device_sync_merge_committing",
                "The synchronization merge is already committing atomically. Let it finish, then review the result.",
            ));
        }
        return Err(CommandError::new(
            "device_sync_session_not_active",
            "That synchronization request is no longer active.",
        ));
    }
    ACTIVE_SYNC_SESSIONS.cancel_if_active(session_id);
    Ok(())
}

pub(crate) fn cancel_superseded_unclaimed_hosts(
    paths: &AppPaths,
) -> Result<Vec<String>, CommandError> {
    let mut connection = open_store(paths, "supersede_waiting_hosts")?;
    let transaction = connection.transaction().map_err(|error| {
        CommandError::database_at(
            "device_sync_database_error",
            "operation=supersede_waiting_hosts_begin",
            error,
        )
    })?;
    let mut statement = transaction
        .prepare(&format!(
            "SELECT id FROM sessions
             WHERE direction = 'host' AND peer_id IS NULL
               AND state IN ({CANCELLABLE_STATES_SQL})"
        ))
        .map_err(|error| {
            CommandError::database_at(
                "device_sync_database_error",
                "operation=supersede_waiting_hosts_query",
                error,
            )
        })?;
    let ids = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|error| {
            CommandError::database_at(
                "device_sync_database_error",
                "operation=supersede_waiting_hosts_rows",
                error,
            )
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            CommandError::database_at(
                "device_sync_database_error",
                "operation=supersede_waiting_hosts_collect",
                error,
            )
        })?;
    drop(statement);
    if !ids.is_empty() {
        let now = unix_time_millis()?;
        transaction
            .execute(
                &format!(
                    "UPDATE sessions
                     SET state = 'canceled', completed_at = ?1,
                         error_code = 'device_sync_superseded',
                         error_message = 'A newer outgoing synchronization offer replaced this waiting offer.'
                     WHERE direction = 'host' AND peer_id IS NULL
                       AND state IN ({CANCELLABLE_STATES_SQL})"
                ),
                [now],
            )
            .map_err(|error| {
                CommandError::database_at(
                    "device_sync_database_error",
                    "operation=supersede_waiting_hosts_update",
                    error,
                )
            })?;
    }
    transaction.commit().map_err(|error| {
        CommandError::database_at(
            "device_sync_database_error",
            "operation=supersede_waiting_hosts_commit",
            error,
        )
    })?;
    for id in &ids {
        ACTIVE_SYNC_SESSIONS.cancel_if_active(id);
    }
    Ok(ids)
}

pub(crate) fn ensure_peer_available(paths: &AppPaths, peer_id: &str) -> Result<(), CommandError> {
    crate::util::validate_entity_id(peer_id, "paired device")?;
    let connection = open_store(paths, "ensure_peer_available")?;
    let active: Option<String> = connection
        .query_row(
            &format!(
                "SELECT id FROM sessions
                 WHERE peer_id = ?1 AND state IN ({ACTIVE_STATES_SQL})
                 ORDER BY created_at DESC LIMIT 1"
            ),
            [peer_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| {
            CommandError::database_at(
                "device_sync_database_error",
                format!("operation=ensure_peer_available peer_id={peer_id}"),
                error,
            )
        })?;
    if active.is_some() {
        return Err(peer_busy_error());
    }
    Ok(())
}

pub(crate) fn claim_peer(
    paths: &AppPaths,
    session_id: &str,
    peer_id: &str,
) -> Result<(), CommandError> {
    crate::util::validate_entity_id(session_id, "sync session")?;
    crate::util::validate_entity_id(peer_id, "paired device")?;
    let mut connection = open_store(paths, "claim_peer")?;
    let transaction = connection.transaction().map_err(|error| {
        CommandError::database_at(
            "device_sync_database_error",
            format!("operation=claim_peer_begin session_id={session_id} peer_id={peer_id}"),
            error,
        )
    })?;
    let active: Option<String> = transaction
        .query_row(
            &format!(
                "SELECT id FROM sessions
                 WHERE peer_id = ?1 AND id != ?2
                   AND state IN ({ACTIVE_STATES_SQL})
                 LIMIT 1"
            ),
            params![peer_id, session_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| {
            CommandError::database_at(
                "device_sync_database_error",
                format!("operation=claim_peer_query session_id={session_id} peer_id={peer_id}"),
                error,
            )
        })?;
    if active.is_some() {
        return Err(peer_busy_error());
    }
    let changed = transaction
        .execute(
            &format!(
                "UPDATE sessions SET peer_id = ?1
                 WHERE id = ?2 AND state IN ({ACTIVE_STATES_SQL})"
            ),
            params![peer_id, session_id],
        )
        .map_err(|error| {
            CommandError::database_at(
                "device_sync_database_error",
                format!("operation=claim_peer_update session_id={session_id} peer_id={peer_id}"),
                error,
            )
        })?;
    if changed != 1 {
        return Err(CommandError::new(
            "device_sync_session_not_active",
            "The synchronization request ended before the peer could be attached.",
        ));
    }
    transaction.commit().map_err(|error| {
        CommandError::database_at(
            "device_sync_database_error",
            format!("operation=claim_peer_commit session_id={session_id} peer_id={peer_id}"),
            error,
        )
    })
}

pub(crate) fn finish_terminal(
    paths: &AppPaths,
    session_id: &str,
    peer_id: Option<&str>,
    state: &str,
    error: Option<(&str, &str)>,
) -> Result<(), CommandError> {
    let connection = open_store(paths, "finish_terminal")?;
    let now = unix_time_millis()?;
    connection
        .execute(
            &format!(
                "UPDATE sessions SET
                     peer_id = COALESCE(?1, peer_id), state = ?2, completed_at = ?3,
                     error_code = ?4, error_message = ?5
                 WHERE id = ?6 AND state IN ({ACTIVE_STATES_SQL})"
            ),
            params![
                peer_id,
                state,
                now,
                error.map(|value| value.0),
                error.map(|value| value.1),
                session_id,
            ],
        )
        .map_err(|database_error| {
            CommandError::database_at(
                "device_sync_database_error",
                format!("operation=finish_terminal session_id={session_id} state={state}"),
                database_error,
            )
        })?;
    Ok(())
}

pub(crate) fn reconcile_startup(paths: &AppPaths) -> Result<(), CommandError> {
    // Network sockets and in-memory cancellation handles cannot survive a
    // process restart. Persisted nonterminal rows from the previous process are
    // therefore historical interruptions, not currently active requests.
    let connection = open_store(paths, "reconcile_startup")?;
    let now = unix_time_millis()?;
    connection
        .execute(
            &format!(
                "UPDATE sessions
                 SET state = 'canceled', completed_at = ?1,
                     error_code = 'device_sync_process_interrupted',
                     error_message = 'The previous ostudy process ended before synchronization completed.'
                 WHERE state IN ({ACTIVE_STATES_SQL})"
            ),
            [now],
        )
        .map_err(|error| {
            CommandError::database_at(
                "device_sync_database_error",
                "operation=reconcile_sync_sessions",
                error,
            )
        })?;
    Ok(())
}

fn open_store(paths: &AppPaths, operation: &str) -> Result<Connection, CommandError> {
    let path = paths.app_data_dir().join("device-sync.sqlite3");
    let connection = Connection::open(&path).map_err(|error| {
        CommandError::database_at(
            "device_sync_database_error",
            format!("operation={operation} phase=open path={}", path.display()),
            error,
        )
    })?;
    connection
        .busy_timeout(Duration::from_secs(5))
        .map_err(|error| {
            CommandError::database_at(
                "device_sync_database_error",
                format!("operation={operation} phase=busy_timeout"),
                error,
            )
        })?;
    connection
        .execute_batch(
            "PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL; PRAGMA synchronous = FULL;",
        )
        .map_err(|error| {
            CommandError::database_at(
                "device_sync_database_error",
                format!("operation={operation} phase=configure"),
                error,
            )
        })?;
    Ok(connection)
}

pub(crate) fn canceled_error() -> CommandError {
    CommandError::new("device_sync_canceled", "Synchronization was canceled.")
}

fn peer_busy_error() -> CommandError {
    CommandError::new(
        "device_sync_peer_busy",
        "A synchronization request with this device is already active. Cancel or finish it before starting another one.",
    )
}

fn registry_error() -> CommandError {
    CommandError::new(
        "device_sync_registry_unavailable",
        "The synchronization task registry is unavailable. Restart ostudy and try again.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::AppPaths;

    fn test_paths(label: &str) -> AppPaths {
        let root = std::env::temp_dir().join(format!(
            "ostudy-sync-cancel-{label}-{}-{}",
            std::process::id(),
            crate::platform::now_utc_millis().unwrap_or_default()
        ));
        AppPaths::new(root).expect("paths should initialize")
    }

    fn initialize_store(paths: &AppPaths) {
        super::super::store::SyncStore::open(paths).expect("store should initialize");
    }

    #[test]
    fn newer_unclaimed_offer_supersedes_only_unclaimed_host_requests() {
        let paths = test_paths("supersede");
        initialize_store(&paths);
        let connection = open_store(&paths, "fixture").unwrap();
        let now = unix_time_millis().unwrap();
        connection
            .execute(
                "INSERT INTO sessions (id, direction, state, endpoint_hint, created_at, expires_at)
                 VALUES (?1, 'host', 'waiting', '192.168.1.2:1', ?3, ?4),
                        (?2, 'join', 'connecting', '192.168.1.3:2', ?3, ?4)",
                params!["1".repeat(32), "2".repeat(32), now, now + 60_000],
            )
            .unwrap();
        let canceled = cancel_superseded_unclaimed_hosts(&paths).unwrap();
        assert_eq!(canceled, vec!["1".repeat(32)]);
        let host_state: String = connection
            .query_row(
                "SELECT state FROM sessions WHERE id = ?1",
                ["1".repeat(32)],
                |row| row.get(0),
            )
            .unwrap();
        let join_state: String = connection
            .query_row(
                "SELECT state FROM sessions WHERE id = ?1",
                ["2".repeat(32)],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(host_state, "canceled");
        assert_eq!(join_state, "connecting");
    }

    #[test]
    fn cancel_marks_persisted_session_and_signals_running_socket() {
        let paths = test_paths("cancel");
        initialize_store(&paths);
        let session_id = "3".repeat(32);
        let now = unix_time_millis().unwrap();
        open_store(&paths, "fixture")
            .unwrap()
            .execute(
                "INSERT INTO sessions (id, direction, state, endpoint_hint, created_at, expires_at)
                 VALUES (?1, 'host', 'waiting', '192.168.1.2:1', ?2, ?3)",
                params![session_id, now, now + 60_000],
            )
            .unwrap();
        let signal = begin(&session_id).unwrap();
        cancel_session(&paths, &session_id).unwrap();
        assert!(signal.is_canceled());
        finish(&session_id);
        let state: String = open_store(&paths, "fixture")
            .unwrap()
            .query_row(
                "SELECT state FROM sessions WHERE id = ?1",
                [session_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(state, "canceled");
    }

    #[test]
    fn merging_session_rejects_cancel_to_preserve_atomic_commit_boundary() {
        let paths = test_paths("merge-cancel");
        initialize_store(&paths);
        let session_id = "7".repeat(32);
        let now = unix_time_millis().unwrap();
        open_store(&paths, "fixture")
            .unwrap()
            .execute(
                "INSERT INTO sessions (id, direction, state, endpoint_hint, created_at, expires_at)
                 VALUES (?1, 'join', 'merging', '192.168.1.2:1', ?2, ?3)",
                params![session_id, now, now + 60_000],
            )
            .unwrap();
        let error = cancel_session(&paths, &session_id).unwrap_err();
        assert_eq!(error.code, "device_sync_merge_committing");
    }

    #[test]
    fn claim_peer_rejects_overlapping_active_session_for_same_peer() {
        let paths = test_paths("peer");
        initialize_store(&paths);
        let peer_id = "4".repeat(32);
        let now = unix_time_millis().unwrap();
        let connection = open_store(&paths, "fixture").unwrap();
        connection
            .execute(
                "INSERT INTO peers (id, display_name, public_key_base64, fingerprint, endpoint_hint, paired_at, revision)
                 VALUES (?1, 'Peer', 'cGVlcg==', ?2, '', ?3, 1)",
                params![peer_id, "a".repeat(64), now],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO sessions (id, peer_id, direction, state, endpoint_hint, created_at, expires_at)
                 VALUES (?1, ?3, 'host', 'transferring', '192.168.1.2:1', ?4, ?5),
                        (?2, NULL, 'host', 'waiting', '192.168.1.2:2', ?4, ?5)",
                params!["5".repeat(32), "6".repeat(32), peer_id, now, now + 60_000],
            )
            .unwrap();
        let error = claim_peer(&paths, &"6".repeat(32), &peer_id).unwrap_err();
        assert_eq!(error.code, "device_sync_peer_busy");
    }
}
