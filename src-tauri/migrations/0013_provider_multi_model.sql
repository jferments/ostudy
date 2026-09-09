-- Provider profiles own credentials and connection policy. Task defaults own the
-- model/reasoning choice so one connection can serve multiple models at once.

ALTER TABLE ai_profile_operations ADD COLUMN model_id TEXT
    CHECK(model_id IS NULL OR length(model_id) <= 300);
ALTER TABLE ai_profile_operations ADD COLUMN reasoning_effort TEXT
    CHECK(reasoning_effort IS NULL OR reasoning_effort IN
        ('automatic','none','minimal','low','medium','high','xhigh','max'));

CREATE TABLE ai_task_defaults_next (
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

INSERT INTO ai_task_defaults_next (
    task_kind, profile_id, model_id, reasoning_effort, updated_at
)
SELECT d.task_kind,
       d.profile_id,
       CASE WHEN d.profile_id IS NULL THEN '' ELSE COALESCE(p.model_id, '') END,
       CASE WHEN d.profile_id IS NULL THEN 'automatic'
            ELSE COALESCE(NULLIF(p.reasoning_effort, ''), 'automatic') END,
       d.updated_at
FROM ai_task_defaults d
LEFT JOIN ai_provider_profiles p ON p.id = d.profile_id
WHERE d.profile_id IS NULL OR length(trim(COALESCE(p.model_id, ''))) >= 1;

DROP TABLE ai_task_defaults;
ALTER TABLE ai_task_defaults_next RENAME TO ai_task_defaults;

CREATE TABLE ai_embedding_options_next (
    profile_id TEXT NOT NULL REFERENCES ai_provider_profiles(id) ON DELETE CASCADE,
    model_id TEXT NOT NULL CHECK(length(trim(model_id)) BETWEEN 1 AND 300),
    dimensions INTEGER CHECK(dimensions IS NULL OR dimensions BETWEEN 32 AND 4096),
    input_policy TEXT NOT NULL DEFAULT 'automatic'
        CHECK(input_policy IN ('automatic','symmetric','qwen_retrieval','search_types')),
    revision INTEGER NOT NULL DEFAULT 1 CHECK(revision >= 1),
    updated_at INTEGER NOT NULL,
    PRIMARY KEY(profile_id, model_id)
) STRICT;

INSERT INTO ai_embedding_options_next (
    profile_id, model_id, dimensions, input_policy, revision, updated_at
)
SELECT o.profile_id, p.model_id, o.dimensions, o.input_policy, o.revision, o.updated_at
FROM ai_embedding_options o
JOIN ai_provider_profiles p ON p.id = o.profile_id
WHERE length(trim(p.model_id)) >= 1;

DROP TABLE ai_embedding_options;
ALTER TABLE ai_embedding_options_next RENAME TO ai_embedding_options;

CREATE TABLE ai_provider_probe_results_next (
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

INSERT INTO ai_provider_probe_results_next
SELECT profile_id, profile_revision, credential_epoch, tested_model_id,
       adapter_revision, result_schema_version, overall_status, result_json, tested_at
FROM ai_provider_probe_results
WHERE length(trim(tested_model_id)) >= 1;

DROP TABLE ai_provider_probe_results;
ALTER TABLE ai_provider_probe_results_next RENAME TO ai_provider_probe_results;

CREATE TABLE ai_provider_capabilities_next (
    profile_id TEXT NOT NULL REFERENCES ai_provider_profiles(id) ON DELETE CASCADE,
    model_id TEXT NOT NULL CHECK(length(trim(model_id)) BETWEEN 1 AND 300),
    capability_key TEXT NOT NULL CHECK(length(capability_key) BETWEEN 1 AND 80),
    status TEXT NOT NULL CHECK(status IN ('supported','unsupported','unknown')),
    source_kind TEXT NOT NULL CHECK(source_kind IN ('declared','probed')),
    value_json TEXT NOT NULL DEFAULT 'null' CHECK(length(value_json) <= 2048),
    checked_at INTEGER NOT NULL,
    PRIMARY KEY(profile_id, model_id, capability_key)
) STRICT;

INSERT INTO ai_provider_capabilities_next (
    profile_id, model_id, capability_key, status, source_kind, value_json, checked_at
)
SELECT c.profile_id, p.model_id, c.capability_key, c.status, c.source_kind,
       c.value_json, c.checked_at
FROM ai_provider_capabilities c
JOIN ai_provider_profiles p ON p.id = c.profile_id
WHERE length(trim(p.model_id)) >= 1;

DROP TABLE ai_provider_capabilities;
ALTER TABLE ai_provider_capabilities_next RENAME TO ai_provider_capabilities;
