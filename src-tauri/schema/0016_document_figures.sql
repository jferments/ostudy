-- Release migration 16: the figures found in a learner's PDF documents.
--
-- document_figure_scans keeps, for each document, whether the learner chose to
-- use its images when they imported it and how the last search for figures
-- ended. document_figures keeps each figure that search found: its page, its
-- box and its caption's box in PDF points from the top-left corner of the page
-- as displayed, the page's size in points, the caption text and its label
-- ("Figure 3.2"), and whether the learner hid it. document_figure_passages keeps
-- the pages whose text explains a figure: passages that name its label, and the
-- pages around a figure that fills most of its page.
--
-- document_figure_readings keeps what the learner's AI model read in each
-- figure, seen on its page with the pages that explain it: a short title, what
-- it shows, how the text uses it, the concepts it supports, the pages that
-- explain it, how important it is (1 to 5) and whether it is worth studying,
-- with the provider and model that read it. The scan row records how the last
-- reading ended. A figure without a reading keeps its caption and passages.
--
-- Crops are not stored: like a zoomed annotator page they are drawn again from
-- the stored PDF when shown. Rows go with their document. The upgrade creates
-- the tables and writes no rows.
CREATE TABLE document_figure_scans (
    document_id  TEXT PRIMARY KEY REFERENCES documents(id) ON DELETE CASCADE,
    use_images   INTEGER NOT NULL CHECK(use_images IN (0, 1)),
    state        TEXT NOT NULL CHECK(state IN ('pending', 'found', 'unsupported', 'failed')),
    detector     TEXT CHECK(detector IS NULL OR length(detector) BETWEEN 1 AND 120),
    scanned_at   INTEGER CHECK(scanned_at IS NULL OR scanned_at >= 0),
    reading      TEXT CHECK(reading IS NULL
                     OR reading IN ('read', 'partial', 'unavailable', 'failed')),
    read_at      INTEGER CHECK(read_at IS NULL OR read_at >= 0),
    updated_at   INTEGER NOT NULL CHECK(updated_at >= 0)
) STRICT;

CREATE TABLE document_figures (
    document_id     TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    figure_id       TEXT NOT NULL CHECK(length(figure_id) BETWEEN 1 AND 32),
    ordinal         INTEGER NOT NULL CHECK(ordinal >= 0),
    page_number     INTEGER NOT NULL CHECK(page_number >= 1),
    page_width      REAL NOT NULL CHECK(page_width > 0),
    page_height     REAL NOT NULL CHECK(page_height > 0),
    kind            TEXT NOT NULL CHECK(kind IN ('image', 'drawing')),
    box_left        REAL NOT NULL CHECK(box_left >= 0),
    box_top         REAL NOT NULL CHECK(box_top >= 0),
    box_width       REAL NOT NULL CHECK(box_width > 0),
    box_height      REAL NOT NULL CHECK(box_height > 0),
    caption_left    REAL,
    caption_top     REAL,
    caption_width   REAL,
    caption_height  REAL,
    label           TEXT CHECK(label IS NULL OR length(label) BETWEEN 1 AND 64),
    caption         TEXT CHECK(caption IS NULL OR length(caption) BETWEEN 1 AND 2000),
    hidden          INTEGER NOT NULL CHECK(hidden IN (0, 1)),
    created_at      INTEGER NOT NULL CHECK(created_at >= 0),
    updated_at      INTEGER NOT NULL CHECK(updated_at >= created_at),
    PRIMARY KEY(document_id, figure_id),
    CHECK(
        (caption_left IS NULL AND caption_top IS NULL
            AND caption_width IS NULL AND caption_height IS NULL)
        OR (caption_left >= 0 AND caption_top >= 0
            AND caption_width > 0 AND caption_height > 0)
    )
) STRICT;

CREATE UNIQUE INDEX document_figures_in_order
    ON document_figures(document_id, ordinal);

CREATE TABLE document_figure_passages (
    document_id  TEXT NOT NULL,
    figure_id    TEXT NOT NULL,
    ordinal      INTEGER NOT NULL CHECK(ordinal >= 0),
    page_number  INTEGER NOT NULL CHECK(page_number >= 1),
    kind         TEXT NOT NULL CHECK(kind IN ('mention', 'neighbour')),
    excerpt      TEXT CHECK(excerpt IS NULL OR length(excerpt) BETWEEN 1 AND 600),
    PRIMARY KEY(document_id, figure_id, ordinal),
    FOREIGN KEY(document_id, figure_id)
        REFERENCES document_figures(document_id, figure_id) ON DELETE CASCADE
) STRICT;

CREATE TABLE document_figure_readings (
    document_id     TEXT NOT NULL,
    figure_id       TEXT NOT NULL,
    title           TEXT NOT NULL CHECK(length(title) BETWEEN 1 AND 160),
    shows           TEXT NOT NULL CHECK(length(shows) BETWEEN 1 AND 1200),
    use_in_text     TEXT CHECK(use_in_text IS NULL OR length(use_in_text) BETWEEN 1 AND 800),
    concepts_json   TEXT NOT NULL CHECK(json_valid(concepts_json)
                        AND json_type(concepts_json) = 'array'
                        AND length(concepts_json) <= 2000),
    explained_json  TEXT NOT NULL CHECK(json_valid(explained_json)
                        AND json_type(explained_json) = 'array'
                        AND length(explained_json) <= 400),
    importance      INTEGER NOT NULL CHECK(importance BETWEEN 1 AND 5),
    worth_studying  INTEGER NOT NULL CHECK(worth_studying IN (0, 1)),
    provider        TEXT NOT NULL CHECK(length(provider) BETWEEN 1 AND 64),
    model           TEXT NOT NULL CHECK(length(model) BETWEEN 1 AND 200),
    read_at         INTEGER NOT NULL CHECK(read_at >= 0),
    PRIMARY KEY(document_id, figure_id),
    FOREIGN KEY(document_id, figure_id)
        REFERENCES document_figures(document_id, figure_id) ON DELETE CASCADE
) STRICT;
