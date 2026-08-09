-- Resumable large-document analysis and generation checkpoints.
--
-- Large PDFs are processed as independent heading-aware semantic partitions.
-- Provider results are cached only after strict parsing and are keyed by the
-- exact source-window hash plus provider/task contract. A retry can therefore
-- reuse completed work without treating raw provider output as durable data.

ALTER TABLE documents
    ADD COLUMN scale_mode TEXT NOT NULL DEFAULT 'standard'
    CHECK(scale_mode IN ('standard', 'large'));

ALTER TABLE documents
    ADD COLUMN estimated_source_tokens INTEGER NOT NULL DEFAULT 0
    CHECK(estimated_source_tokens >= 0);

ALTER TABLE documents
    ADD COLUMN outline_window_count INTEGER NOT NULL DEFAULT 0
    CHECK(outline_window_count >= 0);

ALTER TABLE documents
    ADD COLUMN outline_completed_window_count INTEGER NOT NULL DEFAULT 0
    CHECK(outline_completed_window_count >= 0);

ALTER TABLE documents
    ADD COLUMN generation_window_count INTEGER NOT NULL DEFAULT 0
    CHECK(generation_window_count >= 0);

ALTER TABLE documents
    ADD COLUMN generation_completed_window_count INTEGER NOT NULL DEFAULT 0
    CHECK(generation_completed_window_count >= 0);

CREATE TABLE document_outline_checkpoints (
    document_id       TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    contract_key      TEXT NOT NULL CHECK(length(contract_key) = 64),
    source_sha256     TEXT NOT NULL CHECK(length(source_sha256) = 64),
    window_index      INTEGER NOT NULL CHECK(window_index >= 0),
    partition_index   INTEGER NOT NULL CHECK(partition_index >= 0),
    partition_title   TEXT NOT NULL CHECK(length(partition_title) <= 500),
    page_start        INTEGER NOT NULL CHECK(page_start >= 1),
    page_end          INTEGER NOT NULL CHECK(page_end >= page_start),
    token_estimate    INTEGER NOT NULL CHECK(token_estimate >= 0),
    fragment_json     TEXT NOT NULL CHECK(length(fragment_json) <= 1048576),
    created_at        INTEGER NOT NULL,
    updated_at        INTEGER NOT NULL,
    PRIMARY KEY(document_id, contract_key, source_sha256)
) STRICT;

CREATE INDEX document_outline_checkpoints_order_idx
    ON document_outline_checkpoints(document_id, contract_key, window_index);

CREATE TABLE document_generation_checkpoints (
    document_id       TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    contract_key      TEXT NOT NULL CHECK(length(contract_key) = 64),
    source_sha256     TEXT NOT NULL CHECK(length(source_sha256) = 64),
    window_index      INTEGER NOT NULL CHECK(window_index >= 0),
    partition_index   INTEGER NOT NULL CHECK(partition_index >= 0),
    partition_title   TEXT NOT NULL CHECK(length(partition_title) <= 500),
    page_start        INTEGER NOT NULL CHECK(page_start >= 1),
    page_end          INTEGER NOT NULL CHECK(page_end >= page_start),
    token_estimate    INTEGER NOT NULL CHECK(token_estimate >= 0),
    cards_json        TEXT NOT NULL CHECK(length(cards_json) <= 2097152),
    card_count        INTEGER NOT NULL CHECK(card_count >= 0),
    created_at        INTEGER NOT NULL,
    updated_at        INTEGER NOT NULL,
    PRIMARY KEY(document_id, contract_key, source_sha256)
) STRICT;

CREATE INDEX document_generation_checkpoints_order_idx
    ON document_generation_checkpoints(document_id, contract_key, window_index);
