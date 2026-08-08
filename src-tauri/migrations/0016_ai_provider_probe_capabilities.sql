-- Redacted provider verification snapshots and capability evidence.
--
-- Raw prompts, responses, reasoning content, credentials, and chat templates
-- are never stored in these tables.

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
