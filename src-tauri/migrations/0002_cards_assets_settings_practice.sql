CREATE TABLE notes (
    id                      TEXT PRIMARY KEY CHECK(length(id) = 32),
    deck_id                 TEXT NOT NULL REFERENCES decks(id) ON DELETE CASCADE,
    note_type               TEXT NOT NULL DEFAULT 'basic'
                            CHECK(note_type IN ('basic', 'cloze', 'image_occlusion')),
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
    revision                INTEGER NOT NULL DEFAULT 1 CHECK(revision >= 1)
) STRICT;

CREATE INDEX notes_deck_active_idx
    ON notes(deck_id, updated_at DESC)
    WHERE deleted_at IS NULL;

CREATE TABLE cards (
    id                TEXT PRIMARY KEY CHECK(length(id) = 32),
    note_id           TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    deck_id           TEXT NOT NULL REFERENCES decks(id) ON DELETE CASCADE,
    direction         TEXT NOT NULL DEFAULT 'forward'
                      CHECK(direction IN ('forward', 'reverse', 'cloze', 'image_occlusion')),
    template_ordinal  INTEGER NOT NULL DEFAULT 0 CHECK(template_ordinal >= 0),
    created_at        INTEGER NOT NULL,
    updated_at        INTEGER NOT NULL,
    suspended_at      INTEGER,
    deleted_at        INTEGER,
    UNIQUE(note_id, direction, template_ordinal)
) STRICT;

CREATE INDEX cards_deck_active_idx
    ON cards(deck_id, updated_at DESC)
    WHERE deleted_at IS NULL;

CREATE TABLE assets (
    id                 TEXT PRIMARY KEY CHECK(length(id) = 32),
    original_filename  TEXT NOT NULL,
    mime_type          TEXT NOT NULL,
    byte_size          INTEGER NOT NULL CHECK(byte_size >= 0),
    relative_path      TEXT NOT NULL UNIQUE,
    created_at         INTEGER NOT NULL,
    deleted_at         INTEGER
) STRICT;

CREATE TABLE note_assets (
    note_id   TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    asset_id  TEXT NOT NULL REFERENCES assets(id) ON DELETE RESTRICT,
    side      TEXT NOT NULL CHECK(side IN ('front', 'back')),
    PRIMARY KEY(note_id, asset_id, side)
) STRICT;

CREATE INDEX note_assets_asset_idx ON note_assets(asset_id);

CREATE TABLE app_settings (
    key         TEXT PRIMARY KEY,
    value_json  TEXT NOT NULL,
    updated_at  INTEGER NOT NULL
) STRICT;

INSERT INTO app_settings(key, value_json, updated_at) VALUES
    ('theme', '"dark"', 0),
    ('uiFont', '"system"', 0),
    ('cardFont', '"system"', 0),
    ('uiScale', '100', 0),
    ('cardBaseSize', '18', 0),
    ('reduceMotion', 'false', 0),
    ('confirmDeletions', 'true', 0),
    ('practiceShuffle', 'true', 0);

CREATE TABLE practice_sessions (
    id                       TEXT PRIMARY KEY CHECK(length(id) = 32),
    selected_deck_ids_json   TEXT NOT NULL,
    shuffle                  INTEGER NOT NULL CHECK(shuffle IN (0, 1)),
    card_count               INTEGER NOT NULL CHECK(card_count >= 0),
    viewed_count             INTEGER NOT NULL DEFAULT 0 CHECK(viewed_count >= 0),
    revealed_count           INTEGER NOT NULL DEFAULT 0 CHECK(revealed_count >= 0),
    started_at               INTEGER NOT NULL,
    completed_at             INTEGER
) STRICT;

CREATE TABLE practice_session_cards (
    session_id   TEXT NOT NULL REFERENCES practice_sessions(id) ON DELETE CASCADE,
    card_id      TEXT NOT NULL REFERENCES cards(id) ON DELETE RESTRICT,
    position     INTEGER NOT NULL CHECK(position >= 0),
    viewed_at    INTEGER,
    revealed_at  INTEGER,
    PRIMARY KEY(session_id, card_id),
    UNIQUE(session_id, position)
) STRICT;

CREATE INDEX practice_sessions_started_idx
    ON practice_sessions(started_at DESC);
