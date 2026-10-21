-- Release migration 16: saved voices.
--
-- A model that copies a voice from a short recording speaks every request in
-- that voice when the request carries the recording. A voice is such a
-- recording: a sample sentence the model spoke once, kept as an ordinary
-- managed audio asset with the sentence it says. Trying a voice keeps one
-- `tried` sample; the learner keeps the ones they like (`saved`). A saved
-- voice a lesson still speaks in is `removed` rather than deleted when the
-- learner removes it, so the lesson keeps its voice.
--
-- A lesson is spoken in one voice: the first time any part of it is spoken,
-- the voice in use is recorded for the lesson, and later parts use that voice
-- even after the learner chooses another. Kept audio is keyed by voice, so
-- audio already made keeps the voice it was made in. Deleting the lesson
-- deletes its record. The upgrade creates the tables and writes no rows.
CREATE TABLE speech_voices (
    id           TEXT PRIMARY KEY
                 CHECK(length(id) = 32 AND id NOT GLOB '*[^0-9a-f]*'),
    model_id     TEXT NOT NULL CHECK(length(model_id) BETWEEN 1 AND 200),
    name         TEXT NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 60),
    state        TEXT NOT NULL CHECK(state IN ('tried', 'saved', 'removed')),
    sample_text  TEXT NOT NULL CHECK(length(sample_text) BETWEEN 1 AND 1000),
    asset_id     TEXT NOT NULL UNIQUE REFERENCES assets(id),
    created_at   INTEGER NOT NULL CHECK(created_at >= 0),
    updated_at   INTEGER NOT NULL CHECK(updated_at >= 0)
) STRICT;

CREATE TABLE lesson_voices (
    lesson_id   TEXT PRIMARY KEY REFERENCES lessons(id) ON DELETE CASCADE,
    model_id    TEXT NOT NULL CHECK(length(model_id) BETWEEN 1 AND 200),
    voice       TEXT NOT NULL CHECK(length(voice) <= 100),
    created_at  INTEGER NOT NULL CHECK(created_at >= 0)
) STRICT;
