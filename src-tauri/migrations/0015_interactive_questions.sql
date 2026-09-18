-- Typed authored questions and durable attempts. The native migration runner
-- temporarily disables FK actions for these table rebuilds, then checks the
-- complete graph before committing and restores FK enforcement on every exit.
-- Existing note rowids are retained because card_search_fts uses that identity.

DROP TRIGGER note_tags_search_after_delete;
DROP TRIGGER note_tags_search_after_insert;
DROP TRIGGER tags_search_after_update;
DROP TRIGGER knowledge_deck_deleted;
DROP TRIGGER knowledge_deck_renamed;

CREATE TABLE notes_v15 (
    id TEXT PRIMARY KEY CHECK(length(id) = 32),
    deck_id TEXT NOT NULL REFERENCES decks(id) ON DELETE CASCADE,
    note_type TEXT NOT NULL DEFAULT 'basic'
        CHECK(note_type IN ('basic', 'cloze', 'interactive')),
    content_format TEXT NOT NULL DEFAULT 'tiptap-json'
        CHECK(content_format IN ('tiptap-json', 'question-json')),
    content_schema_version INTEGER NOT NULL DEFAULT 1 CHECK(content_schema_version >= 1),
    front_doc TEXT,
    back_doc TEXT,
    front_text TEXT NOT NULL,
    back_text TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    deleted_at INTEGER,
    revision INTEGER NOT NULL DEFAULT 1 CHECK(revision >= 1),
    duplicate_hash TEXT NOT NULL DEFAULT '',
    reverse_enabled INTEGER NOT NULL DEFAULT 0 CHECK(reverse_enabled IN (0, 1)),
    cloze_source TEXT NOT NULL DEFAULT '',
    cloze_mode TEXT NOT NULL DEFAULT 'one_at_a_time'
        CHECK(cloze_mode IN ('one_at_a_time', 'all_together')),
    question_definition TEXT,
    CHECK(
        (note_type IN ('basic', 'cloze') AND content_format = 'tiptap-json'
            AND front_doc IS NOT NULL AND back_doc IS NOT NULL
            AND question_definition IS NULL)
        OR
        (note_type = 'interactive' AND content_format = 'question-json'
            AND content_schema_version = 1
            AND front_doc IS NULL AND back_doc IS NULL
            AND question_definition IS NOT NULL
            AND length(CAST(question_definition AS BLOB)) <= 1000000
            AND json_valid(question_definition)
            AND json_type(question_definition) IS 'object'
            AND json_extract(question_definition, '$.schemaVersion') IS 1
            AND reverse_enabled = 0 AND cloze_source = '')
    )
) STRICT;
INSERT INTO notes_v15 (
    rowid, id, deck_id, note_type, content_format, content_schema_version,
    front_doc, back_doc, front_text, back_text, created_at, updated_at,
    deleted_at, revision, duplicate_hash, reverse_enabled, cloze_source, cloze_mode
)
SELECT rowid, id, deck_id, note_type, content_format, content_schema_version,
    front_doc, back_doc, front_text, back_text, created_at, updated_at,
    deleted_at, revision, duplicate_hash, reverse_enabled, cloze_source, cloze_mode
FROM notes;
DROP TABLE notes;
ALTER TABLE notes_v15 RENAME TO notes;
CREATE INDEX notes_deck_active_idx ON notes(deck_id, updated_at DESC) WHERE deleted_at IS NULL;
CREATE INDEX notes_deck_duplicate_idx ON notes(deck_id, duplicate_hash) WHERE deleted_at IS NULL;

CREATE TRIGGER notes_search_after_delete AFTER DELETE ON notes BEGIN
    DELETE FROM card_search_fts WHERE rowid = OLD.rowid;
END;
CREATE TRIGGER notes_search_after_insert AFTER INSERT ON notes
WHEN NEW.deleted_at IS NULL BEGIN
    INSERT INTO card_search_fts(rowid, note_id, deck_id, question, answer, tags)
    VALUES (NEW.rowid, NEW.id, NEW.deck_id, NEW.front_text, NEW.back_text, '');
END;
CREATE TRIGGER notes_search_after_update
AFTER UPDATE OF deck_id, front_text, back_text, deleted_at ON notes BEGIN
    DELETE FROM card_search_fts WHERE rowid = OLD.rowid;
    INSERT INTO card_search_fts(rowid, note_id, deck_id, question, answer, tags)
    SELECT NEW.rowid, NEW.id, NEW.deck_id, NEW.front_text, NEW.back_text,
        COALESCE((SELECT group_concat(t.name, ' ') FROM note_tags nt
            JOIN tags t ON t.id = nt.tag_id WHERE nt.note_id = NEW.id), '')
    WHERE NEW.deleted_at IS NULL AND EXISTS (
        SELECT 1 FROM decks d WHERE d.id = NEW.deck_id
            AND d.deleted_at IS NULL AND d.archived_at IS NULL);
END;
CREATE TRIGGER note_tags_search_after_delete AFTER DELETE ON note_tags BEGIN
    UPDATE card_search_fts SET tags = COALESCE((
        SELECT group_concat(t.name, ' ') FROM note_tags nt JOIN tags t ON t.id = nt.tag_id
        WHERE nt.note_id = OLD.note_id), '')
    WHERE rowid = (SELECT rowid FROM notes WHERE id = OLD.note_id);
END;
CREATE TRIGGER note_tags_search_after_insert AFTER INSERT ON note_tags BEGIN
    UPDATE card_search_fts SET tags = COALESCE((
        SELECT group_concat(t.name, ' ') FROM note_tags nt JOIN tags t ON t.id = nt.tag_id
        WHERE nt.note_id = NEW.note_id), '')
    WHERE rowid = (SELECT rowid FROM notes WHERE id = NEW.note_id);
END;
CREATE TRIGGER tags_search_after_update AFTER UPDATE OF name ON tags BEGIN
    UPDATE card_search_fts SET tags = COALESCE((
        SELECT group_concat(t.name, ' ') FROM note_tags nt JOIN tags t ON t.id = nt.tag_id
        WHERE nt.note_id = card_search_fts.note_id), '')
    WHERE note_id IN (SELECT note_id FROM note_tags WHERE tag_id = NEW.id);
END;
CREATE TRIGGER knowledge_note_changed
AFTER UPDATE OF note_type, front_doc, back_doc, front_text, back_text, cloze_source,
    cloze_mode, question_definition, deleted_at, deck_id ON notes
WHEN OLD.note_type IS NOT NEW.note_type OR OLD.front_doc IS NOT NEW.front_doc
    OR OLD.back_doc IS NOT NEW.back_doc OR OLD.front_text IS NOT NEW.front_text
    OR OLD.back_text IS NOT NEW.back_text OR OLD.cloze_source IS NOT NEW.cloze_source
    OR OLD.cloze_mode IS NOT NEW.cloze_mode
    OR OLD.question_definition IS NOT NEW.question_definition
    OR OLD.deleted_at IS NOT NEW.deleted_at OR OLD.deck_id IS NOT NEW.deck_id
BEGIN
    UPDATE knowledge_sources SET active = 0 WHERE note_id = NEW.id AND active = 1;
END;
CREATE TRIGGER knowledge_deck_deleted AFTER UPDATE OF deleted_at ON decks
WHEN NEW.deleted_at IS NOT NULL BEGIN
    UPDATE knowledge_sources SET active = 0 WHERE note_id IN
        (SELECT id FROM notes WHERE deck_id = NEW.id) AND active = 1;
END;
CREATE TRIGGER knowledge_deck_renamed AFTER UPDATE OF name ON decks
WHEN OLD.name <> NEW.name BEGIN
    UPDATE knowledge_sources SET active = 0 WHERE note_id IN
        (SELECT id FROM notes WHERE deck_id = NEW.id) AND active = 1;
END;

CREATE TABLE note_assets_v15 (
    note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    asset_id TEXT NOT NULL REFERENCES assets(id) ON DELETE RESTRICT,
    side TEXT NOT NULL CHECK(side IN ('front', 'back', 'content')),
    PRIMARY KEY(note_id, asset_id, side)
) STRICT;
INSERT INTO note_assets_v15 SELECT note_id, asset_id, side FROM note_assets;
DROP TABLE note_assets;
ALTER TABLE note_assets_v15 RENAME TO note_assets;
CREATE INDEX note_assets_asset_idx ON note_assets(asset_id);

-- A stable question key, not the current editor position, owns each schedule.
CREATE TABLE card_question_presentations (
    card_id TEXT PRIMARY KEY REFERENCES cards(id) ON DELETE CASCADE,
    note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    question_key TEXT NOT NULL CHECK(length(question_key) BETWEEN 1 AND 64),
    UNIQUE(note_id, question_key)
) STRICT;
CREATE TRIGGER card_question_presentation_insert BEFORE INSERT ON card_question_presentations
WHEN NOT EXISTS (
    SELECT 1 FROM cards c JOIN notes n ON n.id = c.note_id
    WHERE c.id = NEW.card_id AND n.id = NEW.note_id
        AND n.note_type = 'interactive' AND c.direction = 'forward'
) BEGIN
    SELECT RAISE(ABORT, 'Question presentation must belong to its interactive note');
END;
CREATE TRIGGER card_question_presentation_identity BEFORE UPDATE ON card_question_presentations
WHEN OLD.card_id <> NEW.card_id OR OLD.note_id <> NEW.note_id OR OLD.question_key <> NEW.question_key
BEGIN
    SELECT RAISE(ABORT, 'Question presentation identity is immutable');
END;

-- Receipts contain identities only, never a second retained copy of private
-- authored content. A replay after deletion cannot recreate the deleted note.
CREATE TABLE question_note_writes (
    operation_id TEXT PRIMARY KEY CHECK(length(operation_id) = 32),
    request_hash TEXT NOT NULL CHECK(length(request_hash) = 64),
    note_id TEXT NOT NULL CHECK(length(note_id) = 32),
    note_revision INTEGER NOT NULL CHECK(note_revision >= 1),
    card_ids_json TEXT NOT NULL CHECK(json_valid(card_ids_json)
        AND json_type(card_ids_json) IS 'array' AND length(card_ids_json) <= 4096),
    committed_at INTEGER NOT NULL
) STRICT;

-- Personal first-attempt evidence remains owned by the retained card/note even
-- if an obsolete session is removed. Material purge removes the complete graph.
CREATE TABLE question_attempts (
    instance_id TEXT PRIMARY KEY CHECK(length(instance_id) = 32),
    session_id TEXT REFERENCES practice_sessions(id) ON DELETE SET NULL,
    card_id TEXT NOT NULL REFERENCES cards(id) ON DELETE CASCADE,
    note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    question_kind TEXT NOT NULL CHECK(question_kind IN (
        'single_choice', 'select_all', 'true_false', 'matching',
        'grouping', 'ordering', 'image_occlusion')),
    instance_hash TEXT NOT NULL CHECK(length(instance_hash) = 64),
    revision INTEGER NOT NULL CHECK(revision >= 1),
    phase TEXT NOT NULL CHECK(phase IN ('answering', 'checked', 'revealed')),
    assisted INTEGER NOT NULL CHECK(assisted IN (0, 1)),
    checkpoint_json TEXT NOT NULL CHECK(json_valid(checkpoint_json)
        AND length(CAST(checkpoint_json AS BLOB)) <= 4000000),
    first_checked_at INTEGER CHECK(first_checked_at IS NULL OR first_checked_at >= 0),
    verdict TEXT CHECK(verdict IS NULL OR verdict IN ('correct', 'incorrect', 'incomplete')),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    UNIQUE(session_id, card_id),
    CHECK(json_extract(checkpoint_json, '$.revision') IS revision),
    CHECK(json_extract(checkpoint_json, '$.assisted') IS assisted),
    CHECK(json_extract(checkpoint_json, '$.state.phase') IS phase),
    CHECK((phase = 'answering' AND first_checked_at IS NULL AND verdict IS NULL)
        OR (phase = 'checked' AND first_checked_at IS NOT NULL AND verdict IS NOT NULL)
        OR (phase = 'revealed' AND first_checked_at IS NOT NULL AND verdict IS NULL))
) STRICT;
CREATE INDEX question_attempts_card_idx ON question_attempts(card_id, created_at);
CREATE INDEX question_attempts_note_idx ON question_attempts(note_id);
CREATE INDEX question_attempts_results_idx ON question_attempts(first_checked_at, question_kind, verdict);
CREATE TABLE question_attempt_assets (
    instance_id TEXT NOT NULL REFERENCES question_attempts(instance_id) ON DELETE CASCADE,
    asset_id TEXT NOT NULL REFERENCES assets(id) ON DELETE RESTRICT,
    PRIMARY KEY(instance_id, asset_id)
) STRICT;
CREATE INDEX question_attempt_assets_asset_idx ON question_attempt_assets(asset_id);

CREATE TABLE practice_session_card_snapshots_v15 (
    session_id TEXT NOT NULL REFERENCES practice_sessions(id) ON DELETE CASCADE,
    card_id TEXT NOT NULL CHECK(length(card_id) = 32),
    note_id TEXT NOT NULL CHECK(length(note_id) = 32),
    note_revision INTEGER NOT NULL CHECK(note_revision >= 1),
    owner_deck_id_at_capture TEXT NOT NULL CHECK(length(owner_deck_id_at_capture) = 32),
    captured_deck_name TEXT NOT NULL,
    direction TEXT NOT NULL CHECK(direction IN ('forward', 'reverse')),
    card_type TEXT NOT NULL CHECK(card_type IN ('basic', 'cloze', 'single_choice', 'select_all',
        'true_false', 'matching', 'grouping', 'ordering', 'image_occlusion')),
    card_label TEXT NOT NULL,
    presentation_variant TEXT NOT NULL,
    front_doc_json TEXT,
    back_doc_json TEXT,
    front_text TEXT NOT NULL,
    back_text TEXT NOT NULL,
    question_excerpt TEXT NOT NULL,
    question_text_hash TEXT NOT NULL CHECK(length(question_text_hash) = 64),
    question_was_truncated INTEGER NOT NULL CHECK(question_was_truncated IN (0, 1)),
    captured_at INTEGER NOT NULL,
    question_instance_id TEXT REFERENCES question_attempts(instance_id) ON DELETE CASCADE,
    PRIMARY KEY(session_id, card_id),
    CHECK((card_type IN ('basic', 'cloze') AND front_doc_json IS NOT NULL
            AND back_doc_json IS NOT NULL AND question_instance_id IS NULL)
        OR (card_type NOT IN ('basic', 'cloze') AND front_doc_json IS NULL
            AND back_doc_json IS NULL AND question_instance_id IS NOT NULL))
) STRICT;
INSERT INTO practice_session_card_snapshots_v15 (
    session_id, card_id, note_id, note_revision, owner_deck_id_at_capture, captured_deck_name,
    direction, card_type, card_label, presentation_variant, front_doc_json, back_doc_json,
    front_text, back_text, question_excerpt, question_text_hash, question_was_truncated, captured_at
)
SELECT session_id, card_id, note_id, note_revision, owner_deck_id_at_capture, captured_deck_name,
    direction, card_type, card_label, presentation_variant, front_doc_json, back_doc_json,
    front_text, back_text, question_excerpt, question_text_hash, question_was_truncated, captured_at
FROM practice_session_card_snapshots;
DROP TABLE practice_session_card_snapshots;
ALTER TABLE practice_session_card_snapshots_v15 RENAME TO practice_session_card_snapshots;
CREATE INDEX practice_session_card_snapshots_session_position_idx
    ON practice_session_card_snapshots(session_id, card_id);

CREATE TABLE review_event_attributions_v15 (
    review_event_id TEXT PRIMARY KEY REFERENCES review_events(id) ON DELETE CASCADE,
    reviewed_at INTEGER NOT NULL,
    review_local_date TEXT NOT NULL
        CHECK(review_local_date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
    time_zone_id TEXT NOT NULL,
    utc_offset_minutes INTEGER NOT NULL CHECK(utc_offset_minutes BETWEEN -1440 AND 1440),
    card_id TEXT NOT NULL CHECK(length(card_id) = 32),
    note_id TEXT NOT NULL CHECK(length(note_id) = 32),
    note_revision INTEGER NOT NULL CHECK(note_revision >= 1),
    owner_deck_id_at_review TEXT NOT NULL CHECK(length(owner_deck_id_at_review) = 32),
    captured_deck_name TEXT NOT NULL,
    direction TEXT NOT NULL CHECK(direction IN ('forward', 'reverse')),
    card_type TEXT NOT NULL CHECK(card_type IN ('basic', 'cloze', 'single_choice', 'select_all',
        'true_false', 'matching', 'grouping', 'ordering', 'image_occlusion')),
    card_label TEXT NOT NULL,
    presentation_variant TEXT NOT NULL,
    question_excerpt TEXT NOT NULL,
    question_text_hash TEXT NOT NULL CHECK(length(question_text_hash) = 64),
    question_was_truncated INTEGER NOT NULL CHECK(question_was_truncated IN (0, 1)),
    attribution_source TEXT NOT NULL
        CHECK(attribution_source IN ('captured', 'legacy_current_state_backfill'))
) STRICT;
INSERT INTO review_event_attributions_v15
SELECT review_event_id, reviewed_at, review_local_date, time_zone_id, utc_offset_minutes,
    card_id, note_id, note_revision, owner_deck_id_at_review, captured_deck_name,
    direction, card_type, card_label, presentation_variant, question_excerpt,
    question_text_hash, question_was_truncated, attribution_source
FROM review_event_attributions;
DROP TABLE review_event_attributions;
ALTER TABLE review_event_attributions_v15 RENAME TO review_event_attributions;
CREATE INDEX review_event_attributions_capture_deck_idx
    ON review_event_attributions(owner_deck_id_at_review, review_local_date, reviewed_at DESC);
CREATE INDEX review_event_attributions_card_date_idx
    ON review_event_attributions(card_id, review_local_date, reviewed_at DESC);
CREATE INDEX review_event_attributions_local_date_idx
    ON review_event_attributions(review_local_date, reviewed_at DESC, review_event_id);

CREATE TABLE question_review_links (
    review_event_id TEXT PRIMARY KEY REFERENCES review_events(id) ON DELETE CASCADE,
    instance_id TEXT NOT NULL UNIQUE REFERENCES question_attempts(instance_id) ON DELETE CASCADE,
    override_reason TEXT CHECK(override_reason IS NULL OR length(trim(override_reason)) BETWEEN 1 AND 2000)
) STRICT;
CREATE TABLE question_remediation_attempts (
    operation_id TEXT PRIMARY KEY CHECK(length(operation_id) = 32),
    instance_id TEXT NOT NULL REFERENCES question_attempts(instance_id) ON DELETE CASCADE,
    request_hash TEXT NOT NULL CHECK(length(request_hash) = 64),
    answer_json TEXT NOT NULL CHECK(json_valid(answer_json)
        AND length(CAST(answer_json AS BLOB)) <= 16384),
    evaluation_json TEXT NOT NULL CHECK(json_valid(evaluation_json)
        AND length(CAST(evaluation_json AS BLOB)) <= 2000000),
    created_at INTEGER NOT NULL
) STRICT;
CREATE INDEX question_remediation_instance_idx ON question_remediation_attempts(instance_id, created_at);
