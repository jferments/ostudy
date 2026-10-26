-- Release migration 24: Tutor answers that a newer version replaced.
--
-- Regenerate, and editing a question and sending it again, write a new turn
-- that replaces the conversation's last one. The replaced turn keeps its
-- text, so the learner can still look back at it, but the chat carries on
-- from the newer version: history, summaries and conversation retrieval
-- leave a replaced turn out. Deleting either turn deletes the row, and a
-- turn is replaced at most once.
--
-- The upgrade creates the table and writes no rows.
CREATE TABLE tutor_turn_replacements (
    turn_id              TEXT PRIMARY KEY REFERENCES tutor_turns(id) ON DELETE CASCADE
                         CHECK(length(turn_id) = 32),
    replaced_by_turn_id  TEXT NOT NULL UNIQUE REFERENCES tutor_turns(id) ON DELETE CASCADE
                         CHECK(length(replaced_by_turn_id) = 32),
    created_at           INTEGER NOT NULL CHECK(created_at >= 0),
    CHECK(replaced_by_turn_id <> turn_id)
) STRICT;
