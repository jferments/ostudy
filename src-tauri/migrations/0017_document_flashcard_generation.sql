-- Managed PDF documents, extracted pages, durable generation jobs, and deck provenance.
--
-- Source PDFs are immutable managed files. Page renders and extracted text are
-- local derivatives. Provider responses are stored only after strict task-level
-- parsing; canonical card creation still runs through the ordinary authoring
-- transaction.

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
    revision                 INTEGER NOT NULL DEFAULT 1 CHECK(revision >= 1),
    CHECK(extracted_page_count <= page_count),
    CHECK(rendered_page_count <= page_count),
    CHECK((lifecycle_state = 'archived') = (archived_at IS NOT NULL))
) STRICT;

CREATE INDEX documents_active_updated_idx
    ON documents(updated_at DESC, id)
    WHERE archived_at IS NULL;
CREATE INDEX documents_archived_updated_idx
    ON documents(updated_at DESC, id)
    WHERE archived_at IS NOT NULL;
CREATE INDEX documents_generated_deck_idx
    ON documents(generated_deck_id)
    WHERE generated_deck_id IS NOT NULL;

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

CREATE INDEX document_pages_document_idx
    ON document_pages(document_id, page_number);

CREATE TABLE document_generation_jobs (
    id                           TEXT PRIMARY KEY CHECK(length(id) = 32),
    document_id                  TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    profile_id                   TEXT REFERENCES ai_provider_profiles(id) ON DELETE SET NULL,
    provider_kind                TEXT NOT NULL
                                 CHECK(provider_kind IN (
                                     'llama_cpp', 'deepseek', 'ollama',
                                     'openai', 'gemini', 'claude'
                                 )),
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

CREATE INDEX document_generation_jobs_document_time_idx
    ON document_generation_jobs(document_id, created_at DESC);
CREATE INDEX document_generation_jobs_active_idx
    ON document_generation_jobs(state, updated_at)
    WHERE state IN ('queued', 'extracting', 'generating', 'committing', 'interrupted');

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

CREATE INDEX document_generation_units_state_idx
    ON document_generation_units(job_id, state, page_number);

CREATE TABLE document_decks (
    document_id     TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    deck_id         TEXT NOT NULL REFERENCES decks(id) ON DELETE CASCADE,
    job_id          TEXT NOT NULL REFERENCES document_generation_jobs(id) ON DELETE CASCADE,
    created_at      INTEGER NOT NULL,
    PRIMARY KEY(document_id, deck_id)
) STRICT;

CREATE INDEX document_decks_deck_idx ON document_decks(deck_id);
