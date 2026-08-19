-- ostudy public schema baseline.
-- Generated from the final pre-release schema and then curated as the
-- immutable starting point for all public installations. Historical
-- migration steps remain available through Git history and the
-- pre-public schema evolution document.


PRAGMA foreign_keys = ON;


CREATE TABLE schema_metadata (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
) STRICT;

CREATE TABLE schema_migrations (
    version INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    checksum TEXT NOT NULL,
    applied_at INTEGER NOT NULL
) STRICT;

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

CREATE TABLE ai_profile_operations (
    operation_id        TEXT PRIMARY KEY CHECK(length(operation_id) = 32),
    operation_kind      TEXT NOT NULL
                        CHECK(operation_kind IN (
                            'create_profile', 'update_profile',
                            'delete_profile', 'set_task_default',
                            'set_credential', 'clear_credential'
                        )),
    profile_id          TEXT,
    committed_at        INTEGER NOT NULL
) STRICT;

CREATE TABLE ai_provider_capabilities (
    profile_id          TEXT NOT NULL
                        REFERENCES ai_provider_profiles(id) ON DELETE CASCADE,
    capability_key      TEXT NOT NULL CHECK(length(capability_key) BETWEEN 1 AND 80),
    status              TEXT NOT NULL
                        CHECK(status IN ('supported', 'unsupported', 'unknown')),
    source_kind         TEXT NOT NULL CHECK(source_kind IN ('declared', 'probed')),
    value_json          TEXT NOT NULL DEFAULT 'null' CHECK(length(value_json) <= 2048),
    checked_at          INTEGER NOT NULL,
    PRIMARY KEY(profile_id, capability_key)
) STRICT;

CREATE TABLE ai_provider_probe_results (
    profile_id              TEXT PRIMARY KEY
                            REFERENCES ai_provider_profiles(id) ON DELETE CASCADE,
    profile_revision        INTEGER NOT NULL CHECK(profile_revision >= 1),
    credential_epoch        INTEGER NOT NULL CHECK(credential_epoch >= 0),
    tested_model_id         TEXT NOT NULL CHECK(length(tested_model_id) <= 300),
    adapter_revision        INTEGER NOT NULL CHECK(adapter_revision >= 1),
    result_schema_version   INTEGER NOT NULL CHECK(result_schema_version >= 1),
    overall_status          TEXT NOT NULL
                            CHECK(overall_status IN ('verified', 'partial', 'failed')),
    result_json             TEXT NOT NULL CHECK(length(result_json) <= 32768),
    tested_at               INTEGER NOT NULL
) STRICT;

CREATE TABLE "ai_provider_profiles" (
    id                      TEXT PRIMARY KEY CHECK(length(id) = 32),
    display_name            TEXT NOT NULL CHECK(length(trim(display_name)) BETWEEN 1 AND 100),
    normalized_name         TEXT NOT NULL CHECK(length(normalized_name) BETWEEN 1 AND 100),
    provider_kind           TEXT NOT NULL CHECK(length(provider_kind) BETWEEN 1 AND 80),
    base_url                TEXT NOT NULL CHECK(length(base_url) BETWEEN 1 AND 2048),
    model_id                TEXT NOT NULL DEFAULT '' CHECK(length(model_id) <= 300),
    auth_mode               TEXT NOT NULL CHECK(auth_mode IN ('none', 'bearer')),
    enabled                 INTEGER NOT NULL DEFAULT 1 CHECK(enabled IN (0, 1)),
    allow_insecure_http     INTEGER NOT NULL DEFAULT 0 CHECK(allow_insecure_http IN (0, 1)),
    require_zero_data_retention INTEGER NOT NULL DEFAULT 0
                                CHECK(require_zero_data_retention IN (0, 1)),
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

CREATE TABLE ai_task_defaults (
    task_kind       TEXT PRIMARY KEY CHECK(task_kind IN ('flashcard_generation')),
    profile_id      TEXT REFERENCES ai_provider_profiles(id) ON DELETE SET NULL,
    updated_at      INTEGER NOT NULL
) STRICT;

CREATE TABLE app_settings (
    key         TEXT PRIMARY KEY,
    value_json  TEXT NOT NULL,
    updated_at  INTEGER NOT NULL
) STRICT;

CREATE TABLE asset_drafts (
    id            TEXT PRIMARY KEY CHECK(length(id) = 32),
    purpose       TEXT NOT NULL CHECK(purpose IN ('card', 'template', 'import')),
    owner_id      TEXT,
    created_at    INTEGER NOT NULL,
    updated_at    INTEGER NOT NULL,
    expires_at    INTEGER NOT NULL,
    committed_at  INTEGER,
    canceled_at   INTEGER, retention_mode TEXT NOT NULL DEFAULT 'ephemeral'
    CHECK(retention_mode IN ('ephemeral', 'persistent_editor')), editor_draft_id TEXT,
    CHECK(expires_at >= created_at),
    CHECK(committed_at IS NULL OR committed_at >= created_at),
    CHECK(canceled_at IS NULL OR canceled_at >= created_at),
    CHECK(NOT (committed_at IS NOT NULL AND canceled_at IS NOT NULL))
) STRICT;

CREATE TABLE asset_reconciliation_events (
    id             TEXT PRIMARY KEY CHECK(length(id) = 32),
    asset_id       TEXT REFERENCES assets(id) ON DELETE SET NULL,
    action         TEXT NOT NULL CHECK(action IN (
                       'staged', 'promoted', 'verified', 'missing',
                       'quarantined', 'restored', 'deleted', 'orphan_quarantined'
                   )),
    from_path      TEXT,
    to_path        TEXT,
    detail         TEXT NOT NULL DEFAULT '',
    occurred_at    INTEGER NOT NULL
) STRICT;

CREATE TABLE assets (
    id                 TEXT PRIMARY KEY CHECK(length(id) = 32),
    original_filename  TEXT NOT NULL,
    mime_type          TEXT NOT NULL,
    byte_size          INTEGER NOT NULL CHECK(byte_size >= 0),
    relative_path      TEXT NOT NULL UNIQUE,
    created_at         INTEGER NOT NULL,
    deleted_at         INTEGER
, lifecycle_state TEXT NOT NULL DEFAULT 'ready'
    CHECK(lifecycle_state IN ('staged', 'ready', 'missing', 'quarantined', 'deleted')), draft_id TEXT REFERENCES asset_drafts(id) ON DELETE SET NULL, staged_relative_path TEXT, quarantine_relative_path TEXT, ready_at INTEGER, verified_at INTEGER, updated_at INTEGER NOT NULL DEFAULT 0, lifecycle_reason TEXT NOT NULL DEFAULT '') STRICT;

CREATE TABLE card_schedules (
    card_id             TEXT PRIMARY KEY REFERENCES cards(id) ON DELETE CASCADE,
    state               INTEGER NOT NULL CHECK(state BETWEEN 0 AND 3),
    due_at              INTEGER NOT NULL,
    stability           REAL NOT NULL DEFAULT 0.0 CHECK(stability >= 0.0),
    difficulty          REAL NOT NULL DEFAULT 0.0 CHECK(difficulty >= 0.0),
    elapsed_days        INTEGER NOT NULL DEFAULT 0 CHECK(elapsed_days >= 0),
    scheduled_days      INTEGER NOT NULL DEFAULT 0 CHECK(scheduled_days >= 0),
    reps                INTEGER NOT NULL DEFAULT 0 CHECK(reps >= 0),
    lapses              INTEGER NOT NULL DEFAULT 0 CHECK(lapses >= 0),
    last_review_at      INTEGER NOT NULL,
    scheduler_name      TEXT NOT NULL,
    scheduler_version   TEXT NOT NULL,
    updated_at          INTEGER NOT NULL
, revision INTEGER NOT NULL DEFAULT 1
    CHECK(revision >= 1)) STRICT;

CREATE VIRTUAL TABLE card_search_fts USING fts5(
    note_id UNINDEXED,
    deck_id UNINDEXED,
    question,
    answer,
    tags,
    tokenize = 'unicode61 remove_diacritics 2'
);

CREATE TABLE card_template_assets (
    template_id  TEXT NOT NULL REFERENCES card_templates(id) ON DELETE CASCADE,
    asset_id     TEXT NOT NULL REFERENCES assets(id) ON DELETE RESTRICT,
    PRIMARY KEY(template_id, asset_id)
) STRICT;

CREATE TABLE card_template_fields (
    id               TEXT PRIMARY KEY CHECK(length(id) = 32),
    template_id      TEXT NOT NULL REFERENCES card_templates(id) ON DELETE CASCADE,
    name             TEXT NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 64),
    normalized_name  TEXT NOT NULL CHECK(length(trim(normalized_name)) BETWEEN 1 AND 64),
    description      TEXT NOT NULL DEFAULT '' CHECK(length(description) <= 500),
    position         INTEGER NOT NULL CHECK(position >= 0),
    UNIQUE(template_id, normalized_name),
    UNIQUE(template_id, position)
) STRICT;

CREATE TABLE card_template_rules (
    id             TEXT PRIMARY KEY CHECK(length(id) = 32),
    template_id    TEXT NOT NULL REFERENCES card_templates(id) ON DELETE CASCADE,
    name           TEXT NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 80),
    position       INTEGER NOT NULL CHECK(position >= 0), front_doc TEXT, back_doc TEXT,
    UNIQUE(template_id, name),
    UNIQUE(template_id, position)
) STRICT;

CREATE TABLE card_templates (
    id               TEXT PRIMARY KEY CHECK(length(id) = 32),
    name             TEXT NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 80),
    normalized_name  TEXT NOT NULL CHECK(length(trim(normalized_name)) BETWEEN 1 AND 80),
    description      TEXT NOT NULL DEFAULT '' CHECK(length(description) <= 2000),
    created_at       INTEGER NOT NULL,
    updated_at       INTEGER NOT NULL,
    deleted_at       INTEGER,
    revision         INTEGER NOT NULL DEFAULT 1 CHECK(revision >= 1)
) STRICT;

CREATE TABLE "cards" (
    id                TEXT PRIMARY KEY CHECK(length(id) = 32),
    note_id           TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    deck_id           TEXT NOT NULL REFERENCES decks(id) ON DELETE CASCADE,
    direction         TEXT NOT NULL DEFAULT 'forward'
                      CHECK(direction IN ('forward', 'reverse')),
    template_ordinal  INTEGER NOT NULL DEFAULT 0 CHECK(template_ordinal >= 0),
    created_at        INTEGER NOT NULL,
    updated_at        INTEGER NOT NULL,
    suspended_at      INTEGER,
    deleted_at        INTEGER,
    buried_until      INTEGER, archive_suspended_at INTEGER,
    UNIQUE(note_id, direction, template_ordinal)
) STRICT;

CREATE TABLE data_repairs (
    version       INTEGER PRIMARY KEY,
    name          TEXT NOT NULL,
    checksum      TEXT NOT NULL,
    applied_at    INTEGER NOT NULL,
    changed_rows  INTEGER NOT NULL DEFAULT 0 CHECK(changed_rows >= 0)
) STRICT;

CREATE TABLE deck_folders (
    id              TEXT PRIMARY KEY NOT NULL,
    name            TEXT NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 120),
    sort_order      INTEGER NOT NULL DEFAULT 0,
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL,
    revision        INTEGER NOT NULL DEFAULT 1 CHECK(revision >= 1)
) STRICT;

CREATE TABLE deck_import_operations (
    id                     TEXT PRIMARY KEY CHECK(length(id) = 32),
    request_fingerprint    TEXT NOT NULL CHECK(length(request_fingerprint) = 64),
    package_format         TEXT NOT NULL,
    package_version        INTEGER NOT NULL CHECK(package_version >= 1),
    status                 TEXT NOT NULL CHECK(status IN ('pending', 'completed', 'failed')),
    -- The committed result is immutable operation history rather than live deck
    -- ownership. Keeping the ID as plain text allows a later deck deletion
    -- without invalidating an already-completed retry record.
    result_deck_id         TEXT,
    result_card_count      INTEGER CHECK(result_card_count IS NULL OR result_card_count >= 0),
    result_asset_count     INTEGER CHECK(result_asset_count IS NULL OR result_asset_count >= 0),
    error_code             TEXT,
    created_at             INTEGER NOT NULL,
    updated_at             INTEGER NOT NULL,
    completed_at           INTEGER,
    CHECK (
        (status = 'completed'
         AND result_deck_id IS NOT NULL
         AND result_card_count IS NOT NULL
         AND result_asset_count IS NOT NULL
         AND completed_at IS NOT NULL
         AND error_code IS NULL)
        OR
        (status = 'pending'
         AND result_deck_id IS NULL
         AND result_card_count IS NULL
         AND result_asset_count IS NULL
         AND completed_at IS NULL
         AND error_code IS NULL)
        OR
        (status = 'failed'
         AND result_deck_id IS NULL
         AND result_card_count IS NULL
         AND result_asset_count IS NULL
         AND completed_at IS NULL
         AND error_code IS NOT NULL)
    )
) STRICT;

CREATE TABLE decks (
    id              TEXT PRIMARY KEY NOT NULL,
    name            TEXT NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 200),
    description     TEXT NOT NULL DEFAULT '' CHECK(length(description) <= 2000),
    sort_order      INTEGER NOT NULL DEFAULT 0,
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL,
    archived_at     INTEGER,
    deleted_at      INTEGER,
    revision        INTEGER NOT NULL DEFAULT 1 CHECK(revision >= 1)
, folder_id TEXT REFERENCES deck_folders(id) ON DELETE SET NULL) STRICT;

CREATE TABLE deletion_operations (
    id             TEXT PRIMARY KEY CHECK(length(id) = 32),
    entity_kind    TEXT NOT NULL CHECK(entity_kind IN ('deck', 'cards')),
    subject_label  TEXT NOT NULL,
    deck_id        TEXT,
    note_ids_json  TEXT NOT NULL,
    card_ids_json  TEXT NOT NULL,
    deleted_at     INTEGER NOT NULL,
    expires_at     INTEGER NOT NULL,
    restored_at    INTEGER,
    finalized_at   INTEGER,
    created_at     INTEGER NOT NULL,
    CHECK(expires_at >= deleted_at),
    CHECK(restored_at IS NULL OR restored_at >= deleted_at),
    CHECK(finalized_at IS NULL OR finalized_at >= deleted_at),
    CHECK(NOT (restored_at IS NOT NULL AND finalized_at IS NOT NULL))
) STRICT;

CREATE TABLE document_decks (
    document_id     TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    deck_id         TEXT NOT NULL REFERENCES decks(id) ON DELETE CASCADE,
    job_id          TEXT NOT NULL REFERENCES document_generation_jobs(id) ON DELETE CASCADE,
    created_at      INTEGER NOT NULL,
    PRIMARY KEY(document_id, deck_id)
) STRICT;

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
    PRIMARY KEY(document_id, contract_key, window_index)
) STRICT;

CREATE TABLE "document_generation_jobs" (
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

CREATE TABLE document_generation_units (
    job_id                  TEXT NOT NULL REFERENCES document_generation_jobs(id) ON DELETE CASCADE,
    page_number             INTEGER NOT NULL CHECK(page_number >= 1),
    state                   TEXT NOT NULL
                            CHECK(state IN (
                                'pending', 'running', 'succeeded', 'skipped', 'failed'
                            )),
    cards_json              TEXT NOT NULL DEFAULT '[]' CHECK(length(cards_json) <= 1048576),
    card_count              INTEGER NOT NULL DEFAULT 0 CHECK(card_count >= 0),
    safe_error_code         TEXT NOT NULL DEFAULT '' CHECK(length(safe_error_code) <= 80),
    safe_error_message      TEXT NOT NULL DEFAULT '' CHECK(length(safe_error_message) <= 1000),
    updated_at              INTEGER NOT NULL,
    PRIMARY KEY(job_id, page_number)
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
    PRIMARY KEY(document_id, contract_key, window_index)
) STRICT;

CREATE TABLE "document_outlines" (
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

CREATE TABLE document_pages (
    document_id              TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    page_number              INTEGER NOT NULL CHECK(page_number >= 1),
    plain_text               TEXT NOT NULL CHECK(length(plain_text) <= 200000),
    text_sha256              TEXT NOT NULL CHECK(length(text_sha256) = 64),
    render_relative_path     TEXT NOT NULL UNIQUE
                             CHECK(length(render_relative_path) BETWEEN 1 AND 1024),
    render_mime_type         TEXT NOT NULL CHECK(render_mime_type = 'image/png'),
    render_byte_size         INTEGER NOT NULL CHECK(render_byte_size > 0),
    render_sha256            TEXT NOT NULL CHECK(length(render_sha256) = 64),
    created_at               INTEGER NOT NULL,
    updated_at               INTEGER NOT NULL,
    PRIMARY KEY(document_id, page_number)
) STRICT;

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

CREATE TABLE documents (
    id                       TEXT PRIMARY KEY CHECK(length(id) = 32),
    title                    TEXT NOT NULL
                             CHECK(length(trim(title)) BETWEEN 1 AND 200),
    original_filename        TEXT NOT NULL
                             CHECK(length(trim(original_filename)) BETWEEN 1 AND 255),
    mime_type                TEXT NOT NULL CHECK(mime_type = 'application/pdf'),
    byte_size                INTEGER NOT NULL CHECK(byte_size > 0),
    sha256                   TEXT NOT NULL UNIQUE CHECK(length(sha256) = 64),
    source_relative_path     TEXT NOT NULL UNIQUE
                             CHECK(length(source_relative_path) BETWEEN 1 AND 1024),
    staged_relative_path     TEXT
                             CHECK(staged_relative_path IS NULL OR
                                   length(staged_relative_path) BETWEEN 1 AND 1024),
    lifecycle_state          TEXT NOT NULL
                             CHECK(lifecycle_state IN (
                                 'processing', 'ready', 'failed', 'archived'
                             )),
    page_count               INTEGER NOT NULL DEFAULT 0 CHECK(page_count >= 0),
    extracted_page_count     INTEGER NOT NULL DEFAULT 0 CHECK(extracted_page_count >= 0),
    rendered_page_count      INTEGER NOT NULL DEFAULT 0 CHECK(rendered_page_count >= 0),
    generated_note_count     INTEGER NOT NULL DEFAULT 0 CHECK(generated_note_count >= 0),
    generated_card_count     INTEGER NOT NULL DEFAULT 0 CHECK(generated_card_count >= 0),
    generated_deck_id        TEXT REFERENCES decks(id) ON DELETE SET NULL,
    safe_error_code          TEXT NOT NULL DEFAULT '' CHECK(length(safe_error_code) <= 80),
    safe_error_message       TEXT NOT NULL DEFAULT '' CHECK(length(safe_error_message) <= 1000),
    created_at               INTEGER NOT NULL,
    updated_at               INTEGER NOT NULL,
    completed_at             INTEGER,
    archived_at              INTEGER,
    revision                 INTEGER NOT NULL DEFAULT 1 CHECK(revision >= 1), extraction_backend TEXT NOT NULL DEFAULT 'legacy'
    CHECK(length(extraction_backend) BETWEEN 1 AND 80), render_backend TEXT NOT NULL DEFAULT 'legacy'
    CHECK(length(render_backend) BETWEEN 1 AND 80), semantic_element_count INTEGER NOT NULL DEFAULT 0
    CHECK(semantic_element_count >= 0), semantic_chunk_count INTEGER NOT NULL DEFAULT 0
    CHECK(semantic_chunk_count >= 0), outline_state TEXT NOT NULL DEFAULT 'pending'
    CHECK(outline_state IN ('pending', 'building', 'ready', 'failed')), scale_mode TEXT NOT NULL DEFAULT 'standard'
    CHECK(scale_mode IN ('standard', 'large')), estimated_source_tokens INTEGER NOT NULL DEFAULT 0
    CHECK(estimated_source_tokens >= 0), outline_window_count INTEGER NOT NULL DEFAULT 0
    CHECK(outline_window_count >= 0), outline_completed_window_count INTEGER NOT NULL DEFAULT 0
    CHECK(outline_completed_window_count >= 0), generation_window_count INTEGER NOT NULL DEFAULT 0
    CHECK(generation_window_count >= 0), generation_completed_window_count INTEGER NOT NULL DEFAULT 0
    CHECK(generation_completed_window_count >= 0), source_format TEXT NOT NULL DEFAULT 'pdf'
    CHECK(length(source_format) BETWEEN 1 AND 40), source_unit_kind TEXT NOT NULL DEFAULT 'page'
    CHECK(length(source_unit_kind) BETWEEN 1 AND 40), extraction_mode TEXT NOT NULL DEFAULT 'native'
    CHECK(extraction_mode IN ('native', 'page_text_fallback', 'ocr_fallback')), ocr_page_count INTEGER NOT NULL DEFAULT 0
    CHECK(ocr_page_count >= 0), ocr_completed_page_count INTEGER NOT NULL DEFAULT 0
    CHECK(ocr_completed_page_count >= 0), ocr_mean_confidence REAL
    CHECK(ocr_mean_confidence IS NULL
          OR (ocr_mean_confidence >= 0.0 AND ocr_mean_confidence <= 1.0)), ocr_engine TEXT NOT NULL DEFAULT ''
    CHECK(length(ocr_engine) <= 120), processing_started_at INTEGER, processing_duration_ms INTEGER
    CHECK(processing_duration_ms IS NULL OR processing_duration_ms >= 0), api_token_count INTEGER
    CHECK(api_token_count IS NULL OR api_token_count >= 0), api_token_usage_complete INTEGER NOT NULL DEFAULT 0
    CHECK(api_token_usage_complete IN (0, 1)),
    CHECK(extracted_page_count <= page_count),
    CHECK(rendered_page_count <= page_count),
    CHECK((lifecycle_state = 'archived') = (archived_at IS NOT NULL))
) STRICT;

CREATE TABLE editor_draft_assets (
    draft_id  TEXT NOT NULL REFERENCES editor_drafts(id) ON DELETE CASCADE,
    asset_id  TEXT NOT NULL REFERENCES assets(id) ON DELETE RESTRICT,
    PRIMARY KEY(draft_id, asset_id)
) STRICT;

CREATE TABLE editor_drafts (
    id                      TEXT PRIMARY KEY CHECK(length(id) = 32),
    open_operation_id       TEXT NOT NULL UNIQUE CHECK(length(open_operation_id) = 32),
    draft_kind              TEXT NOT NULL CHECK(draft_kind IN (
                                'card', 'template', 'bulk_cards', 'ai_profile'
                            )),
    target_entity_id        TEXT,
    context_entity_id       TEXT,
    base_revision           INTEGER CHECK(base_revision IS NULL OR base_revision >= 1),
    payload_schema_version  INTEGER NOT NULL CHECK(payload_schema_version >= 1),
    payload_json            TEXT NOT NULL,
    payload_sha256          TEXT NOT NULL CHECK(length(payload_sha256) = 64),
    revision                INTEGER NOT NULL DEFAULT 1 CHECK(revision >= 1),
    state                   TEXT NOT NULL CHECK(state IN (
                                'active', 'committed', 'discarded', 'conflicted'
                            )),
    asset_draft_id          TEXT,
    created_at              INTEGER NOT NULL,
    updated_at              INTEGER NOT NULL,
    last_opened_at          INTEGER NOT NULL,
    committed_at            INTEGER,
    discarded_at            INTEGER,
    conflicted_at           INTEGER,
    CHECK(NOT (committed_at IS NOT NULL AND discarded_at IS NOT NULL)),
    CHECK(NOT (committed_at IS NOT NULL AND conflicted_at IS NOT NULL)),
    CHECK(NOT (discarded_at IS NOT NULL AND conflicted_at IS NOT NULL))
) STRICT;

CREATE TABLE library_organization (
    singleton_id    INTEGER PRIMARY KEY NOT NULL CHECK(singleton_id = 1),
    revision        INTEGER NOT NULL CHECK(revision >= 1),
    updated_at      INTEGER NOT NULL
) STRICT;

CREATE TABLE note_assets (
    note_id   TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    asset_id  TEXT NOT NULL REFERENCES assets(id) ON DELETE RESTRICT,
    side      TEXT NOT NULL CHECK(side IN ('front', 'back')),
    PRIMARY KEY(note_id, asset_id, side)
) STRICT;

CREATE TABLE note_tags (
    note_id  TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    tag_id   TEXT NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    PRIMARY KEY(note_id, tag_id)
) STRICT;

CREATE TABLE "notes" (
    id                      TEXT PRIMARY KEY CHECK(length(id) = 32),
    deck_id                 TEXT NOT NULL REFERENCES decks(id) ON DELETE CASCADE,
    note_type               TEXT NOT NULL DEFAULT 'basic'
                            CHECK(note_type IN ('basic', 'cloze')),
    content_format          TEXT NOT NULL DEFAULT 'tiptap-json'
                            CHECK(content_format = 'tiptap-json'),
    content_schema_version  INTEGER NOT NULL DEFAULT 1 CHECK(content_schema_version >= 1),
    front_doc               TEXT NOT NULL,
    back_doc                TEXT NOT NULL,
    front_text              TEXT NOT NULL,
    back_text               TEXT NOT NULL,
    created_at              INTEGER NOT NULL,
    updated_at              INTEGER NOT NULL,
    deleted_at              INTEGER,
    revision                INTEGER NOT NULL DEFAULT 1 CHECK(revision >= 1),
    duplicate_hash          TEXT NOT NULL DEFAULT '',
    reverse_enabled         INTEGER NOT NULL DEFAULT 0 CHECK(reverse_enabled IN (0, 1)),
    cloze_source            TEXT NOT NULL DEFAULT '',
    cloze_mode              TEXT NOT NULL DEFAULT 'one_at_a_time'
                            CHECK(cloze_mode IN ('one_at_a_time', 'all_together'))
) STRICT;

CREATE TABLE practice_session_card_snapshots (
    session_id                TEXT NOT NULL
                              REFERENCES practice_sessions(id) ON DELETE CASCADE,
    card_id                   TEXT NOT NULL CHECK(length(card_id) = 32),
    note_id                   TEXT NOT NULL CHECK(length(note_id) = 32),
    note_revision             INTEGER NOT NULL CHECK(note_revision >= 1),
    owner_deck_id_at_capture  TEXT NOT NULL CHECK(length(owner_deck_id_at_capture) = 32),
    captured_deck_name        TEXT NOT NULL,
    direction                 TEXT NOT NULL CHECK(direction IN ('forward', 'reverse')),
    card_type                 TEXT NOT NULL CHECK(card_type IN ('basic', 'cloze')),
    card_label                TEXT NOT NULL,
    presentation_variant      TEXT NOT NULL,
    front_doc_json            TEXT NOT NULL,
    back_doc_json             TEXT NOT NULL,
    front_text                TEXT NOT NULL,
    back_text                 TEXT NOT NULL,
    question_excerpt          TEXT NOT NULL,
    question_text_hash        TEXT NOT NULL CHECK(length(question_text_hash) = 64),
    question_was_truncated    INTEGER NOT NULL CHECK(question_was_truncated IN (0, 1)),
    captured_at               INTEGER NOT NULL,
    PRIMARY KEY(session_id, card_id)
) STRICT;

CREATE TABLE practice_session_card_tag_snapshots (
    session_id  TEXT NOT NULL,
    card_id     TEXT NOT NULL CHECK(length(card_id) = 32),
    tag_id      TEXT NOT NULL CHECK(length(tag_id) = 32),
    tag_name    TEXT NOT NULL,
    PRIMARY KEY(session_id, card_id, tag_id),
    FOREIGN KEY(session_id, card_id)
        REFERENCES practice_session_card_snapshots(session_id, card_id)
        ON DELETE CASCADE
) STRICT;

CREATE TABLE practice_session_cards (
    session_id   TEXT NOT NULL REFERENCES practice_sessions(id) ON DELETE CASCADE,
    card_id      TEXT NOT NULL REFERENCES cards(id) ON DELETE RESTRICT,
    position     INTEGER NOT NULL CHECK(position >= 0),
    viewed_at    INTEGER,
    revealed_at  INTEGER, rated_at INTEGER, rating INTEGER
    CHECK(rating IS NULL OR rating BETWEEN 1 AND 4), response_time_ms INTEGER
    CHECK(response_time_ms IS NULL OR response_time_ms >= 0), presentation_variant TEXT NOT NULL DEFAULT ''
    CHECK(length(presentation_variant) <= 64), dismissed_at INTEGER, disposition TEXT
    CHECK(disposition IS NULL OR disposition IN ('buried', 'suspended')),
    PRIMARY KEY(session_id, card_id),
    UNIQUE(session_id, position)
) STRICT;

CREATE TABLE practice_sessions (
    id                       TEXT PRIMARY KEY CHECK(length(id) = 32),
    selected_deck_ids_json   TEXT NOT NULL,
    shuffle                  INTEGER NOT NULL CHECK(shuffle IN (0, 1)),
    card_count               INTEGER NOT NULL CHECK(card_count >= 0),
    viewed_count             INTEGER NOT NULL DEFAULT 0 CHECK(viewed_count >= 0),
    revealed_count           INTEGER NOT NULL DEFAULT 0 CHECK(revealed_count >= 0),
    started_at               INTEGER NOT NULL,
    completed_at             INTEGER
, rated_count INTEGER NOT NULL DEFAULT 0
    CHECK(rated_count >= 0), correct_count INTEGER NOT NULL DEFAULT 0
    CHECK(correct_count >= 0), mode TEXT NOT NULL DEFAULT 'scheduled'
    CHECK(mode IN ('scheduled', 'extra', 'custom')), start_operation_id TEXT
    CHECK(start_operation_id IS NULL OR length(start_operation_id) = 32), start_request_fingerprint TEXT, selected_tag_ids_json TEXT NOT NULL DEFAULT '[]') STRICT;

CREATE TABLE prepared_exports (
    id                  TEXT PRIMARY KEY CHECK(length(id) = 32),
    operation_id        TEXT NOT NULL UNIQUE CHECK(length(operation_id) = 32),
    purpose             TEXT NOT NULL CHECK(purpose IN ('studydeck', 'library_backup')),
    suggested_filename  TEXT NOT NULL,
    media_type          TEXT NOT NULL,
    byte_size           INTEGER NOT NULL CHECK(byte_size > 0),
    sha256              TEXT NOT NULL CHECK(length(sha256) = 64),
    relative_path       TEXT NOT NULL UNIQUE,
    state               TEXT NOT NULL CHECK(state IN (
                            'prepared', 'published', 'released', 'failed'
                        )),
    summary_json        TEXT NOT NULL,
    created_at          INTEGER NOT NULL,
    updated_at          INTEGER NOT NULL,
    expires_at          INTEGER NOT NULL,
    published_at        INTEGER,
    released_at         INTEGER,
    CHECK(expires_at >= created_at),
    CHECK(published_at IS NULL OR published_at >= created_at),
    CHECK(released_at IS NULL OR released_at >= created_at)
) STRICT;

CREATE TABLE review_event_attributions (
    review_event_id            TEXT PRIMARY KEY
                               REFERENCES review_events(id) ON DELETE CASCADE,
    reviewed_at                INTEGER NOT NULL,
    review_local_date          TEXT NOT NULL
                               CHECK(review_local_date GLOB
                                   '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
    time_zone_id               TEXT NOT NULL,
    utc_offset_minutes         INTEGER NOT NULL CHECK(utc_offset_minutes BETWEEN -1440 AND 1440),
    card_id                    TEXT NOT NULL CHECK(length(card_id) = 32),
    note_id                    TEXT NOT NULL CHECK(length(note_id) = 32),
    note_revision              INTEGER NOT NULL CHECK(note_revision >= 1),
    owner_deck_id_at_review    TEXT NOT NULL CHECK(length(owner_deck_id_at_review) = 32),
    captured_deck_name         TEXT NOT NULL,
    direction                  TEXT NOT NULL CHECK(direction IN ('forward', 'reverse')),
    card_type                  TEXT NOT NULL CHECK(card_type IN ('basic', 'cloze')),
    card_label                 TEXT NOT NULL,
    presentation_variant       TEXT NOT NULL,
    question_excerpt           TEXT NOT NULL,
    question_text_hash         TEXT NOT NULL CHECK(length(question_text_hash) = 64),
    question_was_truncated     INTEGER NOT NULL CHECK(question_was_truncated IN (0, 1)),
    attribution_source         TEXT NOT NULL
                               CHECK(attribution_source IN (
                                   'captured',
                                   'legacy_current_state_backfill'
                               ))
) STRICT;

CREATE TABLE review_event_tag_attributions (
    review_event_id  TEXT NOT NULL
                     REFERENCES review_event_attributions(review_event_id)
                     ON DELETE CASCADE,
    tag_id           TEXT NOT NULL CHECK(length(tag_id) = 32),
    tag_name         TEXT NOT NULL,
    PRIMARY KEY(review_event_id, tag_id)
) STRICT;

CREATE TABLE review_events (
    id                      TEXT PRIMARY KEY CHECK(length(id) = 32),
    submission_id           TEXT NOT NULL UNIQUE CHECK(length(submission_id) = 32),
    session_id              TEXT REFERENCES practice_sessions(id) ON DELETE SET NULL,
    card_id                 TEXT NOT NULL REFERENCES cards(id) ON DELETE RESTRICT,
    reviewed_at             INTEGER NOT NULL,
    rating                  INTEGER NOT NULL CHECK(rating BETWEEN 1 AND 4),
    answer_correct          INTEGER NOT NULL CHECK(answer_correct IN (0, 1)),
    confidence              TEXT NOT NULL
                            CHECK(confidence IN (
                                'wrong',
                                'not_confident',
                                'somewhat_confident',
                                'very_confident'
                            )),
    response_time_ms        INTEGER NOT NULL
                            CHECK(response_time_ms BETWEEN 0 AND 86400000),
    state_before            INTEGER NOT NULL CHECK(state_before BETWEEN 0 AND 3),
    state_after             INTEGER NOT NULL CHECK(state_after BETWEEN 0 AND 3),
    due_before              INTEGER NOT NULL,
    due_after               INTEGER NOT NULL,
    stability_before        REAL NOT NULL CHECK(stability_before >= 0.0),
    stability_after         REAL NOT NULL CHECK(stability_after >= 0.0),
    difficulty_before       REAL NOT NULL CHECK(difficulty_before >= 0.0),
    difficulty_after        REAL NOT NULL CHECK(difficulty_after >= 0.0),
    scheduled_days_before   INTEGER NOT NULL CHECK(scheduled_days_before >= 0),
    scheduled_days_after    INTEGER NOT NULL CHECK(scheduled_days_after >= 0),
    scheduler_name          TEXT NOT NULL,
    scheduler_version       TEXT NOT NULL,
    created_at              INTEGER NOT NULL
, request_fingerprint TEXT, result_completed INTEGER
    CHECK(result_completed IS NULL OR result_completed IN (0, 1)), result_rated_count INTEGER
    CHECK(result_rated_count IS NULL OR result_rated_count >= 0), result_correct_count INTEGER
    CHECK(result_correct_count IS NULL OR result_correct_count >= 0)) STRICT;

CREATE TABLE schedule_action_events (
    id                    TEXT PRIMARY KEY CHECK(length(id) = 32),
    operation_id          TEXT NOT NULL
                          REFERENCES schedule_action_operations(id) ON DELETE RESTRICT,
    session_id            TEXT REFERENCES practice_sessions(id) ON DELETE SET NULL,
    card_id               TEXT NOT NULL REFERENCES cards(id) ON DELETE RESTRICT,
    action                TEXT NOT NULL CHECK(action IN (
                              'suspend', 'resume', 'bury', 'unbury',
                              'reset', 'reschedule'
                          )),
    due_before            INTEGER NOT NULL,
    due_after             INTEGER NOT NULL,
    state_before          INTEGER NOT NULL CHECK(state_before BETWEEN 0 AND 3),
    state_after           INTEGER NOT NULL CHECK(state_after BETWEEN 0 AND 3),
    suspended_before      INTEGER,
    suspended_after       INTEGER,
    buried_until_before   INTEGER,
    buried_until_after    INTEGER,
    schedule_revision_before INTEGER NOT NULL CHECK(schedule_revision_before >= 1),
    schedule_revision_after  INTEGER NOT NULL CHECK(schedule_revision_after >= 1),
    occurred_at           INTEGER NOT NULL,
    created_at            INTEGER NOT NULL,
    UNIQUE(operation_id, card_id)
) STRICT;

CREATE TABLE schedule_action_operations (
    id                    TEXT PRIMARY KEY CHECK(length(id) = 32),
    request_fingerprint   TEXT,
    session_id            TEXT REFERENCES practice_sessions(id) ON DELETE SET NULL,
    action                TEXT NOT NULL CHECK(action IN (
                              'suspend', 'resume', 'bury', 'unbury',
                              'reset', 'reschedule'
                          )),
    due_at                INTEGER,
    requested_count       INTEGER NOT NULL
                          CHECK(requested_count BETWEEN 1 AND 10000),
    changed_count         INTEGER NOT NULL CHECK(changed_count >= 0),
    unchanged_count       INTEGER NOT NULL CHECK(unchanged_count >= 0),
    rejected_count        INTEGER NOT NULL CHECK(rejected_count >= 0),
    occurred_at           INTEGER NOT NULL,
    created_at            INTEGER NOT NULL, reschedule_intent_json TEXT,
    CHECK(changed_count + unchanged_count + rejected_count = requested_count),
    CHECK(
        (action = 'reschedule' AND due_at IS NOT NULL)
        OR (action <> 'reschedule' AND due_at IS NULL)
    )
) STRICT;

CREATE TABLE staged_external_imports (
    id              TEXT PRIMARY KEY CHECK(length(id) = 32),
    operation_id    TEXT NOT NULL UNIQUE CHECK(length(operation_id) = 32),
    purpose         TEXT NOT NULL CHECK(purpose IN (
                        'image', 'pdf_document', 'study_content', 'library_backup'
                    )),
    detected_kind   TEXT NOT NULL CHECK(detected_kind IN (
                        'png', 'jpeg', 'gif', 'webp', 'pdf',
                        'studydeck', 'generated_json', 'ostudybackup'
                    )),
    display_name    TEXT NOT NULL,
    media_type      TEXT NOT NULL,
    byte_size       INTEGER NOT NULL CHECK(byte_size > 0),
    sha256          TEXT NOT NULL CHECK(length(sha256) = 64),
    relative_path   TEXT NOT NULL UNIQUE,
    state           TEXT NOT NULL CHECK(state IN (
                        'staging', 'staged', 'consumed', 'released', 'failed'
                    )),
    result_json     TEXT NOT NULL DEFAULT '',
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL,
    expires_at      INTEGER NOT NULL,
    consumed_at     INTEGER,
    released_at     INTEGER,
    CHECK(expires_at >= created_at),
    CHECK(consumed_at IS NULL OR consumed_at >= created_at),
    CHECK(released_at IS NULL OR released_at >= created_at)
) STRICT;

CREATE TABLE tags (
    id               TEXT PRIMARY KEY CHECK(length(id) = 32),
    name             TEXT NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 64),
    normalized_name  TEXT NOT NULL UNIQUE
                     CHECK(length(trim(normalized_name)) BETWEEN 1 AND 64),
    created_at       INTEGER NOT NULL,
    updated_at       INTEGER NOT NULL
) STRICT;

CREATE INDEX ai_document_usage_samples_model_time_idx
    ON ai_document_usage_samples(provider_kind, model_id, created_at DESC);

CREATE UNIQUE INDEX ai_provider_profiles_active_name_idx
    ON ai_provider_profiles(normalized_name)
    WHERE deleted_at IS NULL;

CREATE INDEX ai_provider_profiles_active_updated_idx
    ON ai_provider_profiles(updated_at DESC)
    WHERE deleted_at IS NULL;

CREATE UNIQUE INDEX asset_drafts_editor_draft_idx
    ON asset_drafts(editor_draft_id)
    WHERE editor_draft_id IS NOT NULL;

CREATE INDEX asset_drafts_expiry_idx
    ON asset_drafts(expires_at)
    WHERE committed_at IS NULL AND canceled_at IS NULL;

CREATE INDEX asset_reconciliation_events_asset_time_idx
    ON asset_reconciliation_events(asset_id, occurred_at DESC);

CREATE INDEX assets_draft_idx
    ON assets(draft_id)
    WHERE draft_id IS NOT NULL;

CREATE INDEX assets_lifecycle_state_idx
    ON assets(lifecycle_state, updated_at);

CREATE UNIQUE INDEX assets_quarantine_path_idx
    ON assets(quarantine_relative_path)
    WHERE quarantine_relative_path IS NOT NULL;

CREATE UNIQUE INDEX assets_staged_path_idx
    ON assets(staged_relative_path)
    WHERE staged_relative_path IS NOT NULL;

CREATE INDEX card_schedules_due_idx
    ON card_schedules(due_at, state);

CREATE INDEX card_template_assets_asset_idx
    ON card_template_assets(asset_id);

CREATE INDEX card_template_fields_template_idx
    ON card_template_fields(template_id, position);

CREATE INDEX card_template_rules_template_idx
    ON card_template_rules(template_id, position);

CREATE UNIQUE INDEX card_templates_active_name_idx
    ON card_templates(normalized_name)
    WHERE deleted_at IS NULL;

CREATE INDEX cards_buried_until_idx
    ON cards(buried_until)
    WHERE buried_until IS NOT NULL;

CREATE INDEX cards_deck_active_idx
    ON cards(deck_id, updated_at DESC)
    WHERE deleted_at IS NULL;

CREATE INDEX cards_studyability_idx
    ON cards(deck_id, suspended_at, archive_suspended_at, buried_until, id)
    WHERE deleted_at IS NULL;

CREATE INDEX deck_folders_sort
    ON deck_folders(sort_order, name COLLATE NOCASE, created_at);

CREATE INDEX deck_import_operations_status_idx
    ON deck_import_operations(status, updated_at);

CREATE INDEX decks_active_sort
    ON decks(deleted_at, archived_at, sort_order, name COLLATE NOCASE);

CREATE INDEX decks_folder_sort
    ON decks(deleted_at, archived_at, folder_id, sort_order, name COLLATE NOCASE);

CREATE INDEX deletion_operations_expiry_idx
    ON deletion_operations(expires_at)
    WHERE restored_at IS NULL AND finalized_at IS NULL;

CREATE INDEX document_decks_deck_idx ON document_decks(deck_id);

CREATE INDEX document_generation_checkpoints_order_idx
    ON document_generation_checkpoints(document_id, contract_key, window_index);

CREATE INDEX document_generation_jobs_active_idx
    ON document_generation_jobs(state, updated_at)
    WHERE state IN ('queued', 'extracting', 'generating', 'committing', 'interrupted');

CREATE INDEX document_generation_jobs_document_time_idx
    ON document_generation_jobs(document_id, created_at DESC);

CREATE INDEX document_generation_units_state_idx
    ON document_generation_units(job_id, state, page_number);

CREATE INDEX document_generation_windows_state_idx
    ON document_generation_windows(job_id, state, window_index);

CREATE INDEX document_outline_checkpoints_order_idx
    ON document_outline_checkpoints(document_id, contract_key, window_index);

CREATE INDEX document_pages_document_idx
    ON document_pages(document_id, page_number);

CREATE INDEX document_semantic_chunks_pages_idx
    ON document_semantic_chunks(document_id, chunk_index);

CREATE INDEX document_semantic_elements_page_idx
    ON document_semantic_elements(document_id, page_number, element_index);

CREATE INDEX document_semantic_elements_type_idx
    ON document_semantic_elements(document_id, element_type, page_number);

CREATE INDEX documents_active_updated_idx
    ON documents(updated_at DESC, id)
    WHERE archived_at IS NULL;

CREATE INDEX documents_archived_updated_idx
    ON documents(updated_at DESC, id)
    WHERE archived_at IS NOT NULL;

CREATE INDEX documents_generated_deck_idx
    ON documents(generated_deck_id)
    WHERE generated_deck_id IS NOT NULL;

CREATE INDEX editor_draft_assets_asset_idx
    ON editor_draft_assets(asset_id);

CREATE INDEX editor_drafts_active_context_idx
    ON editor_drafts(draft_kind, context_entity_id, updated_at DESC)
    WHERE state = 'active';

CREATE UNIQUE INDEX editor_drafts_active_target_idx
    ON editor_drafts(draft_kind, target_entity_id)
    WHERE state = 'active' AND target_entity_id IS NOT NULL;

CREATE INDEX editor_drafts_state_updated_idx
    ON editor_drafts(state, updated_at DESC);

CREATE INDEX note_assets_asset_idx ON note_assets(asset_id);

CREATE INDEX note_tags_tag_idx ON note_tags(tag_id, note_id);

CREATE INDEX notes_deck_active_idx
    ON notes(deck_id, updated_at DESC)
    WHERE deleted_at IS NULL;

CREATE INDEX notes_deck_duplicate_idx
    ON notes(deck_id, duplicate_hash)
    WHERE deleted_at IS NULL;

CREATE INDEX practice_session_card_snapshots_session_position_idx
    ON practice_session_card_snapshots(session_id, card_id);

CREATE INDEX practice_session_card_tag_snapshots_tag_idx
    ON practice_session_card_tag_snapshots(tag_id, session_id, card_id);

CREATE UNIQUE INDEX practice_sessions_start_operation_idx
    ON practice_sessions(start_operation_id)
    WHERE start_operation_id IS NOT NULL;

CREATE INDEX practice_sessions_started_idx
    ON practice_sessions(started_at DESC);

CREATE INDEX prepared_exports_state_expiry_idx
    ON prepared_exports(state, expires_at);

CREATE INDEX review_event_attributions_capture_deck_idx
    ON review_event_attributions(owner_deck_id_at_review, review_local_date, reviewed_at DESC);

CREATE INDEX review_event_attributions_card_date_idx
    ON review_event_attributions(card_id, review_local_date, reviewed_at DESC);

CREATE INDEX review_event_attributions_local_date_idx
    ON review_event_attributions(review_local_date, reviewed_at DESC, review_event_id);

CREATE INDEX review_event_tag_attributions_tag_idx
    ON review_event_tag_attributions(tag_id, review_event_id);

CREATE INDEX review_events_card_reviewed_created_idx
    ON review_events(card_id, reviewed_at DESC, created_at DESC, id DESC);

CREATE INDEX review_events_card_reviewed_idx
    ON review_events(card_id, reviewed_at DESC);

CREATE INDEX review_events_reviewed_idx
    ON review_events(reviewed_at DESC);

CREATE INDEX schedule_action_events_card_time_idx
    ON schedule_action_events(card_id, occurred_at DESC);

CREATE INDEX schedule_action_events_operation_idx
    ON schedule_action_events(operation_id);

CREATE INDEX schedule_action_operations_time_idx
    ON schedule_action_operations(occurred_at DESC);

CREATE INDEX staged_external_imports_state_expiry_idx
    ON staged_external_imports(state, expires_at);

CREATE TRIGGER deck_library_revision_after_delete
AFTER DELETE ON decks
BEGIN
    UPDATE library_organization
    SET revision = revision + 1,
        updated_at = MAX(updated_at, OLD.updated_at)
    WHERE singleton_id = 1;
END;

CREATE TRIGGER deck_library_revision_after_insert
AFTER INSERT ON decks
BEGIN
    UPDATE library_organization
    SET revision = revision + 1,
        updated_at = MAX(updated_at, NEW.updated_at)
    WHERE singleton_id = 1;
END;

CREATE TRIGGER deck_library_revision_after_visibility_change
AFTER UPDATE OF deleted_at, archived_at ON decks
WHEN OLD.deleted_at IS NOT NEW.deleted_at
  OR OLD.archived_at IS NOT NEW.archived_at
BEGIN
    UPDATE library_organization
    SET revision = revision + 1,
        updated_at = MAX(updated_at, NEW.updated_at)
    WHERE singleton_id = 1;
END;

CREATE TRIGGER note_tags_search_after_delete
AFTER DELETE ON note_tags
BEGIN
    UPDATE card_search_fts
    SET tags = COALESCE((
        SELECT group_concat(t.name, ' ')
        FROM note_tags nt
        JOIN tags t ON t.id = nt.tag_id
        WHERE nt.note_id = OLD.note_id
    ), '')
    WHERE rowid = (SELECT rowid FROM notes WHERE id = OLD.note_id);
END;

CREATE TRIGGER note_tags_search_after_insert
AFTER INSERT ON note_tags
BEGIN
    UPDATE card_search_fts
    SET tags = COALESCE((
        SELECT group_concat(t.name, ' ')
        FROM note_tags nt
        JOIN tags t ON t.id = nt.tag_id
        WHERE nt.note_id = NEW.note_id
    ), '')
    WHERE rowid = (SELECT rowid FROM notes WHERE id = NEW.note_id);
END;

CREATE TRIGGER notes_search_after_delete
AFTER DELETE ON notes
BEGIN
    DELETE FROM card_search_fts WHERE rowid = OLD.rowid;
END;

CREATE TRIGGER notes_search_after_insert
AFTER INSERT ON notes
WHEN NEW.deleted_at IS NULL
BEGIN
    INSERT INTO card_search_fts(rowid, note_id, deck_id, question, answer, tags)
    VALUES (NEW.rowid, NEW.id, NEW.deck_id, NEW.front_text, NEW.back_text, '');
END;

CREATE TRIGGER notes_search_after_update
AFTER UPDATE OF deck_id, front_text, back_text, deleted_at ON notes
BEGIN
    DELETE FROM card_search_fts WHERE rowid = OLD.rowid;
    INSERT INTO card_search_fts(rowid, note_id, deck_id, question, answer, tags)
    SELECT
        NEW.rowid,
        NEW.id,
        NEW.deck_id,
        NEW.front_text,
        NEW.back_text,
        COALESCE((
            SELECT group_concat(t.name, ' ')
            FROM note_tags nt
            JOIN tags t ON t.id = nt.tag_id
            WHERE nt.note_id = NEW.id
        ), '')
    WHERE NEW.deleted_at IS NULL
      AND EXISTS (
          SELECT 1
          FROM decks d
          WHERE d.id = NEW.deck_id
            AND d.deleted_at IS NULL
            AND d.archived_at IS NULL
      );
END;

CREATE TRIGGER tags_search_after_update
AFTER UPDATE OF name ON tags
BEGIN
    UPDATE card_search_fts
    SET tags = COALESCE((
        SELECT group_concat(t.name, ' ')
        FROM note_tags nt
        JOIN tags t ON t.id = nt.tag_id
        WHERE nt.note_id = card_search_fts.note_id
    ), '')
    WHERE note_id IN (
        SELECT note_id FROM note_tags WHERE tag_id = NEW.id
    );
END;

CREATE TRIGGER template_rules_canonical_docs_before_insert
BEFORE INSERT ON card_template_rules
WHEN NEW.front_doc IS NULL OR NEW.back_doc IS NULL
BEGIN
    SELECT RAISE(ABORT, 'template layouts are required');
END;

CREATE TRIGGER template_rules_canonical_docs_before_update
BEFORE UPDATE OF front_doc, back_doc ON card_template_rules
WHEN NEW.front_doc IS NULL OR NEW.back_doc IS NULL
BEGIN
    SELECT RAISE(ABORT, 'template layouts are required');
END;

INSERT INTO "app_settings" ("key", "value_json", "updated_at") VALUES ('theme', '"dark"', 0);

INSERT INTO "app_settings" ("key", "value_json", "updated_at") VALUES ('uiFont', '"system"', 0);

INSERT INTO "app_settings" ("key", "value_json", "updated_at") VALUES ('cardFont', '"system"', 0);

INSERT INTO "app_settings" ("key", "value_json", "updated_at") VALUES ('uiScale', '100', 0);

INSERT INTO "app_settings" ("key", "value_json", "updated_at") VALUES ('cardBaseSize', '18', 0);

INSERT INTO "app_settings" ("key", "value_json", "updated_at") VALUES ('reduceMotion', 'false', 0);

INSERT INTO "app_settings" ("key", "value_json", "updated_at") VALUES ('confirmDeletions', 'true', 0);

INSERT INTO "app_settings" ("key", "value_json", "updated_at") VALUES ('practiceShuffle', 'true', 0);

INSERT INTO "app_settings" ("key", "value_json", "updated_at") VALUES ('desiredRetention', '0.9', 0);

INSERT INTO "app_settings" ("key", "value_json", "updated_at") VALUES ('dailyNewLimit', '20', 0);

INSERT INTO "app_settings" ("key", "value_json", "updated_at") VALUES ('dailyReviewLimit', '200', 0);

INSERT INTO "library_organization" ("singleton_id", "revision", "updated_at") VALUES (1, 1, 0);

INSERT INTO schema_metadata (key, value) VALUES ('schema_lineage', 'ostudy-public-v1');
