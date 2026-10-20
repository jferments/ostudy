-- Release migration 13: lecture exports.
--
-- A lesson can be saved as a lecture slide deck (.pptx), so prepared exports
-- gain the purpose 'lecture_pptx'. SQLite cannot change a CHECK in place, so
-- the table is rebuilt with every row and column kept in order and its index
-- recreated. Nothing references prepared_exports, and the rebuild runs in the
-- migration's own transaction. The upgrade writes no new rows.
CREATE TABLE prepared_exports_v13 (
    id                  TEXT PRIMARY KEY CHECK(length(id) = 32),
    operation_id        TEXT NOT NULL UNIQUE CHECK(length(operation_id) = 32),
    purpose             TEXT NOT NULL CHECK(purpose IN (
                            'studydeck', 'library_backup', 'study_guide_pdf',
                            'lecture_pptx'
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

INSERT INTO prepared_exports_v13 SELECT * FROM prepared_exports;
DROP TABLE prepared_exports;
ALTER TABLE prepared_exports_v13 RENAME TO prepared_exports;
CREATE INDEX prepared_exports_state_expiry_idx
    ON prepared_exports(state, expires_at);
