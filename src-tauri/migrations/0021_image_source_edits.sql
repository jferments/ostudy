-- Stable image ownership survives transactional reordering. Positions 128..255
-- are a transaction-local parking range; native entry requires 0..count-1.
CREATE TABLE document_image_members_next (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(id) = 32),
    document_id TEXT NOT NULL REFERENCES document_image_sets(document_id) ON DELETE CASCADE,
    position INTEGER NOT NULL CHECK (position >= 0 AND position < 256),
    original_filename TEXT NOT NULL CHECK (length(original_filename) BETWEEN 1 AND 255),
    media_type TEXT NOT NULL CHECK (media_type IN ('image/png', 'image/jpeg', 'image/webp')),
    relative_path TEXT NOT NULL UNIQUE,
    byte_size INTEGER NOT NULL CHECK (byte_size BETWEEN 1 AND 26214400),
    sha256 TEXT NOT NULL CHECK (length(sha256) = 64),
    width INTEGER NOT NULL CHECK (width BETWEEN 1 AND 8192),
    height INTEGER NOT NULL CHECK (height BETWEEN 1 AND 8192),
    pixel_sha256 TEXT NOT NULL CHECK (length(pixel_sha256) = 64),
    pixel_contract TEXT NOT NULL CHECK (length(pixel_contract) BETWEEN 1 AND 160),
    quarter_turns INTEGER NOT NULL DEFAULT 0 CHECK (quarter_turns BETWEEN 0 AND 3),
    inclusion_json TEXT CHECK (inclusion_json IS NULL OR length(inclusion_json) <= 512),
    UNIQUE (document_id, position),
    CHECK (width * height <= 16000000)
) STRICT;
INSERT INTO document_image_members_next (
    id, document_id, position, original_filename, media_type, relative_path,
    byte_size, sha256, width, height, pixel_sha256, pixel_contract, quarter_turns, inclusion_json
) SELECT id, document_id, position, original_filename, media_type, relative_path,
    byte_size, sha256, width, height, pixel_sha256, pixel_contract, quarter_turns, inclusion_json
  FROM document_image_members;
DROP TABLE document_image_members;
ALTER TABLE document_image_members_next RENAME TO document_image_members;
CREATE INDEX document_image_members_source ON document_image_members(document_id, sha256);

CREATE TABLE document_image_settings (
    document_id TEXT PRIMARY KEY NOT NULL REFERENCES document_image_sets(document_id) ON DELETE CASCADE,
    revision INTEGER NOT NULL CHECK (revision >= 1),
    include_interface INTEGER NOT NULL CHECK (include_interface IN (0, 1))
) STRICT;
CREATE TABLE document_image_overrides (
    document_id TEXT NOT NULL REFERENCES document_image_sets(document_id) ON DELETE CASCADE,
    member_id TEXT NOT NULL REFERENCES document_image_members(id) ON DELETE CASCADE,
    region_id TEXT NOT NULL CHECK (length(region_id) = 64),
    action TEXT NOT NULL CHECK (action IN ('include', 'exclude', 'metadata')),
    PRIMARY KEY (document_id, region_id)
) STRICT;
CREATE TABLE document_image_receipts (
    document_id TEXT PRIMARY KEY NOT NULL REFERENCES document_image_sets(document_id) ON DELETE CASCADE,
    manifest_sha256 TEXT NOT NULL CHECK (length(manifest_sha256) = 64),
    filter_revision INTEGER NOT NULL CHECK (filter_revision >= 1),
    version INTEGER NOT NULL CHECK (version = 1),
    payload_json TEXT NOT NULL CHECK (length(CAST(payload_json AS BLOB)) BETWEEN 1 AND 16777216),
    payload_sha256 TEXT NOT NULL CHECK (length(payload_sha256) = 64)
) STRICT;
CREATE TABLE document_image_chunk_keys (
    document_id TEXT NOT NULL REFERENCES document_image_sets(document_id) ON DELETE CASCADE,
    chunk_index INTEGER NOT NULL CHECK (chunk_index >= 0),
    stable_key TEXT NOT NULL CHECK (length(stable_key) BETWEEN 1 AND 120),
    evidence_sha256 TEXT NOT NULL CHECK (length(evidence_sha256) = 64),
    PRIMARY KEY (document_id, chunk_index),
    UNIQUE (document_id, stable_key)
) STRICT;
CREATE TABLE document_image_edit_receipts (
    operation_id TEXT PRIMARY KEY NOT NULL CHECK (length(operation_id) = 32),
    document_id TEXT REFERENCES documents(id) ON DELETE SET NULL,
    request_sha256 TEXT NOT NULL CHECK (length(request_sha256) = 64),
    result_json TEXT NOT NULL CHECK (length(result_json) BETWEEN 1 AND 1024),
    created_at INTEGER NOT NULL
) STRICT;

-- Only native canonical replacement opens this short transaction-owned guard.
-- Ordinary source deletion keeps its existing invalidation. Startup rejects a
-- surviving guard; rollback erases it together with an unsuccessful projection.
CREATE TABLE document_image_projection_writes (
    document_id TEXT PRIMARY KEY NOT NULL REFERENCES document_image_sets(document_id) ON DELETE CASCADE
) STRICT;
DROP TRIGGER knowledge_document_chunk_changed;
CREATE TRIGGER knowledge_document_chunk_changed AFTER UPDATE ON document_semantic_chunk_sources
WHEN NOT EXISTS (SELECT 1 FROM document_image_projection_writes WHERE document_id = NEW.document_id)
BEGIN
    UPDATE knowledge_sources SET active = 0 WHERE kind = 'document'
        AND document_id = NEW.document_id AND parent_key = CAST(NEW.chunk_index AS TEXT) AND active = 1;
END;
DROP TRIGGER knowledge_document_chunk_deleted;
CREATE TRIGGER knowledge_document_chunk_deleted AFTER DELETE ON document_semantic_chunk_sources
WHEN NOT EXISTS (SELECT 1 FROM document_image_projection_writes WHERE document_id = OLD.document_id)
BEGIN
    DELETE FROM knowledge_sources WHERE kind = 'document'
        AND document_id = OLD.document_id AND parent_key = CAST(OLD.chunk_index AS TEXT);
END;
DROP TRIGGER knowledge_document_asset_changed;
CREATE TRIGGER knowledge_document_asset_changed AFTER UPDATE OF sha256 ON document_source_assets
WHEN OLD.sha256 <> NEW.sha256 AND NOT EXISTS (
    SELECT 1 FROM document_image_projection_writes WHERE document_id = NEW.document_id)
BEGIN
    UPDATE knowledge_sources SET active = 0 WHERE document_id = NEW.document_id AND active = 1;
END;
DROP TRIGGER knowledge_document_asset_deleted;
CREATE TRIGGER knowledge_document_asset_deleted BEFORE DELETE ON document_source_assets
WHEN NOT EXISTS (SELECT 1 FROM document_image_projection_writes WHERE document_id = OLD.document_id)
BEGIN
    UPDATE knowledge_sources SET active = 0 WHERE document_id = OLD.document_id AND active = 1;
END;
DROP TRIGGER knowledge_document_renamed;
CREATE TRIGGER knowledge_document_renamed AFTER UPDATE OF title ON documents
WHEN OLD.title <> NEW.title BEGIN
    UPDATE knowledge_sources SET active = 0 WHERE document_id = NEW.id AND active = 1
      AND (kind <> 'document' OR COALESCE(json_extract(record_json, '$.identityVersion'), 1) NOT IN (2, 3));
END;
