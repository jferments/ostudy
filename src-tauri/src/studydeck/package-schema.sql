PRAGMA foreign_keys = ON;
CREATE TABLE package_metadata (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
) STRICT;
CREATE TABLE deck (
    source_id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    sort_order INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    revision INTEGER NOT NULL
) STRICT;
CREATE TABLE notes (
    source_id TEXT PRIMARY KEY,
    source_deck_id TEXT NOT NULL,
    note_type TEXT NOT NULL CHECK(note_type IN ('basic', 'cloze', 'interactive')),
    content_format TEXT NOT NULL CHECK(content_format IN ('tiptap-json', 'question-json')),
    content_schema_version INTEGER NOT NULL CHECK(content_schema_version = 1),
    front_doc TEXT,
    back_doc TEXT,
    front_text TEXT NOT NULL,
    back_text TEXT NOT NULL,
    duplicate_hash TEXT NOT NULL,
    reverse_enabled INTEGER NOT NULL CHECK(reverse_enabled IN (0, 1)),
    cloze_source TEXT NOT NULL,
    cloze_mode TEXT NOT NULL CHECK(cloze_mode IN ('one_at_a_time', 'all_together')),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    revision INTEGER NOT NULL,
    question_definition TEXT,
    CHECK (
        (note_type = 'interactive' AND content_format = 'question-json'
         AND question_definition IS NOT NULL AND front_doc IS NULL AND back_doc IS NULL
         AND reverse_enabled = 0 AND cloze_source = '')
        OR
        (note_type IN ('basic', 'cloze') AND content_format = 'tiptap-json'
         AND question_definition IS NULL AND front_doc IS NOT NULL AND back_doc IS NOT NULL)
    )
) STRICT;
CREATE TABLE cards (
    source_id TEXT PRIMARY KEY,
    source_note_id TEXT NOT NULL REFERENCES notes(source_id) ON DELETE CASCADE,
    direction TEXT NOT NULL CHECK(direction IN ('forward', 'reverse')),
    template_ordinal INTEGER NOT NULL CHECK(template_ordinal >= 0),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    question_key TEXT CHECK(question_key IS NULL OR length(question_key) BETWEEN 1 AND 64),
    UNIQUE(source_note_id, direction, template_ordinal),
    UNIQUE(source_note_id, question_key)
) STRICT;
CREATE TABLE tags (
    source_id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    normalized_name TEXT NOT NULL UNIQUE,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
) STRICT;
CREATE TABLE note_tags (
    source_note_id TEXT NOT NULL REFERENCES notes(source_id) ON DELETE CASCADE,
    source_tag_id TEXT NOT NULL REFERENCES tags(source_id) ON DELETE CASCADE,
    PRIMARY KEY(source_note_id, source_tag_id)
) STRICT;
CREATE TABLE assets (
    source_id TEXT PRIMARY KEY,
    original_filename TEXT NOT NULL,
    mime_type TEXT NOT NULL,
    byte_size INTEGER NOT NULL CHECK(byte_size > 0),
    data BLOB NOT NULL
) STRICT;
CREATE TABLE note_assets (
    source_note_id TEXT NOT NULL REFERENCES notes(source_id) ON DELETE CASCADE,
    source_asset_id TEXT NOT NULL REFERENCES assets(source_id) ON DELETE RESTRICT,
    side TEXT NOT NULL CHECK(side IN ('front', 'back', 'content')),
    PRIMARY KEY(source_note_id, source_asset_id, side)
) STRICT;
