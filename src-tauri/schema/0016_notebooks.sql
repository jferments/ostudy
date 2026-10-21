-- Release migration 16: notebooks.
--
-- A notebook is a learner's own pages of paper, drawn and typed on with the
-- same tools as a document's annotations, and filed in a subject like a deck
-- or a document (NULL is "No subject"; deleting the subject moves its
-- notebooks there). Each page has its own paper and size in points, and its
-- position among the notebook's pages. Each mark on a page is one
-- notebook_items row whose payload the native owner validates with the same
-- rules as a document annotation; a text mark may also link to a document
-- page, a study guide, a deck or another notebook page. A link names its
-- target and is looked up when the page opens, so deleting the target never
-- rewrites a notebook. The 'image' kind is reserved for pictures placed on a
-- page, which a later build stores through the managed asset store.
--
-- The upgrade creates the tables and writes no rows.
CREATE TABLE notebooks (
    id          TEXT PRIMARY KEY
                CHECK(length(id) = 32 AND id NOT GLOB '*[^0-9a-f]*'),
    subject_id  TEXT REFERENCES document_collections(id) ON DELETE SET NULL,
    title       TEXT NOT NULL CHECK(length(trim(title)) BETWEEN 1 AND 200),
    paper       TEXT NOT NULL CHECK(paper IN ('blank', 'lined', 'grid', 'dots')),
    page_size   TEXT NOT NULL CHECK(page_size IN ('letter', 'a4', 'wide')),
    revision    INTEGER NOT NULL CHECK(revision >= 1),
    created_at  INTEGER NOT NULL CHECK(created_at >= 0),
    updated_at  INTEGER NOT NULL CHECK(updated_at >= created_at)
) STRICT;

CREATE INDEX notebooks_by_subject ON notebooks(subject_id, updated_at);

CREATE TABLE notebook_pages (
    id           TEXT PRIMARY KEY
                 CHECK(length(id) = 32 AND id NOT GLOB '*[^0-9a-f]*'),
    notebook_id  TEXT NOT NULL REFERENCES notebooks(id) ON DELETE CASCADE,
    position     INTEGER NOT NULL CHECK(position >= 1),
    paper        TEXT NOT NULL CHECK(paper IN ('blank', 'lined', 'grid', 'dots')),
    width        REAL NOT NULL CHECK(width BETWEEN 72 AND 14400),
    height       REAL NOT NULL CHECK(height BETWEEN 72 AND 14400),
    created_at   INTEGER NOT NULL CHECK(created_at >= 0),
    UNIQUE(notebook_id, position)
) STRICT;

CREATE TABLE notebook_items (
    id            TEXT PRIMARY KEY
                  CHECK(length(id) = 32 AND id NOT GLOB '*[^0-9a-f]*'),
    page_id       TEXT NOT NULL REFERENCES notebook_pages(id) ON DELETE CASCADE,
    kind          TEXT NOT NULL CHECK(kind IN (
                      'ink', 'highlight', 'line', 'arrow', 'rectangle', 'ellipse', 'text', 'image'
                  )),
    payload_json  TEXT NOT NULL CHECK(
                      json_valid(payload_json) AND length(payload_json) BETWEEN 2 AND 262144
                  ),
    revision      INTEGER NOT NULL CHECK(revision >= 1),
    created_at    INTEGER NOT NULL CHECK(created_at >= 0),
    updated_at    INTEGER NOT NULL CHECK(updated_at >= created_at)
) STRICT;

CREATE INDEX notebook_items_by_page ON notebook_items(page_id, created_at, id);
