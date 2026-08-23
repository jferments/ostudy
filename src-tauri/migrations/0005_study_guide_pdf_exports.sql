-- Extend prepared exports to include local study-guide PDF artifacts.
--
-- Migration 0001 intentionally remains immutable. Rebuild the table so the
-- public CHECK constraint admits the new export purpose while preserving every
-- previously prepared deck/library export row and its lifecycle state.

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
