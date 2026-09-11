-- Native evidence dependencies and resumable semantic-window receipts. Existing
-- source records, cards, guides, conversations and review history are preserved.
CREATE TABLE knowledge_unit_evidence (
    unit_id TEXT NOT NULL REFERENCES knowledge_units(id) ON DELETE CASCADE,
    evidence_id TEXT NOT NULL CHECK(length(evidence_id) = 32),
    source_id TEXT NOT NULL REFERENCES knowledge_sources(id) ON DELETE CASCADE,
    source_hash TEXT NOT NULL CHECK(length(source_hash) = 64),
    PRIMARY KEY(unit_id, evidence_id)
) STRICT;
CREATE INDEX knowledge_unit_evidence_source_idx ON knowledge_unit_evidence(source_id, unit_id);

CREATE TABLE knowledge_analysis_windows (
    id TEXT PRIMARY KEY CHECK(length(id) = 32),
    document_id TEXT REFERENCES documents(id) ON DELETE CASCADE,
    contract TEXT NOT NULL CHECK(length(contract) = 64),
    state TEXT NOT NULL CHECK(state IN ('complete', 'partial')),
    accepted_count INTEGER NOT NULL CHECK(accepted_count >= 0),
    rejected_count INTEGER NOT NULL CHECK(rejected_count >= 0),
    report_json TEXT NOT NULL CHECK(json_valid(report_json)),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
) STRICT;
CREATE INDEX knowledge_analysis_document_idx ON knowledge_analysis_windows(document_id, state);
CREATE TABLE knowledge_analysis_sources (
    window_id TEXT NOT NULL REFERENCES knowledge_analysis_windows(id) ON DELETE CASCADE,
    source_id TEXT NOT NULL REFERENCES knowledge_sources(id) ON DELETE CASCADE,
    source_hash TEXT NOT NULL CHECK(length(source_hash) = 64),
    PRIMARY KEY(window_id, source_id)
) STRICT;
CREATE INDEX knowledge_analysis_source_idx ON knowledge_analysis_sources(source_id, window_id);

-- A cross-page interpretation depends on EVERY cited source, not merely the
-- primary source used by older artifact APIs. Run before old invalidation
-- triggers delete primary units, so all secondary dependencies are still known.
CREATE TRIGGER knowledge_evidence_source_changed
BEFORE UPDATE OF active, content_hash, owner_revision ON knowledge_sources
WHEN (OLD.active = 1 AND NEW.active = 0) OR OLD.content_hash <> NEW.content_hash
    OR OLD.owner_revision <> NEW.owner_revision
BEGIN
    INSERT OR IGNORE INTO knowledge_cache_evictions(source_id)
        SELECT u.source_id FROM knowledge_units u JOIN knowledge_unit_evidence e ON e.unit_id = u.id
        WHERE e.source_id = OLD.id;
    UPDATE knowledge_sources SET extracted_hash = '', extraction_contract = ''
        WHERE id IN (SELECT a.source_id FROM knowledge_analysis_sources a
            WHERE a.window_id IN (SELECT window_id FROM knowledge_analysis_sources WHERE source_id = OLD.id))
        OR id IN (SELECT u.source_id FROM knowledge_units u JOIN knowledge_unit_evidence e ON e.unit_id = u.id
            WHERE e.source_id = OLD.id);
    DELETE FROM knowledge_units WHERE id IN
        (SELECT unit_id FROM knowledge_unit_evidence WHERE source_id = OLD.id);
    DELETE FROM knowledge_analysis_windows WHERE id IN
        (SELECT window_id FROM knowledge_analysis_sources WHERE source_id = OLD.id);
END;
CREATE TRIGGER knowledge_evidence_source_deleted BEFORE DELETE ON knowledge_sources
BEGIN
    INSERT OR IGNORE INTO knowledge_cache_evictions(source_id)
        SELECT u.source_id FROM knowledge_units u JOIN knowledge_unit_evidence e ON e.unit_id = u.id
        WHERE e.source_id = OLD.id;
    UPDATE knowledge_sources SET extracted_hash = '', extraction_contract = ''
        WHERE id IN (SELECT a.source_id FROM knowledge_analysis_sources a
            WHERE a.window_id IN (SELECT window_id FROM knowledge_analysis_sources WHERE source_id = OLD.id))
        OR id IN (SELECT u.source_id FROM knowledge_units u JOIN knowledge_unit_evidence e ON e.unit_id = u.id
            WHERE e.source_id = OLD.id);
    DELETE FROM knowledge_units WHERE id IN
        (SELECT unit_id FROM knowledge_unit_evidence WHERE source_id = OLD.id);
    DELETE FROM knowledge_analysis_windows WHERE id IN
        (SELECT window_id FROM knowledge_analysis_sources WHERE source_id = OLD.id);
END;

-- Global learning views are derived projections, not source facts. Their small
-- core-ID set is advisory: absence from it never means a unit is unimportant.
CREATE TABLE knowledge_document_views (
    document_id TEXT PRIMARY KEY REFERENCES documents(id) ON DELETE CASCADE,
    analysis_hash TEXT NOT NULL CHECK(length(analysis_hash) = 64),
    contract TEXT NOT NULL CHECK(length(contract) = 64),
    outline_json TEXT NOT NULL CHECK(json_valid(outline_json)),
    core_unit_ids_json TEXT NOT NULL CHECK(json_valid(core_unit_ids_json)),
    reduced INTEGER NOT NULL CHECK(reduced IN (0, 1)),
    created_at INTEGER NOT NULL
);
CREATE TRIGGER knowledge_view_after_unit_insert AFTER INSERT ON knowledge_units BEGIN
    DELETE FROM knowledge_document_views WHERE document_id IN
        (SELECT document_id FROM knowledge_sources WHERE id = NEW.source_id);
END;
CREATE TRIGGER knowledge_view_after_unit_change AFTER UPDATE OF content_hash, extraction_contract ON knowledge_units
WHEN OLD.content_hash <> NEW.content_hash OR OLD.extraction_contract <> NEW.extraction_contract BEGIN
    DELETE FROM knowledge_document_views WHERE document_id IN
        (SELECT document_id FROM knowledge_sources WHERE id = NEW.source_id);
END;
CREATE TRIGGER knowledge_view_before_unit_delete BEFORE DELETE ON knowledge_units BEGIN
    DELETE FROM knowledge_document_views WHERE document_id IN
        (SELECT document_id FROM knowledge_sources WHERE id = OLD.source_id);
END;
CREATE TRIGGER knowledge_view_after_window_insert AFTER INSERT ON knowledge_analysis_windows BEGIN
    DELETE FROM knowledge_document_views WHERE document_id = NEW.document_id;
END;
CREATE TRIGGER knowledge_view_after_window_change AFTER UPDATE OF report_json ON knowledge_analysis_windows
WHEN OLD.report_json <> NEW.report_json BEGIN
    DELETE FROM knowledge_document_views WHERE document_id = NEW.document_id;
END;
CREATE TRIGGER knowledge_view_before_window_delete BEFORE DELETE ON knowledge_analysis_windows BEGIN
    DELETE FROM knowledge_document_views WHERE document_id = OLD.document_id;
END;

-- A display rename must not invalidate v2 canonical document evidence. Authored
-- and legacy records retain their existing conservative invalidation behavior.
DROP TRIGGER knowledge_document_renamed;
CREATE TRIGGER knowledge_document_renamed AFTER UPDATE OF title ON documents
WHEN OLD.title <> NEW.title BEGIN
    UPDATE knowledge_sources SET active = 0 WHERE document_id = NEW.id AND active = 1
      AND (kind <> 'document' OR COALESCE(json_extract(record_json, '$.identityVersion'), 1) <> 2);
END;
