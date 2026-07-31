-- Phase 2 completes the local flashcard core without allowing executable card
-- content. Templates are literal strings with exact {{field name}}
-- placeholders. Fill-in-the-blank cards use literal [[answer]] markers. Rust
-- validates every imported or IPC-provided document and renders generated card
-- text into canonical Tiptap JSON; SQLite never evaluates user content.

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

ALTER TABLE cards ADD COLUMN buried_until INTEGER;
ALTER TABLE cards ADD COLUMN struggling_since INTEGER;
ALTER TABLE cards ADD COLUMN struggling_reason TEXT
    CHECK(struggling_reason IS NULL OR length(struggling_reason) <= 1000);
ALTER TABLE cards ADD COLUMN source_card_id TEXT
    REFERENCES cards(id) ON DELETE SET NULL;
ALTER TABLE cards ADD COLUMN display_name TEXT NOT NULL DEFAULT 'Forward'
    CHECK(length(display_name) BETWEEN 1 AND 200);

CREATE INDEX cards_buried_until_idx
    ON cards(buried_until)
    WHERE deleted_at IS NULL AND buried_until IS NOT NULL;

CREATE INDEX cards_struggling_idx
    ON cards(struggling_since)
    WHERE deleted_at IS NULL AND struggling_since IS NOT NULL;

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

CREATE TABLE editor_drafts (
    draft_key    TEXT PRIMARY KEY CHECK(length(draft_key) BETWEEN 1 AND 200),
    deck_id      TEXT NOT NULL REFERENCES decks(id) ON DELETE CASCADE,
    note_id      TEXT REFERENCES notes(id) ON DELETE CASCADE,
    payload_json TEXT NOT NULL
                 CHECK(json_valid(payload_json) AND length(payload_json) <= 4000000),
    updated_at   INTEGER NOT NULL
) STRICT;

CREATE INDEX editor_drafts_deck_idx ON editor_drafts(deck_id, updated_at DESC);

-- Review events remain immutable. Exact schedule snapshots support undo, while
-- a separate reversal row excludes an undone review from analytics and daily
-- limits without deleting history.
CREATE TABLE review_schedule_snapshots (
    review_event_id           TEXT PRIMARY KEY
                              REFERENCES review_events(id) ON DELETE RESTRICT,
    state_before              INTEGER NOT NULL CHECK(state_before BETWEEN 0 AND 3),
    due_before                INTEGER NOT NULL,
    stability_before          REAL NOT NULL CHECK(stability_before >= 0.0),
    difficulty_before         REAL NOT NULL CHECK(difficulty_before >= 0.0),
    elapsed_days_before       INTEGER NOT NULL CHECK(elapsed_days_before >= 0),
    scheduled_days_before     INTEGER NOT NULL CHECK(scheduled_days_before >= 0),
    reps_before               INTEGER NOT NULL CHECK(reps_before >= 0),
    lapses_before             INTEGER NOT NULL CHECK(lapses_before >= 0),
    last_review_at_before     INTEGER NOT NULL,
    scheduler_name_before     TEXT NOT NULL,
    scheduler_version_before  TEXT NOT NULL,
    state_after               INTEGER NOT NULL CHECK(state_after BETWEEN 0 AND 3),
    due_after                 INTEGER NOT NULL,
    stability_after           REAL NOT NULL CHECK(stability_after >= 0.0),
    difficulty_after          REAL NOT NULL CHECK(difficulty_after >= 0.0),
    elapsed_days_after        INTEGER NOT NULL CHECK(elapsed_days_after >= 0),
    scheduled_days_after      INTEGER NOT NULL CHECK(scheduled_days_after >= 0),
    reps_after                INTEGER NOT NULL CHECK(reps_after >= 0),
    lapses_after              INTEGER NOT NULL CHECK(lapses_after >= 0),
    last_review_at_after      INTEGER NOT NULL,
    scheduler_name_after      TEXT NOT NULL,
    scheduler_version_after   TEXT NOT NULL
) STRICT;

CREATE TABLE review_card_state_snapshots (
    review_event_id          TEXT NOT NULL REFERENCES review_events(id) ON DELETE RESTRICT,
    card_id                  TEXT NOT NULL REFERENCES cards(id) ON DELETE RESTRICT,
    suspended_at_before      INTEGER,
    struggling_since_before  INTEGER,
    struggling_reason_before TEXT
                              CHECK(struggling_reason_before IS NULL OR length(struggling_reason_before) <= 1000),
    struggling_after         INTEGER NOT NULL DEFAULT 0 CHECK(struggling_after IN (0, 1)),
    auto_suspended           INTEGER NOT NULL DEFAULT 0 CHECK(auto_suspended IN (0, 1)),
    PRIMARY KEY(review_event_id, card_id)
) STRICT;

CREATE TABLE review_event_reversals (
    id               TEXT PRIMARY KEY CHECK(length(id) = 32),
    review_event_id  TEXT NOT NULL UNIQUE
                     REFERENCES review_events(id) ON DELETE RESTRICT,
    reversed_at      INTEGER NOT NULL,
    reason           TEXT NOT NULL DEFAULT 'user_undo'
                     CHECK(reason IN ('user_undo', 'repair'))
) STRICT;

CREATE INDEX review_event_reversals_time_idx
    ON review_event_reversals(reversed_at DESC);

CREATE TABLE review_sibling_buries (
    review_event_id        TEXT NOT NULL REFERENCES review_events(id) ON DELETE RESTRICT,
    card_id                TEXT NOT NULL REFERENCES cards(id) ON DELETE RESTRICT,
    previous_buried_until  INTEGER,
    PRIMARY KEY(review_event_id, card_id)
) STRICT;

CREATE TABLE card_state_events (
    id            TEXT PRIMARY KEY CHECK(length(id) = 32),
    card_id       TEXT NOT NULL REFERENCES cards(id) ON DELETE RESTRICT,
    action        TEXT NOT NULL CHECK(action IN (
                      'suspend', 'resume', 'bury', 'unbury', 'reset',
                      'reschedule', 'move', 'copy', 'trouble', 'clear_trouble'
                  )),
    details_json  TEXT NOT NULL DEFAULT '{}'
                  CHECK(json_valid(details_json) AND json_type(details_json) = 'object'),
    created_at    INTEGER NOT NULL
) STRICT;

CREATE INDEX card_state_events_card_idx
    ON card_state_events(card_id, created_at DESC);

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

INSERT OR IGNORE INTO app_settings(key, value_json, updated_at) VALUES
    ('newCardsPerDay', '20', 0),
    ('reviewsPerDay', '200', 0),
    ('strugglingCardThreshold', '8', 0),
    ('autoSuspendStrugglingCards', 'false', 0),
    ('buryRelatedCards', 'true', 0);
