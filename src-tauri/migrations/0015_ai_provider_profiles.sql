-- AI provider profiles and explicit task defaults.
--
-- Credentials are intentionally excluded from SQLite. The credential_epoch
-- invalidates provider verification state after any credential mutation.

CREATE TABLE ai_provider_profiles (
    id                      TEXT PRIMARY KEY CHECK(length(id) = 32),
    display_name            TEXT NOT NULL CHECK(length(trim(display_name)) BETWEEN 1 AND 100),
    normalized_name         TEXT NOT NULL CHECK(length(normalized_name) BETWEEN 1 AND 100),
    provider_kind           TEXT NOT NULL
                            CHECK(provider_kind IN (
                                'llama_cpp', 'deepseek', 'ollama',
                                'openai', 'gemini', 'claude'
                            )),
    base_url                TEXT NOT NULL CHECK(length(base_url) BETWEEN 1 AND 2048),
    model_id                TEXT NOT NULL DEFAULT '' CHECK(length(model_id) <= 300),
    auth_mode               TEXT NOT NULL CHECK(auth_mode IN ('none', 'bearer')),
    enabled                 INTEGER NOT NULL DEFAULT 1 CHECK(enabled IN (0, 1)),
    allow_insecure_http     INTEGER NOT NULL DEFAULT 0 CHECK(allow_insecure_http IN (0, 1)),
    request_timeout_ms      INTEGER NOT NULL DEFAULT 60000
                            CHECK(request_timeout_ms BETWEEN 1000 AND 300000),
    credential_epoch        INTEGER NOT NULL DEFAULT 0 CHECK(credential_epoch >= 0),
    revision                INTEGER NOT NULL DEFAULT 1 CHECK(revision >= 1),
    created_at              INTEGER NOT NULL,
    updated_at              INTEGER NOT NULL,
    deleted_at              INTEGER
) STRICT;

CREATE UNIQUE INDEX ai_provider_profiles_active_name_idx
    ON ai_provider_profiles(normalized_name)
    WHERE deleted_at IS NULL;

CREATE INDEX ai_provider_profiles_active_updated_idx
    ON ai_provider_profiles(updated_at DESC)
    WHERE deleted_at IS NULL;

CREATE TABLE ai_task_defaults (
    task_kind       TEXT PRIMARY KEY CHECK(task_kind IN ('flashcard_generation')),
    profile_id      TEXT REFERENCES ai_provider_profiles(id) ON DELETE SET NULL,
    updated_at      INTEGER NOT NULL
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
