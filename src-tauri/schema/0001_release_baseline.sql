-- oStudy 0.1 release schema baseline.
-- Generated from the qualified ostudy-public-v1 schema-30 result.
-- Historical public-v1 migrations remain compatibility input only;
-- fresh 0.1 libraries execute this baseline directly.

PRAGMA foreign_keys = ON;

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

CREATE TABLE "ai_embedding_options" (
    profile_id TEXT NOT NULL REFERENCES ai_provider_profiles(id) ON DELETE CASCADE,
    model_id TEXT NOT NULL CHECK(length(trim(model_id)) BETWEEN 1 AND 300),
    dimensions INTEGER CHECK(dimensions IS NULL OR dimensions BETWEEN 32 AND 4096),
    input_policy TEXT NOT NULL DEFAULT 'automatic'
        CHECK(input_policy IN ('automatic','symmetric','qwen_retrieval','search_types')),
    revision INTEGER NOT NULL DEFAULT 1 CHECK(revision >= 1),
    updated_at INTEGER NOT NULL,
    PRIMARY KEY(profile_id, model_id)
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
, task_kind TEXT
    CHECK(task_kind IS NULL OR task_kind IN ('flashcard_generation', 'tutor', 'embeddings')), model_id TEXT
    CHECK(model_id IS NULL OR length(model_id) <= 300), reasoning_effort TEXT
    CHECK(reasoning_effort IS NULL OR reasoning_effort IN
        ('automatic','none','minimal','low','medium','high','xhigh','max'))) STRICT;

CREATE TABLE "ai_provider_capabilities" (
    profile_id TEXT NOT NULL REFERENCES ai_provider_profiles(id) ON DELETE CASCADE,
    model_id TEXT NOT NULL CHECK(length(trim(model_id)) BETWEEN 1 AND 300),
    capability_key TEXT NOT NULL CHECK(length(capability_key) BETWEEN 1 AND 80),
    status TEXT NOT NULL CHECK(status IN ('supported','unsupported','unknown')),
    source_kind TEXT NOT NULL CHECK(source_kind IN ('declared','probed')),
    value_json TEXT NOT NULL DEFAULT 'null' CHECK(length(value_json) <= 2048),
    checked_at INTEGER NOT NULL,
    PRIMARY KEY(profile_id, model_id, capability_key)
) STRICT;

CREATE TABLE "ai_provider_probe_results" (
    profile_id TEXT NOT NULL REFERENCES ai_provider_profiles(id) ON DELETE CASCADE,
    profile_revision INTEGER NOT NULL CHECK(profile_revision >= 1),
    credential_epoch INTEGER NOT NULL CHECK(credential_epoch >= 0),
    tested_model_id TEXT NOT NULL CHECK(length(tested_model_id) BETWEEN 1 AND 300),
    adapter_revision INTEGER NOT NULL CHECK(adapter_revision >= 1),
    result_schema_version INTEGER NOT NULL CHECK(result_schema_version >= 1),
    overall_status TEXT NOT NULL CHECK(overall_status IN ('verified','partial','failed')),
    result_json TEXT NOT NULL CHECK(length(result_json) <= 32768),
    tested_at INTEGER NOT NULL,
    PRIMARY KEY(profile_id, tested_model_id)
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
, reasoning_effort TEXT NOT NULL DEFAULT 'low'
CHECK(reasoning_effort IN (
    'automatic',
    'none',
    'minimal',
    'low',
    'medium',
    'high',
    'xhigh',
    'max'
))) STRICT;

CREATE TABLE "ai_task_defaults" (
    task_kind TEXT PRIMARY KEY
        CHECK(task_kind IN ('flashcard_generation','tutor','embeddings')),
    profile_id TEXT REFERENCES ai_provider_profiles(id) ON DELETE SET NULL,
    model_id TEXT NOT NULL DEFAULT '' CHECK(length(model_id) <= 300),
    reasoning_effort TEXT NOT NULL DEFAULT 'automatic'
        CHECK(reasoning_effort IN
            ('automatic','none','minimal','low','medium','high','xhigh','max')),
    updated_at INTEGER NOT NULL,
    CHECK((profile_id IS NULL AND model_id = '')
       OR (profile_id IS NOT NULL AND length(trim(model_id)) >= 1))
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

CREATE TABLE card_question_presentations (
    card_id TEXT PRIMARY KEY REFERENCES cards(id) ON DELETE CASCADE,
    note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    question_key TEXT NOT NULL CHECK(length(question_key) BETWEEN 1 AND 64),
    UNIQUE(note_id, question_key)
) STRICT;

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

CREATE TABLE collection_guide_checkpoint_sources (
    cache_key TEXT NOT NULL REFERENCES collection_guide_checkpoints(cache_key) ON DELETE CASCADE,
    source_id TEXT NOT NULL REFERENCES knowledge_sources(id) ON DELETE CASCADE,
    PRIMARY KEY(cache_key, source_id)
) STRICT;

CREATE TABLE collection_guide_checkpoints (
    cache_key TEXT PRIMARY KEY CHECK(length(cache_key) = 64),
    collection_id TEXT NOT NULL REFERENCES document_collections(id) ON DELETE CASCADE,
    contract_hash TEXT NOT NULL CHECK(length(contract_hash) = 64),
    result_json TEXT NOT NULL CHECK(json_valid(result_json) AND length(result_json) <= 2097152),
    result_hash TEXT NOT NULL CHECK(length(result_hash) = 64),
    created_at INTEGER NOT NULL
) STRICT;

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

CREATE TABLE deck_source_citations (
    deck_id     TEXT NOT NULL REFERENCES decks(id) ON DELETE CASCADE,
    citation_id TEXT NOT NULL REFERENCES source_citation_snapshots(id) ON DELETE RESTRICT,
    created_at  INTEGER NOT NULL,
    PRIMARY KEY(deck_id, citation_id)
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

CREATE TABLE document_artifact_jobs (
    id                       TEXT PRIMARY KEY CHECK(length(id) = 32),
    document_id              TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    plan_id                  TEXT REFERENCES document_learning_plans(id) ON DELETE SET NULL,
    artifact_kind            TEXT NOT NULL CHECK(artifact_kind IN ('study_guide', 'flashcards')),
    profile_id               TEXT REFERENCES ai_provider_profiles(id) ON DELETE SET NULL,
    provider_kind            TEXT NOT NULL CHECK(length(provider_kind) <= 80),
    model_id                 TEXT NOT NULL CHECK(length(model_id) <= 300),
    profile_revision         INTEGER NOT NULL DEFAULT 1 CHECK(profile_revision >= 1),
    credential_epoch         INTEGER NOT NULL DEFAULT 0 CHECK(credential_epoch >= 0),
    network_scope            TEXT NOT NULL CHECK(network_scope IN ('local', 'lan', 'cloud')),
    state                    TEXT NOT NULL CHECK(state IN (
                                 'queued', 'preparing', 'generating', 'committing',
                                 'succeeded', 'failed', 'interrupted', 'canceled'
                             )),
    total_units              INTEGER NOT NULL DEFAULT 0 CHECK(total_units >= 0),
    completed_units          INTEGER NOT NULL DEFAULT 0 CHECK(completed_units >= 0),
    safe_error_code          TEXT NOT NULL DEFAULT '' CHECK(length(safe_error_code) <= 80),
    safe_error_message       TEXT NOT NULL DEFAULT '' CHECK(length(safe_error_message) <= 1000),
    api_token_count          INTEGER CHECK(api_token_count IS NULL OR api_token_count >= 0),
    api_token_usage_complete INTEGER NOT NULL DEFAULT 0 CHECK(api_token_usage_complete IN (0, 1)),
    created_at               INTEGER NOT NULL,
    started_at               INTEGER,
    updated_at               INTEGER NOT NULL,
    completed_at             INTEGER,
    CHECK(completed_units <= total_units)
) STRICT;

CREATE TABLE document_artifact_metadata (
    document_id               TEXT PRIMARY KEY REFERENCES documents(id) ON DELETE CASCADE,
    deck_id                   TEXT REFERENCES decks(id) ON DELETE SET NULL,
    source_fingerprint_sha256 TEXT NOT NULL CHECK(length(source_fingerprint_sha256) = 64),
    metadata_origin           TEXT NOT NULL
                              CHECK(metadata_origin IN ('provider', 'deterministic_fallback')),
    generator_version         INTEGER NOT NULL CHECK(generator_version >= 1),
    schema_version            INTEGER NOT NULL CHECK(schema_version >= 1),
    prompt_version            INTEGER NOT NULL CHECK(prompt_version >= 1),
    document_title            TEXT NOT NULL
                              CHECK(length(trim(document_title)) BETWEEN 1 AND 120),
    deck_title                TEXT NOT NULL
                              CHECK(length(trim(deck_title)) BETWEEN 1 AND 120),
    deck_description          TEXT NOT NULL
                              CHECK(length(trim(deck_description)) BETWEEN 1 AND 800),
    applied_document_title    TEXT
                              CHECK(applied_document_title IS NULL OR
                                    length(trim(applied_document_title)) BETWEEN 1 AND 120),
    applied_deck_title        TEXT
                              CHECK(applied_deck_title IS NULL OR
                                    length(trim(applied_deck_title)) BETWEEN 1 AND 120),
    applied_deck_description  TEXT
                              CHECK(applied_deck_description IS NULL OR
                                    length(trim(applied_deck_description)) BETWEEN 1 AND 800),
    created_at                INTEGER NOT NULL,
    updated_at                INTEGER NOT NULL,
    CHECK(updated_at >= created_at)
) STRICT;

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

CREATE TABLE document_collection_operation_receipts (
    operation_id TEXT PRIMARY KEY CHECK (length(operation_id) = 32),
    request_sha256 TEXT NOT NULL CHECK (length(request_sha256) = 64),
    result_json TEXT NOT NULL CHECK (json_valid(result_json)),
    created_at INTEGER NOT NULL
) STRICT;

CREATE TABLE document_collection_organization (
    singleton_id INTEGER PRIMARY KEY CHECK (singleton_id = 1),
    revision INTEGER NOT NULL CHECK (revision >= 1),
    updated_at INTEGER NOT NULL
) STRICT;

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

CREATE TABLE document_collections (
    id TEXT PRIMARY KEY CHECK (length(id) = 32),
    parent_id TEXT REFERENCES document_collections(id) ON DELETE RESTRICT,
    name TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 120),
    sort_order INTEGER NOT NULL CHECK (sort_order >= 0),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    revision INTEGER NOT NULL CHECK (revision >= 1)
) STRICT;

CREATE TABLE document_decks (
    document_id     TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    deck_id         TEXT NOT NULL REFERENCES decks(id) ON DELETE CASCADE,
    job_id          TEXT NOT NULL REFERENCES document_generation_jobs(id) ON DELETE CASCADE,
    created_at      INTEGER NOT NULL,
    PRIMARY KEY(document_id, deck_id)
) STRICT;

CREATE TABLE document_evidence_links (
    occurrence_id TEXT NOT NULL REFERENCES document_evidence_occurrences(id) ON DELETE CASCADE,
    target_node INTEGER NOT NULL CHECK(target_node >= 0 AND target_node < 1000000),
    kind TEXT NOT NULL CHECK(kind IN ('footnote', 'citation', 'internal_link', 'caption', 'label', 'contents', 'cross_reference')),
    target_occurrence_id TEXT REFERENCES document_evidence_occurrences(id) ON DELETE SET NULL,
    PRIMARY KEY(occurrence_id, target_node, kind)
) STRICT;

CREATE TABLE document_evidence_occurrences (
    id TEXT PRIMARY KEY CHECK(length(id) = 32),
    document_id TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    block_index INTEGER CHECK(block_index IS NULL OR block_index >= 0),
    source_file_sha256 TEXT NOT NULL CHECK(length(source_file_sha256) = 64),
    source_unit_number INTEGER NOT NULL CHECK(source_unit_number >= 1),
    payload_sha256 TEXT NOT NULL REFERENCES document_evidence_payloads(sha256),
    location_json TEXT NOT NULL CHECK(json_valid(location_json)),
    identity_state TEXT NOT NULL CHECK(identity_state IN ('exact', 'ambiguous')),
    UNIQUE(document_id, block_index)
) STRICT;

CREATE TABLE document_evidence_payloads (
    sha256 TEXT PRIMARY KEY CHECK(length(sha256) = 64),
    payload_json TEXT NOT NULL CHECK(json_valid(payload_json))
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
    updated_at        INTEGER NOT NULL, source_locator_json TEXT NOT NULL DEFAULT '{}',
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
    completed_at                 INTEGER, source_unit_count INTEGER NOT NULL DEFAULT 0
CHECK (source_unit_count >= 0), processed_source_unit_count INTEGER NOT NULL DEFAULT 0
CHECK (processed_source_unit_count >= 0),
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
    updated_at            INTEGER NOT NULL, source_locator_json TEXT NOT NULL DEFAULT '{}', visual_asset_ids_json TEXT NOT NULL DEFAULT '[]',
    PRIMARY KEY(job_id, window_index)
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

CREATE TABLE document_image_extraction_owners (
    member_id TEXT NOT NULL REFERENCES document_image_members(id) ON DELETE CASCADE,
    extraction_key TEXT NOT NULL REFERENCES document_image_extractions(extraction_key) ON DELETE RESTRICT,
    PRIMARY KEY (member_id, extraction_key)
) STRICT;

CREATE TABLE document_image_extractions (
    extraction_key TEXT PRIMARY KEY NOT NULL CHECK(length(extraction_key) = 64),
    version INTEGER NOT NULL CHECK(version = 1),
    pixel_sha256 TEXT NOT NULL CHECK(length(pixel_sha256) = 64),
    pixel_contract TEXT NOT NULL CHECK(length(pixel_contract) BETWEEN 1 AND 128),
    quarter_turns INTEGER NOT NULL CHECK(quarter_turns BETWEEN 0 AND 3),
    engine_contract TEXT NOT NULL CHECK(length(engine_contract) BETWEEN 1 AND 512),
    result_json TEXT NOT NULL CHECK(length(CAST(result_json AS BLOB)) BETWEEN 1 AND 2097152),
    result_sha256 TEXT NOT NULL CHECK(length(result_sha256) = 64)
) STRICT;

CREATE TABLE document_image_imports (
    operation_id TEXT PRIMARY KEY NOT NULL CHECK (length(operation_id) = 32),
    request_sha256 TEXT NOT NULL CHECK (length(request_sha256) = 64),
    document_id TEXT REFERENCES documents(id) ON DELETE SET NULL,
    created_at INTEGER NOT NULL,
    UNIQUE (document_id)
) STRICT;

CREATE TABLE "document_image_members" (
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

CREATE TABLE document_image_overrides (
    document_id TEXT NOT NULL REFERENCES document_image_sets(document_id) ON DELETE CASCADE,
    member_id TEXT NOT NULL REFERENCES document_image_members(id) ON DELETE CASCADE,
    region_id TEXT NOT NULL CHECK (length(region_id) = 64),
    action TEXT NOT NULL CHECK (action IN ('include', 'exclude', 'metadata')),
    PRIMARY KEY (document_id, region_id)
) STRICT;

CREATE TABLE document_image_projection_writes (
    document_id TEXT PRIMARY KEY NOT NULL REFERENCES document_image_sets(document_id) ON DELETE CASCADE
) STRICT;

CREATE TABLE document_image_receipts (
    document_id TEXT PRIMARY KEY NOT NULL REFERENCES document_image_sets(document_id) ON DELETE CASCADE,
    manifest_sha256 TEXT NOT NULL CHECK (length(manifest_sha256) = 64),
    filter_revision INTEGER NOT NULL CHECK (filter_revision >= 1),
    version INTEGER NOT NULL CHECK (version = 1),
    payload_json TEXT NOT NULL CHECK (length(CAST(payload_json AS BLOB)) BETWEEN 1 AND 16777216),
    payload_sha256 TEXT NOT NULL CHECK (length(payload_sha256) = 64)
) STRICT;

CREATE TABLE document_image_sets (
    document_id TEXT PRIMARY KEY NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    manifest_version INTEGER NOT NULL CHECK (manifest_version = 1),
    revision INTEGER NOT NULL CHECK (revision >= 1),
    manifest_sha256 TEXT NOT NULL CHECK (length(manifest_sha256) = 64),
    prepared_manifest_sha256 TEXT NOT NULL DEFAULT ''
        CHECK (prepared_manifest_sha256 = '' OR length(prepared_manifest_sha256) = 64)
) STRICT;

CREATE TABLE document_image_settings (
    document_id TEXT PRIMARY KEY NOT NULL REFERENCES document_image_sets(document_id) ON DELETE CASCADE,
    revision INTEGER NOT NULL CHECK (revision >= 1),
    include_interface INTEGER NOT NULL CHECK (include_interface IN (0, 1))
) STRICT;

CREATE TABLE "document_learning_plan_topics" (
    plan_id                     TEXT NOT NULL REFERENCES document_learning_plans(id) ON DELETE CASCADE,
    topic_id                    TEXT NOT NULL REFERENCES document_learning_topics(id) ON DELETE CASCADE,
    parent_topic_id             TEXT REFERENCES document_learning_topics(id) ON DELETE SET NULL,
    sort_index                  INTEGER NOT NULL CHECK(sort_index >= 0),
    title                       TEXT NOT NULL CHECK(length(trim(title)) BETWEEN 1 AND 500),
    summary_text                TEXT NOT NULL CHECK(length(summary_text) <= 100000),
    learning_objectives_json    TEXT NOT NULL CHECK(length(learning_objectives_json) <= 131072),
    key_concepts_json           TEXT NOT NULL CHECK(length(key_concepts_json) <= 131072),
    source_locator_json         TEXT NOT NULL CHECK(length(source_locator_json) <= 65536),
    evidence_chunk_indices_json TEXT NOT NULL CHECK(length(evidence_chunk_indices_json) <= 131072),
    evidence_hashes_json        TEXT NOT NULL CHECK(length(evidence_hashes_json) <= 262144),
    evidence_hash               TEXT NOT NULL CHECK(length(evidence_hash) = 64),
    created_at                  INTEGER NOT NULL,
    PRIMARY KEY(plan_id, topic_id),
    UNIQUE(plan_id, sort_index)
) STRICT;

CREATE TABLE document_learning_plans (
    id                       TEXT PRIMARY KEY CHECK(length(id) = 32),
    document_id              TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    revision                 INTEGER NOT NULL CHECK(revision >= 1),
    compiler_version         INTEGER NOT NULL CHECK(compiler_version >= 1),
    outline_fingerprint      TEXT NOT NULL CHECK(length(outline_fingerprint) = 64),
    plan_fingerprint         TEXT NOT NULL CHECK(length(plan_fingerprint) = 64),
    summary_text             TEXT NOT NULL CHECK(length(summary_text) <= 100000),
    learning_objectives_json TEXT NOT NULL CHECK(length(learning_objectives_json) <= 262144),
    key_concepts_json        TEXT NOT NULL CHECK(length(key_concepts_json) <= 262144),
    state                    TEXT NOT NULL CHECK(state IN ('active', 'superseded')),
    created_at               INTEGER NOT NULL,
    updated_at               INTEGER NOT NULL,
    UNIQUE(document_id, revision),
    UNIQUE(document_id, plan_fingerprint)
) STRICT;

CREATE TABLE document_learning_topics (
    id          TEXT PRIMARY KEY CHECK(length(id) = 32),
    document_id TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    anchor_hash TEXT NOT NULL CHECK(length(anchor_hash) = 64),
    created_at  INTEGER NOT NULL,
    UNIQUE(document_id, anchor_hash)
) STRICT;

CREATE TABLE document_note_topics (
    document_id         TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    note_id             TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    topic_id            TEXT NOT NULL REFERENCES document_learning_topics(id) ON DELETE CASCADE,
    relationship_kind   TEXT NOT NULL CHECK(relationship_kind IN ('primary', 'supporting')),
    source_locator_json TEXT NOT NULL CHECK(length(source_locator_json) <= 65536),
    evidence_hash       TEXT NOT NULL CHECK(length(evidence_hash) = 64),
    generation_job_id   TEXT REFERENCES document_generation_jobs(id) ON DELETE SET NULL,
    created_at          INTEGER NOT NULL,
    PRIMARY KEY(note_id, topic_id)
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
    updated_at        INTEGER NOT NULL, source_locator_json TEXT NOT NULL DEFAULT '{}',
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

CREATE TABLE document_study_guide_checkpoints (
    document_id   TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    contract_key  TEXT NOT NULL CHECK(length(contract_key) = 64),
    topic_id      TEXT NOT NULL REFERENCES document_learning_topics(id) ON DELETE CASCADE,
    evidence_hash TEXT NOT NULL CHECK(length(evidence_hash) = 64),
    section_json  TEXT NOT NULL CHECK(length(section_json) <= 2097152),
    content_hash  TEXT NOT NULL CHECK(length(content_hash) = 64),
    created_at    INTEGER NOT NULL,
    updated_at    INTEGER NOT NULL,
    PRIMARY KEY(document_id, contract_key, topic_id)
) STRICT;

CREATE TABLE document_study_guide_overlays (
    id                TEXT PRIMARY KEY CHECK(length(id) = 32),
    document_id       TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    guide_id          TEXT REFERENCES document_study_guides(id) ON DELETE CASCADE,
    topic_id          TEXT REFERENCES document_learning_topics(id) ON DELETE CASCADE,
    overlay_kind      TEXT NOT NULL CHECK(overlay_kind IN (
                          'guide_note', 'topic_note', 'title_override',
                          'body_override', 'highlight'
                      )),
    base_content_hash TEXT CHECK(base_content_hash IS NULL OR length(base_content_hash) = 64),
    rich_doc_json     TEXT NOT NULL CHECK(length(rich_doc_json) <= 2097152),
    plain_text        TEXT NOT NULL CHECK(length(plain_text) <= 500000),
    state             TEXT NOT NULL CHECK(state IN ('active', 'conflicted', 'discarded')),
    revision          INTEGER NOT NULL DEFAULT 1 CHECK(revision >= 1),
    created_at        INTEGER NOT NULL,
    updated_at        INTEGER NOT NULL,
    CHECK(
        (overlay_kind = 'guide_note' AND topic_id IS NULL)
        OR
        (overlay_kind <> 'guide_note' AND topic_id IS NOT NULL)
    )
) STRICT;

CREATE TABLE document_study_guide_sections (
    guide_id            TEXT NOT NULL REFERENCES document_study_guides(id) ON DELETE CASCADE,
    topic_id            TEXT NOT NULL REFERENCES document_learning_topics(id) ON DELETE CASCADE,
    sort_index          INTEGER NOT NULL CHECK(sort_index >= 0),
    content_json        TEXT NOT NULL CHECK(length(content_json) <= 2097152),
    content_text        TEXT NOT NULL CHECK(length(content_text) <= 500000),
    content_hash        TEXT NOT NULL CHECK(length(content_hash) = 64),
    source_locator_json TEXT NOT NULL CHECK(length(source_locator_json) <= 65536),
    created_at          INTEGER NOT NULL,
    updated_at          INTEGER NOT NULL,
    PRIMARY KEY(guide_id, topic_id),
    UNIQUE(guide_id, sort_index)
) STRICT;

CREATE TABLE document_study_guides (
    id                       TEXT PRIMARY KEY CHECK(length(id) = 32),
    document_id              TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    plan_id                  TEXT NOT NULL REFERENCES document_learning_plans(id) ON DELETE CASCADE,
    job_id                   TEXT REFERENCES document_artifact_jobs(id) ON DELETE SET NULL,
    revision                 INTEGER NOT NULL CHECK(revision >= 1),
    schema_version           INTEGER NOT NULL CHECK(schema_version >= 1),
    prompt_version           INTEGER NOT NULL CHECK(prompt_version >= 1),
    assembly_version         INTEGER NOT NULL CHECK(assembly_version >= 1),
    overview_json            TEXT NOT NULL CHECK(length(overview_json) <= 1048576),
    overview_text            TEXT NOT NULL CHECK(length(overview_text) <= 200000),
    state                    TEXT NOT NULL CHECK(state IN ('active', 'superseded')),
    created_at               INTEGER NOT NULL,
    updated_at               INTEGER NOT NULL,
    published_at             INTEGER NOT NULL,
    UNIQUE(document_id, revision)
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
    CHECK(api_token_usage_complete IN (0, 1)), canonical_schema_version INTEGER NOT NULL DEFAULT 1
CHECK (canonical_schema_version >= 1), adapter_backend TEXT NOT NULL DEFAULT 'legacy-pdf-adapter-v0'
CHECK (length(adapter_backend) BETWEEN 1 AND 160), source_file_count INTEGER NOT NULL DEFAULT 1
CHECK (source_file_count >= 1), source_unit_count INTEGER NOT NULL DEFAULT 0
CHECK (source_unit_count >= 0), extracted_source_unit_count INTEGER NOT NULL DEFAULT 0
CHECK (extracted_source_unit_count >= 0), visualized_source_unit_count INTEGER NOT NULL DEFAULT 0
CHECK (visualized_source_unit_count >= 0), source_asset_count INTEGER NOT NULL DEFAULT 0
CHECK (source_asset_count >= 0), source_media_type TEXT NOT NULL DEFAULT 'application/pdf'
CHECK (length(source_media_type) BETWEEN 3 AND 160),
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

CREATE TABLE generation_operations (
    id TEXT PRIMARY KEY CHECK(length(id) = 32),
    purpose TEXT NOT NULL CHECK(length(purpose) BETWEEN 1 AND 80),
    document_id TEXT REFERENCES documents(id) ON DELETE CASCADE,
    collection_id TEXT REFERENCES document_collections(id) ON DELETE CASCADE,
    started_at INTEGER NOT NULL,
    finished_at INTEGER,
    state TEXT NOT NULL CHECK(state IN ('running','complete','failed','interrupted')), outcome_json TEXT
    CHECK (outcome_json IS NULL OR
        (length(outcome_json) <= 1024 AND json_valid(outcome_json)
         AND json_type(outcome_json) = 'object')),
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

CREATE TABLE generation_reuse_events (
    id INTEGER PRIMARY KEY,
    operation_id TEXT NOT NULL REFERENCES generation_operations(id) ON DELETE CASCADE,
    stage TEXT NOT NULL CHECK(length(stage) BETWEEN 1 AND 80),
    cache_key TEXT NOT NULL CHECK(length(cache_key) = 64),
    created_at INTEGER NOT NULL
) STRICT;

CREATE TABLE knowledge_analysis_sources (
    window_id TEXT NOT NULL REFERENCES knowledge_analysis_windows(id) ON DELETE CASCADE,
    source_id TEXT NOT NULL REFERENCES knowledge_sources(id) ON DELETE CASCADE,
    source_hash TEXT NOT NULL CHECK(length(source_hash) = 64),
    PRIMARY KEY(window_id, source_id)
) STRICT;

CREATE TABLE knowledge_analysis_windows (
    id TEXT PRIMARY KEY CHECK(length(id) = 32),
    document_id TEXT REFERENCES documents(id) ON DELETE CASCADE,
    contract TEXT NOT NULL CHECK(length(contract) = 64),
    state TEXT NOT NULL CHECK(state IN ('complete', 'partial')),
    accepted_count INTEGER NOT NULL CHECK(accepted_count >= 0),
    rejected_count INTEGER NOT NULL CHECK(rejected_count >= 0),
    report_json TEXT NOT NULL CHECK(json_valid(report_json)),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
) STRICT;

CREATE TABLE knowledge_artifact_links (
    source_id TEXT NOT NULL REFERENCES knowledge_sources(id) ON DELETE CASCADE,
    unit_id TEXT NOT NULL REFERENCES knowledge_units(id) ON DELETE CASCADE,
    source_hash TEXT NOT NULL CHECK(length(source_hash) = 64),
    relationship TEXT NOT NULL CHECK(relationship IN ('assesses', 'explains', 'mentions')),
    created_at INTEGER NOT NULL,
    PRIMARY KEY(source_id, unit_id)
) STRICT;

CREATE TABLE knowledge_cache_evictions (
    source_id TEXT PRIMARY KEY CHECK(length(source_id) = 32)
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
, reduction_request_key TEXT
CHECK(reduction_request_key IS NULL OR length(reduction_request_key) = 64)) STRICT;

CREATE TABLE knowledge_document_views (
    document_id TEXT PRIMARY KEY REFERENCES documents(id) ON DELETE CASCADE,
    analysis_hash TEXT NOT NULL CHECK(length(analysis_hash) = 64),
    contract TEXT NOT NULL CHECK(length(contract) = 64),
    outline_json TEXT NOT NULL CHECK(json_valid(outline_json)),
    core_unit_ids_json TEXT NOT NULL CHECK(json_valid(core_unit_ids_json)),
    reduced INTEGER NOT NULL CHECK(reduced IN (0, 1)),
    created_at INTEGER NOT NULL
);

CREATE TABLE knowledge_preparations (
    operation_id TEXT PRIMARY KEY CHECK(length(operation_id) = 32),
    scope_key TEXT NOT NULL CHECK(length(scope_key) = 64),
    scope_json TEXT NOT NULL CHECK(json_valid(scope_json) AND length(scope_json) <= 1024),
    consent_hash TEXT NOT NULL CHECK(length(consent_hash) = 64),
    embedding_space TEXT NOT NULL CHECK(length(embedding_space) = 64),
    extraction_contract TEXT NOT NULL CHECK(length(extraction_contract) = 64),
    state TEXT NOT NULL CHECK(state IN ('running', 'complete', 'failed', 'interrupted', 'canceled')),
    total_sources INTEGER NOT NULL CHECK(total_sources BETWEEN 0 AND 100000),
    completed_sources INTEGER NOT NULL CHECK(completed_sources >= 0 AND completed_sources <= total_sources),
    safe_error_code TEXT NOT NULL DEFAULT '' CHECK(length(safe_error_code) <= 80),
    safe_error_message TEXT NOT NULL DEFAULT '' CHECK(length(safe_error_message) <= 1000),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
) STRICT;

CREATE TABLE knowledge_sources (
    id TEXT PRIMARY KEY CHECK(length(id) = 32),
    kind TEXT NOT NULL CHECK(kind IN ('document', 'card', 'guide', 'conversation')),
    owner_id TEXT NOT NULL CHECK(length(owner_id) = 32),
    segment_key TEXT NOT NULL CHECK(length(segment_key) BETWEEN 1 AND 160),
    parent_key TEXT NOT NULL CHECK(length(parent_key) BETWEEN 1 AND 160),
    owner_revision TEXT NOT NULL CHECK(length(owner_revision) BETWEEN 1 AND 160),
    content_hash TEXT NOT NULL CHECK(length(content_hash) = 64),
    record_json TEXT NOT NULL CHECK(json_valid(record_json) AND length(record_json) <= 262144),
    active INTEGER NOT NULL DEFAULT 1 CHECK(active IN (0, 1)),
    document_id TEXT REFERENCES documents(id) ON DELETE CASCADE,
    note_id TEXT REFERENCES notes(id) ON DELETE CASCADE,
    card_id TEXT REFERENCES cards(id) ON DELETE CASCADE,
    guide_id TEXT REFERENCES document_study_guides(id) ON DELETE CASCADE,
    conversation_id TEXT REFERENCES tutor_conversations(id) ON DELETE CASCADE,
    turn_id TEXT REFERENCES tutor_turns(id) ON DELETE CASCADE,
    extraction_contract TEXT NOT NULL DEFAULT '' CHECK(length(extraction_contract) IN (0, 64)),
    extracted_hash TEXT NOT NULL DEFAULT '' CHECK(length(extracted_hash) IN (0, 64)),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    UNIQUE(kind, owner_id, segment_key),
    CHECK((kind = 'document' AND document_id IS NOT NULL AND document_id = owner_id
              AND note_id IS NULL AND card_id IS NULL AND guide_id IS NULL
              AND conversation_id IS NULL AND turn_id IS NULL)
       OR (kind = 'card' AND card_id IS NOT NULL AND card_id = owner_id AND note_id IS NOT NULL
              AND guide_id IS NULL AND conversation_id IS NULL AND turn_id IS NULL)
       OR (kind = 'guide' AND guide_id IS NOT NULL AND guide_id = owner_id AND document_id IS NOT NULL
              AND note_id IS NULL AND card_id IS NULL AND conversation_id IS NULL AND turn_id IS NULL)
       OR (kind = 'conversation' AND conversation_id IS NOT NULL AND conversation_id = owner_id
              AND turn_id IS NOT NULL AND document_id IS NULL AND note_id IS NULL
              AND card_id IS NULL AND guide_id IS NULL))
) STRICT;

CREATE TABLE knowledge_unit_evidence (
    unit_id TEXT NOT NULL REFERENCES knowledge_units(id) ON DELETE CASCADE,
    evidence_id TEXT NOT NULL CHECK(length(evidence_id) = 32),
    source_id TEXT NOT NULL REFERENCES knowledge_sources(id) ON DELETE CASCADE,
    source_hash TEXT NOT NULL CHECK(length(source_hash) = 64),
    PRIMARY KEY(unit_id, evidence_id)
) STRICT;

CREATE TABLE knowledge_units (
    id TEXT PRIMARY KEY CHECK(length(id) = 32),
    source_id TEXT NOT NULL REFERENCES knowledge_sources(id) ON DELETE CASCADE,
    source_hash TEXT NOT NULL CHECK(length(source_hash) = 64),
    content_hash TEXT NOT NULL CHECK(length(content_hash) = 64),
    record_json TEXT NOT NULL CHECK(json_valid(record_json) AND length(record_json) <= 65536),
    extraction_contract TEXT NOT NULL CHECK(length(extraction_contract) = 64),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    UNIQUE(source_id, content_hash)
) STRICT;

CREATE TABLE library_organization (
    singleton_id    INTEGER PRIMARY KEY NOT NULL CHECK(singleton_id = 1),
    revision        INTEGER NOT NULL CHECK(revision >= 1),
    updated_at      INTEGER NOT NULL
) STRICT;

CREATE TABLE "note_assets" (
    note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    asset_id TEXT NOT NULL REFERENCES assets(id) ON DELETE RESTRICT,
    side TEXT NOT NULL CHECK(side IN ('front', 'back', 'content')),
    PRIMARY KEY(note_id, asset_id, side)
) STRICT;

CREATE TABLE note_source_citations (
    note_id             TEXT PRIMARY KEY REFERENCES notes(id) ON DELETE CASCADE,
    citation_id         TEXT NOT NULL
                        REFERENCES source_citation_snapshots(id) ON DELETE RESTRICT,
    source_locator_json TEXT
                        CHECK(
                            source_locator_json IS NULL OR
                            (
                                length(source_locator_json) <= 65536 AND
                                json_valid(source_locator_json)
                            )
                        ),
    created_at          INTEGER NOT NULL,
    updated_at          INTEGER NOT NULL,
    CHECK(updated_at >= created_at)
) STRICT;

CREATE TABLE note_tags (
    note_id  TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    tag_id   TEXT NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    PRIMARY KEY(note_id, tag_id)
) STRICT;

CREATE TABLE "notes" (
    id TEXT PRIMARY KEY CHECK(length(id) = 32),
    deck_id TEXT NOT NULL REFERENCES decks(id) ON DELETE CASCADE,
    note_type TEXT NOT NULL DEFAULT 'basic'
        CHECK(note_type IN ('basic', 'cloze', 'interactive')),
    content_format TEXT NOT NULL DEFAULT 'tiptap-json'
        CHECK(content_format IN ('tiptap-json', 'question-json')),
    content_schema_version INTEGER NOT NULL DEFAULT 1 CHECK(content_schema_version >= 1),
    front_doc TEXT,
    back_doc TEXT,
    front_text TEXT NOT NULL,
    back_text TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    deleted_at INTEGER,
    revision INTEGER NOT NULL DEFAULT 1 CHECK(revision >= 1),
    duplicate_hash TEXT NOT NULL DEFAULT '',
    reverse_enabled INTEGER NOT NULL DEFAULT 0 CHECK(reverse_enabled IN (0, 1)),
    cloze_source TEXT NOT NULL DEFAULT '',
    cloze_mode TEXT NOT NULL DEFAULT 'one_at_a_time'
        CHECK(cloze_mode IN ('one_at_a_time', 'all_together')),
    question_definition TEXT,
    CHECK(
        (note_type IN ('basic', 'cloze') AND content_format = 'tiptap-json'
            AND front_doc IS NOT NULL AND back_doc IS NOT NULL
            AND question_definition IS NULL)
        OR
        (note_type = 'interactive' AND content_format = 'question-json'
            AND content_schema_version = 1
            AND front_doc IS NULL AND back_doc IS NULL
            AND question_definition IS NOT NULL
            AND length(CAST(question_definition AS BLOB)) <= 1000000
            AND json_valid(question_definition)
            AND json_type(question_definition) IS 'object'
            AND json_extract(question_definition, '$.schemaVersion') IS 1
            AND reverse_enabled = 0 AND cloze_source = '')
    )
) STRICT;

CREATE TABLE "practice_session_card_snapshots" (
    session_id TEXT NOT NULL REFERENCES practice_sessions(id) ON DELETE CASCADE,
    card_id TEXT NOT NULL CHECK(length(card_id) = 32),
    note_id TEXT NOT NULL CHECK(length(note_id) = 32),
    note_revision INTEGER NOT NULL CHECK(note_revision >= 1),
    owner_deck_id_at_capture TEXT NOT NULL CHECK(length(owner_deck_id_at_capture) = 32),
    captured_deck_name TEXT NOT NULL,
    direction TEXT NOT NULL CHECK(direction IN ('forward', 'reverse')),
    card_type TEXT NOT NULL CHECK(card_type IN ('basic', 'cloze', 'single_choice', 'select_all',
        'true_false', 'matching', 'grouping', 'ordering', 'image_occlusion')),
    card_label TEXT NOT NULL,
    presentation_variant TEXT NOT NULL,
    front_doc_json TEXT,
    back_doc_json TEXT,
    front_text TEXT NOT NULL,
    back_text TEXT NOT NULL,
    question_excerpt TEXT NOT NULL,
    question_text_hash TEXT NOT NULL CHECK(length(question_text_hash) = 64),
    question_was_truncated INTEGER NOT NULL CHECK(question_was_truncated IN (0, 1)),
    captured_at INTEGER NOT NULL,
    question_instance_id TEXT REFERENCES question_attempts(instance_id) ON DELETE CASCADE,
    PRIMARY KEY(session_id, card_id),
    CHECK((card_type IN ('basic', 'cloze') AND front_doc_json IS NOT NULL
            AND back_doc_json IS NOT NULL AND question_instance_id IS NULL)
        OR (card_type NOT IN ('basic', 'cloze') AND front_doc_json IS NULL
            AND back_doc_json IS NULL AND question_instance_id IS NOT NULL))
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

CREATE TABLE "prepared_exports" (
    id                  TEXT PRIMARY KEY CHECK(length(id) = 32),
    operation_id        TEXT NOT NULL UNIQUE CHECK(length(operation_id) = 32),
    purpose             TEXT NOT NULL CHECK(purpose IN (
                            'studydeck', 'library_backup', 'study_guide_pdf'
                        )),
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

CREATE TABLE question_attempt_assets (
    instance_id TEXT NOT NULL REFERENCES question_attempts(instance_id) ON DELETE CASCADE,
    asset_id TEXT NOT NULL REFERENCES assets(id) ON DELETE RESTRICT,
    PRIMARY KEY(instance_id, asset_id)
) STRICT;

CREATE TABLE question_attempts (
    instance_id TEXT PRIMARY KEY CHECK(length(instance_id) = 32),
    session_id TEXT REFERENCES practice_sessions(id) ON DELETE SET NULL,
    card_id TEXT NOT NULL REFERENCES cards(id) ON DELETE CASCADE,
    note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    question_kind TEXT NOT NULL CHECK(question_kind IN (
        'single_choice', 'select_all', 'true_false', 'matching',
        'grouping', 'ordering', 'image_occlusion')),
    instance_hash TEXT NOT NULL CHECK(length(instance_hash) = 64),
    revision INTEGER NOT NULL CHECK(revision >= 1),
    phase TEXT NOT NULL CHECK(phase IN ('answering', 'checked', 'revealed')),
    assisted INTEGER NOT NULL CHECK(assisted IN (0, 1)),
    checkpoint_json TEXT NOT NULL CHECK(json_valid(checkpoint_json)
        AND length(CAST(checkpoint_json AS BLOB)) <= 4000000),
    first_checked_at INTEGER CHECK(first_checked_at IS NULL OR first_checked_at >= 0),
    verdict TEXT CHECK(verdict IS NULL OR verdict IN ('correct', 'incorrect', 'incomplete')),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    UNIQUE(session_id, card_id),
    CHECK(json_extract(checkpoint_json, '$.revision') IS revision),
    CHECK(json_extract(checkpoint_json, '$.assisted') IS assisted),
    CHECK(json_extract(checkpoint_json, '$.state.phase') IS phase),
    CHECK((phase = 'answering' AND first_checked_at IS NULL AND verdict IS NULL)
        OR (phase = 'checked' AND first_checked_at IS NOT NULL AND verdict IS NOT NULL)
        OR (phase = 'revealed' AND first_checked_at IS NOT NULL AND verdict IS NULL))
) STRICT;

CREATE TABLE question_generation_evidence (
    note_id TEXT PRIMARY KEY NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    note_revision INTEGER NOT NULL CHECK (note_revision > 0),
    document_id TEXT REFERENCES documents(id) ON DELETE SET NULL,
    question_hash TEXT NOT NULL CHECK (length(question_hash) = 64),
    opportunity_id TEXT NOT NULL CHECK (length(opportunity_id) = 64),
    policy_hash TEXT NOT NULL CHECK (length(policy_hash) = 64),
    record_json TEXT NOT NULL CHECK (
        json_valid(record_json) AND length(record_json) <= 262144
        AND json_extract(record_json, '$.schemaVersion') IS 1
    ),
    created_at INTEGER NOT NULL CHECK (created_at >= 0)
) STRICT;

CREATE TABLE question_generation_reports (
    job_id TEXT PRIMARY KEY REFERENCES document_generation_jobs(id) ON DELETE CASCADE,
    document_id TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    policy_json TEXT NOT NULL CHECK (
        json_valid(policy_json) AND json_type(policy_json) = 'object'
        AND length(CAST(policy_json AS BLOB)) <= 16384
    ),
    available_knowledge_units INTEGER NOT NULL CHECK (available_knowledge_units BETWEEN 0 AND 10000000),
    candidate_count INTEGER CHECK (candidate_count BETWEEN 0 AND 10000),
    report_json TEXT CHECK (
        report_json IS NULL OR (
            json_valid(report_json) AND json_extract(report_json, '$.schemaVersion') IS 1
            AND length(CAST(report_json AS BLOB)) <= 65536
        )
    ),
    created_at INTEGER NOT NULL CHECK (created_at >= 0)
) STRICT;

CREATE TABLE question_generation_settings (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    revision INTEGER NOT NULL CHECK (revision > 0),
    policy_json TEXT NOT NULL CHECK (json_valid(policy_json) AND length(policy_json) <= 16384 AND json_type(policy_json) = 'object')
) STRICT;

CREATE TABLE question_note_writes (
    operation_id TEXT PRIMARY KEY CHECK(length(operation_id) = 32),
    request_hash TEXT NOT NULL CHECK(length(request_hash) = 64),
    note_id TEXT NOT NULL CHECK(length(note_id) = 32),
    note_revision INTEGER NOT NULL CHECK(note_revision >= 1),
    card_ids_json TEXT NOT NULL CHECK(json_valid(card_ids_json)
        AND json_type(card_ids_json) IS 'array' AND length(card_ids_json) <= 4096),
    committed_at INTEGER NOT NULL
) STRICT;

CREATE TABLE question_remediation_attempts (
    operation_id TEXT PRIMARY KEY CHECK(length(operation_id) = 32),
    instance_id TEXT NOT NULL REFERENCES question_attempts(instance_id) ON DELETE CASCADE,
    request_hash TEXT NOT NULL CHECK(length(request_hash) = 64),
    answer_json TEXT NOT NULL CHECK(json_valid(answer_json)
        AND length(CAST(answer_json AS BLOB)) <= 16384),
    evaluation_json TEXT NOT NULL CHECK(json_valid(evaluation_json)
        AND length(CAST(evaluation_json AS BLOB)) <= 2000000),
    created_at INTEGER NOT NULL
) STRICT;

CREATE TABLE question_review_links (
    review_event_id TEXT PRIMARY KEY REFERENCES review_events(id) ON DELETE CASCADE,
    instance_id TEXT NOT NULL UNIQUE REFERENCES question_attempts(instance_id) ON DELETE CASCADE,
    override_reason TEXT CHECK(override_reason IS NULL OR length(trim(override_reason)) BETWEEN 1 AND 2000)
) STRICT;

CREATE TABLE "review_event_attributions" (
    review_event_id TEXT PRIMARY KEY REFERENCES review_events(id) ON DELETE CASCADE,
    reviewed_at INTEGER NOT NULL,
    review_local_date TEXT NOT NULL
        CHECK(review_local_date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
    time_zone_id TEXT NOT NULL,
    utc_offset_minutes INTEGER NOT NULL CHECK(utc_offset_minutes BETWEEN -1440 AND 1440),
    card_id TEXT NOT NULL CHECK(length(card_id) = 32),
    note_id TEXT NOT NULL CHECK(length(note_id) = 32),
    note_revision INTEGER NOT NULL CHECK(note_revision >= 1),
    owner_deck_id_at_review TEXT NOT NULL CHECK(length(owner_deck_id_at_review) = 32),
    captured_deck_name TEXT NOT NULL,
    direction TEXT NOT NULL CHECK(direction IN ('forward', 'reverse')),
    card_type TEXT NOT NULL CHECK(card_type IN ('basic', 'cloze', 'single_choice', 'select_all',
        'true_false', 'matching', 'grouping', 'ordering', 'image_occlusion')),
    card_label TEXT NOT NULL,
    presentation_variant TEXT NOT NULL,
    question_excerpt TEXT NOT NULL,
    question_text_hash TEXT NOT NULL CHECK(length(question_text_hash) = 64),
    question_was_truncated INTEGER NOT NULL CHECK(question_was_truncated IN (0, 1)),
    attribution_source TEXT NOT NULL
        CHECK(attribution_source IN ('captured', 'legacy_current_state_backfill'))
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

CREATE TABLE source_citation_snapshots (
    id                        TEXT PRIMARY KEY CHECK(length(id) = 32),
    schema_version            INTEGER NOT NULL DEFAULT 1
                              CHECK(schema_version = 1),
    source_document_id        TEXT NOT NULL CHECK(length(source_document_id) = 32),
    source_identity           TEXT NOT NULL
                              CHECK(length(source_identity) BETWEEN 1 AND 200),
    source_fingerprint_sha256 TEXT NOT NULL
                              CHECK(length(source_fingerprint_sha256) = 64),
    source_title              TEXT NOT NULL
                              CHECK(length(trim(source_title)) BETWEEN 1 AND 200),
    original_filename         TEXT NOT NULL
                              CHECK(length(trim(original_filename)) BETWEEN 1 AND 255),
    source_format             TEXT NOT NULL
                              CHECK(length(source_format) BETWEEN 1 AND 40),
    source_media_type         TEXT NOT NULL
                              CHECK(length(source_media_type) BETWEEN 3 AND 160),
    source_unit_kind          TEXT NOT NULL
                              CHECK(length(source_unit_kind) BETWEEN 1 AND 40),
    source_unit_count         INTEGER NOT NULL CHECK(source_unit_count >= 0),
    source_removed            INTEGER NOT NULL DEFAULT 0
                              CHECK(source_removed IN (0, 1)),
    created_at                INTEGER NOT NULL,
    updated_at                INTEGER NOT NULL,
    CHECK(updated_at >= created_at)
) STRICT;

CREATE TABLE "staged_external_imports" (
    id              TEXT PRIMARY KEY CHECK(length(id) = 32),
    operation_id    TEXT NOT NULL UNIQUE CHECK(length(operation_id) = 32),
    purpose         TEXT NOT NULL CHECK(purpose IN (
                        'image', 'pdf_document', 'document',
                        'study_content', 'library_backup'
                    )),
    detected_kind   TEXT NOT NULL CHECK(detected_kind IN (
                        'png', 'jpeg', 'gif', 'webp', 'pdf',
                        'doc', 'docx', 'ppt', 'pptx', 'odt', 'odp',
                        'rtf', 'epub', 'excel', 'ods', 'csv',
                        'studydeck', 'anki_apkg', 'generated_json', 'ostudybackup'
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

CREATE TABLE study_target_artifact_jobs (
    batch_id           TEXT NOT NULL REFERENCES study_target_batches(operation_id) ON DELETE CASCADE,
    artifact_kind      TEXT NOT NULL CHECK(artifact_kind IN ('flashcards', 'study_guide')),
    state              TEXT NOT NULL CHECK(state IN (
                           'pending', 'running', 'committing', 'succeeded', 'failed',
                           'canceled', 'suppressed', 'interrupted'
                       )),
    artifact_id        TEXT,
    total_units        INTEGER NOT NULL DEFAULT 0 CHECK(total_units >= 0),
    completed_units    INTEGER NOT NULL DEFAULT 0 CHECK(completed_units >= 0),
    safe_error_code    TEXT NOT NULL DEFAULT '' CHECK(length(safe_error_code) <= 80),
    safe_error_message TEXT NOT NULL DEFAULT '' CHECK(length(safe_error_message) <= 1000),
    created_at         INTEGER NOT NULL,
    updated_at         INTEGER NOT NULL,
    completed_at       INTEGER,
    PRIMARY KEY(batch_id, artifact_kind),
    CHECK(completed_units <= total_units),
    CHECK(updated_at >= created_at)
) STRICT;

CREATE TABLE study_target_batches (
    operation_id         TEXT PRIMARY KEY CHECK(length(operation_id) = 32),
    target_id            TEXT NOT NULL REFERENCES study_targets(id) ON DELETE CASCADE,
    request_sha256       TEXT NOT NULL CHECK(length(request_sha256) = 64),
    source_snapshot_id   TEXT NOT NULL CHECK(length(source_snapshot_id) = 64),
    source_snapshot_json TEXT NOT NULL
                         CHECK(json_valid(source_snapshot_json)
                               AND json_type(source_snapshot_json) = 'object'
                               AND length(source_snapshot_json) <= 2097152),
    output_policy_json   TEXT NOT NULL
                         CHECK(json_valid(output_policy_json)
                               AND json_type(output_policy_json) = 'object'
                               AND length(output_policy_json) <= 16384),
    provider_authorized  INTEGER NOT NULL DEFAULT 0 CHECK(provider_authorized IN (0, 1)),
    state                TEXT NOT NULL CHECK(state IN (
                             'queued', 'preparing', 'composing', 'committing',
                             'complete', 'partial', 'failed', 'canceled', 'interrupted'
                         )),
    safe_error_code      TEXT NOT NULL DEFAULT '' CHECK(length(safe_error_code) <= 80),
    safe_error_message   TEXT NOT NULL DEFAULT '' CHECK(length(safe_error_message) <= 1000),
    created_at           INTEGER NOT NULL,
    updated_at           INTEGER NOT NULL,
    completed_at         INTEGER,
    CHECK(updated_at >= created_at)
) STRICT;

CREATE TABLE study_target_card_origins (
    target_id              TEXT NOT NULL REFERENCES study_targets(id) ON DELETE CASCADE,
    target_note_id         TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    source_document_id     TEXT NOT NULL CHECK(length(source_document_id) = 32),
    source_note_id         TEXT NOT NULL CHECK(length(source_note_id) = 32),
    source_note_revision   INTEGER NOT NULL CHECK(source_note_revision >= 1),
    source_identity_sha256 TEXT NOT NULL CHECK(length(source_identity_sha256) = 64),
    created_at             INTEGER NOT NULL,
    PRIMARY KEY(target_id, target_note_id),
    UNIQUE(target_id, source_note_id)
) STRICT;

CREATE TABLE study_target_guide_overlays (
    id                TEXT PRIMARY KEY CHECK(length(id) = 32),
    target_id         TEXT NOT NULL REFERENCES study_targets(id) ON DELETE CASCADE,
    guide_id          TEXT REFERENCES study_target_guides(id) ON DELETE CASCADE,
    block_key         TEXT CHECK(block_key IS NULL OR length(block_key) <= 160),
    overlay_kind      TEXT NOT NULL CHECK(overlay_kind IN (
                          'guide_note', 'topic_note', 'title_override',
                          'body_override', 'highlight'
                      )),
    base_content_hash TEXT CHECK(base_content_hash IS NULL OR length(base_content_hash) = 64),
    rich_doc_json     TEXT NOT NULL CHECK(length(rich_doc_json) <= 2097152),
    plain_text        TEXT NOT NULL CHECK(length(plain_text) <= 500000),
    state             TEXT NOT NULL CHECK(state IN ('active', 'conflicted', 'discarded')),
    revision          INTEGER NOT NULL DEFAULT 1 CHECK(revision >= 1),
    created_at        INTEGER NOT NULL,
    updated_at        INTEGER NOT NULL,
    CHECK(
        (overlay_kind = 'guide_note' AND block_key IS NULL)
        OR
        (overlay_kind <> 'guide_note' AND block_key IS NOT NULL)
    )
) STRICT;

CREATE TABLE study_target_guides (
    id                      TEXT PRIMARY KEY CHECK(length(id) = 32),
    target_id               TEXT NOT NULL REFERENCES study_targets(id) ON DELETE CASCADE,
    batch_id                TEXT REFERENCES study_target_batches(operation_id) ON DELETE SET NULL,
    revision                INTEGER NOT NULL CHECK(revision >= 1),
    source_snapshot_id      TEXT NOT NULL CHECK(length(source_snapshot_id) = 64),
    synthesis_analysis_hash TEXT CHECK(synthesis_analysis_hash IS NULL OR length(synthesis_analysis_hash) = 64),
    content_json            TEXT NOT NULL
                            CHECK(json_valid(content_json)
                                  AND json_type(content_json) = 'object'
                                  AND length(content_json) <= 2097152),
    content_hash            TEXT NOT NULL CHECK(length(content_hash) = 64),
    state                   TEXT NOT NULL CHECK(state IN ('active', 'superseded')),
    created_at              INTEGER NOT NULL,
    updated_at              INTEGER NOT NULL,
    published_at            INTEGER NOT NULL,
    UNIQUE(target_id, revision),
    CHECK(updated_at >= created_at)
) STRICT;

CREATE TABLE study_target_suppressions (
    target_id      TEXT NOT NULL REFERENCES study_targets(id) ON DELETE CASCADE,
    artifact_kind  TEXT NOT NULL CHECK(artifact_kind IN ('flashcards', 'study_guide')),
    artifact_id    TEXT,
    operation_id   TEXT NOT NULL CHECK(length(operation_id) = 32),
    reason         TEXT NOT NULL CHECK(reason IN ('user_deleted')),
    created_at     INTEGER NOT NULL,
    PRIMARY KEY(target_id, artifact_kind)
) STRICT;

CREATE TABLE study_targets (
    id                   TEXT PRIMARY KEY CHECK(length(id) = 32),
    target_kind          TEXT NOT NULL CHECK(target_kind IN ('document', 'collection')),
    document_id          TEXT REFERENCES documents(id) ON DELETE CASCADE,
    collection_id        TEXT REFERENCES document_collections(id) ON DELETE CASCADE,
    output_policy_json   TEXT NOT NULL DEFAULT '{"flashcards":true,"studyGuide":true}'
                         CHECK(json_valid(output_policy_json)
                               AND json_type(output_policy_json) = 'object'
                               AND length(output_policy_json) <= 16384),
    active_snapshot_id   TEXT NOT NULL DEFAULT ''
                         CHECK(active_snapshot_id = '' OR length(active_snapshot_id) = 64),
    active_snapshot_json TEXT NOT NULL DEFAULT '{}'
                         CHECK(json_valid(active_snapshot_json)
                               AND json_type(active_snapshot_json) = 'object'
                               AND length(active_snapshot_json) <= 2097152),
    deck_id              TEXT REFERENCES decks(id) ON DELETE SET NULL,
    revision             INTEGER NOT NULL DEFAULT 1 CHECK(revision >= 1),
    created_at           INTEGER NOT NULL,
    updated_at           INTEGER NOT NULL,
    CHECK(
        (target_kind = 'document' AND document_id IS NOT NULL AND collection_id IS NULL)
        OR
        (target_kind = 'collection' AND collection_id IS NOT NULL AND document_id IS NULL)
    ),
    CHECK(updated_at >= created_at)
) STRICT;

CREATE TABLE tags (
    id               TEXT PRIMARY KEY CHECK(length(id) = 32),
    name             TEXT NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 64),
    normalized_name  TEXT NOT NULL UNIQUE
                     CHECK(length(trim(normalized_name)) BETWEEN 1 AND 64),
    created_at       INTEGER NOT NULL,
    updated_at       INTEGER NOT NULL
) STRICT;

CREATE TABLE tutor_conversations (
    id TEXT PRIMARY KEY CHECK(length(id) = 32),
    operation_id TEXT NOT NULL UNIQUE CHECK(length(operation_id) = 32),
    title TEXT NOT NULL CHECK(length(trim(title)) BETWEEN 1 AND 200),
    scope_json TEXT NOT NULL CHECK(json_valid(scope_json) AND length(scope_json) <= 1024),
    summary_text TEXT NOT NULL DEFAULT '' CHECK(length(summary_text) <= 8000),
    summary_through_sequence INTEGER NOT NULL DEFAULT 0 CHECK(summary_through_sequence >= 0),
    revision INTEGER NOT NULL DEFAULT 1 CHECK(revision >= 1),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
, history_enabled INTEGER NOT NULL DEFAULT 1
    CHECK(history_enabled IN (0, 1)), retrieval_enabled INTEGER NOT NULL DEFAULT 0
    CHECK(retrieval_enabled IN (0, 1)), memory_start_sequence INTEGER NOT NULL DEFAULT 0
    CHECK(memory_start_sequence >= 0), project_id TEXT
    REFERENCES tutor_projects(id) ON DELETE SET NULL, organization_revision INTEGER NOT NULL DEFAULT 1
    CHECK(organization_revision >= 1), archived_at INTEGER) STRICT;

CREATE TABLE tutor_memory_operations (
    operation_id TEXT PRIMARY KEY CHECK(length(operation_id) = 32),
    conversation_id TEXT NOT NULL REFERENCES tutor_conversations(id) ON DELETE CASCADE,
    request_hash TEXT NOT NULL CHECK(length(request_hash) = 64),
    created_at INTEGER NOT NULL
) STRICT;

CREATE TABLE tutor_organization_operations (
    operation_id TEXT PRIMARY KEY CHECK(length(operation_id) = 32),
    action TEXT NOT NULL CHECK(action IN (
        'create_project', 'update_project', 'archive_project', 'restore_project',
        'delete_project', 'rename_conversation', 'move_conversation',
        'archive_conversation', 'restore_conversation', 'delete_conversation'
    )),
    target_kind TEXT NOT NULL CHECK(target_kind IN ('project', 'conversation')),
    target_id TEXT NOT NULL CHECK(length(target_id) = 32),
    request_hash TEXT NOT NULL CHECK(length(request_hash) = 64),
    result_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(result_json) AND length(result_json) <= 16384),
    created_at INTEGER NOT NULL
) STRICT;

CREATE TABLE tutor_projects (
    id TEXT PRIMARY KEY CHECK(length(id) = 32),
    title TEXT NOT NULL CHECK(length(trim(title)) BETWEEN 1 AND 160),
    description TEXT NOT NULL DEFAULT '' CHECK(length(description) <= 4000),
    instructions TEXT NOT NULL DEFAULT '' CHECK(length(instructions) <= 8000),
    default_scope_json TEXT NOT NULL
        CHECK(json_valid(default_scope_json) AND length(default_scope_json) <= 4096),
    revision INTEGER NOT NULL DEFAULT 1 CHECK(revision >= 1),
    archived_at INTEGER,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
) STRICT;

CREATE TABLE tutor_turn_sources (
    turn_id TEXT NOT NULL REFERENCES tutor_turns(id) ON DELETE CASCADE,
    source_id TEXT NOT NULL REFERENCES knowledge_sources(id) ON DELETE CASCADE,
    content_hash TEXT NOT NULL CHECK(length(content_hash) = 64),
    PRIMARY KEY(turn_id, source_id)
) STRICT;

CREATE TABLE tutor_turns (
    id TEXT PRIMARY KEY CHECK(length(id) = 32),
    conversation_id TEXT NOT NULL REFERENCES tutor_conversations(id) ON DELETE CASCADE,
    operation_id TEXT NOT NULL UNIQUE CHECK(length(operation_id) = 32),
    sequence INTEGER NOT NULL CHECK(sequence >= 1),
    request_hash TEXT NOT NULL CHECK(length(request_hash) = 64),
    request_json TEXT NOT NULL CHECK(json_valid(request_json) AND length(request_json) <= 65536),
    mode TEXT NOT NULL CHECK(mode IN ('explain', 'hint', 'quiz', 'answer')),
    user_text TEXT NOT NULL CHECK(length(user_text) <= 16000),
    assistant_text TEXT NOT NULL DEFAULT '' CHECK(length(assistant_text) <= 24000),
    state TEXT NOT NULL CHECK(state IN ('running', 'complete', 'failed', 'interrupted', 'canceled')),
    citations_json TEXT NOT NULL DEFAULT '[]'
        CHECK(json_valid(citations_json) AND length(citations_json) <= 262144),
    quiz_json TEXT CHECK(quiz_json IS NULL OR (json_valid(quiz_json) AND length(quiz_json) <= 24000)),
    answered_question_id TEXT REFERENCES tutor_turns(id) ON DELETE SET NULL,
    evidence_current INTEGER NOT NULL DEFAULT 1 CHECK(evidence_current IN (0, 1)),
    profile_id TEXT REFERENCES ai_provider_profiles(id) ON DELETE SET NULL,
    profile_revision INTEGER NOT NULL CHECK(profile_revision >= 1),
    provider_kind TEXT NOT NULL CHECK(length(provider_kind) <= 80),
    model_id TEXT NOT NULL CHECK(length(model_id) <= 300),
    safe_error_code TEXT NOT NULL DEFAULT '' CHECK(length(safe_error_code) <= 80),
    safe_error_message TEXT NOT NULL DEFAULT '' CHECK(length(safe_error_message) <= 1000),
    total_tokens INTEGER CHECK(total_tokens IS NULL OR total_tokens >= 0),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL, captured_scope_json TEXT
    CHECK(captured_scope_json IS NULL OR
          (json_valid(captured_scope_json) AND length(captured_scope_json) <= 1024)),
    UNIQUE(conversation_id, sequence)
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

CREATE INDEX deck_source_citations_citation_idx
ON deck_source_citations(citation_id, deck_id);

CREATE INDEX decks_active_sort
    ON decks(deleted_at, archived_at, sort_order, name COLLATE NOCASE);

CREATE INDEX decks_folder_sort
    ON decks(deleted_at, archived_at, folder_id, sort_order, name COLLATE NOCASE);

CREATE INDEX deletion_operations_expiry_idx
    ON deletion_operations(expires_at)
    WHERE restored_at IS NULL AND finalized_at IS NULL;

CREATE INDEX document_artifact_jobs_document_idx
ON document_artifact_jobs(document_id, artifact_kind, created_at DESC);

CREATE INDEX document_artifact_metadata_deck_idx
ON document_artifact_metadata(deck_id);

CREATE INDEX document_collection_memberships_collection
ON document_collection_memberships(collection_id, sort_order, id);

CREATE INDEX document_collection_memberships_document
ON document_collection_memberships(document_id, collection_id);

CREATE INDEX document_collection_ranges_membership
ON document_collection_ranges(membership_id, start_unit, end_unit);

CREATE INDEX document_collections_parent_order
ON document_collections(parent_id, sort_order, id);

CREATE INDEX document_decks_deck_idx ON document_decks(deck_id);

CREATE INDEX document_evidence_link_targets ON document_evidence_links(target_occurrence_id);

CREATE INDEX document_evidence_payload_owners
ON document_evidence_occurrences(payload_sha256);

CREATE INDEX document_evidence_source_units
ON document_evidence_occurrences(document_id, source_unit_number);

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

CREATE INDEX document_image_extraction_owners_by_key
ON document_image_extraction_owners(extraction_key);

CREATE INDEX document_image_members_source ON document_image_members(document_id, sha256);

CREATE INDEX document_learning_plan_topics_topic_idx
ON document_learning_plan_topics(topic_id, plan_id);

CREATE UNIQUE INDEX document_learning_plans_one_active_idx
ON document_learning_plans(document_id)
WHERE state = 'active';

CREATE UNIQUE INDEX document_note_topics_one_primary_idx
ON document_note_topics(note_id)
WHERE relationship_kind = 'primary';

CREATE INDEX document_note_topics_topic_idx
ON document_note_topics(document_id, topic_id, note_id);

CREATE INDEX document_outline_checkpoints_order_idx
    ON document_outline_checkpoints(document_id, contract_key, window_index);

CREATE INDEX document_pages_document_idx
    ON document_pages(document_id, page_number);

CREATE INDEX document_semantic_chunk_sources_document_idx
ON document_semantic_chunk_sources(document_id, chunk_index);

CREATE INDEX document_semantic_chunks_pages_idx
    ON document_semantic_chunks(document_id, chunk_index);

CREATE INDEX document_semantic_elements_page_idx
    ON document_semantic_elements(document_id, page_number, element_index);

CREATE INDEX document_semantic_elements_type_idx
    ON document_semantic_elements(document_id, element_type, page_number);

CREATE INDEX document_source_assets_unit_idx
ON document_source_assets(document_id, source_unit_number, role);

CREATE INDEX document_source_blocks_role_idx
ON document_source_blocks(document_id, role, source_unit_number);

CREATE INDEX document_source_blocks_unit_idx
ON document_source_blocks(document_id, source_unit_number, block_ordinal);

CREATE INDEX document_source_files_document_idx
ON document_source_files(document_id, file_ordinal);

CREATE INDEX document_source_units_kind_idx
ON document_source_units(document_id, unit_kind, unit_number);

CREATE INDEX document_study_guide_overlays_scope_idx
ON document_study_guide_overlays(document_id, guide_id, topic_id, state);

CREATE UNIQUE INDEX document_study_guides_one_active_idx
ON document_study_guides(document_id)
WHERE state = 'active';

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

CREATE INDEX generation_operations_recent ON generation_operations(started_at DESC, id);

CREATE INDEX generation_requests_operation_stage
ON generation_requests(operation_id, stage, started_at);

CREATE INDEX knowledge_analysis_document_idx ON knowledge_analysis_windows(document_id, state);

CREATE INDEX knowledge_analysis_source_idx ON knowledge_analysis_sources(source_id, window_id);

CREATE INDEX knowledge_artifact_links_unit_idx ON knowledge_artifact_links(unit_id, source_id);

CREATE INDEX knowledge_collection_view_dependency_lookup
ON knowledge_collection_view_dependencies(dependency_kind, dependency_id);

CREATE UNIQUE INDEX knowledge_preparations_one_running_idx
    ON knowledge_preparations(scope_key) WHERE state = 'running';

CREATE INDEX knowledge_preparations_scope_idx ON knowledge_preparations(scope_key, updated_at DESC);

CREATE INDEX knowledge_sources_card_idx ON knowledge_sources(card_id, active);

CREATE INDEX knowledge_sources_conversation_idx ON knowledge_sources(conversation_id, active);

CREATE INDEX knowledge_sources_document_idx ON knowledge_sources(document_id, kind, active);

CREATE INDEX knowledge_sources_guide_idx ON knowledge_sources(guide_id, parent_key, active);

CREATE INDEX knowledge_sources_note_idx ON knowledge_sources(note_id, active);

CREATE INDEX knowledge_sources_preparation_idx ON knowledge_sources(active, extracted_hash);

CREATE INDEX knowledge_sources_turn_idx ON knowledge_sources(turn_id);

CREATE INDEX knowledge_unit_evidence_source_idx ON knowledge_unit_evidence(source_id, unit_id);

CREATE INDEX knowledge_units_source_idx ON knowledge_units(source_id, source_hash);

CREATE INDEX note_assets_asset_idx ON note_assets(asset_id);

CREATE INDEX note_source_citations_citation_idx
ON note_source_citations(citation_id, note_id);

CREATE INDEX note_tags_tag_idx ON note_tags(tag_id, note_id);

CREATE INDEX notes_deck_active_idx ON notes(deck_id, updated_at DESC) WHERE deleted_at IS NULL;

CREATE INDEX notes_deck_duplicate_idx ON notes(deck_id, duplicate_hash) WHERE deleted_at IS NULL;

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

CREATE INDEX question_attempt_assets_asset_idx ON question_attempt_assets(asset_id);

CREATE INDEX question_attempts_card_idx ON question_attempts(card_id, created_at);

CREATE INDEX question_attempts_note_idx ON question_attempts(note_id);

CREATE INDEX question_attempts_results_idx ON question_attempts(first_checked_at, question_kind, verdict);

CREATE INDEX question_generation_evidence_document
    ON question_generation_evidence(document_id, note_id);

CREATE INDEX question_generation_reports_document
    ON question_generation_reports(document_id, created_at DESC, job_id DESC);

CREATE INDEX question_remediation_instance_idx ON question_remediation_attempts(instance_id, created_at);

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

CREATE UNIQUE INDEX source_citation_snapshots_document_idx
ON source_citation_snapshots(source_document_id);

CREATE INDEX source_citation_snapshots_identity_idx
ON source_citation_snapshots(source_identity, source_fingerprint_sha256);

CREATE INDEX staged_external_imports_state_expiry_idx
ON staged_external_imports(state, expires_at);

CREATE INDEX study_target_artifact_jobs_state_idx
ON study_target_artifact_jobs(state, updated_at DESC);

CREATE INDEX study_target_batches_target_idx
ON study_target_batches(target_id, created_at DESC, operation_id);

CREATE INDEX study_target_card_origins_document_idx
ON study_target_card_origins(source_document_id, target_id);

CREATE INDEX study_target_guide_overlays_scope_idx
ON study_target_guide_overlays(target_id, guide_id, block_key, state);

CREATE UNIQUE INDEX study_target_guides_one_active_idx
ON study_target_guides(target_id)
WHERE state = 'active';

CREATE INDEX study_target_suppressions_operation_idx
ON study_target_suppressions(operation_id);

CREATE UNIQUE INDEX study_targets_collection_idx
ON study_targets(collection_id)
WHERE target_kind = 'collection';

CREATE INDEX study_targets_deck_idx
ON study_targets(deck_id)
WHERE deck_id IS NOT NULL;

CREATE UNIQUE INDEX study_targets_document_idx
ON study_targets(document_id)
WHERE target_kind = 'document';

CREATE INDEX tutor_conversations_project_idx
    ON tutor_conversations(project_id, archived_at, updated_at DESC, id);

CREATE INDEX tutor_conversations_updated_idx ON tutor_conversations(updated_at DESC, id);

CREATE INDEX tutor_organization_operations_target_idx
    ON tutor_organization_operations(target_kind, target_id, created_at);

CREATE INDEX tutor_projects_active_idx
    ON tutor_projects(archived_at, title COLLATE NOCASE, id);

CREATE INDEX tutor_turn_sources_source_idx ON tutor_turn_sources(source_id, turn_id);

CREATE INDEX tutor_turns_history_idx ON tutor_turns(conversation_id, sequence);

CREATE UNIQUE INDEX tutor_turns_one_answer_idx ON tutor_turns(answered_question_id)
    WHERE answered_question_id IS NOT NULL AND state IN ('running', 'complete');

CREATE UNIQUE INDEX tutor_turns_one_running_idx
    ON tutor_turns(conversation_id) WHERE state = 'running';

CREATE TRIGGER card_question_presentation_identity BEFORE UPDATE ON card_question_presentations
WHEN OLD.card_id <> NEW.card_id OR OLD.note_id <> NEW.note_id OR OLD.question_key <> NEW.question_key
BEGIN
    SELECT RAISE(ABORT, 'Question presentation identity is immutable');
END;

CREATE TRIGGER card_question_presentation_insert BEFORE INSERT ON card_question_presentations
WHEN NOT EXISTS (
    SELECT 1 FROM cards c JOIN notes n ON n.id = c.note_id
    WHERE c.id = NEW.card_id AND n.id = NEW.note_id
        AND n.note_type = 'interactive' AND c.direction = 'forward'
) BEGIN
    SELECT RAISE(ABORT, 'Question presentation must belong to its interactive note');
END;

CREATE TRIGGER collection_guide_checkpoint_source_changed
AFTER UPDATE OF content_hash, active, extraction_contract ON knowledge_sources
WHEN OLD.content_hash <> NEW.content_hash OR OLD.active <> NEW.active
     OR OLD.extraction_contract <> NEW.extraction_contract
BEGIN
    DELETE FROM collection_guide_checkpoints WHERE cache_key IN (
        SELECT cache_key FROM collection_guide_checkpoint_sources WHERE source_id = NEW.id
    );
END;

CREATE TRIGGER collection_guide_checkpoint_source_deleted
BEFORE DELETE ON knowledge_sources BEGIN
    DELETE FROM collection_guide_checkpoints WHERE cache_key IN (
        SELECT cache_key FROM collection_guide_checkpoint_sources WHERE source_id = OLD.id
    );
END;

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

CREATE TRIGGER document_artifact_metadata_after_generated_deck_link
AFTER UPDATE OF generated_deck_id ON documents
WHEN NEW.generated_deck_id IS NOT NULL
 AND NEW.generated_deck_id IS NOT OLD.generated_deck_id
BEGIN
    UPDATE document_artifact_metadata
    SET deck_id = NEW.generated_deck_id,
        updated_at = MAX(updated_at, NEW.updated_at)
    WHERE document_id = NEW.id;

    UPDATE decks
    SET name = CASE
            WHEN lower(trim(name)) = lower(trim(COALESCE((
                    SELECT citation.source_title
                    FROM source_citation_snapshots AS citation
                    WHERE citation.source_document_id = NEW.id
                ), '')))
              OR lower(trim(name)) = lower(trim(COALESCE((
                    SELECT citation.original_filename
                    FROM source_citation_snapshots AS citation
                    WHERE citation.source_document_id = NEW.id
                ), '')))
              OR lower(trim(name)) = lower(trim(replace(COALESCE((
                    SELECT citation.original_filename
                    FROM source_citation_snapshots AS citation
                    WHERE citation.source_document_id = NEW.id
                ), ''), '.' || COALESCE((
                    SELECT citation.source_format
                    FROM source_citation_snapshots AS citation
                    WHERE citation.source_document_id = NEW.id
                ), ''), '')))
            THEN (SELECT metadata.deck_title
                  FROM document_artifact_metadata AS metadata
                  WHERE metadata.document_id = NEW.id)
            ELSE name
        END,
        description = CASE
            WHEN trim(description) = ''
              OR description = 'AI-generated flashcards from ' || COALESCE((
                    SELECT citation.original_filename
                    FROM source_citation_snapshots AS citation
                    WHERE citation.source_document_id = NEW.id
                ), '') || '. Edit or delete these cards through the normal deck tools.'
            THEN (SELECT metadata.deck_description
                  FROM document_artifact_metadata AS metadata
                  WHERE metadata.document_id = NEW.id)
            ELSE description
        END,
        updated_at = MAX(updated_at, NEW.updated_at),
        revision = revision + 1
    WHERE id = NEW.generated_deck_id
      AND deleted_at IS NULL
      AND EXISTS (
          SELECT 1
          FROM document_artifact_metadata AS metadata
          WHERE metadata.document_id = NEW.id
      )
      AND (
          lower(trim(name)) = lower(trim(COALESCE((
              SELECT citation.source_title
              FROM source_citation_snapshots AS citation
              WHERE citation.source_document_id = NEW.id
          ), '')))
          OR lower(trim(name)) = lower(trim(COALESCE((
              SELECT citation.original_filename
              FROM source_citation_snapshots AS citation
              WHERE citation.source_document_id = NEW.id
          ), '')))
          OR lower(trim(name)) = lower(trim(replace(COALESCE((
              SELECT citation.original_filename
              FROM source_citation_snapshots AS citation
              WHERE citation.source_document_id = NEW.id
          ), ''), '.' || COALESCE((
              SELECT citation.source_format
              FROM source_citation_snapshots AS citation
              WHERE citation.source_document_id = NEW.id
          ), ''), '')))
          OR trim(description) = ''
          OR description = 'AI-generated flashcards from ' || COALESCE((
              SELECT citation.original_filename
              FROM source_citation_snapshots AS citation
              WHERE citation.source_document_id = NEW.id
          ), '') || '. Edit or delete these cards through the normal deck tools.'
      );

    UPDATE document_artifact_metadata
    SET applied_deck_title = CASE
            WHEN (SELECT deck.name FROM decks AS deck WHERE deck.id = NEW.generated_deck_id) = deck_title
            THEN deck_title
            ELSE applied_deck_title
        END,
        applied_deck_description = CASE
            WHEN (SELECT deck.description FROM decks AS deck WHERE deck.id = NEW.generated_deck_id) = deck_description
            THEN deck_description
            ELSE applied_deck_description
        END
    WHERE document_id = NEW.id;
END;

CREATE TRIGGER document_collections_one_level_insert
BEFORE INSERT ON document_collections
WHEN NEW.parent_id IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'document_collections_parent_must_be_null');
END;

CREATE TRIGGER document_collections_one_level_update
BEFORE UPDATE OF parent_id ON document_collections
WHEN NEW.parent_id IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'document_collections_parent_must_be_null');
END;

CREATE TRIGGER document_evidence_link_authority
BEFORE INSERT ON document_evidence_links WHEN NEW.target_occurrence_id IS NOT NULL
BEGIN
    SELECT CASE WHEN
        (SELECT document_id FROM document_evidence_occurrences WHERE id = NEW.occurrence_id)
        IS NOT
        (SELECT document_id FROM document_evidence_occurrences WHERE id = NEW.target_occurrence_id)
    THEN RAISE(ABORT, 'Evidence links must retain their source document') END;
END;

CREATE TRIGGER document_evidence_payload_immutable
BEFORE UPDATE ON document_evidence_payloads
BEGIN
    SELECT RAISE(ABORT, 'Canonical evidence payloads are immutable');
END;

CREATE TRIGGER document_evidence_payload_release
AFTER DELETE ON document_evidence_occurrences
BEGIN
    DELETE FROM document_evidence_payloads WHERE sha256 = OLD.payload_sha256
      AND NOT EXISTS (SELECT 1 FROM document_evidence_occurrences WHERE payload_sha256 = OLD.payload_sha256);
END;

CREATE TRIGGER document_image_extraction_last_owner
AFTER DELETE ON document_image_extraction_owners
BEGIN
    DELETE FROM document_image_extractions
    WHERE extraction_key = OLD.extraction_key
      AND NOT EXISTS (
          SELECT 1 FROM document_image_extraction_owners
          WHERE extraction_key = OLD.extraction_key
      );
END;

CREATE TRIGGER knowledge_asset_changed AFTER UPDATE OF relative_path, byte_size, mime_type, deleted_at ON assets
WHEN OLD.relative_path <> NEW.relative_path OR OLD.byte_size <> NEW.byte_size
  OR OLD.mime_type <> NEW.mime_type OR OLD.deleted_at IS NOT NEW.deleted_at
BEGIN
    UPDATE knowledge_sources SET active = 0 WHERE active = 1 AND EXISTS (
        SELECT 1 FROM json_each(record_json, '$.images') image
        WHERE json_extract(image.value, '$.kind') = 'card_asset'
          AND json_extract(image.value, '$.id') = NEW.id);
END;

CREATE TRIGGER knowledge_asset_deleted BEFORE DELETE ON assets
BEGIN
    UPDATE knowledge_sources SET active = 0 WHERE active = 1 AND EXISTS (
        SELECT 1 FROM json_each(record_json, '$.images') image
        WHERE json_extract(image.value, '$.kind') = 'card_asset'
          AND json_extract(image.value, '$.id') = OLD.id);
END;

CREATE TRIGGER knowledge_card_changed AFTER UPDATE OF direction, template_ordinal, deleted_at ON cards
WHEN OLD.direction <> NEW.direction OR OLD.template_ordinal <> NEW.template_ordinal
  OR OLD.deleted_at IS NOT NEW.deleted_at
BEGIN
    UPDATE knowledge_sources SET active = 0 WHERE card_id = NEW.id AND active = 1;
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

CREATE TRIGGER knowledge_collection_view_analysis_source_delete
BEFORE DELETE ON knowledge_analysis_sources BEGIN
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

CREATE TRIGGER knowledge_collection_view_collection_delete
BEFORE DELETE ON document_collections BEGIN
    UPDATE knowledge_collection_views SET stale = 1;
END;

CREATE TRIGGER knowledge_collection_view_collection_insert
AFTER INSERT ON document_collections BEGIN
    UPDATE knowledge_collection_views SET stale = 1;
END;

CREATE TRIGGER knowledge_collection_view_collection_update
AFTER UPDATE ON document_collections BEGIN
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

CREATE TRIGGER knowledge_collection_view_membership_delete
BEFORE DELETE ON document_collection_memberships BEGIN
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

CREATE TRIGGER knowledge_collection_view_range_delete
BEFORE DELETE ON document_collection_ranges BEGIN
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

CREATE TRIGGER knowledge_collection_view_source_insert
AFTER INSERT ON knowledge_sources WHEN NEW.document_id IS NOT NULL BEGIN
    UPDATE knowledge_collection_views SET stale = 1
    WHERE collection_id IN (
        SELECT collection_id FROM knowledge_collection_view_dependencies
        WHERE (dependency_kind = 'document' AND dependency_id = NEW.document_id)
           OR (dependency_kind = 'source' AND dependency_id = NEW.id)
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

CREATE TRIGGER knowledge_collection_view_unit_insert
AFTER INSERT ON knowledge_units BEGIN
    UPDATE knowledge_collection_views SET stale = 1
    WHERE collection_id IN (
        SELECT collection_id FROM knowledge_collection_view_dependencies
        WHERE dependency_kind = 'source' AND dependency_id = NEW.source_id
    );
END;

CREATE TRIGGER knowledge_conversation_admission BEFORE INSERT ON knowledge_sources
WHEN NEW.kind = 'conversation' AND NEW.active = 1 AND NOT EXISTS (
    SELECT 1 FROM tutor_turns t JOIN tutor_conversations c ON c.id = t.conversation_id
    WHERE t.id = NEW.turn_id AND c.id = NEW.conversation_id AND t.state = 'complete'
      AND t.evidence_current = 1 AND c.retrieval_enabled = 1 AND t.sequence > c.memory_start_sequence)
BEGIN
    SELECT RAISE(ABORT, 'Conversation is not eligible for semantic preparation');
END;

CREATE TRIGGER knowledge_conversation_readmission BEFORE UPDATE OF active ON knowledge_sources
WHEN NEW.kind = 'conversation' AND NEW.active = 1 AND NOT EXISTS (
    SELECT 1 FROM tutor_turns t JOIN tutor_conversations c ON c.id = t.conversation_id
    WHERE t.id = NEW.turn_id AND c.id = NEW.conversation_id AND t.state = 'complete'
      AND t.evidence_current = 1 AND c.retrieval_enabled = 1 AND t.sequence > c.memory_start_sequence)
BEGIN
    SELECT RAISE(ABORT, 'Conversation is not eligible for semantic preparation');
END;

CREATE TRIGGER knowledge_deck_deleted AFTER UPDATE OF deleted_at ON decks
WHEN NEW.deleted_at IS NOT NULL BEGIN
    UPDATE knowledge_sources SET active = 0 WHERE note_id IN
        (SELECT id FROM notes WHERE deck_id = NEW.id) AND active = 1;
END;

CREATE TRIGGER knowledge_deck_renamed AFTER UPDATE OF name ON decks
WHEN OLD.name <> NEW.name BEGIN
    UPDATE knowledge_sources SET active = 0 WHERE note_id IN
        (SELECT id FROM notes WHERE deck_id = NEW.id) AND active = 1;
END;

CREATE TRIGGER knowledge_document_asset_changed AFTER UPDATE OF sha256 ON document_source_assets
WHEN OLD.sha256 <> NEW.sha256 AND NOT EXISTS (
    SELECT 1 FROM document_image_projection_writes WHERE document_id = NEW.document_id)
BEGIN
    UPDATE knowledge_sources SET active = 0 WHERE document_id = NEW.document_id AND active = 1;
END;

CREATE TRIGGER knowledge_document_asset_deleted BEFORE DELETE ON document_source_assets
WHEN NOT EXISTS (SELECT 1 FROM document_image_projection_writes WHERE document_id = OLD.document_id)
BEGIN
    UPDATE knowledge_sources SET active = 0 WHERE document_id = OLD.document_id AND active = 1;
END;

CREATE TRIGGER knowledge_document_chunk_changed AFTER UPDATE ON document_semantic_chunk_sources
WHEN NOT EXISTS (SELECT 1 FROM document_image_projection_writes WHERE document_id = NEW.document_id)
BEGIN
    UPDATE knowledge_sources SET active = 0 WHERE kind = 'document'
        AND document_id = NEW.document_id AND parent_key = CAST(NEW.chunk_index AS TEXT) AND active = 1;
END;

CREATE TRIGGER knowledge_document_chunk_deleted AFTER DELETE ON document_semantic_chunk_sources
WHEN NOT EXISTS (SELECT 1 FROM document_image_projection_writes WHERE document_id = OLD.document_id)
BEGIN
    DELETE FROM knowledge_sources WHERE kind = 'document'
        AND document_id = OLD.document_id AND parent_key = CAST(OLD.chunk_index AS TEXT);
END;

CREATE TRIGGER knowledge_document_renamed AFTER UPDATE OF title ON documents
WHEN OLD.title <> NEW.title BEGIN
    UPDATE knowledge_sources SET active = 0 WHERE document_id = NEW.id AND active = 1
      AND (kind <> 'document' OR COALESCE(json_extract(record_json, '$.identityVersion'), 1) NOT IN (2, 3));
END;

CREATE TRIGGER knowledge_evidence_source_changed
BEFORE UPDATE OF active, content_hash, owner_revision ON knowledge_sources
WHEN (OLD.active = 1 AND NEW.active = 0) OR OLD.content_hash <> NEW.content_hash
    OR OLD.owner_revision <> NEW.owner_revision
BEGIN
    INSERT OR IGNORE INTO knowledge_cache_evictions(source_id)
        SELECT u.source_id FROM knowledge_units u JOIN knowledge_unit_evidence e ON e.unit_id = u.id
        WHERE e.source_id = OLD.id;
    UPDATE knowledge_sources SET extracted_hash = '', extraction_contract = ''
        WHERE id IN (SELECT a.source_id FROM knowledge_analysis_sources a
            WHERE a.window_id IN (SELECT window_id FROM knowledge_analysis_sources WHERE source_id = OLD.id))
        OR id IN (SELECT u.source_id FROM knowledge_units u JOIN knowledge_unit_evidence e ON e.unit_id = u.id
            WHERE e.source_id = OLD.id);
    DELETE FROM knowledge_units WHERE id IN
        (SELECT unit_id FROM knowledge_unit_evidence WHERE source_id = OLD.id);
    DELETE FROM knowledge_analysis_windows WHERE id IN
        (SELECT window_id FROM knowledge_analysis_sources WHERE source_id = OLD.id);
END;

CREATE TRIGGER knowledge_evidence_source_deleted BEFORE DELETE ON knowledge_sources
BEGIN
    INSERT OR IGNORE INTO knowledge_cache_evictions(source_id)
        SELECT u.source_id FROM knowledge_units u JOIN knowledge_unit_evidence e ON e.unit_id = u.id
        WHERE e.source_id = OLD.id;
    UPDATE knowledge_sources SET extracted_hash = '', extraction_contract = ''
        WHERE id IN (SELECT a.source_id FROM knowledge_analysis_sources a
            WHERE a.window_id IN (SELECT window_id FROM knowledge_analysis_sources WHERE source_id = OLD.id))
        OR id IN (SELECT u.source_id FROM knowledge_units u JOIN knowledge_unit_evidence e ON e.unit_id = u.id
            WHERE e.source_id = OLD.id);
    DELETE FROM knowledge_units WHERE id IN
        (SELECT unit_id FROM knowledge_unit_evidence WHERE source_id = OLD.id);
    DELETE FROM knowledge_analysis_windows WHERE id IN
        (SELECT window_id FROM knowledge_analysis_sources WHERE source_id = OLD.id);
END;

CREATE TRIGGER knowledge_guide_overview_changed AFTER UPDATE OF overview_text ON document_study_guides
WHEN OLD.overview_text <> NEW.overview_text
BEGIN
    UPDATE knowledge_sources SET active = 0 WHERE guide_id = NEW.id AND parent_key = 'overview' AND active = 1;
END;

CREATE TRIGGER knowledge_guide_section_changed AFTER UPDATE OF content_hash ON document_study_guide_sections
WHEN OLD.content_hash <> NEW.content_hash
BEGIN
    UPDATE knowledge_sources SET active = 0 WHERE guide_id = NEW.guide_id
        AND parent_key = NEW.topic_id AND active = 1;
END;

CREATE TRIGGER knowledge_guide_section_deleted AFTER DELETE ON document_study_guide_sections
BEGIN
    DELETE FROM knowledge_sources WHERE guide_id = OLD.guide_id AND parent_key = OLD.topic_id;
END;

CREATE TRIGGER knowledge_guide_superseded AFTER UPDATE OF state ON document_study_guides
WHEN NEW.state <> 'active'
BEGIN
    UPDATE knowledge_sources SET active = 0 WHERE guide_id = NEW.id AND active = 1;
END;

CREATE TRIGGER knowledge_guide_topic_changed AFTER UPDATE OF title ON document_learning_plan_topics
WHEN OLD.title <> NEW.title
BEGIN
    UPDATE knowledge_sources SET active = 0 WHERE parent_key = NEW.topic_id AND active = 1
    AND guide_id IN (SELECT id FROM document_study_guides WHERE plan_id = NEW.plan_id);
END;

CREATE TRIGGER knowledge_note_changed
AFTER UPDATE OF note_type, front_doc, back_doc, front_text, back_text, cloze_source,
    cloze_mode, question_definition, deleted_at, deck_id ON notes
WHEN OLD.note_type IS NOT NEW.note_type OR OLD.front_doc IS NOT NEW.front_doc
    OR OLD.back_doc IS NOT NEW.back_doc OR OLD.front_text IS NOT NEW.front_text
    OR OLD.back_text IS NOT NEW.back_text OR OLD.cloze_source IS NOT NEW.cloze_source
    OR OLD.cloze_mode IS NOT NEW.cloze_mode
    OR OLD.question_definition IS NOT NEW.question_definition
    OR OLD.deleted_at IS NOT NEW.deleted_at OR OLD.deck_id IS NOT NEW.deck_id
BEGIN
    UPDATE knowledge_sources SET active = 0 WHERE note_id = NEW.id AND active = 1;
END;

CREATE TRIGGER knowledge_overlay_changed AFTER UPDATE ON document_study_guide_overlays
BEGIN
    UPDATE knowledge_sources SET active = 0 WHERE kind = 'guide'
        AND document_id = NEW.document_id AND active = 1;
END;

CREATE TRIGGER knowledge_overlay_deleted AFTER DELETE ON document_study_guide_overlays
BEGIN
    UPDATE knowledge_sources SET active = 0 WHERE kind = 'guide'
        AND document_id = OLD.document_id AND active = 1;
END;

CREATE TRIGGER knowledge_overlay_inserted AFTER INSERT ON document_study_guide_overlays
BEGIN
    UPDATE knowledge_sources SET active = 0 WHERE kind = 'guide'
        AND document_id = NEW.document_id AND active = 1;
END;

CREATE TRIGGER knowledge_source_deleted BEFORE DELETE ON knowledge_sources
BEGIN
    INSERT INTO knowledge_cache_evictions (source_id)
    SELECT OLD.id WHERE NOT EXISTS (
        SELECT 1 FROM knowledge_cache_evictions WHERE source_id = OLD.id
    );
    UPDATE tutor_conversations SET summary_text = '', summary_through_sequence = memory_start_sequence
    WHERE id IN (SELECT conversation_id FROM tutor_turns WHERE id IN
        (SELECT turn_id FROM tutor_turn_sources WHERE source_id = OLD.id));
    UPDATE tutor_turns SET evidence_current = 0
    WHERE evidence_current = 1 AND id IN (SELECT turn_id FROM tutor_turn_sources WHERE source_id = OLD.id);
END;

CREATE TRIGGER knowledge_source_invalidated AFTER UPDATE OF active, content_hash ON knowledge_sources
WHEN (NEW.active = 0 AND OLD.active = 1) OR OLD.content_hash <> NEW.content_hash
BEGIN
    UPDATE tutor_conversations SET summary_text = '', summary_through_sequence = memory_start_sequence
    WHERE id IN (SELECT conversation_id FROM tutor_turns WHERE id IN
        (SELECT turn_id FROM tutor_turn_sources WHERE source_id = OLD.id));
    UPDATE tutor_turns SET evidence_current = 0
    WHERE evidence_current = 1 AND id IN (SELECT turn_id FROM tutor_turn_sources WHERE source_id = OLD.id);
    INSERT INTO knowledge_cache_evictions (source_id)
    SELECT OLD.id WHERE NOT EXISTS (
        SELECT 1 FROM knowledge_cache_evictions WHERE source_id = OLD.id
    );
    DELETE FROM knowledge_artifact_links WHERE source_id = OLD.id;
    DELETE FROM knowledge_units WHERE source_id = OLD.id;
    UPDATE knowledge_sources SET extraction_contract = '', extracted_hash = '' WHERE id = NEW.id;
END;

CREATE TRIGGER knowledge_turn_evidence_invalidated AFTER UPDATE OF evidence_current ON tutor_turns
WHEN OLD.evidence_current = 1 AND NEW.evidence_current = 0
BEGIN
    UPDATE knowledge_sources SET active = 0 WHERE turn_id = NEW.id AND active = 1;
END;

CREATE TRIGGER knowledge_view_after_unit_change AFTER UPDATE OF content_hash, extraction_contract ON knowledge_units
WHEN OLD.content_hash <> NEW.content_hash OR OLD.extraction_contract <> NEW.extraction_contract BEGIN
    DELETE FROM knowledge_document_views WHERE document_id IN
        (SELECT document_id FROM knowledge_sources WHERE id = NEW.source_id);
END;

CREATE TRIGGER knowledge_view_after_unit_insert AFTER INSERT ON knowledge_units BEGIN
    DELETE FROM knowledge_document_views WHERE document_id IN
        (SELECT document_id FROM knowledge_sources WHERE id = NEW.source_id);
END;

CREATE TRIGGER knowledge_view_after_window_change AFTER UPDATE OF report_json ON knowledge_analysis_windows
WHEN OLD.report_json <> NEW.report_json BEGIN
    DELETE FROM knowledge_document_views WHERE document_id = NEW.document_id;
END;

CREATE TRIGGER knowledge_view_after_window_insert AFTER INSERT ON knowledge_analysis_windows BEGIN
    DELETE FROM knowledge_document_views WHERE document_id = NEW.document_id;
END;

CREATE TRIGGER knowledge_view_before_unit_delete BEFORE DELETE ON knowledge_units BEGIN
    DELETE FROM knowledge_document_views WHERE document_id IN
        (SELECT document_id FROM knowledge_sources WHERE id = OLD.source_id);
END;

CREATE TRIGGER knowledge_view_before_window_delete BEFORE DELETE ON knowledge_analysis_windows BEGIN
    DELETE FROM knowledge_document_views WHERE document_id = OLD.document_id;
END;

CREATE TRIGGER note_tags_search_after_delete AFTER DELETE ON note_tags BEGIN
    UPDATE card_search_fts SET tags = COALESCE((
        SELECT group_concat(t.name, ' ') FROM note_tags nt JOIN tags t ON t.id = nt.tag_id
        WHERE nt.note_id = OLD.note_id), '')
    WHERE rowid = (SELECT rowid FROM notes WHERE id = OLD.note_id);
END;

CREATE TRIGGER note_tags_search_after_insert AFTER INSERT ON note_tags BEGIN
    UPDATE card_search_fts SET tags = COALESCE((
        SELECT group_concat(t.name, ' ') FROM note_tags nt JOIN tags t ON t.id = nt.tag_id
        WHERE nt.note_id = NEW.note_id), '')
    WHERE rowid = (SELECT rowid FROM notes WHERE id = NEW.note_id);
END;

CREATE TRIGGER notes_search_after_delete AFTER DELETE ON notes BEGIN
    DELETE FROM card_search_fts WHERE rowid = OLD.rowid;
END;

CREATE TRIGGER notes_search_after_insert AFTER INSERT ON notes
WHEN NEW.deleted_at IS NULL BEGIN
    INSERT INTO card_search_fts(rowid, note_id, deck_id, question, answer, tags)
    VALUES (NEW.rowid, NEW.id, NEW.deck_id, NEW.front_text, NEW.back_text, '');
END;

CREATE TRIGGER notes_search_after_update
AFTER UPDATE OF deck_id, front_text, back_text, deleted_at ON notes BEGIN
    DELETE FROM card_search_fts WHERE rowid = OLD.rowid;
    INSERT INTO card_search_fts(rowid, note_id, deck_id, question, answer, tags)
    SELECT NEW.rowid, NEW.id, NEW.deck_id, NEW.front_text, NEW.back_text,
        COALESCE((SELECT group_concat(t.name, ' ') FROM note_tags nt
            JOIN tags t ON t.id = nt.tag_id WHERE nt.note_id = NEW.id), '')
    WHERE NEW.deleted_at IS NULL AND EXISTS (
        SELECT 1 FROM decks d WHERE d.id = NEW.deck_id
            AND d.deleted_at IS NULL AND d.archived_at IS NULL);
END;

CREATE TRIGGER question_generation_reports_immutable
BEFORE UPDATE ON question_generation_reports WHEN OLD.report_json IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'completed generation reports are immutable');
END;

CREATE TRIGGER source_citation_snapshot_title_immutable
AFTER UPDATE OF source_title ON source_citation_snapshots
WHEN NEW.source_title <> OLD.source_title
BEGIN
    UPDATE source_citation_snapshots
    SET source_title = OLD.source_title
    WHERE id = NEW.id AND source_title = NEW.source_title;
END;

CREATE TRIGGER tags_search_after_update AFTER UPDATE OF name ON tags BEGIN
    UPDATE card_search_fts SET tags = COALESCE((
        SELECT group_concat(t.name, ' ') FROM note_tags nt JOIN tags t ON t.id = nt.tag_id
        WHERE nt.note_id = card_search_fts.note_id), '')
    WHERE note_id IN (SELECT note_id FROM note_tags WHERE tag_id = NEW.id);
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

CREATE TRIGGER tutor_memory_changed
AFTER UPDATE OF history_enabled, retrieval_enabled, memory_start_sequence ON tutor_conversations
WHEN OLD.history_enabled <> NEW.history_enabled OR OLD.retrieval_enabled <> NEW.retrieval_enabled
  OR OLD.memory_start_sequence <> NEW.memory_start_sequence
BEGIN
    UPDATE tutor_conversations SET summary_text = '', summary_through_sequence = NEW.memory_start_sequence
    WHERE id = NEW.id;
    DELETE FROM knowledge_sources WHERE conversation_id = NEW.id;
END;

-- Seed rows retained by a fresh schema-30 library.
INSERT INTO "app_settings" ("key", "value_json", "updated_at") VALUES ('cardBaseSize', '18', 0);
INSERT INTO "app_settings" ("key", "value_json", "updated_at") VALUES ('cardFont', '"system"', 0);
INSERT INTO "app_settings" ("key", "value_json", "updated_at") VALUES ('confirmDeletions', 'true', 0);
INSERT INTO "app_settings" ("key", "value_json", "updated_at") VALUES ('dailyNewLimit', '20', 0);
INSERT INTO "app_settings" ("key", "value_json", "updated_at") VALUES ('dailyReviewLimit', '200', 0);
INSERT INTO "app_settings" ("key", "value_json", "updated_at") VALUES ('desiredRetention', '0.9', 0);
INSERT INTO "app_settings" ("key", "value_json", "updated_at") VALUES ('practiceShuffle', 'true', 0);
INSERT INTO "app_settings" ("key", "value_json", "updated_at") VALUES ('reduceMotion', 'false', 0);
INSERT INTO "app_settings" ("key", "value_json", "updated_at") VALUES ('theme', '"dark"', 0);
INSERT INTO "app_settings" ("key", "value_json", "updated_at") VALUES ('uiFont', '"system"', 0);
INSERT INTO "app_settings" ("key", "value_json", "updated_at") VALUES ('uiScale', '100', 0);

-- Seed rows retained by a fresh schema-30 library.
INSERT INTO "document_collection_organization" ("singleton_id", "revision", "updated_at") VALUES (1, 1, 0);

-- Seed rows retained by a fresh schema-30 library.
INSERT INTO "library_organization" ("singleton_id", "revision", "updated_at") VALUES (1, 1, 0);

