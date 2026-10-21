-- Release migration 21: ink on lecture slides and study guide pages.
--
-- A learner writes and draws on a lesson's designed lecture slides and on a
-- document study guide's designed section pages with the annotator's tools.
-- Each mark is one material_ink row on one page of one material: a lesson
-- (page_key is the slide key) or a document's study guide (page_key is the
-- section's topic id). The payload is validated by the native owner with the
-- same rules as a document annotation. Deleting the lesson or document
-- deletes its marks.
--
-- The upgrade creates the table and writes no rows.
CREATE TABLE material_ink (
    id            TEXT PRIMARY KEY
                  CHECK(length(id) = 32 AND id NOT GLOB '*[^0-9a-f]*'),
    lesson_id     TEXT REFERENCES lessons(id) ON DELETE CASCADE,
    document_id   TEXT REFERENCES documents(id) ON DELETE CASCADE,
    page_key      TEXT NOT NULL CHECK(length(page_key) BETWEEN 1 AND 96),
    kind          TEXT NOT NULL CHECK(kind IN (
                      'ink', 'highlight', 'line', 'arrow', 'rectangle', 'ellipse', 'text'
                  )),
    payload_json  TEXT NOT NULL CHECK(
                      json_valid(payload_json) AND length(payload_json) BETWEEN 2 AND 262144
                  ),
    revision      INTEGER NOT NULL CHECK(revision >= 1),
    created_at    INTEGER NOT NULL CHECK(created_at >= 0),
    updated_at    INTEGER NOT NULL CHECK(updated_at >= created_at),
    CHECK((lesson_id IS NULL) <> (document_id IS NULL))
) STRICT;

CREATE INDEX material_ink_by_lesson ON material_ink(lesson_id, created_at, id)
    WHERE lesson_id IS NOT NULL;
CREATE INDEX material_ink_by_document ON material_ink(document_id, created_at, id)
    WHERE document_id IS NOT NULL;
