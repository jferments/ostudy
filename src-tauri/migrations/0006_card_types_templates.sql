-- Phase 2 Stage 3: reverse cards, Fill in the blank notes, and reusable
-- data-only card-generation templates.
--
-- Templates are intentionally independent from generated cards. Filling a
-- template creates ordinary Basic notes/cards in the chosen deck. Deleting or
-- editing the template later cannot change or delete cards that were already
-- generated from it.

CREATE TABLE card_templates (
    id               TEXT PRIMARY KEY CHECK(length(id) = 32),
    name             TEXT NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 80),
    normalized_name  TEXT NOT NULL CHECK(length(trim(normalized_name)) BETWEEN 1 AND 80),
    description      TEXT NOT NULL DEFAULT '' CHECK(length(description) <= 2000),
    fields_json      TEXT NOT NULL CHECK(json_valid(fields_json)),
    rules_json       TEXT NOT NULL CHECK(json_valid(rules_json)),
    created_at       INTEGER NOT NULL,
    updated_at       INTEGER NOT NULL,
    deleted_at       INTEGER,
    revision         INTEGER NOT NULL DEFAULT 1 CHECK(revision >= 1)
) STRICT;

CREATE UNIQUE INDEX card_templates_active_name_idx
    ON card_templates(normalized_name)
    WHERE deleted_at IS NULL;

-- Existing notes remain Basic forward-only notes because every new column has a
-- safe default. These columns are durable source data, not legacy API shims.
ALTER TABLE notes ADD COLUMN reverse_enabled
    INTEGER NOT NULL DEFAULT 0 CHECK(reverse_enabled IN (0, 1));
ALTER TABLE notes ADD COLUMN cloze_text
    TEXT NOT NULL DEFAULT '';
ALTER TABLE notes ADD COLUMN cloze_mode
    TEXT NOT NULL DEFAULT 'one_at_a_time'
    CHECK(cloze_mode IN ('one_at_a_time', 'all_together'));
