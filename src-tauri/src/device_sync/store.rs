use std::{collections::HashMap, path::PathBuf, time::Duration};

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::{error::CommandError, paths::AppPaths, util::unix_time_millis};

const STORE_SCHEMA: &str = r#"
PRAGMA foreign_keys = ON;
CREATE TABLE IF NOT EXISTS device_identity (
    singleton_id       INTEGER PRIMARY KEY NOT NULL CHECK(singleton_id = 1),
    device_id          TEXT NOT NULL UNIQUE CHECK(length(device_id) = 32),
    display_name       TEXT NOT NULL CHECK(length(trim(display_name)) BETWEEN 1 AND 120),
    public_key_base64  TEXT NOT NULL DEFAULT '' CHECK(length(public_key_base64) <= 512),
    created_at         INTEGER NOT NULL,
    updated_at         INTEGER NOT NULL
) STRICT;
CREATE TABLE IF NOT EXISTS peers (
    id                 TEXT PRIMARY KEY CHECK(length(id) = 32),
    display_name       TEXT NOT NULL CHECK(length(trim(display_name)) BETWEEN 1 AND 120),
    public_key_base64  TEXT NOT NULL CHECK(length(public_key_base64) BETWEEN 1 AND 512),
    fingerprint        TEXT NOT NULL CHECK(length(fingerprint) = 64),
    endpoint_hint      TEXT NOT NULL DEFAULT '' CHECK(length(endpoint_hint) <= 512),
    paired_at          INTEGER NOT NULL,
    last_synced_at     INTEGER,
    revision           INTEGER NOT NULL DEFAULT 1 CHECK(revision >= 1)
) STRICT;
CREATE INDEX IF NOT EXISTS peers_last_sync_idx
    ON peers(last_synced_at DESC, display_name COLLATE NOCASE);
CREATE TABLE IF NOT EXISTS baselines (
    peer_id       TEXT NOT NULL REFERENCES peers(id) ON DELETE CASCADE,
    table_name    TEXT NOT NULL CHECK(length(table_name) BETWEEN 1 AND 80),
    row_key       TEXT NOT NULL CHECK(length(row_key) BETWEEN 1 AND 4096),
    payload_json  TEXT,
    payload_hash  TEXT NOT NULL CHECK(length(payload_hash) = 64),
    updated_at    INTEGER NOT NULL,
    PRIMARY KEY(peer_id, table_name, row_key)
) STRICT;
CREATE TABLE IF NOT EXISTS conflicts (
    id                       TEXT PRIMARY KEY CHECK(length(id) = 32),
    peer_id                  TEXT NOT NULL REFERENCES peers(id) ON DELETE CASCADE,
    table_name               TEXT NOT NULL CHECK(length(table_name) BETWEEN 1 AND 80),
    row_key                  TEXT NOT NULL CHECK(length(row_key) BETWEEN 1 AND 4096),
    left_device_id           TEXT NOT NULL CHECK(length(left_device_id) = 32),
    left_payload_json        TEXT,
    right_device_id          TEXT NOT NULL CHECK(length(right_device_id) = 32),
    right_payload_json       TEXT,
    selected_payload_json    TEXT,
    conflicting_fields_json  TEXT NOT NULL,
    status                   TEXT NOT NULL DEFAULT 'pending'
                             CHECK(status IN ('pending', 'resolved')),
    resolution               TEXT CHECK(resolution IS NULL OR resolution IN (
                                 'keep_left', 'keep_right', 'merged', 'delete'
                             )),
    created_at               INTEGER NOT NULL,
    resolved_at              INTEGER
) STRICT;
CREATE UNIQUE INDEX IF NOT EXISTS conflicts_pending_identity_idx
    ON conflicts(peer_id, table_name, row_key)
    WHERE status = 'pending';
CREATE INDEX IF NOT EXISTS conflicts_status_idx
    ON conflicts(status, created_at DESC);
CREATE TABLE IF NOT EXISTS sessions (
    id             TEXT PRIMARY KEY CHECK(length(id) = 32),
    peer_id        TEXT REFERENCES peers(id) ON DELETE SET NULL,
    direction      TEXT NOT NULL CHECK(direction IN ('host', 'join')),
    state          TEXT NOT NULL CHECK(state IN (
                       'waiting', 'connecting', 'transferring', 'merging',
                       'completed', 'failed', 'expired', 'canceled'
                   )),
    endpoint_hint  TEXT NOT NULL DEFAULT '' CHECK(length(endpoint_hint) <= 512),
    created_at     INTEGER NOT NULL,
    expires_at     INTEGER NOT NULL,
    completed_at   INTEGER,
    error_code     TEXT,
    error_message  TEXT,
    CHECK(expires_at >= created_at)
) STRICT;
CREATE INDEX IF NOT EXISTS sessions_state_idx
    ON sessions(state, created_at DESC);
"#;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeviceIdentityRecord {
    pub device_id: String,
    pub display_name: String,
    pub public_key_base64: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PeerRecord {
    pub id: String,
    pub display_name: String,
    pub public_key_base64: String,
    pub fingerprint: String,
    pub endpoint_hint: String,
    pub paired_at: i64,
    pub last_synced_at: Option<i64>,
    pub revision: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ConflictRecord {
    pub id: String,
    pub peer_id: String,
    pub table_name: String,
    pub row_key: String,
    pub left_device_id: String,
    pub left_payload_json: Option<String>,
    pub right_device_id: String,
    pub right_payload_json: Option<String>,
    pub selected_payload_json: Option<String>,
    pub conflicting_fields: Vec<String>,
    pub created_at: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SessionRecord {
    pub id: String,
    pub peer_id: Option<String>,
    pub direction: String,
    pub state: String,
    pub endpoint_hint: String,
    pub created_at: i64,
    pub expires_at: i64,
    pub completed_at: Option<i64>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct BaselineRecord {
    pub payload_json: Option<String>,
    pub payload_hash: String,
}

pub(crate) struct NewConflict<'a> {
    pub peer_id: &'a str,
    pub table_name: &'a str,
    pub row_key: &'a str,
    pub left_device_id: &'a str,
    pub left_payload_json: Option<&'a str>,
    pub right_device_id: &'a str,
    pub right_payload_json: Option<&'a str>,
    pub selected_payload_json: Option<&'a str>,
    pub conflicting_fields: &'a [String],
}

pub(crate) struct SyncStore {
    path: PathBuf,
}

impl SyncStore {
    pub(crate) fn open(paths: &AppPaths) -> Result<Self, CommandError> {
        let path = paths.app_data_dir().join("device-sync.sqlite3");
        let store = Self { path };
        store.with_connection(|connection| {
            connection
                .execute_batch(STORE_SCHEMA)
                .map_err(CommandError::database)
        })?;
        Ok(store)
    }

    fn with_connection<T>(
        &self,
        operation: impl FnOnce(&mut Connection) -> Result<T, CommandError>,
    ) -> Result<T, CommandError> {
        let mut connection = Connection::open(&self.path).map_err(CommandError::database)?;
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(CommandError::database)?;
        connection
            .execute_batch(
                "PRAGMA foreign_keys = ON;
                 PRAGMA journal_mode = WAL;
                 PRAGMA synchronous = FULL;",
            )
            .map_err(CommandError::database)?;
        operation(&mut connection)
    }

    pub(crate) fn ensure_identity(
        &self,
        default_name: &str,
    ) -> Result<DeviceIdentityRecord, CommandError> {
        let name = validate_display_name(default_name)?;
        self.with_connection(|connection| {
            if let Some(identity) = load_identity(connection)? {
                return Ok(identity);
            }
            let device_id: String = connection
                .query_row("SELECT lower(hex(randomblob(16)))", [], |row| row.get(0))
                .map_err(CommandError::database)?;
            let now = unix_time_millis()?;
            connection
                .execute(
                    "INSERT INTO device_identity (
                         singleton_id, device_id, display_name,
                         public_key_base64, created_at, updated_at
                     ) VALUES (1, ?1, ?2, '', ?3, ?3)",
                    params![device_id, name, now],
                )
                .map_err(CommandError::database)?;
            load_identity(connection)?
                .ok_or_else(|| store_error("The device identity was not saved."))
        })
    }

    pub(crate) fn set_identity_name(
        &self,
        display_name: &str,
    ) -> Result<DeviceIdentityRecord, CommandError> {
        let name = validate_display_name(display_name)?;
        self.with_connection(|connection| {
            let now = unix_time_millis()?;
            let changed = connection
                .execute(
                    "UPDATE device_identity
                     SET display_name = ?1, updated_at = ?2
                     WHERE singleton_id = 1",
                    params![name, now],
                )
                .map_err(CommandError::database)?;
            if changed != 1 {
                return Err(store_error("The device identity has not been initialized."));
            }
            load_identity(connection)?.ok_or_else(|| store_error("The device identity is missing."))
        })
    }

    pub(crate) fn set_identity_public_key(
        &self,
        public_key_base64: &str,
    ) -> Result<DeviceIdentityRecord, CommandError> {
        if public_key_base64.is_empty() || public_key_base64.len() > 512 {
            return Err(store_error("The device public key is invalid."));
        }
        self.with_connection(|connection| {
            let current = load_identity(connection)?
                .ok_or_else(|| store_error("The device identity has not been initialized."))?;
            if !current.public_key_base64.is_empty()
                && current.public_key_base64 != public_key_base64
            {
                return Err(CommandError::new(
                    "device_sync_identity_mismatch",
                    "The secure device identity does not match the saved public identity.",
                ));
            }
            let now = unix_time_millis()?;
            connection
                .execute(
                    "UPDATE device_identity
                     SET public_key_base64 = ?1, updated_at = ?2
                     WHERE singleton_id = 1",
                    params![public_key_base64, now],
                )
                .map_err(CommandError::database)?;
            load_identity(connection)?.ok_or_else(|| store_error("The device identity is missing."))
        })
    }

    pub(crate) fn identity(&self) -> Result<Option<DeviceIdentityRecord>, CommandError> {
        self.with_connection(|connection| load_identity(connection))
    }

    pub(crate) fn list_peers(&self) -> Result<Vec<PeerRecord>, CommandError> {
        self.with_connection(|connection| {
            let mut statement = connection
                .prepare(
                    "SELECT id, display_name, public_key_base64, fingerprint,
                            endpoint_hint, paired_at, last_synced_at, revision
                     FROM peers
                     ORDER BY COALESCE(last_synced_at, paired_at) DESC,
                              display_name COLLATE NOCASE",
                )
                .map_err(CommandError::database)?;
            let peers = statement
                .query_map([], peer_from_row)
                .map_err(CommandError::database)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(CommandError::database)?;
            Ok(peers)
        })
    }

    pub(crate) fn peer(&self, peer_id: &str) -> Result<Option<PeerRecord>, CommandError> {
        self.with_connection(|connection| {
            connection
                .query_row(
                    "SELECT id, display_name, public_key_base64, fingerprint,
                            endpoint_hint, paired_at, last_synced_at, revision
                     FROM peers WHERE id = ?1",
                    [peer_id],
                    peer_from_row,
                )
                .optional()
                .map_err(CommandError::database)
        })
    }

    pub(crate) fn upsert_peer(
        &self,
        peer_id: &str,
        display_name: &str,
        public_key_base64: &str,
        fingerprint: &str,
        endpoint_hint: &str,
    ) -> Result<PeerRecord, CommandError> {
        crate::util::validate_entity_id(peer_id, "paired device")?;
        let name = validate_display_name(display_name)?;
        if public_key_base64.is_empty() || public_key_base64.len() > 512 || fingerprint.len() != 64
        {
            return Err(store_error("The paired device identity is invalid."));
        }
        if endpoint_hint.len() > 512 {
            return Err(store_error("The paired device endpoint is too long."));
        }
        self.with_connection(|connection| {
            let existing: Option<String> = connection
                .query_row(
                    "SELECT public_key_base64 FROM peers WHERE id = ?1",
                    [peer_id],
                    |row| row.get(0),
                )
                .optional()
                .map_err(CommandError::database)?;
            if existing
                .as_deref()
                .is_some_and(|value| value != public_key_base64)
            {
                return Err(CommandError::new(
                    "device_sync_peer_key_changed",
                    "The paired device presented a different identity key.",
                ));
            }
            let now = unix_time_millis()?;
            connection
                .execute(
                    "INSERT INTO peers (
                         id, display_name, public_key_base64, fingerprint,
                         endpoint_hint, paired_at, last_synced_at, revision
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, 1)
                     ON CONFLICT(id) DO UPDATE SET
                         display_name = excluded.display_name,
                         fingerprint = excluded.fingerprint,
                         endpoint_hint = excluded.endpoint_hint,
                         revision = peers.revision + 1",
                    params![
                        peer_id,
                        name,
                        public_key_base64,
                        fingerprint,
                        endpoint_hint,
                        now
                    ],
                )
                .map_err(CommandError::database)?;
            connection
                .query_row(
                    "SELECT id, display_name, public_key_base64, fingerprint,
                            endpoint_hint, paired_at, last_synced_at, revision
                     FROM peers WHERE id = ?1",
                    [peer_id],
                    peer_from_row,
                )
                .map_err(CommandError::database)
        })
    }

    pub(crate) fn mark_peer_synced(
        &self,
        peer_id: &str,
        endpoint_hint: &str,
    ) -> Result<(), CommandError> {
        self.with_connection(|connection| {
            let now = unix_time_millis()?;
            connection
                .execute(
                    "UPDATE peers
                     SET endpoint_hint = ?1, last_synced_at = ?2, revision = revision + 1
                     WHERE id = ?3",
                    params![endpoint_hint, now, peer_id],
                )
                .map_err(CommandError::database)?;
            Ok(())
        })
    }

    pub(crate) fn remove_peer(&self, peer_id: &str) -> Result<(), CommandError> {
        crate::util::validate_entity_id(peer_id, "paired device")?;
        self.with_connection(|connection| {
            let changed = connection
                .execute("DELETE FROM peers WHERE id = ?1", [peer_id])
                .map_err(CommandError::database)?;
            if changed != 1 {
                return Err(CommandError::new(
                    "device_sync_peer_not_found",
                    "That paired device no longer exists.",
                ));
            }
            Ok(())
        })
    }

    pub(crate) fn load_baselines(
        &self,
        peer_id: &str,
        table_name: &str,
    ) -> Result<HashMap<String, BaselineRecord>, CommandError> {
        self.with_connection(|connection| {
            let mut statement = connection
                .prepare(
                    "SELECT row_key, payload_json, payload_hash
                     FROM baselines
                     WHERE peer_id = ?1 AND table_name = ?2",
                )
                .map_err(CommandError::database)?;
            let rows = statement
                .query_map(params![peer_id, table_name], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        BaselineRecord {
                            payload_json: row.get(1)?,
                            payload_hash: row.get(2)?,
                        },
                    ))
                })
                .map_err(CommandError::database)?;
            let mut output = HashMap::new();
            for row in rows {
                let (key, baseline) = row.map_err(CommandError::database)?;
                output.insert(key, baseline);
            }
            Ok(output)
        })
    }

    pub(crate) fn replace_table_baselines(
        &self,
        peer_id: &str,
        table_name: &str,
        rows: &[(String, Option<String>, String)],
    ) -> Result<(), CommandError> {
        self.with_connection(|connection| {
            let transaction = connection.transaction().map_err(CommandError::database)?;
            transaction
                .execute(
                    "DELETE FROM baselines WHERE peer_id = ?1 AND table_name = ?2",
                    params![peer_id, table_name],
                )
                .map_err(CommandError::database)?;
            let now = unix_time_millis()?;
            for (row_key, payload_json, payload_hash) in rows {
                transaction
                    .execute(
                        "INSERT INTO baselines (
                             peer_id, table_name, row_key,
                             payload_json, payload_hash, updated_at
                         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                        params![
                            peer_id,
                            table_name,
                            row_key,
                            payload_json,
                            payload_hash,
                            now
                        ],
                    )
                    .map_err(CommandError::database)?;
            }
            transaction.commit().map_err(CommandError::database)
        })
    }

    pub(crate) fn record_conflict(
        &self,
        conflict: NewConflict<'_>,
    ) -> Result<String, CommandError> {
        self.with_connection(|connection| {
            let existing: Option<String> = connection
                .query_row(
                    "SELECT id FROM conflicts
                     WHERE peer_id = ?1 AND table_name = ?2 AND row_key = ?3
                       AND status = 'pending'",
                    params![conflict.peer_id, conflict.table_name, conflict.row_key],
                    |row| row.get(0),
                )
                .optional()
                .map_err(CommandError::database)?;
            let fields = serde_json::to_string(conflict.conflicting_fields)
                .map_err(CommandError::serialization)?;
            let now = unix_time_millis()?;
            if let Some(id) = existing {
                connection
                    .execute(
                        "UPDATE conflicts SET
                             left_device_id = ?1, left_payload_json = ?2,
                             right_device_id = ?3, right_payload_json = ?4,
                             selected_payload_json = ?5,
                             conflicting_fields_json = ?6, created_at = ?7
                         WHERE id = ?8",
                        params![
                            conflict.left_device_id,
                            conflict.left_payload_json,
                            conflict.right_device_id,
                            conflict.right_payload_json,
                            conflict.selected_payload_json,
                            fields,
                            now,
                            id,
                        ],
                    )
                    .map_err(CommandError::database)?;
                return Ok(id);
            }
            let id: String = connection
                .query_row("SELECT lower(hex(randomblob(16)))", [], |row| row.get(0))
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "INSERT INTO conflicts (
                         id, peer_id, table_name, row_key,
                         left_device_id, left_payload_json,
                         right_device_id, right_payload_json,
                         selected_payload_json, conflicting_fields_json,
                         status, created_at
                     ) VALUES (
                         ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8,
                         ?9, ?10, 'pending', ?11
                     )",
                    params![
                        id,
                        conflict.peer_id,
                        conflict.table_name,
                        conflict.row_key,
                        conflict.left_device_id,
                        conflict.left_payload_json,
                        conflict.right_device_id,
                        conflict.right_payload_json,
                        conflict.selected_payload_json,
                        fields,
                        now,
                    ],
                )
                .map_err(CommandError::database)?;
            Ok(id)
        })
    }

    pub(crate) fn list_conflicts(&self) -> Result<Vec<ConflictRecord>, CommandError> {
        self.with_connection(|connection| {
            let mut statement = connection
                .prepare(
                    "SELECT id, peer_id, table_name, row_key,
                            left_device_id, left_payload_json,
                            right_device_id, right_payload_json,
                            selected_payload_json, conflicting_fields_json,
                            created_at
                     FROM conflicts
                     WHERE status = 'pending'
                     ORDER BY created_at DESC",
                )
                .map_err(CommandError::database)?;
            let rows = statement
                .query_map([], |row| {
                    let fields_json: String = row.get(9)?;
                    let fields = serde_json::from_str(&fields_json).unwrap_or_default();
                    Ok(ConflictRecord {
                        id: row.get(0)?,
                        peer_id: row.get(1)?,
                        table_name: row.get(2)?,
                        row_key: row.get(3)?,
                        left_device_id: row.get(4)?,
                        left_payload_json: row.get(5)?,
                        right_device_id: row.get(6)?,
                        right_payload_json: row.get(7)?,
                        selected_payload_json: row.get(8)?,
                        conflicting_fields: fields,
                        created_at: row.get(10)?,
                    })
                })
                .map_err(CommandError::database)?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(CommandError::database)
        })
    }

    pub(crate) fn conflict(&self, conflict_id: &str) -> Result<ConflictRecord, CommandError> {
        crate::util::validate_entity_id(conflict_id, "sync conflict")?;
        self.with_connection(|connection| {
            connection
                .query_row(
                    "SELECT id, peer_id, table_name, row_key,
                            left_device_id, left_payload_json,
                            right_device_id, right_payload_json,
                            selected_payload_json, conflicting_fields_json,
                            created_at
                     FROM conflicts WHERE id = ?1 AND status = 'pending'",
                    [conflict_id],
                    |row| {
                        let fields_json: String = row.get(9)?;
                        let fields = serde_json::from_str(&fields_json).unwrap_or_default();
                        Ok(ConflictRecord {
                            id: row.get(0)?,
                            peer_id: row.get(1)?,
                            table_name: row.get(2)?,
                            row_key: row.get(3)?,
                            left_device_id: row.get(4)?,
                            left_payload_json: row.get(5)?,
                            right_device_id: row.get(6)?,
                            right_payload_json: row.get(7)?,
                            selected_payload_json: row.get(8)?,
                            conflicting_fields: fields,
                            created_at: row.get(10)?,
                        })
                    },
                )
                .optional()
                .map_err(CommandError::database)?
                .ok_or_else(|| {
                    CommandError::new(
                        "device_sync_conflict_not_found",
                        "That synchronization conflict no longer exists.",
                    )
                })
        })
    }

    pub(crate) fn resolve_conflict(
        &self,
        conflict_id: &str,
        resolution: &str,
    ) -> Result<(), CommandError> {
        self.with_connection(|connection| {
            let now = unix_time_millis()?;
            let changed = connection
                .execute(
                    "UPDATE conflicts
                     SET status = 'resolved', resolution = ?1, resolved_at = ?2
                     WHERE id = ?3 AND status = 'pending'",
                    params![resolution, now, conflict_id],
                )
                .map_err(CommandError::database)?;
            if changed != 1 {
                return Err(CommandError::new(
                    "device_sync_conflict_not_found",
                    "That synchronization conflict no longer exists.",
                ));
            }
            Ok(())
        })
    }

    pub(crate) fn create_session(
        &self,
        id: &str,
        direction: &str,
        endpoint_hint: &str,
        expires_at: i64,
    ) -> Result<(), CommandError> {
        crate::util::validate_entity_id(id, "sync session")?;
        self.with_connection(|connection| {
            let now = unix_time_millis()?;
            let changed = connection
                .execute(
                    "INSERT INTO sessions (
                         id, direction, state, endpoint_hint, created_at, expires_at
                     ) VALUES (?1, ?2, 'waiting', ?3, ?4, ?5)
                     ON CONFLICT(id) DO UPDATE SET
                         peer_id = NULL,
                         direction = excluded.direction,
                         state = 'waiting',
                         endpoint_hint = excluded.endpoint_hint,
                         created_at = excluded.created_at,
                         expires_at = excluded.expires_at,
                         completed_at = NULL,
                         error_code = NULL,
                         error_message = NULL
                     WHERE excluded.direction = 'join'
                       AND sessions.direction = 'join'
                       AND sessions.state IN ('failed', 'canceled')",
                    params![id, direction, endpoint_hint, now, expires_at],
                )
                .map_err(CommandError::database)?;
            if changed == 1 {
                return Ok(());
            }

            let existing: Option<(String, String)> = connection
                .query_row(
                    "SELECT direction, state FROM sessions WHERE id = ?1",
                    [id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()
                .map_err(CommandError::database)?;
            match existing {
                Some((existing_direction, existing_state))
                    if direction == "join"
                        && existing_direction == "join"
                        && matches!(
                            existing_state.as_str(),
                            "waiting" | "connecting" | "transferring" | "merging"
                        ) => Err(CommandError::new(
                    "device_sync_session_already_active",
                    "This synchronization offer is already being accepted. Wait for it to finish or cancel the active request before retrying.",
                )),
                Some((existing_direction, existing_state))
                    if direction == "join"
                        && existing_direction == "join"
                        && existing_state == "completed" => Err(CommandError::new(
                    "device_sync_session_already_completed",
                    "This synchronization offer already completed on this device. Create a fresh offer before synchronizing again.",
                )),
                Some(_) => Err(CommandError::new(
                    "device_sync_session_id_collision",
                    "The synchronization offer conflicts with an existing local session. Create a fresh offer on the sending device.",
                )),
                None => Err(store_error("The synchronization session could not be saved.")),
            }
        })
    }

    pub(crate) fn update_session(
        &self,
        id: &str,
        peer_id: Option<&str>,
        state: &str,
        error: Option<(&str, &str)>,
    ) -> Result<(), CommandError> {
        self.with_connection(|connection| {
            let now = unix_time_millis()?;
            let terminal = matches!(state, "completed" | "failed" | "expired" | "canceled");
            connection
                .execute(
                    "UPDATE sessions SET
                         peer_id = COALESCE(?1, peer_id),
                         state = ?2,
                         completed_at = CASE WHEN ?3 THEN ?4 ELSE completed_at END,
                         error_code = ?5,
                         error_message = ?6
                     WHERE id = ?7",
                    params![
                        peer_id,
                        state,
                        terminal,
                        now,
                        error.map(|value| value.0),
                        error.map(|value| value.1),
                        id,
                    ],
                )
                .map_err(CommandError::database)?;
            Ok(())
        })
    }

    pub(crate) fn list_sessions(&self) -> Result<Vec<SessionRecord>, CommandError> {
        self.with_connection(|connection| {
            let mut statement = connection
                .prepare(
                    "SELECT id, peer_id, direction, state, endpoint_hint,
                            created_at, expires_at, completed_at,
                            error_code, error_message
                     FROM sessions
                     ORDER BY created_at DESC
                     LIMIT 20",
                )
                .map_err(CommandError::database)?;
            let sessions = statement
                .query_map([], |row| {
                    Ok(SessionRecord {
                        id: row.get(0)?,
                        peer_id: row.get(1)?,
                        direction: row.get(2)?,
                        state: row.get(3)?,
                        endpoint_hint: row.get(4)?,
                        created_at: row.get(5)?,
                        expires_at: row.get(6)?,
                        completed_at: row.get(7)?,
                        error_code: row.get(8)?,
                        error_message: row.get(9)?,
                    })
                })
                .map_err(CommandError::database)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(CommandError::database)?;
            Ok(sessions)
        })
    }
}

fn load_identity(connection: &Connection) -> Result<Option<DeviceIdentityRecord>, CommandError> {
    connection
        .query_row(
            "SELECT device_id, display_name, public_key_base64
             FROM device_identity WHERE singleton_id = 1",
            [],
            |row| {
                Ok(DeviceIdentityRecord {
                    device_id: row.get(0)?,
                    display_name: row.get(1)?,
                    public_key_base64: row.get(2)?,
                })
            },
        )
        .optional()
        .map_err(CommandError::database)
}

fn peer_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<PeerRecord> {
    Ok(PeerRecord {
        id: row.get(0)?,
        display_name: row.get(1)?,
        public_key_base64: row.get(2)?,
        fingerprint: row.get(3)?,
        endpoint_hint: row.get(4)?,
        paired_at: row.get(5)?,
        last_synced_at: row.get(6)?,
        revision: row.get(7)?,
    })
}

fn validate_display_name(value: &str) -> Result<String, CommandError> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.chars().count() > 120 {
        return Err(CommandError::new(
            "device_sync_name_invalid",
            "Device names must contain between 1 and 120 characters.",
        ));
    }
    if trimmed.chars().any(char::is_control) {
        return Err(CommandError::new(
            "device_sync_name_invalid",
            "Device names cannot contain control characters.",
        ));
    }
    Ok(trimmed.to_string())
}

fn store_error(message: &str) -> CommandError {
    CommandError::new("device_sync_store_error", message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_store(name: &str) -> (PathBuf, SyncStore) {
        let root = std::env::temp_dir().join(format!(
            "ostudy-sync-store-{name}-{}-{}",
            std::process::id(),
            crate::platform::now_utc_millis().unwrap_or_default()
        ));
        let paths = AppPaths::new(root.clone()).expect("paths should initialize");
        let store = SyncStore::open(&paths).expect("store should open");
        (root, store)
    }

    #[test]
    fn peer_store_round_trips_identity_peer_baseline_and_conflict() {
        let (root, store) = test_store("peer-round-trip");
        let identity = store
            .ensure_identity("Test device")
            .expect("identity should be created");
        assert_eq!(identity.device_id.len(), 32);
        store
            .set_identity_public_key("cHVibGljLWtleQ==")
            .expect("public key should save");
        let peer_id = "0123456789abcdef0123456789abcdef";
        store
            .upsert_peer(
                peer_id,
                "Peer",
                "cGVlci1rZXk=",
                &"a".repeat(64),
                "192.168.1.2:40331",
            )
            .expect("peer should save");
        store
            .replace_table_baselines(
                peer_id,
                "decks",
                &[("key".to_string(), Some("{}".to_string()), "b".repeat(64))],
            )
            .expect("baseline should save");
        assert_eq!(
            store
                .load_baselines(peer_id, "decks")
                .expect("baselines should load")
                .len(),
            1
        );
        let conflict_id = store
            .record_conflict(NewConflict {
                peer_id,
                table_name: "decks",
                row_key: "key",
                left_device_id: &identity.device_id,
                left_payload_json: Some("{}"),
                right_device_id: peer_id,
                right_payload_json: Some("{\"name\":\"remote\"}"),
                selected_payload_json: Some("{}"),
                conflicting_fields: &["name".to_string()],
            })
            .expect("conflict should save");
        assert_eq!(
            store.list_conflicts().expect("conflicts should load").len(),
            1
        );
        store
            .resolve_conflict(&conflict_id, "keep_left")
            .expect("conflict should resolve");
        assert!(store
            .list_conflicts()
            .expect("conflicts should load")
            .is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn failed_join_offer_can_retry_without_duplicate_session_rows() {
        let (root, store) = test_store("join-retry");
        let session_id = "f".repeat(32);
        let first_endpoint = "192.168.1.10:44000";
        let retry_endpoint = "192.168.1.10:44001";
        let expires_at = unix_time_millis().expect("clock should work") + 60_000;

        store
            .create_session(&session_id, "join", first_endpoint, expires_at)
            .expect("first join session should be created");
        store
            .update_session(&session_id, None, "connecting", None)
            .expect("join session should start connecting");
        store
            .update_session(
                &session_id,
                None,
                "failed",
                Some((
                    "device_sync_connection_failed",
                    "The sender could not be reached.",
                )),
            )
            .expect("join session should record its failure");

        store
            .create_session(&session_id, "join", retry_endpoint, expires_at)
            .expect("the same invitation should be retryable after a terminal failure");
        let sessions = store.list_sessions().expect("sessions should load");
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id, session_id);
        assert_eq!(sessions[0].direction, "join");
        assert_eq!(sessions[0].state, "waiting");
        assert_eq!(sessions[0].endpoint_hint, retry_endpoint);
        assert!(sessions[0].peer_id.is_none());
        assert!(sessions[0].completed_at.is_none());
        assert!(sessions[0].error_code.is_none());
        assert!(sessions[0].error_message.is_none());

        store
            .update_session(&session_id, None, "connecting", None)
            .expect("retry should enter connecting");
        let duplicate = store
            .create_session(&session_id, "join", retry_endpoint, expires_at)
            .expect_err("an active retry must not be started twice");
        assert_eq!(duplicate.code, "device_sync_session_already_active");
        assert_eq!(store.list_sessions().expect("sessions should load").len(), 1);

        let _ = std::fs::remove_dir_all(root);
    }
}
