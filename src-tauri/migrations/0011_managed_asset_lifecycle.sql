-- Add managed asset lifecycle, durable draft ownership, reconciliation logging,
-- and short-lived soft-deletion undo operations.
--
-- Historical asset IDs and relative paths remain unchanged. Existing rows are
-- promoted to the ready state so references created by older versions continue
-- to render without rewriting card documents.

CREATE TABLE asset_drafts (
    id            TEXT PRIMARY KEY CHECK(length(id) = 32),
    purpose       TEXT NOT NULL CHECK(purpose IN ('card', 'template', 'import')),
    owner_id      TEXT,
    created_at    INTEGER NOT NULL,
    updated_at    INTEGER NOT NULL,
    expires_at    INTEGER NOT NULL,
    committed_at  INTEGER,
    canceled_at   INTEGER,
    CHECK(expires_at >= created_at),
    CHECK(committed_at IS NULL OR committed_at >= created_at),
    CHECK(canceled_at IS NULL OR canceled_at >= created_at),
    CHECK(NOT (committed_at IS NOT NULL AND canceled_at IS NOT NULL))
) STRICT;

ALTER TABLE assets ADD COLUMN lifecycle_state TEXT NOT NULL DEFAULT 'ready'
    CHECK(lifecycle_state IN ('staged', 'ready', 'missing', 'quarantined', 'deleted'));
ALTER TABLE assets ADD COLUMN draft_id TEXT REFERENCES asset_drafts(id) ON DELETE SET NULL;
ALTER TABLE assets ADD COLUMN staged_relative_path TEXT;
ALTER TABLE assets ADD COLUMN quarantine_relative_path TEXT;
ALTER TABLE assets ADD COLUMN ready_at INTEGER;
ALTER TABLE assets ADD COLUMN verified_at INTEGER;
ALTER TABLE assets ADD COLUMN updated_at INTEGER NOT NULL DEFAULT 0;
ALTER TABLE assets ADD COLUMN lifecycle_reason TEXT NOT NULL DEFAULT '';

UPDATE assets
SET lifecycle_state = CASE WHEN deleted_at IS NULL THEN 'ready' ELSE 'deleted' END,
    ready_at = CASE WHEN deleted_at IS NULL THEN created_at ELSE NULL END,
    verified_at = CASE WHEN deleted_at IS NULL THEN created_at ELSE NULL END,
    updated_at = created_at;

CREATE INDEX assets_lifecycle_state_idx
    ON assets(lifecycle_state, updated_at);
CREATE INDEX assets_draft_idx
    ON assets(draft_id)
    WHERE draft_id IS NOT NULL;
CREATE UNIQUE INDEX assets_staged_path_idx
    ON assets(staged_relative_path)
    WHERE staged_relative_path IS NOT NULL;
CREATE UNIQUE INDEX assets_quarantine_path_idx
    ON assets(quarantine_relative_path)
    WHERE quarantine_relative_path IS NOT NULL;
CREATE INDEX asset_drafts_expiry_idx
    ON asset_drafts(expires_at)
    WHERE committed_at IS NULL AND canceled_at IS NULL;

CREATE TABLE asset_reconciliation_events (
    id             TEXT PRIMARY KEY CHECK(length(id) = 32),
    asset_id       TEXT REFERENCES assets(id) ON DELETE SET NULL,
    action         TEXT NOT NULL CHECK(action IN (
                       'staged', 'promoted', 'verified', 'missing',
                       'quarantined', 'restored', 'deleted', 'orphan_quarantined'
                   )),
    from_path      TEXT,
    to_path        TEXT,
    detail         TEXT NOT NULL DEFAULT '',
    occurred_at    INTEGER NOT NULL
) STRICT;

CREATE INDEX asset_reconciliation_events_asset_time_idx
    ON asset_reconciliation_events(asset_id, occurred_at DESC);

CREATE TABLE deletion_operations (
    id             TEXT PRIMARY KEY CHECK(length(id) = 32),
    entity_kind    TEXT NOT NULL CHECK(entity_kind IN ('deck', 'cards')),
    subject_label  TEXT NOT NULL,
    deck_id        TEXT,
    note_ids_json  TEXT NOT NULL,
    card_ids_json  TEXT NOT NULL,
    deleted_at     INTEGER NOT NULL,
    expires_at     INTEGER NOT NULL,
    restored_at    INTEGER,
    finalized_at   INTEGER,
    created_at     INTEGER NOT NULL,
    CHECK(expires_at >= deleted_at),
    CHECK(restored_at IS NULL OR restored_at >= deleted_at),
    CHECK(finalized_at IS NULL OR finalized_at >= deleted_at),
    CHECK(NOT (restored_at IS NOT NULL AND finalized_at IS NOT NULL))
) STRICT;

CREATE INDEX deletion_operations_expiry_idx
    ON deletion_operations(expires_at)
    WHERE restored_at IS NULL AND finalized_at IS NULL;
