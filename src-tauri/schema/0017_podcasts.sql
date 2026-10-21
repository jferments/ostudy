-- Release migration 17: podcasts.
--
-- A podcast is two hosts talking through some of a subject's lessons. The AI
-- writes its script once from the lessons' saved revisions; the native owner
-- validates it before it is kept and again whenever it is read. The script
-- holds a chapter for each lesson it covers, with the lesson's identity and
-- revision, and every turn the hosts say, so it plays on after a lesson is
-- rewritten or deleted. Each host speaks in the voice chosen when the
-- podcast was made, with the speech model it was made for; kept audio is
-- keyed by voice like lesson narration. Deleting the subject deletes its
-- podcasts. The upgrade creates the table and writes no rows.
CREATE TABLE podcasts (
    id               TEXT PRIMARY KEY
                     CHECK(length(id) = 32 AND id NOT GLOB '*[^0-9a-f]*'),
    subject_id       TEXT NOT NULL REFERENCES document_collections(id) ON DELETE CASCADE
                     CHECK(length(subject_id) = 32),
    title            TEXT NOT NULL CHECK(length(trim(title)) BETWEEN 1 AND 160),
    target_minutes   INTEGER NOT NULL CHECK(target_minutes BETWEEN 5 AND 30),
    model_id         TEXT NOT NULL CHECK(length(model_id) BETWEEN 1 AND 200),
    first_voice      TEXT NOT NULL CHECK(length(first_voice) <= 100),
    second_voice     TEXT NOT NULL CHECK(length(second_voice) <= 100),
    script_json      TEXT NOT NULL CHECK(
                         json_valid(script_json)
                         AND json_type(script_json) = 'object'
                         AND length(script_json) BETWEEN 2 AND 524288
                     ),
    provenance_json  TEXT NOT NULL CHECK(
                         json_valid(provenance_json) AND length(provenance_json) <= 65536
                     ),
    created_at       INTEGER NOT NULL CHECK(created_at >= 0)
) STRICT;

CREATE INDEX podcasts_by_subject ON podcasts(subject_id, created_at);
