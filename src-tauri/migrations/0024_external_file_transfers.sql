-- Opaque external-file acquisition and prepared export publication.
--
-- Native platform references are never durable application values. Selected
-- bytes are copied into app-private staging and exports are prepared privately
-- before the operating system is asked where to publish them.

CREATE TABLE staged_external_imports (
    id              TEXT PRIMARY KEY CHECK(length(id) = 32),
    operation_id    TEXT NOT NULL UNIQUE CHECK(length(operation_id) = 32),
    purpose         TEXT NOT NULL CHECK(purpose IN (
                        'image', 'pdf_document', 'study_content', 'library_backup'
                    )),
    detected_kind   TEXT NOT NULL CHECK(detected_kind IN (
                        'png', 'jpeg', 'gif', 'webp', 'pdf',
                        'studydeck', 'generated_json', 'ostudybackup'
                    )),
    display_name    TEXT NOT NULL,
    media_type      TEXT NOT NULL,
    byte_size       INTEGER NOT NULL CHECK(byte_size > 0),
    sha256          TEXT NOT NULL CHECK(length(sha256) = 64),
    relative_path   TEXT NOT NULL UNIQUE,
    state           TEXT NOT NULL CHECK(state IN (
                        'staging', 'staged', 'consumed', 'released', 'failed'
                    )),
    result_json     TEXT NOT NULL DEFAULT '',
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL,
    expires_at      INTEGER NOT NULL,
    consumed_at     INTEGER,
    released_at     INTEGER,
    CHECK(expires_at >= created_at),
    CHECK(consumed_at IS NULL OR consumed_at >= created_at),
    CHECK(released_at IS NULL OR released_at >= created_at)
) STRICT;

CREATE INDEX staged_external_imports_state_expiry_idx
    ON staged_external_imports(state, expires_at);

CREATE TABLE prepared_exports (
    id                  TEXT PRIMARY KEY CHECK(length(id) = 32),
    operation_id        TEXT NOT NULL UNIQUE CHECK(length(operation_id) = 32),
    purpose             TEXT NOT NULL CHECK(purpose IN ('studydeck', 'library_backup')),
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

CREATE INDEX prepared_exports_state_expiry_idx
    ON prepared_exports(state, expires_at);
