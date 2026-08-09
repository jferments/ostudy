-- Generalize document/provider metadata and retain safe empirical AI usage.
--
-- Provider wire adapters remain an application-level registry. Durable rows store
-- bounded adapter keys instead of repeating a closed provider enum in every
-- document table, so adding a future adapter does not require rewriting document
-- history. Existing PDF rows are explicitly identified as page-based sources.

ALTER TABLE documents
    ADD COLUMN source_format TEXT NOT NULL DEFAULT 'pdf'
    CHECK(length(source_format) BETWEEN 1 AND 40);

ALTER TABLE documents
    ADD COLUMN source_unit_kind TEXT NOT NULL DEFAULT 'page'
    CHECK(length(source_unit_kind) BETWEEN 1 AND 40);

CREATE TABLE ai_document_usage_samples (
    id                       INTEGER PRIMARY KEY,
    provider_kind            TEXT NOT NULL CHECK(length(provider_kind) BETWEEN 1 AND 80),
    model_id                 TEXT NOT NULL CHECK(length(model_id) BETWEEN 1 AND 300),
    task_kind                TEXT NOT NULL CHECK(length(task_kind) BETWEEN 1 AND 80),
    source_format            TEXT NOT NULL CHECK(length(source_format) BETWEEN 1 AND 40),
    source_unit_count        INTEGER NOT NULL CHECK(source_unit_count >= 0),
    source_text_chars        INTEGER NOT NULL CHECK(source_text_chars >= 0),
    estimated_source_tokens  INTEGER NOT NULL CHECK(estimated_source_tokens >= 0),
    image_count              INTEGER NOT NULL CHECK(image_count >= 0),
    max_output_tokens        INTEGER NOT NULL CHECK(max_output_tokens >= 0),
    prompt_tokens            INTEGER NOT NULL CHECK(prompt_tokens >= 0),
    completion_tokens        INTEGER NOT NULL CHECK(completion_tokens >= 0),
    reasoning_tokens         INTEGER NOT NULL CHECK(reasoning_tokens >= 0),
    created_at               INTEGER NOT NULL
) STRICT;

CREATE INDEX ai_document_usage_samples_model_time_idx
    ON ai_document_usage_samples(provider_kind, model_id, created_at DESC);

CREATE TABLE ai_provider_profiles_v20 (
    id                      TEXT PRIMARY KEY CHECK(length(id) = 32),
    display_name            TEXT NOT NULL CHECK(length(trim(display_name)) BETWEEN 1 AND 100),
    normalized_name         TEXT NOT NULL CHECK(length(normalized_name) BETWEEN 1 AND 100),
    provider_kind           TEXT NOT NULL CHECK(length(provider_kind) BETWEEN 1 AND 80),
    base_url                TEXT NOT NULL CHECK(length(base_url) BETWEEN 1 AND 2048),
    model_id                TEXT NOT NULL DEFAULT '' CHECK(length(model_id) <= 300),
    auth_mode               TEXT NOT NULL CHECK(auth_mode IN ('none', 'bearer')),
    enabled                 INTEGER NOT NULL DEFAULT 1 CHECK(enabled IN (0, 1)),
    allow_insecure_http     INTEGER NOT NULL DEFAULT 0 CHECK(allow_insecure_http IN (0, 1)),
    request_timeout_ms      INTEGER NOT NULL DEFAULT 60000
                            CHECK(request_timeout_ms BETWEEN 1000 AND 300000),
    credential_storage      TEXT NOT NULL DEFAULT 'none'
                            CHECK(credential_storage IN ('none', 'secure', 'session')),
    credential_epoch        INTEGER NOT NULL DEFAULT 0 CHECK(credential_epoch >= 0),
    revision                INTEGER NOT NULL DEFAULT 1 CHECK(revision >= 1),
    created_at              INTEGER NOT NULL,
    updated_at              INTEGER NOT NULL,
    deleted_at              INTEGER
) STRICT;

INSERT INTO ai_provider_profiles_v20
SELECT id, display_name, normalized_name, provider_kind, base_url, model_id,
       auth_mode, enabled, allow_insecure_http, request_timeout_ms,
       credential_storage, credential_epoch, revision, created_at, updated_at,
       deleted_at
FROM ai_provider_profiles;

DROP TABLE ai_provider_profiles;
ALTER TABLE ai_provider_profiles_v20 RENAME TO ai_provider_profiles;

CREATE UNIQUE INDEX ai_provider_profiles_active_name_idx
    ON ai_provider_profiles(normalized_name)
    WHERE deleted_at IS NULL;
CREATE INDEX ai_provider_profiles_active_updated_idx
    ON ai_provider_profiles(updated_at DESC)
    WHERE deleted_at IS NULL;

CREATE TABLE document_generation_jobs_v20 (
    id                           TEXT PRIMARY KEY CHECK(length(id) = 32),
    document_id                  TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    profile_id                   TEXT REFERENCES ai_provider_profiles(id) ON DELETE SET NULL,
    provider_kind                TEXT NOT NULL CHECK(length(provider_kind) BETWEEN 1 AND 80),
    model_id                     TEXT NOT NULL CHECK(length(model_id) BETWEEN 1 AND 300),
    profile_revision             INTEGER NOT NULL CHECK(profile_revision >= 1),
    credential_epoch            INTEGER NOT NULL CHECK(credential_epoch >= 0),
    network_scope                TEXT NOT NULL CHECK(network_scope IN ('local', 'lan', 'cloud')),
    state                        TEXT NOT NULL
                                 CHECK(state IN (
                                     'queued', 'extracting', 'generating', 'committing',
                                     'succeeded', 'failed', 'interrupted', 'canceled'
                                 )),
    page_count                   INTEGER NOT NULL CHECK(page_count >= 1),
    processed_page_count         INTEGER NOT NULL DEFAULT 0 CHECK(processed_page_count >= 0),
    generated_candidate_count    INTEGER NOT NULL DEFAULT 0
                                 CHECK(generated_candidate_count >= 0),
    generated_deck_id            TEXT REFERENCES decks(id) ON DELETE SET NULL,
    safe_error_code              TEXT NOT NULL DEFAULT '' CHECK(length(safe_error_code) <= 80),
    safe_error_message           TEXT NOT NULL DEFAULT '' CHECK(length(safe_error_message) <= 1000),
    created_at                   INTEGER NOT NULL,
    started_at                   INTEGER,
    updated_at                   INTEGER NOT NULL,
    completed_at                 INTEGER,
    CHECK(processed_page_count <= page_count)
) STRICT;

INSERT INTO document_generation_jobs_v20
SELECT id, document_id, profile_id, provider_kind, model_id, profile_revision,
       credential_epoch, network_scope, state, page_count, processed_page_count,
       generated_candidate_count, generated_deck_id, safe_error_code,
       safe_error_message, created_at, started_at, updated_at, completed_at
FROM document_generation_jobs;

DROP TABLE document_generation_jobs;
ALTER TABLE document_generation_jobs_v20 RENAME TO document_generation_jobs;

CREATE INDEX document_generation_jobs_document_time_idx
    ON document_generation_jobs(document_id, created_at DESC);
CREATE INDEX document_generation_jobs_active_idx
    ON document_generation_jobs(state, updated_at)
    WHERE state IN ('queued', 'extracting', 'generating', 'committing', 'interrupted');

CREATE TABLE document_outlines_v20 (
    document_id          TEXT PRIMARY KEY REFERENCES documents(id) ON DELETE CASCADE,
    schema_version       INTEGER NOT NULL CHECK(schema_version >= 1),
    prompt_version       INTEGER NOT NULL CHECK(prompt_version >= 1),
    profile_id           TEXT REFERENCES ai_provider_profiles(id) ON DELETE SET NULL,
    provider_kind        TEXT NOT NULL CHECK(length(provider_kind) BETWEEN 1 AND 80),
    model_id             TEXT NOT NULL CHECK(length(model_id) BETWEEN 1 AND 300),
    profile_revision     INTEGER NOT NULL CHECK(profile_revision >= 1),
    credential_epoch     INTEGER NOT NULL CHECK(credential_epoch >= 0),
    extraction_backend   TEXT NOT NULL CHECK(length(extraction_backend) BETWEEN 1 AND 80),
    outline_json         TEXT NOT NULL CHECK(length(outline_json) <= 2097152),
    summary_text         TEXT NOT NULL CHECK(length(summary_text) <= 100000),
    created_at           INTEGER NOT NULL,
    updated_at           INTEGER NOT NULL
) STRICT;

INSERT INTO document_outlines_v20
SELECT document_id, schema_version, prompt_version, profile_id, provider_kind,
       model_id, profile_revision, credential_epoch, extraction_backend,
       outline_json, summary_text, created_at, updated_at
FROM document_outlines;

DROP TABLE document_outlines;
ALTER TABLE document_outlines_v20 RENAME TO document_outlines;
