-- What FSRS knew when it scheduled one review, plus how long the card was
-- visible before it was rated. Append-only evidence written in the review's own
-- transaction. Reviews recorded before this table existed have no row.
CREATE TABLE review_event_scheduler_context (
    review_event_id              TEXT PRIMARY KEY
                                 REFERENCES review_events(id) ON DELETE CASCADE
                                 CHECK(length(review_event_id) = 32),
    previous_review_at           INTEGER
                                 CHECK(previous_review_at IS NULL OR previous_review_at >= 0),
    elapsed_days                 INTEGER NOT NULL CHECK(elapsed_days >= 0),
    retrievability               REAL
                                 CHECK(retrievability IS NULL
                                       OR retrievability BETWEEN 0.0 AND 1.0),
    reps_before                  INTEGER NOT NULL CHECK(reps_before >= 0),
    lapses_before                INTEGER NOT NULL CHECK(lapses_before >= 0),
    desired_retention            REAL NOT NULL
                                 CHECK(desired_retention BETWEEN 0.80 AND 0.97),
    scheduler_parameters_sha256  TEXT NOT NULL
                                 CHECK(length(scheduler_parameters_sha256) = 64
                                       AND scheduler_parameters_sha256 NOT GLOB '*[^0-9a-f]*'),
    active_time_ms               INTEGER
                                 CHECK(active_time_ms IS NULL
                                       OR active_time_ms BETWEEN 0 AND 86400000)
) STRICT;
