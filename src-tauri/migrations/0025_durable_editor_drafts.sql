-- Durable, typed editor drafts with persistent managed image ownership.
--
-- Drafts are device-local recovery state. They may contain incomplete authoring
-- data, but never credentials. Canonical card/template/profile validation still
-- runs at final commit.

CREATE TABLE editor_drafts (
    id                      TEXT PRIMARY KEY CHECK(length(id) = 32),
    open_operation_id       TEXT NOT NULL UNIQUE CHECK(length(open_operation_id) = 32),
    draft_kind              TEXT NOT NULL CHECK(draft_kind IN (
                                'card', 'template', 'bulk_cards', 'ai_profile'
                            )),
    target_entity_id        TEXT,
    context_entity_id       TEXT,
    base_revision           INTEGER CHECK(base_revision IS NULL OR base_revision >= 1),
    payload_schema_version  INTEGER NOT NULL CHECK(payload_schema_version >= 1),
    payload_json            TEXT NOT NULL,
    payload_sha256          TEXT NOT NULL CHECK(length(payload_sha256) = 64),
    revision                INTEGER NOT NULL DEFAULT 1 CHECK(revision >= 1),
    state                   TEXT NOT NULL CHECK(state IN (
                                'active', 'committed', 'discarded', 'conflicted'
                            )),
    asset_draft_id          TEXT,
    created_at              INTEGER NOT NULL,
    updated_at              INTEGER NOT NULL,
    last_opened_at          INTEGER NOT NULL,
    committed_at            INTEGER,
    discarded_at            INTEGER,
    conflicted_at           INTEGER,
    CHECK(NOT (committed_at IS NOT NULL AND discarded_at IS NOT NULL)),
    CHECK(NOT (committed_at IS NOT NULL AND conflicted_at IS NOT NULL)),
    CHECK(NOT (discarded_at IS NOT NULL AND conflicted_at IS NOT NULL))
) STRICT;

CREATE UNIQUE INDEX editor_drafts_active_target_idx
    ON editor_drafts(draft_kind, target_entity_id)
    WHERE state = 'active' AND target_entity_id IS NOT NULL;

CREATE INDEX editor_drafts_active_context_idx
    ON editor_drafts(draft_kind, context_entity_id, updated_at DESC)
    WHERE state = 'active';

CREATE INDEX editor_drafts_state_updated_idx
    ON editor_drafts(state, updated_at DESC);

CREATE TABLE editor_draft_assets (
    draft_id  TEXT NOT NULL REFERENCES editor_drafts(id) ON DELETE CASCADE,
    asset_id  TEXT NOT NULL REFERENCES assets(id) ON DELETE RESTRICT,
    PRIMARY KEY(draft_id, asset_id)
) STRICT;

CREATE INDEX editor_draft_assets_asset_idx
    ON editor_draft_assets(asset_id);

ALTER TABLE asset_drafts ADD COLUMN retention_mode TEXT NOT NULL DEFAULT 'ephemeral'
    CHECK(retention_mode IN ('ephemeral', 'persistent_editor'));
ALTER TABLE asset_drafts ADD COLUMN editor_draft_id TEXT;

CREATE UNIQUE INDEX asset_drafts_editor_draft_idx
    ON asset_drafts(editor_draft_id)
    WHERE editor_draft_id IS NOT NULL;
