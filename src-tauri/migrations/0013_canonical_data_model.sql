-- Physically narrow the active schema after versioned repairs have canonicalized
-- every released data set. Historical migration files remain immutable evidence.
--
-- Foreign-key enforcement is temporarily disabled by the migration runner while
-- the two parent tables are rebuilt. The migration transaction runs a complete
-- foreign-key check before commit and enforcement is restored immediately after.

-- These triggers reference notes by name. Drop every one of them before the
-- parent table is replaced so SQLite never has to compile a trigger against the
-- short interval in which the old notes table has been removed.
DROP TRIGGER IF EXISTS notes_duplicate_hash_after_legacy_update;
DROP TRIGGER IF EXISTS notes_search_after_insert;
DROP TRIGGER IF EXISTS notes_search_after_update;
DROP TRIGGER IF EXISTS notes_search_after_delete;
DROP TRIGGER IF EXISTS note_tags_search_after_insert;
DROP TRIGGER IF EXISTS note_tags_search_after_delete;
DROP TRIGGER IF EXISTS tags_search_after_update;

CREATE TABLE notes_canonical (
    id                      TEXT PRIMARY KEY CHECK(length(id) = 32),
    deck_id                 TEXT NOT NULL REFERENCES decks(id) ON DELETE CASCADE,
    note_type               TEXT NOT NULL DEFAULT 'basic'
                            CHECK(note_type IN ('basic', 'cloze')),
    content_format          TEXT NOT NULL DEFAULT 'tiptap-json'
                            CHECK(content_format = 'tiptap-json'),
    content_schema_version  INTEGER NOT NULL DEFAULT 1 CHECK(content_schema_version >= 1),
    front_doc               TEXT NOT NULL,
    back_doc                TEXT NOT NULL,
    front_text              TEXT NOT NULL,
    back_text               TEXT NOT NULL,
    created_at              INTEGER NOT NULL,
    updated_at              INTEGER NOT NULL,
    deleted_at              INTEGER,
    revision                INTEGER NOT NULL DEFAULT 1 CHECK(revision >= 1),
    duplicate_hash          TEXT NOT NULL DEFAULT '',
    reverse_enabled         INTEGER NOT NULL DEFAULT 0 CHECK(reverse_enabled IN (0, 1)),
    cloze_source            TEXT NOT NULL DEFAULT '',
    cloze_mode              TEXT NOT NULL DEFAULT 'one_at_a_time'
                            CHECK(cloze_mode IN ('one_at_a_time', 'all_together'))
) STRICT;

INSERT INTO notes_canonical (
    id, deck_id, note_type, content_format, content_schema_version,
    front_doc, back_doc, front_text, back_text,
    created_at, updated_at, deleted_at, revision, duplicate_hash,
    reverse_enabled, cloze_source, cloze_mode
)
SELECT
    id, deck_id, note_type, content_format, content_schema_version,
    front_doc, back_doc, front_text, back_text,
    created_at, updated_at, deleted_at, revision, duplicate_hash,
    reverse_enabled, cloze_source, cloze_mode
FROM notes;

CREATE TABLE cards_canonical (
    id                TEXT PRIMARY KEY CHECK(length(id) = 32),
    note_id           TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    deck_id           TEXT NOT NULL REFERENCES decks(id) ON DELETE CASCADE,
    direction         TEXT NOT NULL DEFAULT 'forward'
                      CHECK(direction IN ('forward', 'reverse')),
    template_ordinal  INTEGER NOT NULL DEFAULT 0 CHECK(template_ordinal >= 0),
    created_at        INTEGER NOT NULL,
    updated_at        INTEGER NOT NULL,
    suspended_at      INTEGER,
    deleted_at        INTEGER,
    buried_until      INTEGER,
    UNIQUE(note_id, direction, template_ordinal)
) STRICT;

INSERT INTO cards_canonical (
    id, note_id, deck_id, direction, template_ordinal,
    created_at, updated_at, suspended_at, deleted_at, buried_until
)
SELECT
    id, note_id, deck_id, direction, template_ordinal,
    created_at, updated_at, suspended_at, deleted_at, buried_until
FROM cards;

DELETE FROM card_search_fts;
DROP TABLE cards;
DROP TABLE notes;
ALTER TABLE notes_canonical RENAME TO notes;
ALTER TABLE cards_canonical RENAME TO cards;

CREATE INDEX notes_deck_active_idx
    ON notes(deck_id, updated_at DESC)
    WHERE deleted_at IS NULL;
CREATE INDEX notes_deck_duplicate_idx
    ON notes(deck_id, duplicate_hash)
    WHERE deleted_at IS NULL;
CREATE INDEX cards_deck_active_idx
    ON cards(deck_id, updated_at DESC)
    WHERE deleted_at IS NULL;
CREATE INDEX cards_buried_until_idx
    ON cards(buried_until)
    WHERE buried_until IS NOT NULL;

INSERT INTO card_search_fts(rowid, note_id, deck_id, question, answer, tags)
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

ALTER TABLE card_template_rules DROP COLUMN front_pattern;
ALTER TABLE card_template_rules DROP COLUMN back_pattern;

CREATE TRIGGER template_rules_canonical_docs_before_insert
BEFORE INSERT ON card_template_rules
WHEN NEW.front_doc IS NULL OR NEW.back_doc IS NULL
BEGIN
    SELECT RAISE(ABORT, 'template layouts are required');
END;

CREATE TRIGGER template_rules_canonical_docs_before_update
BEFORE UPDATE OF front_doc, back_doc ON card_template_rules
WHEN NEW.front_doc IS NULL OR NEW.back_doc IS NULL
BEGIN
    SELECT RAISE(ABORT, 'template layouts are required');
END;
