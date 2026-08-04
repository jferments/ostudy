-- Bind retry identifiers to their canonical requests and preserve committed responses.
--
-- Review submissions already used unique submission identifiers, but a repeated
-- identifier was not proven to represent the same request. Practice-session
-- starts also had no operation identity, so a lost response could create a
-- second durable session on retry.
--
-- New rows populate these nullable columns. Existing rows remain valid and use
-- compatibility comparisons when replayed.
ALTER TABLE review_events ADD COLUMN request_fingerprint TEXT;
ALTER TABLE review_events ADD COLUMN result_completed INTEGER
    CHECK(result_completed IS NULL OR result_completed IN (0, 1));
ALTER TABLE review_events ADD COLUMN result_rated_count INTEGER
    CHECK(result_rated_count IS NULL OR result_rated_count >= 0);
ALTER TABLE review_events ADD COLUMN result_correct_count INTEGER
    CHECK(result_correct_count IS NULL OR result_correct_count >= 0);

ALTER TABLE practice_sessions ADD COLUMN start_operation_id TEXT
    CHECK(start_operation_id IS NULL OR length(start_operation_id) = 32);
ALTER TABLE practice_sessions ADD COLUMN start_request_fingerprint TEXT;

CREATE UNIQUE INDEX practice_sessions_start_operation_idx
    ON practice_sessions(start_operation_id)
    WHERE start_operation_id IS NOT NULL;
