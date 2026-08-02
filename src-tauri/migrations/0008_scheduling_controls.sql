-- Add Stage 5 scheduling controls and bounded workload metadata.
--
-- Existing schedules and review history remain authoritative. Manual schedule
-- actions append audit rows rather than rewriting review_events.

ALTER TABLE cards ADD COLUMN buried_until INTEGER;

ALTER TABLE card_schedules ADD COLUMN revision INTEGER NOT NULL DEFAULT 1
    CHECK(revision >= 1);

ALTER TABLE practice_sessions ADD COLUMN mode TEXT NOT NULL DEFAULT 'scheduled'
    CHECK(mode IN ('scheduled', 'extra', 'custom'));

ALTER TABLE practice_session_cards ADD COLUMN dismissed_at INTEGER;
ALTER TABLE practice_session_cards ADD COLUMN disposition TEXT
    CHECK(disposition IS NULL OR disposition IN ('buried', 'suspended'));

CREATE TABLE schedule_action_events (
    id                    TEXT PRIMARY KEY CHECK(length(id) = 32),
    mutation_id           TEXT NOT NULL UNIQUE CHECK(length(mutation_id) = 32),
    session_id            TEXT REFERENCES practice_sessions(id) ON DELETE SET NULL,
    card_id               TEXT NOT NULL REFERENCES cards(id) ON DELETE RESTRICT,
    action                TEXT NOT NULL CHECK(action IN (
                              'suspend', 'resume', 'bury', 'unbury',
                              'reset', 'reschedule'
                          )),
    due_before            INTEGER NOT NULL,
    due_after             INTEGER NOT NULL,
    state_before          INTEGER NOT NULL CHECK(state_before BETWEEN 0 AND 3),
    state_after           INTEGER NOT NULL CHECK(state_after BETWEEN 0 AND 3),
    suspended_before      INTEGER,
    suspended_after       INTEGER,
    buried_until_before   INTEGER,
    buried_until_after    INTEGER,
    schedule_revision_before INTEGER NOT NULL CHECK(schedule_revision_before >= 1),
    schedule_revision_after  INTEGER NOT NULL CHECK(schedule_revision_after >= 1),
    occurred_at           INTEGER NOT NULL,
    created_at            INTEGER NOT NULL
) STRICT;

CREATE INDEX schedule_action_events_card_time_idx
    ON schedule_action_events(card_id, occurred_at DESC);

CREATE INDEX cards_buried_until_idx
    ON cards(buried_until)
    WHERE buried_until IS NOT NULL;

INSERT OR IGNORE INTO app_settings(key, value_json, updated_at) VALUES
    ('dailyNewLimit', '20', 0),
    ('dailyReviewLimit', '200', 0);
