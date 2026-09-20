-- Durable study targets and coherent bundle lifecycle.
--
-- A StudyTarget is a real document or collection. Collection targets never
-- invent a synthetic document. Exact source snapshots are captured per explicit
-- batch; ordinary deck/note/card state remains owned by the existing flashcard
-- graph, while collection guide revisions live here until the richer semantic
-- guide surface consumes them in LK-08.

CREATE TABLE study_targets (
    id                   TEXT PRIMARY KEY CHECK(length(id) = 32),
    target_kind          TEXT NOT NULL CHECK(target_kind IN ('document', 'collection')),
    document_id          TEXT REFERENCES documents(id) ON DELETE CASCADE,
    collection_id        TEXT REFERENCES document_collections(id) ON DELETE CASCADE,
    output_policy_json   TEXT NOT NULL DEFAULT '{"flashcards":true,"studyGuide":true}'
                         CHECK(json_valid(output_policy_json)
                               AND json_type(output_policy_json) = 'object'
                               AND length(output_policy_json) <= 16384),
    active_snapshot_id   TEXT NOT NULL DEFAULT ''
                         CHECK(active_snapshot_id = '' OR length(active_snapshot_id) = 64),
    active_snapshot_json TEXT NOT NULL DEFAULT '{}'
                         CHECK(json_valid(active_snapshot_json)
                               AND json_type(active_snapshot_json) = 'object'
                               AND length(active_snapshot_json) <= 2097152),
    deck_id              TEXT REFERENCES decks(id) ON DELETE SET NULL,
    revision             INTEGER NOT NULL DEFAULT 1 CHECK(revision >= 1),
    created_at           INTEGER NOT NULL,
    updated_at           INTEGER NOT NULL,
    CHECK(
        (target_kind = 'document' AND document_id IS NOT NULL AND collection_id IS NULL)
        OR
        (target_kind = 'collection' AND collection_id IS NOT NULL AND document_id IS NULL)
    ),
    CHECK(updated_at >= created_at)
) STRICT;

CREATE UNIQUE INDEX study_targets_document_idx
ON study_targets(document_id)
WHERE target_kind = 'document';

CREATE UNIQUE INDEX study_targets_collection_idx
ON study_targets(collection_id)
WHERE target_kind = 'collection';

CREATE INDEX study_targets_deck_idx
ON study_targets(deck_id)
WHERE deck_id IS NOT NULL;

CREATE TABLE study_target_batches (
    operation_id         TEXT PRIMARY KEY CHECK(length(operation_id) = 32),
    target_id            TEXT NOT NULL REFERENCES study_targets(id) ON DELETE CASCADE,
    request_sha256       TEXT NOT NULL CHECK(length(request_sha256) = 64),
    source_snapshot_id   TEXT NOT NULL CHECK(length(source_snapshot_id) = 64),
    source_snapshot_json TEXT NOT NULL
                         CHECK(json_valid(source_snapshot_json)
                               AND json_type(source_snapshot_json) = 'object'
                               AND length(source_snapshot_json) <= 2097152),
    output_policy_json   TEXT NOT NULL
                         CHECK(json_valid(output_policy_json)
                               AND json_type(output_policy_json) = 'object'
                               AND length(output_policy_json) <= 16384),
    provider_authorized  INTEGER NOT NULL DEFAULT 0 CHECK(provider_authorized IN (0, 1)),
    state                TEXT NOT NULL CHECK(state IN (
                             'queued', 'preparing', 'composing', 'committing',
                             'complete', 'partial', 'failed', 'canceled', 'interrupted'
                         )),
    safe_error_code      TEXT NOT NULL DEFAULT '' CHECK(length(safe_error_code) <= 80),
    safe_error_message   TEXT NOT NULL DEFAULT '' CHECK(length(safe_error_message) <= 1000),
    created_at           INTEGER NOT NULL,
    updated_at           INTEGER NOT NULL,
    completed_at         INTEGER,
    CHECK(updated_at >= created_at)
) STRICT;

CREATE INDEX study_target_batches_target_idx
ON study_target_batches(target_id, created_at DESC, operation_id);

CREATE TABLE study_target_artifact_jobs (
    batch_id           TEXT NOT NULL REFERENCES study_target_batches(operation_id) ON DELETE CASCADE,
    artifact_kind      TEXT NOT NULL CHECK(artifact_kind IN ('flashcards', 'study_guide')),
    state              TEXT NOT NULL CHECK(state IN (
                           'pending', 'running', 'committing', 'succeeded', 'failed',
                           'canceled', 'suppressed', 'interrupted'
                       )),
    artifact_id        TEXT,
    total_units        INTEGER NOT NULL DEFAULT 0 CHECK(total_units >= 0),
    completed_units    INTEGER NOT NULL DEFAULT 0 CHECK(completed_units >= 0),
    safe_error_code    TEXT NOT NULL DEFAULT '' CHECK(length(safe_error_code) <= 80),
    safe_error_message TEXT NOT NULL DEFAULT '' CHECK(length(safe_error_message) <= 1000),
    created_at         INTEGER NOT NULL,
    updated_at         INTEGER NOT NULL,
    completed_at       INTEGER,
    PRIMARY KEY(batch_id, artifact_kind),
    CHECK(completed_units <= total_units),
    CHECK(updated_at >= created_at)
) STRICT;

CREATE INDEX study_target_artifact_jobs_state_idx
ON study_target_artifact_jobs(state, updated_at DESC);

CREATE TABLE study_target_guides (
    id                      TEXT PRIMARY KEY CHECK(length(id) = 32),
    target_id               TEXT NOT NULL REFERENCES study_targets(id) ON DELETE CASCADE,
    batch_id                TEXT REFERENCES study_target_batches(operation_id) ON DELETE SET NULL,
    revision                INTEGER NOT NULL CHECK(revision >= 1),
    source_snapshot_id      TEXT NOT NULL CHECK(length(source_snapshot_id) = 64),
    synthesis_analysis_hash TEXT CHECK(synthesis_analysis_hash IS NULL OR length(synthesis_analysis_hash) = 64),
    content_json            TEXT NOT NULL
                            CHECK(json_valid(content_json)
                                  AND json_type(content_json) = 'object'
                                  AND length(content_json) <= 2097152),
    content_hash            TEXT NOT NULL CHECK(length(content_hash) = 64),
    state                   TEXT NOT NULL CHECK(state IN ('active', 'superseded')),
    created_at              INTEGER NOT NULL,
    updated_at              INTEGER NOT NULL,
    published_at            INTEGER NOT NULL,
    UNIQUE(target_id, revision),
    CHECK(updated_at >= created_at)
) STRICT;

CREATE UNIQUE INDEX study_target_guides_one_active_idx
ON study_target_guides(target_id)
WHERE state = 'active';

CREATE TABLE study_target_guide_overlays (
    id                TEXT PRIMARY KEY CHECK(length(id) = 32),
    target_id         TEXT NOT NULL REFERENCES study_targets(id) ON DELETE CASCADE,
    guide_id          TEXT REFERENCES study_target_guides(id) ON DELETE CASCADE,
    block_key         TEXT CHECK(block_key IS NULL OR length(block_key) <= 160),
    overlay_kind      TEXT NOT NULL CHECK(overlay_kind IN (
                          'guide_note', 'topic_note', 'title_override',
                          'body_override', 'highlight'
                      )),
    base_content_hash TEXT CHECK(base_content_hash IS NULL OR length(base_content_hash) = 64),
    rich_doc_json     TEXT NOT NULL CHECK(length(rich_doc_json) <= 2097152),
    plain_text        TEXT NOT NULL CHECK(length(plain_text) <= 500000),
    state             TEXT NOT NULL CHECK(state IN ('active', 'conflicted', 'discarded')),
    revision          INTEGER NOT NULL DEFAULT 1 CHECK(revision >= 1),
    created_at        INTEGER NOT NULL,
    updated_at        INTEGER NOT NULL,
    CHECK(
        (overlay_kind = 'guide_note' AND block_key IS NULL)
        OR
        (overlay_kind <> 'guide_note' AND block_key IS NOT NULL)
    )
) STRICT;

CREATE INDEX study_target_guide_overlays_scope_idx
ON study_target_guide_overlays(target_id, guide_id, block_key, state);

CREATE TABLE study_target_card_origins (
    target_id              TEXT NOT NULL REFERENCES study_targets(id) ON DELETE CASCADE,
    target_note_id         TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    source_document_id     TEXT NOT NULL CHECK(length(source_document_id) = 32),
    source_note_id         TEXT NOT NULL CHECK(length(source_note_id) = 32),
    source_note_revision   INTEGER NOT NULL CHECK(source_note_revision >= 1),
    source_identity_sha256 TEXT NOT NULL CHECK(length(source_identity_sha256) = 64),
    created_at             INTEGER NOT NULL,
    PRIMARY KEY(target_id, target_note_id),
    UNIQUE(target_id, source_note_id)
) STRICT;

CREATE INDEX study_target_card_origins_document_idx
ON study_target_card_origins(source_document_id, target_id);

CREATE TABLE study_target_suppressions (
    target_id      TEXT NOT NULL REFERENCES study_targets(id) ON DELETE CASCADE,
    artifact_kind  TEXT NOT NULL CHECK(artifact_kind IN ('flashcards', 'study_guide')),
    artifact_id    TEXT,
    operation_id   TEXT NOT NULL CHECK(length(operation_id) = 32),
    reason         TEXT NOT NULL CHECK(reason IN ('user_deleted')),
    created_at     INTEGER NOT NULL,
    PRIMARY KEY(target_id, artifact_kind)
) STRICT;

CREATE INDEX study_target_suppressions_operation_idx
ON study_target_suppressions(operation_id);
