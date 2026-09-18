-- Pending inputs are not completion evidence. The final report is published in
-- the same transaction as its notes, presentations, provenance and job success.
CREATE TABLE question_generation_reports (
    job_id TEXT PRIMARY KEY REFERENCES document_generation_jobs(id) ON DELETE CASCADE,
    document_id TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    policy_json TEXT NOT NULL CHECK (
        json_valid(policy_json) AND json_type(policy_json) = 'object'
        AND length(CAST(policy_json AS BLOB)) <= 16384
    ),
    available_knowledge_units INTEGER NOT NULL CHECK (available_knowledge_units BETWEEN 0 AND 10000000),
    candidate_count INTEGER CHECK (candidate_count BETWEEN 0 AND 10000),
    report_json TEXT CHECK (
        report_json IS NULL OR (
            json_valid(report_json) AND json_extract(report_json, '$.schemaVersion') IS 1
            AND length(CAST(report_json AS BLOB)) <= 65536
        )
    ),
    created_at INTEGER NOT NULL CHECK (created_at >= 0)
) STRICT;
CREATE INDEX question_generation_reports_document
    ON question_generation_reports(document_id, created_at DESC, job_id DESC);
CREATE TRIGGER question_generation_reports_immutable
BEFORE UPDATE ON question_generation_reports WHEN OLD.report_json IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'completed generation reports are immutable');
END;
