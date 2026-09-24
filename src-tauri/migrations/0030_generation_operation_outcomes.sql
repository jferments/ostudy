-- Operation results exist independently of physical provider requests.
-- NULL preserves unknown historical outcomes; no prior error is inferred.
ALTER TABLE generation_operations ADD COLUMN outcome_json TEXT
    CHECK (outcome_json IS NULL OR
        (length(outcome_json) <= 1024 AND json_valid(outcome_json)
         AND json_type(outcome_json) = 'object'));
