-- Trusted-device synchronization metadata.
--
-- Device pairing secrets remain in the platform secure store. SQLite keeps only
-- peer identities, endpoint hints, merge baselines, and explicit conflicts.
-- Application data continues to use its existing stable entity identifiers.

CREATE TABLE device_sync_identity (
    singleton_id   INTEGER PRIMARY KEY NOT NULL CHECK(singleton_id = 1),
    device_id      TEXT NOT NULL UNIQUE CHECK(length(device_id) = 32),
    display_name   TEXT NOT NULL CHECK(length(trim(display_name)) BETWEEN 1 AND 120),
    created_at     INTEGER NOT NULL,
    updated_at     INTEGER NOT NULL
) STRICT;

CREATE TABLE device_sync_peers (
    id                 TEXT PRIMARY KEY CHECK(length(id) = 32),
    display_name       TEXT NOT NULL CHECK(length(trim(display_name)) BETWEEN 1 AND 120),
    endpoint_hint      TEXT NOT NULL DEFAULT '' CHECK(length(endpoint_hint) <= 512),
    paired_at          INTEGER NOT NULL,
    last_synced_at     INTEGER,
    removed_at         INTEGER,
    revision           INTEGER NOT NULL DEFAULT 1 CHECK(revision >= 1)
) STRICT;

CREATE INDEX device_sync_peers_active_idx
    ON device_sync_peers(removed_at, last_synced_at DESC, display_name COLLATE NOCASE);

CREATE TABLE device_sync_baselines (
    peer_id            TEXT NOT NULL
                       REFERENCES device_sync_peers(id) ON DELETE CASCADE,
    entity_table       TEXT NOT NULL CHECK(length(entity_table) BETWEEN 1 AND 80),
    entity_key         TEXT NOT NULL CHECK(length(entity_key) BETWEEN 1 AND 1024),
    payload_json       TEXT,
    payload_hash       TEXT NOT NULL CHECK(length(payload_hash) = 64),
    updated_at         INTEGER NOT NULL,
    PRIMARY KEY(peer_id, entity_table, entity_key)
) STRICT;

CREATE INDEX device_sync_baselines_table_idx
    ON device_sync_baselines(peer_id, entity_table, entity_key);

CREATE TABLE device_sync_conflicts (
    id                   TEXT PRIMARY KEY CHECK(length(id) = 32),
    peer_id              TEXT NOT NULL
                         REFERENCES device_sync_peers(id) ON DELETE CASCADE,
    entity_table         TEXT NOT NULL CHECK(length(entity_table) BETWEEN 1 AND 80),
    entity_key           TEXT NOT NULL CHECK(length(entity_key) BETWEEN 1 AND 1024),
    base_payload_json    TEXT,
    local_payload_json   TEXT,
    remote_payload_json  TEXT,
    conflicting_fields_json TEXT NOT NULL DEFAULT '[]',
    status               TEXT NOT NULL DEFAULT 'pending'
                         CHECK(status IN ('pending', 'resolved')),
    resolution           TEXT
                         CHECK(resolution IS NULL OR resolution IN (
                             'keep_local', 'keep_remote', 'merged', 'delete'
                         )),
    created_at           INTEGER NOT NULL,
    resolved_at          INTEGER,
    UNIQUE(peer_id, entity_table, entity_key, status)
) STRICT;

CREATE INDEX device_sync_conflicts_pending_idx
    ON device_sync_conflicts(status, created_at DESC);

CREATE TABLE device_sync_sessions (
    id                TEXT PRIMARY KEY CHECK(length(id) = 32),
    peer_id           TEXT REFERENCES device_sync_peers(id) ON DELETE SET NULL,
    direction         TEXT NOT NULL CHECK(direction IN ('host', 'join')),
    state             TEXT NOT NULL CHECK(state IN (
                          'waiting', 'transferring', 'merging',
                          'completed', 'failed', 'expired', 'canceled'
                      )),
    endpoint_hint     TEXT NOT NULL DEFAULT '' CHECK(length(endpoint_hint) <= 512),
    created_at        INTEGER NOT NULL,
    expires_at        INTEGER NOT NULL,
    completed_at      INTEGER,
    error_code        TEXT,
    error_message     TEXT,
    CHECK(expires_at >= created_at)
) STRICT;

CREATE INDEX device_sync_sessions_state_idx
    ON device_sync_sessions(state, created_at DESC);
