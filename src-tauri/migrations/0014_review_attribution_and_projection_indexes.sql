-- Immutable review/session attribution and indexes for bounded Stage 3.6-3.8 projections.
--
-- Historical rows deliberately do not reference mutable deck/tag rows. Retained
-- deck and tag names are resolved by ID at read time so renames display under the
-- current name. Final deck deletion explicitly purges every snapshot and review
-- source before the deck's Undo window is finalized.

CREATE TABLE practice_session_card_snapshots (
    session_id              TEXT NOT NULL
                            REFERENCES practice_sessions(id) ON DELETE CASCADE,
    card_id                 TEXT NOT NULL,
    note_id                 TEXT NOT NULL,
    note_revision           INTEGER NOT NULL CHECK(note_revision >= 1),
    deck_id                 TEXT NOT NULL,
    captured_deck_name      TEXT NOT NULL,
    card_type               TEXT NOT NULL CHECK(card_type IN ('basic', 'cloze')),
    card_label              TEXT NOT NULL,
    presentation_variant    TEXT NOT NULL DEFAULT '',
    question_doc            TEXT NOT NULL,
    answer_doc              TEXT NOT NULL,
    answer_text             TEXT NOT NULL,
    question_excerpt        TEXT NOT NULL,
    question_text_hash      TEXT NOT NULL CHECK(length(question_text_hash) = 64),
    question_was_truncated  INTEGER NOT NULL CHECK(question_was_truncated IN (0, 1)),
    captured_at             INTEGER NOT NULL,
    PRIMARY KEY(session_id, card_id)
) STRICT;

CREATE TABLE practice_session_card_tag_snapshots (
    session_id      TEXT NOT NULL,
    card_id         TEXT NOT NULL,
    tag_id          TEXT NOT NULL CHECK(length(tag_id) = 32),
    captured_name   TEXT NOT NULL,
    PRIMARY KEY(session_id, card_id, tag_id),
    FOREIGN KEY(session_id, card_id)
        REFERENCES practice_session_card_snapshots(session_id, card_id)
        ON DELETE CASCADE
) STRICT;

CREATE TABLE practice_session_card_asset_snapshots (
    session_id  TEXT NOT NULL,
    card_id     TEXT NOT NULL,
    asset_id    TEXT NOT NULL CHECK(length(asset_id) = 32),
    PRIMARY KEY(session_id, card_id, asset_id),
    FOREIGN KEY(session_id, card_id)
        REFERENCES practice_session_card_snapshots(session_id, card_id)
        ON DELETE CASCADE
) STRICT;

CREATE TABLE review_event_attributions (
    review_event_id        TEXT PRIMARY KEY
                           REFERENCES review_events(id) ON DELETE CASCADE,
    reviewed_at            INTEGER NOT NULL,
    review_local_date      TEXT NOT NULL
                           CHECK(review_local_date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
    time_zone_id           TEXT NOT NULL,
    utc_offset_minutes     INTEGER NOT NULL CHECK(utc_offset_minutes BETWEEN -1440 AND 1440),
    card_id                TEXT NOT NULL,
    note_id                TEXT NOT NULL,
    note_revision          INTEGER NOT NULL CHECK(note_revision >= 1),
    deck_id                TEXT NOT NULL,
    captured_deck_name     TEXT NOT NULL,
    card_type              TEXT NOT NULL CHECK(card_type IN ('basic', 'cloze')),
    card_label             TEXT NOT NULL,
    presentation_variant   TEXT NOT NULL DEFAULT '',
    question_excerpt       TEXT NOT NULL,
    question_text_hash     TEXT NOT NULL CHECK(length(question_text_hash) = 64),
    question_was_truncated INTEGER NOT NULL CHECK(question_was_truncated IN (0, 1)),
    attribution_source     TEXT NOT NULL
                           CHECK(attribution_source IN (
                               'captured',
                               'legacy_current_state_backfill'
                           )),
    captured_at            INTEGER NOT NULL
) STRICT;

CREATE TABLE review_event_tag_attributions (
    review_event_id  TEXT NOT NULL
                     REFERENCES review_event_attributions(review_event_id)
                     ON DELETE CASCADE,
    tag_id           TEXT NOT NULL CHECK(length(tag_id) = 32),
    captured_name    TEXT NOT NULL,
    PRIMARY KEY(review_event_id, tag_id)
) STRICT;

-- Archive ownership records which suspensions were introduced by archiving.
-- Unarchive clears only those exact suspensions and preserves manual suspension.
CREATE TABLE deck_archive_cards (
    deck_id               TEXT NOT NULL REFERENCES decks(id) ON DELETE CASCADE,
    card_id               TEXT NOT NULL REFERENCES cards(id) ON DELETE CASCADE,
    archived_at           INTEGER NOT NULL,
    suspended_before      INTEGER,
    PRIMARY KEY(deck_id, card_id)
) STRICT;

ALTER TABLE schedule_action_operations
    ADD COLUMN reschedule_intent_json TEXT;

CREATE INDEX practice_snapshot_deck_idx
    ON practice_session_card_snapshots(deck_id, captured_at DESC);
CREATE INDEX review_attribution_date_idx
    ON review_event_attributions(review_local_date, reviewed_at DESC);
CREATE INDEX review_attribution_deck_date_idx
    ON review_event_attributions(deck_id, review_local_date, reviewed_at DESC);
CREATE INDEX review_attribution_card_idx
    ON review_event_attributions(card_id, reviewed_at DESC);
CREATE INDEX review_tag_attribution_idx
    ON review_event_tag_attributions(tag_id, review_event_id);
CREATE INDEX cards_studyability_idx
    ON cards(deck_id, suspended_at, buried_until, id)
    WHERE deleted_at IS NULL;
CREATE INDEX review_events_learning_epoch_idx
    ON review_events(card_id, reviewed_at DESC, created_at DESC, id DESC);
