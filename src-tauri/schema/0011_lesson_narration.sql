-- Release migration 11: lesson narration.
--
-- Spoken narration is made once and kept. A lesson step's narration is cut
-- into segments of sentences; each segment's audio is keyed by its exact
-- spoken text, the speech engine, model and voice, and the audio format, so
-- changing the voice makes only the segments that voice is missing and
-- switching back reuses the old ones. The audio bytes are ordinary managed
-- assets; a segment row keeps its asset referenced. Paid audio is never
-- evicted like a cache.
--
-- The speech setting is a single row: the AI connection that pays for cloud
-- speech, and the model and voice to use. The native owner validates every
-- value before it is written. The upgrade creates the tables and writes no
-- rows.
--
-- A lesson can also be saved as a lecture deck (.pptx), so prepared exports
-- gain the purpose 'lecture_pptx'. SQLite cannot change a CHECK in place, so
-- the table is rebuilt with every row and column kept in order. Nothing
-- references prepared_exports, and the rebuild runs in the migration's own
-- transaction.
CREATE TABLE speech_settings (
    id          INTEGER PRIMARY KEY CHECK(id = 1),
    profile_id  TEXT REFERENCES ai_provider_profiles(id) ON DELETE SET NULL,
    model_id    TEXT NOT NULL CHECK(length(model_id) BETWEEN 1 AND 200),
    voice       TEXT NOT NULL CHECK(length(voice) <= 100),
    revision    INTEGER NOT NULL CHECK(revision >= 1),
    updated_at  INTEGER NOT NULL CHECK(updated_at >= 0)
) STRICT;

CREATE TABLE speech_segments (
    id           TEXT PRIMARY KEY
                 CHECK(length(id) = 32 AND id NOT GLOB '*[^0-9a-f]*'),
    text_sha256  TEXT NOT NULL
                 CHECK(length(text_sha256) = 64 AND text_sha256 NOT GLOB '*[^0-9a-f]*'),
    engine       TEXT NOT NULL CHECK(length(engine) BETWEEN 1 AND 40),
    model_id     TEXT NOT NULL CHECK(length(model_id) BETWEEN 1 AND 200),
    voice        TEXT NOT NULL CHECK(length(voice) <= 100),
    format       TEXT NOT NULL CHECK(length(format) BETWEEN 1 AND 16),
    asset_id     TEXT NOT NULL UNIQUE REFERENCES assets(id),
    characters   INTEGER NOT NULL CHECK(characters > 0),
    created_at   INTEGER NOT NULL CHECK(created_at >= 0),
    UNIQUE(text_sha256, engine, model_id, voice, format)
) STRICT;

CREATE TABLE prepared_exports_v11 (
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

INSERT INTO prepared_exports_v11 SELECT * FROM prepared_exports;
DROP TABLE prepared_exports;
ALTER TABLE prepared_exports_v11 RENAME TO prepared_exports;
CREATE INDEX prepared_exports_state_expiry_idx
    ON prepared_exports(state, expires_at);
