-- LK-06 collection synthesis is a derived, compact projection over retained
-- source knowledge. It stores only synthesis/dependency identities; source facts,
-- canonical evidence and embeddings remain owned by their existing tables.
CREATE TABLE knowledge_collection_views (
    collection_id TEXT PRIMARY KEY
        REFERENCES document_collections(id) ON DELETE CASCADE,
    snapshot_id TEXT NOT NULL CHECK(length(snapshot_id) = 64),
    analysis_hash TEXT NOT NULL CHECK(length(analysis_hash) = 64),
    view_version INTEGER NOT NULL CHECK(view_version = 1),
    synthesis_json TEXT NOT NULL
        CHECK(json_valid(synthesis_json) AND length(CAST(synthesis_json AS BLOB)) <= 2097152),
    reduced INTEGER NOT NULL CHECK(reduced IN (0, 1)),
    stale INTEGER NOT NULL DEFAULT 0 CHECK(stale IN (0, 1)),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
) STRICT;

CREATE TABLE knowledge_collection_view_dependencies (
    collection_id TEXT NOT NULL
        REFERENCES knowledge_collection_views(collection_id) ON DELETE CASCADE,
    dependency_kind TEXT NOT NULL
        CHECK(dependency_kind IN ('document', 'source', 'unit', 'coverage', 'disposition')),
    dependency_id TEXT NOT NULL CHECK(length(dependency_id) BETWEEN 1 AND 160),
    dependency_hash TEXT NOT NULL CHECK(length(dependency_hash) = 64),
    PRIMARY KEY(collection_id, dependency_kind, dependency_id)
) STRICT;
CREATE INDEX knowledge_collection_view_dependency_lookup
ON knowledge_collection_view_dependencies(dependency_kind, dependency_id);

CREATE TRIGGER knowledge_collection_view_collection_insert
AFTER INSERT ON document_collections BEGIN
    UPDATE knowledge_collection_views SET stale = 1;
END;
CREATE TRIGGER knowledge_collection_view_collection_update
AFTER UPDATE ON document_collections BEGIN
    UPDATE knowledge_collection_views SET stale = 1;
END;
CREATE TRIGGER knowledge_collection_view_collection_delete
BEFORE DELETE ON document_collections BEGIN
    UPDATE knowledge_collection_views SET stale = 1;
END;
CREATE TRIGGER knowledge_collection_view_membership_insert
AFTER INSERT ON document_collection_memberships BEGIN
    UPDATE knowledge_collection_views SET stale = 1;
END;
CREATE TRIGGER knowledge_collection_view_membership_update
AFTER UPDATE ON document_collection_memberships BEGIN
    UPDATE knowledge_collection_views SET stale = 1;
END;
CREATE TRIGGER knowledge_collection_view_membership_delete
BEFORE DELETE ON document_collection_memberships BEGIN
    UPDATE knowledge_collection_views SET stale = 1;
END;
CREATE TRIGGER knowledge_collection_view_range_insert
AFTER INSERT ON document_collection_ranges BEGIN
    UPDATE knowledge_collection_views SET stale = 1;
END;
CREATE TRIGGER knowledge_collection_view_range_update
AFTER UPDATE ON document_collection_ranges BEGIN
    UPDATE knowledge_collection_views SET stale = 1;
END;
CREATE TRIGGER knowledge_collection_view_range_delete
BEFORE DELETE ON document_collection_ranges BEGIN
    UPDATE knowledge_collection_views SET stale = 1;
END;

CREATE TRIGGER knowledge_collection_view_document_change
AFTER UPDATE OF sha256, revision, source_unit_count, source_unit_kind ON documents
WHEN OLD.sha256 <> NEW.sha256 OR OLD.revision <> NEW.revision
    OR OLD.source_unit_count <> NEW.source_unit_count
    OR OLD.source_unit_kind <> NEW.source_unit_kind
BEGIN
    UPDATE knowledge_collection_views SET stale = 1
    WHERE collection_id IN (
        SELECT collection_id FROM knowledge_collection_view_dependencies
        WHERE dependency_kind = 'document' AND dependency_id = NEW.id
    );
END;

CREATE TRIGGER knowledge_collection_view_source_insert
AFTER INSERT ON knowledge_sources WHEN NEW.document_id IS NOT NULL BEGIN
    UPDATE knowledge_collection_views SET stale = 1
    WHERE collection_id IN (
        SELECT collection_id FROM knowledge_collection_view_dependencies
        WHERE (dependency_kind = 'document' AND dependency_id = NEW.document_id)
           OR (dependency_kind = 'source' AND dependency_id = NEW.id)
    );
END;
CREATE TRIGGER knowledge_collection_view_source_change
AFTER UPDATE OF active, content_hash, owner_revision, extraction_contract ON knowledge_sources
WHEN OLD.active <> NEW.active OR OLD.content_hash <> NEW.content_hash
    OR OLD.owner_revision <> NEW.owner_revision
    OR OLD.extraction_contract <> NEW.extraction_contract
BEGIN
    UPDATE knowledge_collection_views SET stale = 1
    WHERE collection_id IN (
        SELECT collection_id FROM knowledge_collection_view_dependencies
        WHERE (dependency_kind = 'source' AND dependency_id = NEW.id)
           OR (dependency_kind = 'document' AND dependency_id = NEW.document_id)
    );
END;
CREATE TRIGGER knowledge_collection_view_source_delete
BEFORE DELETE ON knowledge_sources BEGIN
    UPDATE knowledge_collection_views SET stale = 1
    WHERE collection_id IN (
        SELECT collection_id FROM knowledge_collection_view_dependencies
        WHERE dependency_kind = 'source' AND dependency_id = OLD.id
    );
END;

CREATE TRIGGER knowledge_collection_view_unit_insert
AFTER INSERT ON knowledge_units BEGIN
    UPDATE knowledge_collection_views SET stale = 1
    WHERE collection_id IN (
        SELECT collection_id FROM knowledge_collection_view_dependencies
        WHERE dependency_kind = 'source' AND dependency_id = NEW.source_id
    );
END;
CREATE TRIGGER knowledge_collection_view_unit_change
AFTER UPDATE OF content_hash, source_hash, extraction_contract ON knowledge_units
WHEN OLD.content_hash <> NEW.content_hash OR OLD.source_hash <> NEW.source_hash
    OR OLD.extraction_contract <> NEW.extraction_contract
BEGIN
    UPDATE knowledge_collection_views SET stale = 1
    WHERE collection_id IN (
        SELECT collection_id FROM knowledge_collection_view_dependencies
        WHERE dependency_kind = 'source'
          AND dependency_id IN (OLD.source_id, NEW.source_id)
    );
END;
CREATE TRIGGER knowledge_collection_view_unit_delete
BEFORE DELETE ON knowledge_units BEGIN
    UPDATE knowledge_collection_views SET stale = 1
    WHERE collection_id IN (
        SELECT collection_id FROM knowledge_collection_view_dependencies
        WHERE dependency_kind = 'source' AND dependency_id = OLD.source_id
    );
END;

CREATE TRIGGER knowledge_collection_view_analysis_source_insert
AFTER INSERT ON knowledge_analysis_sources BEGIN
    UPDATE knowledge_collection_views SET stale = 1
    WHERE collection_id IN (
        SELECT collection_id FROM knowledge_collection_view_dependencies
        WHERE dependency_kind = 'source' AND dependency_id = NEW.source_id
    );
END;
CREATE TRIGGER knowledge_collection_view_analysis_source_delete
BEFORE DELETE ON knowledge_analysis_sources BEGIN
    UPDATE knowledge_collection_views SET stale = 1
    WHERE collection_id IN (
        SELECT collection_id FROM knowledge_collection_view_dependencies
        WHERE dependency_kind = 'source' AND dependency_id = OLD.source_id
    );
END;
CREATE TRIGGER knowledge_collection_view_analysis_change
AFTER UPDATE OF state, report_json ON knowledge_analysis_windows
WHEN OLD.state <> NEW.state OR OLD.report_json <> NEW.report_json
BEGIN
    UPDATE knowledge_collection_views SET stale = 1
    WHERE collection_id IN (
        SELECT d.collection_id
        FROM knowledge_collection_view_dependencies d
        JOIN knowledge_analysis_sources s ON s.source_id = d.dependency_id
        WHERE d.dependency_kind = 'source' AND s.window_id = NEW.id
    );
END;
