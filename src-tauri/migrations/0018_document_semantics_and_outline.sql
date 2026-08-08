-- Structured document semantics, reusable AI outline, and semantic generation windows.
--
-- Migration 17 stores immutable PDFs, page text/renders, and direct generation
-- jobs. This migration preserves richer extraction semantics so flashcards,
-- future study guides, search, and tutoring can share one document model.
--
-- Existing page/job tables remain compatible with pre-migration documents.
-- Semantic extraction is rebuildable from the immutable managed source PDF.

ALTER TABLE documents
    ADD COLUMN extraction_backend TEXT NOT NULL DEFAULT 'legacy'
    CHECK(length(extraction_backend) BETWEEN 1 AND 80);

ALTER TABLE documents
    ADD COLUMN render_backend TEXT NOT NULL DEFAULT 'legacy'
    CHECK(length(render_backend) BETWEEN 1 AND 80);

ALTER TABLE documents
    ADD COLUMN semantic_element_count INTEGER NOT NULL DEFAULT 0
    CHECK(semantic_element_count >= 0);

ALTER TABLE documents
    ADD COLUMN semantic_chunk_count INTEGER NOT NULL DEFAULT 0
    CHECK(semantic_chunk_count >= 0);

ALTER TABLE documents
    ADD COLUMN outline_state TEXT NOT NULL DEFAULT 'pending'
    CHECK(outline_state IN ('pending', 'building', 'ready', 'failed'));

CREATE TABLE document_semantic_elements (
    document_id       TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    element_index     INTEGER NOT NULL CHECK(element_index >= 0),
    page_number       INTEGER NOT NULL CHECK(page_number >= 1),
    element_type      TEXT NOT NULL
                      CHECK(element_type IN (
                          'title', 'paragraph', 'table', 'header', 'footer',
                          'list_item', 'image', 'code_block', 'key_value'
                      )),
    text              TEXT NOT NULL CHECK(length(text) <= 250000),
    parent_heading    TEXT CHECK(parent_heading IS NULL OR length(parent_heading) <= 1000),
    heading_path_json TEXT NOT NULL CHECK(length(heading_path_json) <= 16000),
    bbox_x            REAL NOT NULL,
    bbox_y            REAL NOT NULL,
    bbox_width        REAL NOT NULL CHECK(bbox_width >= 0),
    bbox_height       REAL NOT NULL CHECK(bbox_height >= 0),
    confidence        REAL NOT NULL CHECK(confidence >= 0 AND confidence <= 1),
    font_name         TEXT CHECK(font_name IS NULL OR length(font_name) <= 500),
    font_size         REAL,
    is_bold           INTEGER NOT NULL CHECK(is_bold IN (0, 1)),
    is_italic         INTEGER NOT NULL CHECK(is_italic IN (0, 1)),
    text_sha256       TEXT NOT NULL CHECK(length(text_sha256) = 64),
    created_at        INTEGER NOT NULL,
    PRIMARY KEY(document_id, element_index)
) STRICT;

CREATE INDEX document_semantic_elements_page_idx
    ON document_semantic_elements(document_id, page_number, element_index);
CREATE INDEX document_semantic_elements_type_idx
    ON document_semantic_elements(document_id, element_type, page_number);

CREATE TABLE document_semantic_chunks (
    document_id          TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    chunk_index          INTEGER NOT NULL CHECK(chunk_index >= 0),
    text                 TEXT NOT NULL CHECK(length(text) <= 1000000),
    full_text            TEXT NOT NULL CHECK(length(full_text) <= 1100000),
    page_numbers_json    TEXT NOT NULL CHECK(length(page_numbers_json) <= 32000),
    bounding_boxes_json  TEXT NOT NULL CHECK(length(bounding_boxes_json) <= 200000),
    element_types_json   TEXT NOT NULL CHECK(length(element_types_json) <= 32000),
    heading_context      TEXT CHECK(heading_context IS NULL OR length(heading_context) <= 2000),
    token_estimate       INTEGER NOT NULL CHECK(token_estimate >= 0),
    is_oversized         INTEGER NOT NULL CHECK(is_oversized IN (0, 1)),
    text_sha256          TEXT NOT NULL CHECK(length(text_sha256) = 64),
    created_at           INTEGER NOT NULL,
    PRIMARY KEY(document_id, chunk_index)
) STRICT;

CREATE INDEX document_semantic_chunks_pages_idx
    ON document_semantic_chunks(document_id, chunk_index);

CREATE TABLE document_outlines (
    document_id          TEXT PRIMARY KEY REFERENCES documents(id) ON DELETE CASCADE,
    schema_version       INTEGER NOT NULL CHECK(schema_version >= 1),
    prompt_version       INTEGER NOT NULL CHECK(prompt_version >= 1),
    profile_id           TEXT REFERENCES ai_provider_profiles(id) ON DELETE SET NULL,
    provider_kind        TEXT NOT NULL
                         CHECK(provider_kind IN (
                             'llama_cpp', 'deepseek', 'ollama',
                             'openai', 'gemini', 'claude'
                         )),
    model_id             TEXT NOT NULL CHECK(length(model_id) BETWEEN 1 AND 300),
    profile_revision     INTEGER NOT NULL CHECK(profile_revision >= 1),
    credential_epoch     INTEGER NOT NULL CHECK(credential_epoch >= 0),
    extraction_backend   TEXT NOT NULL CHECK(length(extraction_backend) BETWEEN 1 AND 80),
    outline_json         TEXT NOT NULL CHECK(length(outline_json) <= 2097152),
    summary_text         TEXT NOT NULL CHECK(length(summary_text) <= 100000),
    created_at           INTEGER NOT NULL,
    updated_at           INTEGER NOT NULL
) STRICT;

CREATE TABLE document_generation_windows (
    job_id                TEXT NOT NULL REFERENCES document_generation_jobs(id) ON DELETE CASCADE,
    window_index          INTEGER NOT NULL CHECK(window_index >= 0),
    first_chunk_index     INTEGER NOT NULL CHECK(first_chunk_index >= 0),
    last_chunk_index      INTEGER NOT NULL CHECK(last_chunk_index >= first_chunk_index),
    page_start            INTEGER NOT NULL CHECK(page_start >= 1),
    page_end              INTEGER NOT NULL CHECK(page_end >= page_start),
    page_numbers_json     TEXT NOT NULL CHECK(length(page_numbers_json) <= 32000),
    token_estimate        INTEGER NOT NULL CHECK(token_estimate >= 0),
    state                 TEXT NOT NULL
                          CHECK(state IN (
                              'pending', 'running', 'succeeded', 'skipped', 'failed'
                          )),
    card_count            INTEGER NOT NULL DEFAULT 0 CHECK(card_count >= 0),
    safe_error_code       TEXT NOT NULL DEFAULT '' CHECK(length(safe_error_code) <= 80),
    safe_error_message    TEXT NOT NULL DEFAULT '' CHECK(length(safe_error_message) <= 1000),
    updated_at            INTEGER NOT NULL,
    PRIMARY KEY(job_id, window_index)
) STRICT;

CREATE INDEX document_generation_windows_state_idx
    ON document_generation_windows(job_id, state, window_index);
