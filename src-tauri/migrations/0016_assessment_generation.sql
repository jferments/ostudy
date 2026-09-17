-- Native component evidence is private library provenance, not user-authored
-- question content or an objective statement that a model proved a claim true.
CREATE TABLE question_generation_evidence (
    note_id TEXT PRIMARY KEY NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    note_revision INTEGER NOT NULL CHECK (note_revision > 0),
    document_id TEXT REFERENCES documents(id) ON DELETE SET NULL,
    question_hash TEXT NOT NULL CHECK (length(question_hash) = 64),
    opportunity_id TEXT NOT NULL CHECK (length(opportunity_id) = 64),
    policy_hash TEXT NOT NULL CHECK (length(policy_hash) = 64),
    record_json TEXT NOT NULL CHECK (
        json_valid(record_json) AND length(record_json) <= 262144
        AND json_extract(record_json, '$.schemaVersion') IS 1
    ),
    created_at INTEGER NOT NULL CHECK (created_at >= 0)
) STRICT;
CREATE INDEX question_generation_evidence_document
    ON question_generation_evidence(document_id, note_id);

CREATE TABLE question_generation_settings (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    revision INTEGER NOT NULL CHECK (revision > 0),
    policy_json TEXT NOT NULL CHECK (json_valid(policy_json) AND length(policy_json) <= 16384 AND json_type(policy_json) = 'object')
) STRICT;
