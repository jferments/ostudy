-- Restore archived documents to a non-archived terminal state and introduce
-- portable source-citation snapshots for generated learning artifacts.
--
-- Migrations 0001-0005 intentionally remain immutable. The legacy documents
-- archive columns and CHECK constraint remain in place for released-schema
-- compatibility, but application code stops exposing or writing them.

CREATE TABLE source_citation_snapshots (
    id                        TEXT PRIMARY KEY CHECK(length(id) = 32),
    schema_version            INTEGER NOT NULL DEFAULT 1
                              CHECK(schema_version = 1),
    source_document_id        TEXT NOT NULL CHECK(length(source_document_id) = 32),
    source_identity           TEXT NOT NULL
                              CHECK(length(source_identity) BETWEEN 1 AND 200),
    source_fingerprint_sha256 TEXT NOT NULL
                              CHECK(length(source_fingerprint_sha256) = 64),
    source_title              TEXT NOT NULL
                              CHECK(length(trim(source_title)) BETWEEN 1 AND 200),
    original_filename         TEXT NOT NULL
                              CHECK(length(trim(original_filename)) BETWEEN 1 AND 255),
    source_format             TEXT NOT NULL
                              CHECK(length(source_format) BETWEEN 1 AND 40),
    source_media_type         TEXT NOT NULL
                              CHECK(length(source_media_type) BETWEEN 3 AND 160),
    source_unit_kind          TEXT NOT NULL
                              CHECK(length(source_unit_kind) BETWEEN 1 AND 40),
    source_unit_count         INTEGER NOT NULL CHECK(source_unit_count >= 0),
    source_removed            INTEGER NOT NULL DEFAULT 0
                              CHECK(source_removed IN (0, 1)),
    created_at                INTEGER NOT NULL,
    updated_at                INTEGER NOT NULL,
    CHECK(updated_at >= created_at)
) STRICT;

CREATE UNIQUE INDEX source_citation_snapshots_document_idx
ON source_citation_snapshots(source_document_id);

CREATE INDEX source_citation_snapshots_identity_idx
ON source_citation_snapshots(source_identity, source_fingerprint_sha256);

CREATE TABLE deck_source_citations (
    deck_id     TEXT NOT NULL REFERENCES decks(id) ON DELETE CASCADE,
    citation_id TEXT NOT NULL REFERENCES source_citation_snapshots(id) ON DELETE RESTRICT,
    created_at  INTEGER NOT NULL,
    PRIMARY KEY(deck_id, citation_id)
) STRICT;

CREATE INDEX deck_source_citations_citation_idx
ON deck_source_citations(citation_id, deck_id);

CREATE TABLE note_source_citations (
    note_id             TEXT PRIMARY KEY REFERENCES notes(id) ON DELETE CASCADE,
    citation_id         TEXT NOT NULL
                        REFERENCES source_citation_snapshots(id) ON DELETE RESTRICT,
    source_locator_json TEXT
                        CHECK(
                            source_locator_json IS NULL OR
                            (
                                length(source_locator_json) <= 65536 AND
                                json_valid(source_locator_json)
                            )
                        ),
    created_at          INTEGER NOT NULL,
    updated_at          INTEGER NOT NULL,
    CHECK(updated_at >= created_at)
) STRICT;

CREATE INDEX note_source_citations_citation_idx
ON note_source_citations(citation_id, note_id);

INSERT INTO source_citation_snapshots (
    id, schema_version, source_document_id, source_identity,
    source_fingerprint_sha256, source_title, original_filename,
    source_format, source_media_type, source_unit_kind, source_unit_count,
    source_removed, created_at, updated_at
)
SELECT
    id, 1, id, 'sha256:' || sha256, sha256, title, original_filename,
    source_format, source_media_type, source_unit_kind, source_unit_count,
    0, created_at, updated_at
FROM documents;

INSERT OR IGNORE INTO deck_source_citations (deck_id, citation_id, created_at)
SELECT relation.deck_id, relation.document_id, relation.created_at
FROM (
    SELECT document_id, deck_id, created_at
    FROM document_decks
    UNION
    SELECT id, generated_deck_id, created_at
    FROM documents
    WHERE generated_deck_id IS NOT NULL
) AS relation
JOIN decks AS deck
  ON deck.id = relation.deck_id;

INSERT OR IGNORE INTO note_source_citations (
    note_id, citation_id, source_locator_json, created_at, updated_at
)
SELECT
    relation.note_id,
    relation.document_id,
    relation.source_locator_json,
    relation.created_at,
    relation.created_at
FROM document_note_topics AS relation
JOIN source_citation_snapshots AS citation
  ON citation.source_document_id = relation.document_id
WHERE relation.relationship_kind = 'primary';

UPDATE documents
SET lifecycle_state = CASE
        WHEN EXISTS (
            SELECT 1
            FROM decks AS deck
            WHERE deck.id = documents.generated_deck_id
              AND deck.deleted_at IS NULL
              AND deck.archived_at IS NULL
        )
        OR EXISTS (
            SELECT 1
            FROM document_decks AS relation
            JOIN decks AS deck ON deck.id = relation.deck_id
            WHERE relation.document_id = documents.id
              AND deck.deleted_at IS NULL
              AND deck.archived_at IS NULL
        )
        OR EXISTS (
            SELECT 1
            FROM document_study_guides AS guide
            WHERE guide.document_id = documents.id
              AND guide.state = 'active'
        )
        THEN 'ready'
        ELSE 'failed'
    END,
    archived_at = NULL,
    updated_at = MAX(updated_at, archived_at),
    revision = revision + 1
WHERE lifecycle_state = 'archived'
   OR archived_at IS NOT NULL;
