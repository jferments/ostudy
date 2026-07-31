-- Phase 2, Stage 1: secure authoring foundation.
--
-- Card templates are inert data. Rust accepts only literal text with exact
-- {{field name}} placeholders and renders values into canonical JSON text
-- nodes. Fill-in-the-blank notes use literal [[answer]] markers. SQLite never
-- evaluates card text, templates, JavaScript, HTML, CSS, or SQL supplied by a
-- deck author.

CREATE TABLE card_templates (
    id           TEXT PRIMARY KEY CHECK(length(id) = 32),
    name         TEXT NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 120),
    description  TEXT NOT NULL DEFAULT '' CHECK(length(description) <= 2000),
    fields_json  TEXT NOT NULL
                 CHECK(json_valid(fields_json) AND json_type(fields_json) = 'array'),
    cards_json   TEXT NOT NULL
                 CHECK(json_valid(cards_json) AND json_type(cards_json) = 'array'),
    is_builtin   INTEGER NOT NULL DEFAULT 0 CHECK(is_builtin IN (0, 1)),
    created_at   INTEGER NOT NULL,
    updated_at   INTEGER NOT NULL,
    revision     INTEGER NOT NULL DEFAULT 1 CHECK(revision >= 1),
    deleted_at   INTEGER
) STRICT;

CREATE UNIQUE INDEX card_templates_active_name_idx
    ON card_templates(name COLLATE NOCASE)
    WHERE deleted_at IS NULL;

INSERT INTO card_templates (
    id, name, description, fields_json, cards_json, is_builtin,
    created_at, updated_at, revision
) VALUES (
    'fffffffffffffffffffffffffffffff1',
    'Pharmacology',
    'Generate several medication questions from one set of fields.',
    '["medication name","drug class","medical conditions","side effects"]',
    '[{"name":"Drug class","front":"What drug class is {{medication name}}?","back":"{{drug class}}"},{"name":"Medical conditions","front":"What medical conditions does {{medication name}} treat?","back":"{{medical conditions}}"},{"name":"Side effects","front":"What are the major side effects of {{medication name}}?","back":"{{side effects}}"},{"name":"Identify the medication","front":"Which medication is in the {{drug class}} class and treats {{medical conditions}}?","back":"{{medication name}}"}]',
    1,
    0,
    0,
    1
);

ALTER TABLE notes ADD COLUMN template_id TEXT
    REFERENCES card_templates(id) ON DELETE RESTRICT;
ALTER TABLE notes ADD COLUMN fields_json TEXT NOT NULL DEFAULT '{}'
    CHECK(json_valid(fields_json) AND json_type(fields_json) = 'object');
ALTER TABLE notes ADD COLUMN duplicate_key TEXT NOT NULL DEFAULT ''
    CHECK(duplicate_key = '' OR length(duplicate_key) = 32);

CREATE INDEX notes_duplicate_key_idx
    ON notes(deck_id, duplicate_key)
    WHERE deleted_at IS NULL AND duplicate_key <> '';

ALTER TABLE cards ADD COLUMN display_name TEXT NOT NULL DEFAULT 'Forward'
    CHECK(length(display_name) BETWEEN 1 AND 200);

CREATE TABLE tags (
    id               TEXT PRIMARY KEY CHECK(length(id) = 32),
    name             TEXT NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 80),
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

-- FTS is content-only and refreshed explicitly by trusted Rust transactions.
-- Search input is tokenized and quoted before reaching MATCH, so user-entered
-- FTS operators are treated as literal search terms.
CREATE VIRTUAL TABLE notes_fts USING fts5(
    note_id UNINDEXED,
    deck_id UNINDEXED,
    front_text,
    back_text,
    tags,
    fields,
    tokenize = 'unicode61 remove_diacritics 2'
);

INSERT INTO notes_fts(note_id, deck_id, front_text, back_text, tags, fields)
SELECT id, deck_id, front_text, back_text, '', fields_json
FROM notes
WHERE deleted_at IS NULL;
