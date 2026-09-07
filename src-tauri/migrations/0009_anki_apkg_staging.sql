-- Extend the staged transfer ledger for native Anki deck packages.
--
-- Migration 3 is immutable because existing public databases record its checksum.
-- Rebuild the STRICT table so the detected_kind CHECK constraint admits the
-- additive `anki_apkg` kind while preserving every existing staged transfer row.

CREATE TABLE staged_external_imports_v4 (
    id              TEXT PRIMARY KEY CHECK(length(id) = 32),
    operation_id    TEXT NOT NULL UNIQUE CHECK(length(operation_id) = 32),
    purpose         TEXT NOT NULL CHECK(purpose IN (
                        'image', 'pdf_document', 'document',
                        'study_content', 'library_backup'
                    )),
    detected_kind   TEXT NOT NULL CHECK(detected_kind IN (
                        'png', 'jpeg', 'gif', 'webp', 'pdf',
                        'doc', 'docx', 'ppt', 'pptx', 'odt', 'odp',
                        'rtf', 'epub', 'excel', 'ods', 'csv',
                        'studydeck', 'anki_apkg', 'generated_json', 'ostudybackup'
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

INSERT INTO staged_external_imports_v4 (
    id, operation_id, purpose, detected_kind, display_name, media_type,
    byte_size, sha256, relative_path, state, result_json, created_at,
    updated_at, expires_at, consumed_at, released_at
)
SELECT
    id, operation_id, purpose, detected_kind, display_name, media_type,
    byte_size, sha256, relative_path, state, result_json, created_at,
    updated_at, expires_at, consumed_at, released_at
FROM staged_external_imports;

DROP INDEX staged_external_imports_state_expiry_idx;
DROP TABLE staged_external_imports;
ALTER TABLE staged_external_imports_v4 RENAME TO staged_external_imports;

CREATE INDEX staged_external_imports_state_expiry_idx
ON staged_external_imports(state, expires_at);
