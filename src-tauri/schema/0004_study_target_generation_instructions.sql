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
