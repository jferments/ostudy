-- Add explicit archive ownership and immutable review-time attribution.
--
-- FSRS due timestamps remain exact UTC instants. Local study dates and captured
-- attribution are append-only evidence used by daily limits and historical
-- Analytics. Mutable deck/tag names are intentionally not duplicated as the
-- canonical display label: Analytics resolves retained IDs through current rows
-- so renames appear under their new names. Deleted-deck history is hidden
-- immediately and purged when the deletion Undo window is finalized.

CREATE TABLE deck_archive_cards (
    deck_id       TEXT NOT NULL REFERENCES decks(id) ON DELETE CASCADE,
    card_id       TEXT NOT NULL REFERENCES cards(id) ON DELETE CASCADE,
    archived_at   INTEGER NOT NULL,
    PRIMARY KEY(deck_id, card_id)
) STRICT;

CREATE INDEX deck_archive_cards_card_idx
    ON deck_archive_cards(card_id);

CREATE TABLE practice_session_card_snapshots (
    session_id              TEXT NOT NULL REFERENCES practice_sessions(id) ON DELETE CASCADE,
    card_id                 TEXT NOT NULL REFERENCES cards(id) ON DELETE RESTRICT,
    note_id                 TEXT NOT NULL CHECK(length(note_id) = 32),
    note_revision           INTEGER NOT NULL CHECK(note_revision >= 1),
    deck_id                 TEXT NOT NULL CHECK(length(deck_id) = 32),
    card_type               TEXT NOT NULL CHECK(card_type IN ('basic', 'cloze')),
    card_label              TEXT NOT NULL,
    presentation_variant    TEXT NOT NULL,
    question_excerpt        TEXT NOT NULL,
    question_text_hash      TEXT NOT NULL CHECK(length(question_text_hash) = 64),
    question_was_truncated  INTEGER NOT NULL CHECK(question_was_truncated IN (0, 1)),
    captured_at             INTEGER NOT NULL,
    PRIMARY KEY(session_id, card_id)
) STRICT;

CREATE INDEX practice_session_card_snapshots_deck_idx
    ON practice_session_card_snapshots(deck_id, captured_at DESC);

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

CREATE INDEX practice_session_card_tag_snapshots_tag_idx
    ON practice_session_card_tag_snapshots(tag_id, session_id);

CREATE TABLE review_event_attributions (
    review_event_id          TEXT PRIMARY KEY REFERENCES review_events(id) ON DELETE CASCADE,
    card_id                  TEXT NOT NULL CHECK(length(card_id) = 32),
    note_id                  TEXT NOT NULL CHECK(length(note_id) = 32),
    note_revision            INTEGER NOT NULL CHECK(note_revision >= 1),
    deck_id                  TEXT NOT NULL CHECK(length(deck_id) = 32),
    card_type                TEXT NOT NULL CHECK(card_type IN ('basic', 'cloze')),
    card_label               TEXT NOT NULL,
    presentation_variant     TEXT NOT NULL,
    question_excerpt         TEXT NOT NULL,
    question_text_hash       TEXT NOT NULL CHECK(length(question_text_hash) = 64),
    question_was_truncated   INTEGER NOT NULL CHECK(question_was_truncated IN (0, 1)),
    review_local_date        TEXT NOT NULL CHECK(
                                 length(review_local_date) = 10
                                 AND substr(review_local_date, 5, 1) = '-'
                                 AND substr(review_local_date, 8, 1) = '-'
                              ),
    utc_offset_minutes       INTEGER NOT NULL CHECK(utc_offset_minutes BETWEEN -1440 AND 1440),
    time_zone_id             TEXT NOT NULL,
    attribution_source       TEXT NOT NULL CHECK(attribution_source IN (
                                 'captured', 'legacy_current_state_backfill'
                              )),
    captured_at              INTEGER NOT NULL
) STRICT;

CREATE INDEX review_event_attributions_local_date_idx
    ON review_event_attributions(review_local_date, review_event_id);

CREATE INDEX review_event_attributions_deck_date_idx
    ON review_event_attributions(deck_id, review_local_date, review_event_id);

CREATE INDEX review_event_attributions_card_idx
    ON review_event_attributions(card_id, review_event_id);

CREATE TABLE review_event_tag_attributions (
    review_event_id  TEXT NOT NULL REFERENCES review_events(id) ON DELETE CASCADE,
    tag_id           TEXT NOT NULL CHECK(length(tag_id) = 32),
    captured_name    TEXT NOT NULL,
    PRIMARY KEY(review_event_id, tag_id)
) STRICT;

CREATE INDEX review_event_tag_attributions_tag_idx
    ON review_event_tag_attributions(tag_id, review_event_id);

CREATE INDEX cards_studyability_idx
    ON cards(deck_id, suspended_at, buried_until, deleted_at, id);

CREATE INDEX review_events_session_reviewed_idx
    ON review_events(session_id, reviewed_at DESC)
    WHERE session_id IS NOT NULL;
