-- Derived generation work and content-free, per-attempt usage diagnostics.
-- Completed source knowledge, guides and learner annotations are not rewritten.
CREATE TABLE generation_operations (
    id TEXT PRIMARY KEY CHECK(length(id) = 32),
    purpose TEXT NOT NULL CHECK(length(purpose) BETWEEN 1 AND 80),
    document_id TEXT REFERENCES documents(id) ON DELETE CASCADE,
    collection_id TEXT REFERENCES document_collections(id) ON DELETE CASCADE,
    started_at INTEGER NOT NULL,
    finished_at INTEGER,
    state TEXT NOT NULL CHECK(state IN ('running','complete','failed','interrupted')),
    CHECK(document_id IS NULL OR collection_id IS NULL)
) STRICT;

CREATE TABLE generation_requests (
    id TEXT PRIMARY KEY CHECK(length(id) = 32),
    operation_id TEXT NOT NULL REFERENCES generation_operations(id) ON DELETE CASCADE,
    stage TEXT NOT NULL CHECK(length(stage) BETWEEN 1 AND 80),
    request_key TEXT NOT NULL CHECK(length(request_key) = 64),
    attempt INTEGER NOT NULL CHECK(attempt >= 1),
    provider TEXT NOT NULL CHECK(length(provider) <= 80),
    model TEXT NOT NULL CHECK(length(model) <= 256),
    status TEXT NOT NULL CHECK(status IN ('started','response','failed','interrupted')),
    started_at INTEGER NOT NULL,
    finished_at INTEGER,
    input_tokens INTEGER CHECK(input_tokens >= 0),
    output_tokens INTEGER CHECK(output_tokens >= 0),
    reasoning_tokens INTEGER CHECK(reasoning_tokens >= 0),
    cached_input_tokens INTEGER CHECK(cached_input_tokens >= 0),
    cache_write_tokens INTEGER CHECK(cache_write_tokens >= 0),
    total_tokens INTEGER CHECK(total_tokens >= 0),
    cost REAL CHECK(cost >= 0),
    response_id TEXT CHECK(length(response_id) <= 256),
    finish_reason TEXT CHECK(length(finish_reason) <= 80),
    error_code TEXT CHECK(length(error_code) <= 80),
    payload_bytes_json TEXT NOT NULL CHECK(json_valid(payload_bytes_json)),
    max_output_tokens INTEGER NOT NULL CHECK(max_output_tokens >= 0)
) STRICT;
CREATE INDEX generation_requests_operation_stage
ON generation_requests(operation_id, stage, started_at);
CREATE INDEX generation_operations_recent ON generation_operations(started_at DESC, id);

CREATE TABLE generation_reuse_events (
    id INTEGER PRIMARY KEY,
    operation_id TEXT NOT NULL REFERENCES generation_operations(id) ON DELETE CASCADE,
    stage TEXT NOT NULL CHECK(length(stage) BETWEEN 1 AND 80),
    cache_key TEXT NOT NULL CHECK(length(cache_key) = 64),
    created_at INTEGER NOT NULL
) STRICT;

CREATE TABLE collection_guide_checkpoints (
    cache_key TEXT PRIMARY KEY CHECK(length(cache_key) = 64),
    collection_id TEXT NOT NULL REFERENCES document_collections(id) ON DELETE CASCADE,
    contract_hash TEXT NOT NULL CHECK(length(contract_hash) = 64),
    result_json TEXT NOT NULL CHECK(json_valid(result_json) AND length(result_json) <= 2097152),
    result_hash TEXT NOT NULL CHECK(length(result_hash) = 64),
    created_at INTEGER NOT NULL
) STRICT;
CREATE TABLE collection_guide_checkpoint_sources (
    cache_key TEXT NOT NULL REFERENCES collection_guide_checkpoints(cache_key) ON DELETE CASCADE,
    source_id TEXT NOT NULL REFERENCES knowledge_sources(id) ON DELETE CASCADE,
    PRIMARY KEY(cache_key, source_id)
) STRICT;
CREATE TRIGGER collection_guide_checkpoint_source_deleted
BEFORE DELETE ON knowledge_sources BEGIN
    DELETE FROM collection_guide_checkpoints WHERE cache_key IN (
        SELECT cache_key FROM collection_guide_checkpoint_sources WHERE source_id = OLD.id
    );
END;

CREATE TABLE collection_guide_manifests (
    guide_id TEXT PRIMARY KEY REFERENCES study_target_guides(id) ON DELETE CASCADE,
    request_key TEXT NOT NULL CHECK(length(request_key) = 64),
    created_at INTEGER NOT NULL
) STRICT;

CREATE TABLE collection_reduction_receipts (
    collection_id TEXT NOT NULL REFERENCES document_collections(id) ON DELETE CASCADE,
    request_key TEXT NOT NULL CHECK(length(request_key) = 64),
    created_at INTEGER NOT NULL,
    PRIMARY KEY(collection_id, request_key)
) STRICT;

CREATE TRIGGER collection_guide_checkpoint_source_changed
AFTER UPDATE OF content_hash, active, extraction_contract ON knowledge_sources
WHEN OLD.content_hash <> NEW.content_hash OR OLD.active <> NEW.active
     OR OLD.extraction_contract <> NEW.extraction_contract
BEGIN
    DELETE FROM collection_guide_checkpoints WHERE cache_key IN (
        SELECT cache_key FROM collection_guide_checkpoint_sources WHERE source_id = NEW.id
    );
END;
