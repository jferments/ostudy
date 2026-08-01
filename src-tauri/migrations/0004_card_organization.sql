-- Add tags, duplicate fingerprints, and a SQLite FTS5 card index.
--
-- Tags are global labels attached to notes so future generated cards can share
-- one source classification. The FTS rowid mirrors notes.rowid, while triggers
-- keep searchable question, answer, deck, and tag text synchronized for every
-- supported write path (editor, bulk entry, move/copy, and deck import).

ALTER TABLE notes ADD COLUMN duplicate_hash TEXT NOT NULL DEFAULT '';

CREATE INDEX notes_deck_duplicate_idx
    ON notes(deck_id, duplicate_hash)
    WHERE deleted_at IS NULL;

-- Legacy write commands from earlier builds do not know about duplicate_hash.
-- If one of those paths changes card text without changing the fingerprint,
-- clear it so duplicate checks fall back to a full normalized-text comparison.
CREATE TRIGGER notes_duplicate_hash_after_legacy_update
AFTER UPDATE OF front_text, back_text ON notes
WHEN NEW.duplicate_hash = OLD.duplicate_hash
 AND (NEW.front_text != OLD.front_text OR NEW.back_text != OLD.back_text)
BEGIN
    UPDATE notes SET duplicate_hash = '' WHERE rowid = NEW.rowid;
END;

CREATE TABLE tags (
    id               TEXT PRIMARY KEY CHECK(length(id) = 32),
    name             TEXT NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 64),
    normalized_name  TEXT NOT NULL UNIQUE
                     CHECK(length(trim(normalized_name)) BETWEEN 1 AND 64),
    created_at       INTEGER NOT NULL,
    updated_at       INTEGER NOT NULL
) STRICT;

CREATE TABLE note_tags (
    note_id  TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    tag_id   TEXT NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    PRIMARY KEY(note_id, tag_id)
) STRICT;

CREATE INDEX note_tags_tag_idx ON note_tags(tag_id, note_id);

CREATE VIRTUAL TABLE card_search_fts USING fts5(
    note_id UNINDEXED,
    deck_id UNINDEXED,
    question,
    answer,
    tags,
    tokenize = 'unicode61 remove_diacritics 2'
);

INSERT INTO card_search_fts(rowid, note_id, deck_id, question, answer, tags)
SELECT
    n.rowid,
    n.id,
    n.deck_id,
    n.front_text,
    n.back_text,
    ''
FROM notes n
JOIN decks d ON d.id = n.deck_id
WHERE n.deleted_at IS NULL
  AND d.deleted_at IS NULL
  AND d.archived_at IS NULL;

CREATE TRIGGER notes_search_after_insert
AFTER INSERT ON notes
WHEN NEW.deleted_at IS NULL
BEGIN
    INSERT INTO card_search_fts(rowid, note_id, deck_id, question, answer, tags)
    VALUES (NEW.rowid, NEW.id, NEW.deck_id, NEW.front_text, NEW.back_text, '');
END;

CREATE TRIGGER notes_search_after_update
AFTER UPDATE OF deck_id, front_text, back_text, deleted_at ON notes
BEGIN
    DELETE FROM card_search_fts WHERE rowid = OLD.rowid;
    INSERT INTO card_search_fts(rowid, note_id, deck_id, question, answer, tags)
    SELECT
        NEW.rowid,
        NEW.id,
        NEW.deck_id,
        NEW.front_text,
        NEW.back_text,
        COALESCE((
            SELECT group_concat(t.name, ' ')
            FROM note_tags nt
            JOIN tags t ON t.id = nt.tag_id
            WHERE nt.note_id = NEW.id
        ), '')
    WHERE NEW.deleted_at IS NULL
      AND EXISTS (
          SELECT 1
          FROM decks d
          WHERE d.id = NEW.deck_id
            AND d.deleted_at IS NULL
            AND d.archived_at IS NULL
      );
END;

CREATE TRIGGER notes_search_after_delete
AFTER DELETE ON notes
BEGIN
    DELETE FROM card_search_fts WHERE rowid = OLD.rowid;
END;

CREATE TRIGGER note_tags_search_after_insert
AFTER INSERT ON note_tags
BEGIN
    UPDATE card_search_fts
    SET tags = COALESCE((
        SELECT group_concat(t.name, ' ')
        FROM note_tags nt
        JOIN tags t ON t.id = nt.tag_id
        WHERE nt.note_id = NEW.note_id
    ), '')
    WHERE rowid = (SELECT rowid FROM notes WHERE id = NEW.note_id);
END;

CREATE TRIGGER note_tags_search_after_delete
AFTER DELETE ON note_tags
BEGIN
    UPDATE card_search_fts
    SET tags = COALESCE((
        SELECT group_concat(t.name, ' ')
        FROM note_tags nt
        JOIN tags t ON t.id = nt.tag_id
        WHERE nt.note_id = OLD.note_id
    ), '')
    WHERE rowid = (SELECT rowid FROM notes WHERE id = OLD.note_id);
END;

CREATE TRIGGER tags_search_after_update
AFTER UPDATE OF name ON tags
BEGIN
    UPDATE card_search_fts
    SET tags = COALESCE((
        SELECT group_concat(t.name, ' ')
        FROM note_tags nt
        JOIN tags t ON t.id = nt.tag_id
        WHERE nt.note_id = card_search_fts.note_id
    ), '')
    WHERE note_id IN (
        SELECT note_id FROM note_tags WHERE tag_id = NEW.id
    );
END;
