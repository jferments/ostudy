-- Normalize Fill in the blank notes to one scheduled card and add rich template layouts.
--
-- Schema 6 remains immutable because users may already have opened the Stage 3
-- branch. Schema 7 appends the durable fields needed for session-stable cloze
-- presentations and validated structured template documents.

ALTER TABLE practice_session_cards ADD COLUMN presentation_variant TEXT NOT NULL DEFAULT ''
    CHECK(length(presentation_variant) <= 64);

ALTER TABLE card_template_rules ADD COLUMN front_doc TEXT;
ALTER TABLE card_template_rules ADD COLUMN back_doc TEXT;

CREATE TABLE card_template_assets (
    template_id  TEXT NOT NULL REFERENCES card_templates(id) ON DELETE CASCADE,
    asset_id     TEXT NOT NULL REFERENCES assets(id) ON DELETE RESTRICT,
    PRIMARY KEY(template_id, asset_id)
) STRICT;

CREATE INDEX card_template_assets_asset_idx
    ON card_template_assets(asset_id);
