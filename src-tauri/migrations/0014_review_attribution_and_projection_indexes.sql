-- Stage 3.6-3.8: authoritative studyability, durable review presentation
-- attribution, and indexes for bounded summary/aggregate projections.
--
-- Review history remains owned by the generated card. Moving a note/card to a
-- new deck therefore carries its complete review history to the new owner.
-- Captured deck labels below are audit facts about the presentation session;
-- analytics resolves deck ownership and display names from the card's current
-- owner. Permanent card/deck deletion purges the owned review rows after the
-- undo window expires.

ALTER TABLE cards ADD COLUMN archive_suspended_at INTEGER;
ALTER TABLE schedule_action_operations ADD COLUMN reschedule_intent_json TEXT;
ALTER TABLE practice_sessions ADD COLUMN selected_tag_ids_json TEXT NOT NULL DEFAULT '[]';

CREATE TABLE practice_session_card_snapshots (
    session_id                TEXT NOT NULL
                              REFERENCES practice_sessions(id) ON DELETE CASCADE,
    card_id                   TEXT NOT NULL CHECK(length(card_id) = 32),
    note_id                   TEXT NOT NULL CHECK(length(note_id) = 32),
    note_revision             INTEGER NOT NULL CHECK(note_revision >= 1),
    owner_deck_id_at_capture  TEXT NOT NULL CHECK(length(owner_deck_id_at_capture) = 32),
    captured_deck_name        TEXT NOT NULL,
    direction                 TEXT NOT NULL CHECK(direction IN ('forward', 'reverse')),
    card_type                 TEXT NOT NULL CHECK(card_type IN ('basic', 'cloze')),
    card_label                TEXT NOT NULL,
    presentation_variant      TEXT NOT NULL,
    front_doc_json            TEXT NOT NULL,
    back_doc_json             TEXT NOT NULL,
    front_text                TEXT NOT NULL,
    back_text                 TEXT NOT NULL,
    question_excerpt          TEXT NOT NULL,
    question_text_hash        TEXT NOT NULL CHECK(length(question_text_hash) = 64),
    question_was_truncated    INTEGER NOT NULL CHECK(question_was_truncated IN (0, 1)),
    captured_at               INTEGER NOT NULL,
    PRIMARY KEY(session_id, card_id)
) STRICT;

CREATE TABLE practice_session_card_tag_snapshots (
    session_id  TEXT NOT NULL,
    card_id     TEXT NOT NULL CHECK(length(card_id) = 32),
    tag_id      TEXT NOT NULL CHECK(length(tag_id) = 32),
    tag_name    TEXT NOT NULL,
    PRIMARY KEY(session_id, card_id, tag_id),
    FOREIGN KEY(session_id, card_id)
        REFERENCES practice_session_card_snapshots(session_id, card_id)
        ON DELETE CASCADE
) STRICT;

CREATE TABLE review_event_attributions (
    review_event_id            TEXT PRIMARY KEY
                               REFERENCES review_events(id) ON DELETE CASCADE,
    reviewed_at                INTEGER NOT NULL,
    review_local_date          TEXT NOT NULL
                               CHECK(review_local_date GLOB
                                   '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
    time_zone_id               TEXT NOT NULL,
    utc_offset_minutes         INTEGER NOT NULL CHECK(utc_offset_minutes BETWEEN -1440 AND 1440),
    card_id                    TEXT NOT NULL CHECK(length(card_id) = 32),
    note_id                    TEXT NOT NULL CHECK(length(note_id) = 32),
    note_revision              INTEGER NOT NULL CHECK(note_revision >= 1),
    owner_deck_id_at_review    TEXT NOT NULL CHECK(length(owner_deck_id_at_review) = 32),
    captured_deck_name         TEXT NOT NULL,
    direction                  TEXT NOT NULL CHECK(direction IN ('forward', 'reverse')),
    card_type                  TEXT NOT NULL CHECK(card_type IN ('basic', 'cloze')),
    card_label                 TEXT NOT NULL,
    presentation_variant       TEXT NOT NULL,
    question_excerpt           TEXT NOT NULL,
    question_text_hash         TEXT NOT NULL CHECK(length(question_text_hash) = 64),
    question_was_truncated     INTEGER NOT NULL CHECK(question_was_truncated IN (0, 1)),
    attribution_source         TEXT NOT NULL
                               CHECK(attribution_source IN (
                                   'captured',
                                   'legacy_current_state_backfill'
                               ))
) STRICT;

CREATE TABLE review_event_tag_attributions (
    review_event_id  TEXT NOT NULL
                     REFERENCES review_event_attributions(review_event_id)
                     ON DELETE CASCADE,
    tag_id           TEXT NOT NULL CHECK(length(tag_id) = 32),
    tag_name         TEXT NOT NULL,
    PRIMARY KEY(review_event_id, tag_id)
) STRICT;

CREATE INDEX practice_session_card_snapshots_session_position_idx
    ON practice_session_card_snapshots(session_id, card_id);
CREATE INDEX practice_session_card_tag_snapshots_tag_idx
    ON practice_session_card_tag_snapshots(tag_id, session_id, card_id);
CREATE INDEX review_event_attributions_local_date_idx
    ON review_event_attributions(review_local_date, reviewed_at DESC, review_event_id);
CREATE INDEX review_event_attributions_card_date_idx
    ON review_event_attributions(card_id, review_local_date, reviewed_at DESC);
CREATE INDEX review_event_attributions_capture_deck_idx
    ON review_event_attributions(owner_deck_id_at_review, review_local_date, reviewed_at DESC);
CREATE INDEX review_event_tag_attributions_tag_idx
    ON review_event_tag_attributions(tag_id, review_event_id);
CREATE INDEX cards_studyability_idx
    ON cards(deck_id, suspended_at, archive_suspended_at, buried_until, id)
    WHERE deleted_at IS NULL;
CREATE INDEX review_events_card_reviewed_created_idx
    ON review_events(card_id, reviewed_at DESC, created_at DESC, id DESC);
