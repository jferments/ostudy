-- Phase 2 Stage 2: tags and indexed card discovery.
--
-- Search content is deliberately derived from validated note text. The FTS
-- table stores only search projections; notes remain the source of truth.

CREATE TABLE tags (
    id               TEXT PRIMARY KEY CHECK(length(id) = 32),
    name             TEXT NOT NULL CHECK(length(name) BETWEEN 1 AND 80),
    normalized_name  TEXT NOT NULL UNIQUE CHECK(length(normalized_name) BETWEEN 1 AND 80),
    created_at       INTEGER NOT NULL,
    updated_at       INTEGER NOT NULL
) STRICT;

CREATE TABLE note_tags (
    note_id  TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    tag_id   TEXT NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    PRIMARY KEY(note_id, tag_id)
) STRICT;

CREATE INDEX note_tags_tag_idx ON note_tags(tag_id, note_id);

CREATE INDEX notes_active_duplicate_idx
    ON notes(deck_id, lower(trim(front_text)), lower(trim(back_text)))
    WHERE deleted_at IS NULL;

CREATE VIRTUAL TABLE card_search USING fts5(
    note_id UNINDEXED,
    deck_id UNINDEXED,
    front_text,
    back_text,
    tags,
    tokenize = 'unicode61 remove_diacritics 2'
);

INSERT INTO card_search(rowid, note_id, deck_id, front_text, back_text, tags)
SELECT
    n.rowid,
    n.id,
    n.deck_id,
    n.front_text,
    n.back_text,
    COALESCE((
        SELECT group_concat(t.name, ' ')
        FROM note_tags nt
        JOIN tags t ON t.id = nt.tag_id
        WHERE nt.note_id = n.id
    ), '')
FROM notes n
WHERE n.deleted_at IS NULL;

CREATE TRIGGER card_search_note_insert
AFTER INSERT ON notes
WHEN NEW.deleted_at IS NULL
BEGIN
    INSERT INTO card_search(rowid, note_id, deck_id, front_text, back_text, tags)
    VALUES (NEW.rowid, NEW.id, NEW.deck_id, NEW.front_text, NEW.back_text, '');
END;

CREATE TRIGGER card_search_note_update
AFTER UPDATE OF deck_id, front_text, back_text, deleted_at ON notes
BEGIN
    DELETE FROM card_search WHERE rowid = OLD.rowid;
    INSERT INTO card_search(rowid, note_id, deck_id, front_text, back_text, tags)
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
    WHERE NEW.deleted_at IS NULL;
END;

CREATE TRIGGER card_search_note_delete
AFTER DELETE ON notes
BEGIN
    DELETE FROM card_search WHERE rowid = OLD.rowid;
END;

CREATE TRIGGER card_search_note_tag_insert
AFTER INSERT ON note_tags
BEGIN
    UPDATE card_search
    SET tags = COALESCE((
        SELECT group_concat(t.name, ' ')
        FROM note_tags nt
        JOIN tags t ON t.id = nt.tag_id
        WHERE nt.note_id = NEW.note_id
    ), '')
    WHERE note_id = NEW.note_id;
END;

CREATE TRIGGER card_search_note_tag_delete
AFTER DELETE ON note_tags
BEGIN
    UPDATE card_search
    SET tags = COALESCE((
        SELECT group_concat(t.name, ' ')
        FROM note_tags nt
        JOIN tags t ON t.id = nt.tag_id
        WHERE nt.note_id = OLD.note_id
    ), '')
    WHERE note_id = OLD.note_id;
END;

CREATE TRIGGER card_search_tag_rename
AFTER UPDATE OF name ON tags
BEGIN
    UPDATE card_search
    SET tags = COALESCE((
        SELECT group_concat(t.name, ' ')
        FROM note_tags nt
        JOIN tags t ON t.id = nt.tag_id
        WHERE nt.note_id = card_search.note_id
    ), '')
    WHERE note_id IN (
        SELECT note_id FROM note_tags WHERE tag_id = NEW.id
    );
END;
