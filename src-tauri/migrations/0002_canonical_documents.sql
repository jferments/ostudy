-- Generic canonical document representation.
--
-- Migration 1 remains the immutable public baseline. This migration adds the
-- format-neutral source ledger and backfills every existing PDF transactionally.
-- Page-shaped columns remain temporary compatibility projections for the current
-- PDF UI/backup surface; canonical source records are the new adapter boundary.

ALTER TABLE documents
ADD COLUMN canonical_schema_version INTEGER NOT NULL DEFAULT 1
CHECK (canonical_schema_version >= 1);

ALTER TABLE documents
ADD COLUMN adapter_backend TEXT NOT NULL DEFAULT 'legacy-pdf-adapter-v0'
CHECK (length(adapter_backend) BETWEEN 1 AND 160);

ALTER TABLE documents
ADD COLUMN source_file_count INTEGER NOT NULL DEFAULT 1
CHECK (source_file_count >= 1);

ALTER TABLE documents
ADD COLUMN source_unit_count INTEGER NOT NULL DEFAULT 0
CHECK (source_unit_count >= 0);

ALTER TABLE documents
ADD COLUMN extracted_source_unit_count INTEGER NOT NULL DEFAULT 0
CHECK (extracted_source_unit_count >= 0);

ALTER TABLE documents
ADD COLUMN visualized_source_unit_count INTEGER NOT NULL DEFAULT 0
CHECK (visualized_source_unit_count >= 0);

ALTER TABLE documents
ADD COLUMN source_asset_count INTEGER NOT NULL DEFAULT 0
CHECK (source_asset_count >= 0);

UPDATE documents
SET source_unit_count = page_count,
    extracted_source_unit_count = extracted_page_count,
    visualized_source_unit_count = rendered_page_count;

ALTER TABLE document_generation_jobs
ADD COLUMN source_unit_count INTEGER NOT NULL DEFAULT 0
CHECK (source_unit_count >= 0);

ALTER TABLE document_generation_jobs
ADD COLUMN processed_source_unit_count INTEGER NOT NULL DEFAULT 0
CHECK (processed_source_unit_count >= 0);

UPDATE document_generation_jobs
SET source_unit_count = page_count,
    processed_source_unit_count = processed_page_count;

ALTER TABLE document_generation_windows
ADD COLUMN source_locator_json TEXT NOT NULL DEFAULT '{}';

ALTER TABLE document_generation_windows
ADD COLUMN visual_asset_ids_json TEXT NOT NULL DEFAULT '[]';

-- Public-v1 generation-window rows did not persist a partition title. Keep the
-- locator label absent rather than fabricating one during migration; semantic
-- planning can derive a display label from canonical chunks when needed.
UPDATE document_generation_windows
SET source_locator_json = json_object(
        'unitKind', 'page',
        'unitNumbers', json(page_numbers_json),
        'sectionLabel', NULL,
        'assetIds', json_array(),
        'timeRangeMs', NULL
    );

ALTER TABLE document_outline_checkpoints
ADD COLUMN source_locator_json TEXT NOT NULL DEFAULT '{}';

ALTER TABLE document_generation_checkpoints
ADD COLUMN source_locator_json TEXT NOT NULL DEFAULT '{}';

-- Migration-1 checkpoints remain rebuildable compatibility data. Their stored
-- source hash includes the old contract, so the versioned canonical checkpoint
-- contract will naturally recompute them when adapter output changes.
UPDATE document_outline_checkpoints
SET source_locator_json = json_object(
        'unitKind', 'page',
        'unitNumbers', CASE
            WHEN page_start = page_end THEN json_array(page_start)
            ELSE json_array(page_start, page_end)
        END,
        'sectionLabel', partition_title,
        'assetIds', json_array(),
        'timeRangeMs', NULL
    );

UPDATE document_generation_checkpoints
SET source_locator_json = json_object(
        'unitKind', 'page',
        'unitNumbers', CASE
            WHEN page_start = page_end THEN json_array(page_start)
            ELSE json_array(page_start, page_end)
        END,
        'sectionLabel', partition_title,
        'assetIds', json_array(),
        'timeRangeMs', NULL
    );

CREATE TABLE document_source_files (
    document_id TEXT NOT NULL
        REFERENCES documents(id) ON DELETE CASCADE,
    file_ordinal INTEGER NOT NULL
        CHECK (file_ordinal >= 1),
    original_filename TEXT NOT NULL
        CHECK (length(original_filename) BETWEEN 1 AND 255),
    media_type TEXT NOT NULL
        CHECK (length(media_type) BETWEEN 3 AND 160),
    relative_path TEXT NOT NULL UNIQUE
        CHECK (length(relative_path) BETWEEN 1 AND 1024),
    byte_size INTEGER NOT NULL
        CHECK (byte_size > 0),
    sha256 TEXT NOT NULL
        CHECK (length(sha256) = 64),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (document_id, file_ordinal)
) STRICT;

CREATE TABLE document_source_units (
    document_id TEXT NOT NULL
        REFERENCES documents(id) ON DELETE CASCADE,
    unit_number INTEGER NOT NULL
        CHECK (unit_number >= 1),
    unit_kind TEXT NOT NULL
        CHECK (unit_kind IN (
            'page', 'slide', 'chapter', 'section',
            'segment', 'image', 'time_segment'
        )),
    label TEXT NOT NULL
        CHECK (length(label) BETWEEN 1 AND 500),
    title TEXT,
    plain_text TEXT NOT NULL,
    text_sha256 TEXT NOT NULL
        CHECK (length(text_sha256) = 64),
    source_file_ordinal INTEGER NOT NULL,
    locator_json TEXT NOT NULL,
    extraction_confidence REAL
        CHECK (
            extraction_confidence IS NULL OR
            extraction_confidence BETWEEN 0.0 AND 1.0
        ),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (document_id, unit_number),
    FOREIGN KEY (document_id, source_file_ordinal)
        REFERENCES document_source_files(document_id, file_ordinal)
        ON DELETE RESTRICT
) STRICT;

CREATE TABLE document_source_blocks (
    document_id TEXT NOT NULL,
    block_index INTEGER NOT NULL
        CHECK (block_index >= 0),
    block_ordinal INTEGER NOT NULL
        CHECK (block_ordinal >= 0),
    source_unit_number INTEGER NOT NULL
        CHECK (source_unit_number >= 1),
    role TEXT NOT NULL
        CHECK (role IN (
            'title', 'heading', 'paragraph', 'list_item', 'table',
            'code_block', 'quote', 'equation', 'caption', 'header',
            'footer', 'speaker_note', 'footnote', 'endnote',
            'key_value', 'author_learning_objective', 'image',
            'image_description'
        )),
    text TEXT NOT NULL,
    parent_heading TEXT,
    heading_path_json TEXT NOT NULL,
    geometry_json TEXT,
    confidence REAL
        CHECK (confidence IS NULL OR confidence BETWEEN 0.0 AND 1.0),
    asset_ids_json TEXT NOT NULL,
    attributes_json TEXT NOT NULL,
    content_sha256 TEXT NOT NULL
        CHECK (length(content_sha256) = 64),
    created_at INTEGER NOT NULL,
    PRIMARY KEY (document_id, block_index),
    FOREIGN KEY (document_id, source_unit_number)
        REFERENCES document_source_units(document_id, unit_number)
        ON DELETE CASCADE
) STRICT;

CREATE TABLE document_source_assets (
    document_id TEXT NOT NULL
        REFERENCES documents(id) ON DELETE CASCADE,
    asset_id TEXT NOT NULL
        CHECK (length(asset_id) BETWEEN 1 AND 160),
    source_unit_number INTEGER,
    block_index INTEGER,
    role TEXT NOT NULL
        CHECK (role IN (
            'unit_render', 'original_note_image', 'embedded_image',
            'diagram', 'chart', 'equation_image', 'cover_image',
            'thumbnail'
        )),
    media_type TEXT NOT NULL
        CHECK (length(media_type) BETWEEN 3 AND 160),
    relative_path TEXT NOT NULL UNIQUE
        CHECK (length(relative_path) BETWEEN 1 AND 1024),
    byte_size INTEGER NOT NULL
        CHECK (byte_size > 0),
    sha256 TEXT NOT NULL
        CHECK (length(sha256) = 64),
    width INTEGER
        CHECK (width IS NULL OR width > 0),
    height INTEGER
        CHECK (height IS NULL OR height > 0),
    alt_text TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (document_id, asset_id),
    FOREIGN KEY (document_id, source_unit_number)
        REFERENCES document_source_units(document_id, unit_number)
        ON DELETE CASCADE,
    FOREIGN KEY (document_id, block_index)
        REFERENCES document_source_blocks(document_id, block_index)
        ON DELETE CASCADE
) STRICT;

-- Canonical semantic chunks are self-contained and do not depend on the legacy
-- page-shaped semantic table. This table is the downstream read path for
-- planning, multimodal evidence selection, and future embedding derivatives.
CREATE TABLE document_semantic_chunk_sources (
    document_id TEXT NOT NULL
        REFERENCES documents(id) ON DELETE CASCADE,
    chunk_index INTEGER NOT NULL
        CHECK (chunk_index >= 0),
    text TEXT NOT NULL
        CHECK (length(text) <= 1000000),
    full_text TEXT NOT NULL
        CHECK (length(full_text) <= 1100000),
    source_locator_json TEXT NOT NULL,
    visual_asset_ids_json TEXT NOT NULL,
    element_types_json TEXT NOT NULL
        CHECK (length(element_types_json) <= 32000),
    heading_context TEXT
        CHECK (heading_context IS NULL OR length(heading_context) <= 2000),
    token_estimate INTEGER NOT NULL
        CHECK (token_estimate >= 0),
    is_oversized INTEGER NOT NULL
        CHECK (is_oversized IN (0, 1)),
    content_sha256 TEXT NOT NULL
        CHECK (length(content_sha256) = 64),
    canonical_schema_version INTEGER NOT NULL
        CHECK (canonical_schema_version >= 1),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (document_id, chunk_index)
) STRICT;

INSERT INTO document_source_files (
    document_id, file_ordinal, original_filename, media_type,
    relative_path, byte_size, sha256, created_at, updated_at
)
SELECT id, 1, original_filename, mime_type,
       source_relative_path, byte_size, sha256, created_at, updated_at
FROM documents;

INSERT INTO document_source_units (
    document_id, unit_number, unit_kind, label, title,
    plain_text, text_sha256, source_file_ordinal, locator_json,
    extraction_confidence, created_at, updated_at
)
SELECT page.document_id,
       page.page_number,
       document.source_unit_kind,
       'Page ' || page.page_number,
       NULL,
       page.plain_text,
       page.text_sha256,
       1,
       json_object(
           'unitKind', document.source_unit_kind,
           'unitNumbers', json_array(page.page_number),
           'sectionLabel', NULL,
           'assetIds', json_array('unit-render:' || printf('%06d', page.page_number)),
           'timeRangeMs', NULL
       ),
       NULL,
       page.created_at,
       page.updated_at
FROM document_pages AS page
JOIN documents AS document ON document.id = page.document_id
ORDER BY page.document_id, page.page_number;

INSERT INTO document_source_assets (
    document_id, asset_id, source_unit_number, block_index, role,
    media_type, relative_path, byte_size, sha256, width, height,
    alt_text, created_at, updated_at
)
SELECT document_id,
       'unit-render:' || printf('%06d', page_number),
       page_number,
       NULL,
       'unit_render',
       render_mime_type,
       render_relative_path,
       render_byte_size,
       render_sha256,
       NULL,
       NULL,
       NULL,
       created_at,
       updated_at
FROM document_pages
ORDER BY document_id, page_number;

INSERT INTO document_source_blocks (
    document_id, block_index, block_ordinal, source_unit_number,
    role, text, parent_heading, heading_path_json, geometry_json,
    confidence, asset_ids_json, attributes_json, content_sha256,
    created_at
)
SELECT element.document_id,
       element.element_index,
       ROW_NUMBER() OVER (
           PARTITION BY element.document_id, element.page_number
           ORDER BY element.element_index
       ) - 1,
       element.page_number,
       element.element_type,
       element.text,
       element.parent_heading,
       element.heading_path_json,
       json_object(
           'x', element.bbox_x,
           'y', element.bbox_y,
           'width', element.bbox_width,
           'height', element.bbox_height
       ),
       element.confidence,
       CASE
           WHEN element.element_type IN ('image', 'table', 'key_value')
           THEN json_array('unit-render:' || printf('%06d', element.page_number))
           ELSE json_array()
       END,
       json_object(
           'fontName', element.font_name,
           'fontSize', element.font_size,
           'isBold', element.is_bold,
           'isItalic', element.is_italic
       ),
       element.text_sha256,
       element.created_at
FROM document_semantic_elements AS element
ORDER BY element.document_id, element.element_index;

-- Preserve the same page-render evidence that the live PDF adapter would attach
-- to a visual semantic chunk. This keeps migrated public-v1 documents equivalent
-- to newly ingested PDFs for later multimodal enrichment and retrieval.
WITH canonical_chunks AS (
    SELECT chunk.*,
           document.source_unit_kind,
           CASE
               WHEN EXISTS (
                   SELECT 1
                   FROM json_each(chunk.element_types_json)
                   WHERE value IN ('image', 'table', 'key_value')
               ) OR instr(chunk.full_text, 'has no extractable structured text') > 0
               THEN COALESCE((
                   SELECT json_group_array(
                       'unit-render:' || printf('%06d', CAST(value AS INTEGER))
                   )
                   FROM json_each(chunk.page_numbers_json)
               ), json_array())
               ELSE json_array()
           END AS visual_asset_ids_json
    FROM document_semantic_chunks AS chunk
    JOIN documents AS document ON document.id = chunk.document_id
)
INSERT INTO document_semantic_chunk_sources (
    document_id, chunk_index, text, full_text, source_locator_json,
    visual_asset_ids_json, element_types_json, heading_context,
    token_estimate, is_oversized, content_sha256,
    canonical_schema_version, created_at, updated_at
)
SELECT chunk.document_id,
       chunk.chunk_index,
       chunk.text,
       chunk.full_text,
       json_object(
           'unitKind', chunk.source_unit_kind,
           'unitNumbers', json(chunk.page_numbers_json),
           'sectionLabel', chunk.heading_context,
           'assetIds', json(chunk.visual_asset_ids_json),
           'timeRangeMs', NULL
       ),
       chunk.visual_asset_ids_json,
       chunk.element_types_json,
       chunk.heading_context,
       chunk.token_estimate,
       chunk.is_oversized,
       chunk.text_sha256,
       1,
       chunk.created_at,
       chunk.created_at
FROM canonical_chunks AS chunk
ORDER BY chunk.document_id, chunk.chunk_index;

UPDATE documents
SET source_asset_count = (
    SELECT COUNT(*)
    FROM document_source_assets AS asset
    WHERE asset.document_id = documents.id
);

CREATE INDEX document_source_files_document_idx
ON document_source_files(document_id, file_ordinal);

CREATE INDEX document_source_units_kind_idx
ON document_source_units(document_id, unit_kind, unit_number);

CREATE INDEX document_source_blocks_unit_idx
ON document_source_blocks(document_id, source_unit_number, block_ordinal);

CREATE INDEX document_source_blocks_role_idx
ON document_source_blocks(document_id, role, source_unit_number);

CREATE INDEX document_source_assets_unit_idx
ON document_source_assets(document_id, source_unit_number, role);

CREATE INDEX document_semantic_chunk_sources_document_idx
ON document_semantic_chunk_sources(document_id, chunk_index);
