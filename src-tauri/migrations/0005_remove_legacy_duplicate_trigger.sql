-- Remove the temporary duplicate-hash trigger now that every supported card
-- write path updates duplicate_hash atomically with the card text.
DROP TRIGGER IF EXISTS notes_duplicate_hash_after_legacy_update;
