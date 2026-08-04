-- Promote manual schedule changes from one-card mutations to transactional batches.
--
-- Each committed request now has one parent operation row containing its
-- canonical identity and aggregate result. Per-card audit rows reference that
-- operation. Existing one-card events are preserved as one-card operations.
DROP INDEX schedule_action_events_card_time_idx;

ALTER TABLE schedule_action_events RENAME TO schedule_action_events_legacy;

CREATE TABLE schedule_action_operations (
    id                    TEXT PRIMARY KEY CHECK(length(id) = 32),
    request_fingerprint   TEXT,
    session_id            TEXT REFERENCES practice_sessions(id) ON DELETE SET NULL,
    action                TEXT NOT NULL CHECK(action IN (
                              'suspend', 'resume', 'bury', 'unbury',
                              'reset', 'reschedule'
                          )),
    due_at                INTEGER,
    requested_count       INTEGER NOT NULL
                          CHECK(requested_count BETWEEN 1 AND 10000),
    changed_count         INTEGER NOT NULL CHECK(changed_count >= 0),
    unchanged_count       INTEGER NOT NULL CHECK(unchanged_count >= 0),
    rejected_count        INTEGER NOT NULL CHECK(rejected_count >= 0),
    occurred_at           INTEGER NOT NULL,
    created_at            INTEGER NOT NULL,
    CHECK(changed_count + unchanged_count + rejected_count = requested_count),
    CHECK(
        (action = 'reschedule' AND due_at IS NOT NULL)
        OR (action <> 'reschedule' AND due_at IS NULL)
    )
) STRICT;

INSERT INTO schedule_action_operations (
    id, request_fingerprint, session_id, action, due_at,
    requested_count, changed_count, unchanged_count, rejected_count,
    occurred_at, created_at
)
SELECT mutation_id, NULL, session_id, action,
       CASE WHEN action = 'reschedule' THEN due_after ELSE NULL END,
       1, 1, 0, 0, occurred_at, created_at
FROM schedule_action_events_legacy;

CREATE TABLE schedule_action_events (
    id                    TEXT PRIMARY KEY CHECK(length(id) = 32),
    operation_id          TEXT NOT NULL
                          REFERENCES schedule_action_operations(id) ON DELETE RESTRICT,
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
    created_at            INTEGER NOT NULL,
    UNIQUE(operation_id, card_id)
) STRICT;

INSERT INTO schedule_action_events (
    id, operation_id, session_id, card_id, action,
    due_before, due_after, state_before, state_after,
    suspended_before, suspended_after,
    buried_until_before, buried_until_after,
    schedule_revision_before, schedule_revision_after,
    occurred_at, created_at
)
SELECT id, mutation_id, session_id, card_id, action,
       due_before, due_after, state_before, state_after,
       suspended_before, suspended_after,
       buried_until_before, buried_until_after,
       schedule_revision_before, schedule_revision_after,
       occurred_at, created_at
FROM schedule_action_events_legacy;

DROP TABLE schedule_action_events_legacy;

CREATE INDEX schedule_action_events_card_time_idx
    ON schedule_action_events(card_id, occurred_at DESC);

CREATE INDEX schedule_action_events_operation_idx
    ON schedule_action_events(operation_id);

CREATE INDEX schedule_action_operations_time_idx
    ON schedule_action_operations(occurred_at DESC);
