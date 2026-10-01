-- Immutable replay receipts for an atomic deck merge. These rows own no cards,
-- decks, or source files; later edits/deletion must not turn retry into a write.
CREATE TABLE deck_merge_operations (
    operation_id      TEXT PRIMARY KEY CHECK(length(operation_id) = 32),
    request_sha256    TEXT NOT NULL CHECK(length(request_sha256) = 64),
    result_json       TEXT NOT NULL CHECK(
        json_valid(result_json) AND length(result_json) BETWEEN 2 AND 32768
    ),
    created_at        INTEGER NOT NULL CHECK(created_at >= 0)
) STRICT;

-- Exact learner-authored flashcard guidance captured for one StudyTarget batch.
-- It is operation intent, not source evidence or a global provider preference.
CREATE TABLE study_target_generation_instructions (
    operation_id             TEXT PRIMARY KEY
                             REFERENCES study_target_batches(operation_id)
                             ON DELETE CASCADE
                             CHECK(length(operation_id) = 32),
    generation_instructions  TEXT NOT NULL
                             CHECK(length(CAST(generation_instructions AS BLOB))
                                   BETWEEN 1 AND 16384),
    created_at               INTEGER NOT NULL CHECK(created_at >= 0)
) STRICT;
