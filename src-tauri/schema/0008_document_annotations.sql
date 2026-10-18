-- What a learner draws or types over a document's rendered pages: ink and
-- highlighter strokes, lines, arrows, boxes, ellipses and text notes. The
-- document file is never rewritten. Geometry is in PDF points from the top-left
-- corner of the page as rendered, and the native owner validates every payload
-- before it is written.
CREATE TABLE document_annotations (
    id            TEXT PRIMARY KEY
                  CHECK(length(id) = 32 AND id NOT GLOB '*[^0-9a-f]*'),
    document_id   TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE
                  CHECK(length(document_id) = 32),
    page_number   INTEGER NOT NULL CHECK(page_number >= 1),
    kind          TEXT NOT NULL CHECK(kind IN (
                      'ink', 'highlight', 'line', 'arrow', 'rectangle', 'ellipse', 'text'
                  )),
    payload_json  TEXT NOT NULL CHECK(
                      json_valid(payload_json) AND length(payload_json) BETWEEN 2 AND 262144
                  ),
    revision      INTEGER NOT NULL CHECK(revision >= 1),
    created_at    INTEGER NOT NULL CHECK(created_at >= 0),
    updated_at    INTEGER NOT NULL CHECK(updated_at >= created_at)
) STRICT;

CREATE INDEX document_annotations_by_page
    ON document_annotations(document_id, page_number, created_at, id);
