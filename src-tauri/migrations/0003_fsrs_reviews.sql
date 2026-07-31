-- Add deterministic spaced-repetition scheduling and review history.
--
-- The mutable schedule is stored separately from the immutable review ledger:
--   * card_schedules answers "what is due now?" efficiently.
--   * review_events records every accepted answer and makes later analytics,
--     debugging, and scheduler migrations possible.
--
-- Existing cards begin in the FSRS New state and are immediately due. Imported
-- decks intentionally receive fresh local schedules rather than inheriting the
-- original learner's history.

CREATE TABLE card_schedules (
    card_id             TEXT PRIMARY KEY REFERENCES cards(id) ON DELETE CASCADE,
    state               INTEGER NOT NULL CHECK(state BETWEEN 0 AND 3),
    due_at              INTEGER NOT NULL,
    stability           REAL NOT NULL DEFAULT 0.0 CHECK(stability >= 0.0),
    difficulty          REAL NOT NULL DEFAULT 0.0 CHECK(difficulty >= 0.0),
    elapsed_days        INTEGER NOT NULL DEFAULT 0 CHECK(elapsed_days >= 0),
    scheduled_days      INTEGER NOT NULL DEFAULT 0 CHECK(scheduled_days >= 0),
    reps                INTEGER NOT NULL DEFAULT 0 CHECK(reps >= 0),
    lapses              INTEGER NOT NULL DEFAULT 0 CHECK(lapses >= 0),
    last_review_at      INTEGER NOT NULL,
    scheduler_name      TEXT NOT NULL,
    scheduler_version   TEXT NOT NULL,
    updated_at          INTEGER NOT NULL
) STRICT;

CREATE INDEX card_schedules_due_idx
    ON card_schedules(due_at, state);

INSERT INTO card_schedules (
    card_id, state, due_at, stability, difficulty, elapsed_days,
    scheduled_days, reps, lapses, last_review_at,
    scheduler_name, scheduler_version, updated_at
)
SELECT
    c.id, 0, c.created_at, 0.0, 0.0, 0,
    0, 0, 0, c.created_at,
    'fsrs', 'rs-fsrs-1.2.1', c.created_at
FROM cards c
JOIN notes n ON n.id = c.note_id
WHERE c.deleted_at IS NULL
  AND n.deleted_at IS NULL;

CREATE TABLE review_events (
    id                      TEXT PRIMARY KEY CHECK(length(id) = 32),
    submission_id           TEXT NOT NULL UNIQUE CHECK(length(submission_id) = 32),
    session_id              TEXT REFERENCES practice_sessions(id) ON DELETE SET NULL,
    card_id                 TEXT NOT NULL REFERENCES cards(id) ON DELETE RESTRICT,
    reviewed_at             INTEGER NOT NULL,
    rating                  INTEGER NOT NULL CHECK(rating BETWEEN 1 AND 4),
    answer_correct          INTEGER NOT NULL CHECK(answer_correct IN (0, 1)),
    confidence              TEXT NOT NULL
                            CHECK(confidence IN (
                                'wrong',
                                'not_confident',
                                'somewhat_confident',
                                'very_confident'
                            )),
    response_time_ms        INTEGER NOT NULL
                            CHECK(response_time_ms BETWEEN 0 AND 86400000),
    state_before            INTEGER NOT NULL CHECK(state_before BETWEEN 0 AND 3),
    state_after             INTEGER NOT NULL CHECK(state_after BETWEEN 0 AND 3),
    due_before              INTEGER NOT NULL,
    due_after               INTEGER NOT NULL,
    stability_before        REAL NOT NULL CHECK(stability_before >= 0.0),
    stability_after         REAL NOT NULL CHECK(stability_after >= 0.0),
    difficulty_before       REAL NOT NULL CHECK(difficulty_before >= 0.0),
    difficulty_after        REAL NOT NULL CHECK(difficulty_after >= 0.0),
    scheduled_days_before   INTEGER NOT NULL CHECK(scheduled_days_before >= 0),
    scheduled_days_after    INTEGER NOT NULL CHECK(scheduled_days_after >= 0),
    scheduler_name          TEXT NOT NULL,
    scheduler_version       TEXT NOT NULL,
    created_at              INTEGER NOT NULL
) STRICT;

CREATE INDEX review_events_card_reviewed_idx
    ON review_events(card_id, reviewed_at DESC);

CREATE INDEX review_events_reviewed_idx
    ON review_events(reviewed_at DESC);

ALTER TABLE practice_session_cards ADD COLUMN rated_at INTEGER;
ALTER TABLE practice_session_cards ADD COLUMN rating INTEGER
    CHECK(rating IS NULL OR rating BETWEEN 1 AND 4);
ALTER TABLE practice_session_cards ADD COLUMN response_time_ms INTEGER
    CHECK(response_time_ms IS NULL OR response_time_ms >= 0);

ALTER TABLE practice_sessions ADD COLUMN rated_count INTEGER NOT NULL DEFAULT 0
    CHECK(rated_count >= 0);
ALTER TABLE practice_sessions ADD COLUMN correct_count INTEGER NOT NULL DEFAULT 0
    CHECK(correct_count >= 0);

INSERT OR IGNORE INTO app_settings(key, value_json, updated_at)
VALUES ('desiredRetention', '0.9', 0);
