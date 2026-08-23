-- Extend prepared exports to include local study-guide PDF artifacts and
-- remove the restrictive self-reference that could block document-owned topic
-- cascades during permanent document deletion.
--
-- Migrations 0001-0004 intentionally remain immutable.

CREATE TABLE document_learning_plan_topics_v5 (
    plan_id                     TEXT NOT NULL REFERENCES document_learning_plans(id) ON DELETE CASCADE,
    topic_id                    TEXT NOT NULL REFERENCES document_learning_topics(id) ON DELETE CASCADE,
    parent_topic_id             TEXT REFERENCES document_learning_topics(id) ON DELETE SET NULL,
    sort_index                  INTEGER NOT NULL CHECK(sort_index >= 0),
    title                       TEXT NOT NULL CHECK(length(trim(title)) BETWEEN 1 AND 500),
    summary_text                TEXT NOT NULL CHECK(length(summary_text) <= 100000),
    learning_objectives_json    TEXT NOT NULL CHECK(length(learning_objectives_json) <= 131072),
    key_concepts_json           TEXT NOT NULL CHECK(length(key_concepts_json) <= 131072),
    source_locator_json         TEXT NOT NULL CHECK(length(source_locator_json) <= 65536),
    evidence_chunk_indices_json TEXT NOT NULL CHECK(length(evidence_chunk_indices_json) <= 131072),
    evidence_hashes_json        TEXT NOT NULL CHECK(length(evidence_hashes_json) <= 262144),
    evidence_hash               TEXT NOT NULL CHECK(length(evidence_hash) = 64),
    created_at                  INTEGER NOT NULL,
    PRIMARY KEY(plan_id, topic_id),
    UNIQUE(plan_id, sort_index)
) STRICT;

INSERT INTO document_learning_plan_topics_v5 (
    plan_id, topic_id, parent_topic_id, sort_index, title, summary_text,
    learning_objectives_json, key_concepts_json, source_locator_json,
    evidence_chunk_indices_json, evidence_hashes_json, evidence_hash, created_at
)
SELECT
    plan_id, topic_id, parent_topic_id, sort_index, title, summary_text,
    learning_objectives_json, key_concepts_json, source_locator_json,
    evidence_chunk_indices_json, evidence_hashes_json, evidence_hash, created_at
FROM document_learning_plan_topics;

DROP TABLE document_learning_plan_topics;

ALTER TABLE document_learning_plan_topics_v5 RENAME TO document_learning_plan_topics;

CREATE INDEX document_learning_plan_topics_topic_idx
ON document_learning_plan_topics(topic_id, plan_id);

CREATE TABLE prepared_exports_v5 (
    id                  TEXT PRIMARY KEY CHECK(length(id) = 32),
    operation_id        TEXT NOT NULL UNIQUE CHECK(length(operation_id) = 32),
    purpose             TEXT NOT NULL CHECK(purpose IN (
                            'studydeck', 'library_backup', 'study_guide_pdf'
                        )),
    suggested_filename  TEXT NOT NULL,
    media_type          TEXT NOT NULL,
    byte_size           INTEGER NOT NULL CHECK(byte_size > 0),
    sha256              TEXT NOT NULL CHECK(length(sha256) = 64),
    relative_path       TEXT NOT NULL UNIQUE,
    state               TEXT NOT NULL CHECK(state IN (
                            'prepared', 'published', 'released', 'failed'
                        )),
    summary_json        TEXT NOT NULL,
    created_at          INTEGER NOT NULL,
    updated_at          INTEGER NOT NULL,
    expires_at          INTEGER NOT NULL,
    published_at        INTEGER,
    released_at         INTEGER,
    CHECK(expires_at >= created_at),
    CHECK(published_at IS NULL OR published_at >= created_at),
    CHECK(released_at IS NULL OR released_at >= created_at)
) STRICT;

INSERT INTO prepared_exports_v5 (
    id, operation_id, purpose, suggested_filename, media_type, byte_size,
    sha256, relative_path, state, summary_json, created_at, updated_at,
    expires_at, published_at, released_at
)
SELECT
    id, operation_id, purpose, suggested_filename, media_type, byte_size,
    sha256, relative_path, state, summary_json, created_at, updated_at,
    expires_at, published_at, released_at
FROM prepared_exports;

DROP TABLE prepared_exports;

ALTER TABLE prepared_exports_v5 RENAME TO prepared_exports;

CREATE INDEX prepared_exports_state_expiry_idx
    ON prepared_exports(state, expires_at);
