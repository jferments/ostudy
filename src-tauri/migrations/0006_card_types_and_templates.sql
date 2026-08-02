-- Add generated reverse/cloze card metadata and reusable data-only authoring templates.
--
-- Generated presentations keep stable `(note_id, direction, template_ordinal)`
-- identities. Disabling a presentation soft-deletes its `cards` row so its
-- schedule and immutable review history can be reused if the presentation is
-- enabled again. Reusable templates are authoring shortcuts only: generated
-- Basic cards deliberately have no foreign key back to a template.

ALTER TABLE notes ADD COLUMN reverse_enabled INTEGER NOT NULL DEFAULT 0
    CHECK(reverse_enabled IN (0, 1));
ALTER TABLE notes ADD COLUMN cloze_source TEXT NOT NULL DEFAULT '';
ALTER TABLE notes ADD COLUMN cloze_mode TEXT NOT NULL DEFAULT 'one_at_a_time'
    CHECK(cloze_mode IN ('one_at_a_time', 'all_together'));

CREATE TABLE card_templates (
    id               TEXT PRIMARY KEY CHECK(length(id) = 32),
    name             TEXT NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 80),
    normalized_name  TEXT NOT NULL CHECK(length(trim(normalized_name)) BETWEEN 1 AND 80),
    description      TEXT NOT NULL DEFAULT '' CHECK(length(description) <= 2000),
    created_at       INTEGER NOT NULL,
    updated_at       INTEGER NOT NULL,
    deleted_at       INTEGER,
    revision         INTEGER NOT NULL DEFAULT 1 CHECK(revision >= 1)
) STRICT;

CREATE UNIQUE INDEX card_templates_active_name_idx
    ON card_templates(normalized_name)
    WHERE deleted_at IS NULL;

CREATE TABLE card_template_fields (
    id               TEXT PRIMARY KEY CHECK(length(id) = 32),
    template_id      TEXT NOT NULL REFERENCES card_templates(id) ON DELETE CASCADE,
    name             TEXT NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 64),
    normalized_name  TEXT NOT NULL CHECK(length(trim(normalized_name)) BETWEEN 1 AND 64),
    description      TEXT NOT NULL DEFAULT '' CHECK(length(description) <= 500),
    position         INTEGER NOT NULL CHECK(position >= 0),
    UNIQUE(template_id, normalized_name),
    UNIQUE(template_id, position)
) STRICT;

CREATE INDEX card_template_fields_template_idx
    ON card_template_fields(template_id, position);

CREATE TABLE card_template_rules (
    id             TEXT PRIMARY KEY CHECK(length(id) = 32),
    template_id    TEXT NOT NULL REFERENCES card_templates(id) ON DELETE CASCADE,
    name           TEXT NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 80),
    front_pattern  TEXT NOT NULL CHECK(length(front_pattern) BETWEEN 1 AND 4000),
    back_pattern   TEXT NOT NULL CHECK(length(back_pattern) BETWEEN 1 AND 4000),
    position       INTEGER NOT NULL CHECK(position >= 0),
    UNIQUE(template_id, name),
    UNIQUE(template_id, position)
) STRICT;

CREATE INDEX card_template_rules_template_idx
    ON card_template_rules(template_id, position);
