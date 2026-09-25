-- LK-05 document collections are organization over retained documents, never
-- duplicate source ownership. Ranges are bound to the canonical source identity
-- that was current when the learner selected them.
CREATE TABLE document_collection_organization (
    singleton_id INTEGER PRIMARY KEY CHECK (singleton_id = 1),
    revision INTEGER NOT NULL CHECK (revision >= 1),
    updated_at INTEGER NOT NULL
) STRICT;
INSERT INTO document_collection_organization(singleton_id, revision, updated_at)
VALUES (1, 1, 0);

CREATE TABLE document_collections (
    id TEXT PRIMARY KEY CHECK (length(id) = 32),
    parent_id TEXT REFERENCES document_collections(id) ON DELETE RESTRICT,
    name TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 120),
    sort_order INTEGER NOT NULL CHECK (sort_order >= 0),
    archived_at INTEGER,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    revision INTEGER NOT NULL CHECK (revision >= 1)
) STRICT;
CREATE INDEX document_collections_parent_order
ON document_collections(parent_id, sort_order, id);

CREATE TABLE document_collection_memberships (
    id TEXT PRIMARY KEY CHECK (length(id) = 32),
    collection_id TEXT NOT NULL REFERENCES document_collections(id) ON DELETE CASCADE,
    document_id TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    sort_order INTEGER NOT NULL CHECK (sort_order >= 0),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    revision INTEGER NOT NULL CHECK (revision >= 1),
    UNIQUE(collection_id, document_id)
) STRICT;
CREATE INDEX document_collection_memberships_collection
ON document_collection_memberships(collection_id, sort_order, id);
CREATE INDEX document_collection_memberships_document
ON document_collection_memberships(document_id, collection_id);

CREATE TABLE document_collection_ranges (
    id TEXT PRIMARY KEY CHECK (length(id) = 32),
    membership_id TEXT NOT NULL REFERENCES document_collection_memberships(id) ON DELETE CASCADE,
    unit_kind TEXT NOT NULL CHECK (unit_kind IN (
        'page', 'slide', 'chapter', 'section', 'segment', 'image', 'time_segment'
    )),
    start_unit INTEGER NOT NULL CHECK (start_unit >= 1),
    end_unit INTEGER NOT NULL CHECK (end_unit >= start_unit),
    source_sha256 TEXT NOT NULL CHECK (length(source_sha256) = 64),
    created_at INTEGER NOT NULL,
    UNIQUE(membership_id, unit_kind, start_unit, end_unit)
) STRICT;
CREATE INDEX document_collection_ranges_membership
ON document_collection_ranges(membership_id, start_unit, end_unit);

CREATE TABLE document_collection_operation_receipts (
    operation_id TEXT PRIMARY KEY CHECK (length(operation_id) = 32),
    request_sha256 TEXT NOT NULL CHECK (length(request_sha256) = 64),
    result_json TEXT NOT NULL CHECK (json_valid(result_json)),
    created_at INTEGER NOT NULL
) STRICT;
