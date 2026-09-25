-- Ordered screenshot sources retain immutable originals independently of
-- canonical extraction. No learner content or existing source is rewritten.
CREATE TABLE document_image_sets (
    document_id TEXT PRIMARY KEY NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    manifest_version INTEGER NOT NULL CHECK (manifest_version = 1),
    revision INTEGER NOT NULL CHECK (revision >= 1),
    manifest_sha256 TEXT NOT NULL CHECK (length(manifest_sha256) = 64),
    prepared_manifest_sha256 TEXT NOT NULL DEFAULT ''
        CHECK (prepared_manifest_sha256 = '' OR length(prepared_manifest_sha256) = 64)
) STRICT;

CREATE TABLE document_image_members (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(id) = 32),
    document_id TEXT NOT NULL REFERENCES document_image_sets(document_id) ON DELETE CASCADE,
    position INTEGER NOT NULL CHECK (position >= 0 AND position < 128),
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
CREATE INDEX document_image_members_source ON document_image_members(document_id, sha256);

-- A minimal receipt survives source deletion with a NULL owner. Replaying an
-- old operation then refuses resurrection; this table contains no source text.
CREATE TABLE document_image_imports (
    operation_id TEXT PRIMARY KEY NOT NULL CHECK (length(operation_id) = 32),
    request_sha256 TEXT NOT NULL CHECK (length(request_sha256) = 64),
    document_id TEXT REFERENCES documents(id) ON DELETE SET NULL,
    created_at INTEGER NOT NULL,
    UNIQUE (document_id)
) STRICT;
