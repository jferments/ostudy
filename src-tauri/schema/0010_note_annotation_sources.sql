-- Release migration 10: the document marks that cards were made from.
--
-- A learner can select a highlight or a note in the PDF annotator and make a
-- flashcard from it. Each such note keeps one row here: the document, the
-- annotation and the page it came from, and a copy of the marked words, so the
-- source still reads correctly after the mark or the document is deleted. The
-- note owns the row; deleting the mark or the document clears that link and
-- keeps the card. The upgrade creates the table and writes no rows.
CREATE TABLE note_annotation_sources (
    note_id        TEXT PRIMARY KEY REFERENCES notes(id) ON DELETE CASCADE
                   CHECK(length(note_id) = 32),
    document_id    TEXT REFERENCES documents(id) ON DELETE SET NULL
                   CHECK(document_id IS NULL OR length(document_id) = 32),
    annotation_id  TEXT REFERENCES document_annotations(id) ON DELETE SET NULL
                   CHECK(annotation_id IS NULL OR length(annotation_id) = 32),
    page_number    INTEGER NOT NULL CHECK(page_number >= 1),
    quote          TEXT NOT NULL CHECK(length(quote) <= 20000),
    created_at     INTEGER NOT NULL CHECK(created_at >= 0),
    updated_at     INTEGER NOT NULL CHECK(updated_at >= created_at)
) STRICT;

CREATE INDEX note_annotation_sources_by_document
    ON note_annotation_sources(document_id, page_number);

CREATE INDEX note_annotation_sources_by_annotation
    ON note_annotation_sources(annotation_id);
