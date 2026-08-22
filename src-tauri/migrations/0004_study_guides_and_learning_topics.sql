-- Shared document learning plans, durable study guides, learner overlays,
-- topic-linked generated notes, and generic document artifact jobs.
--
-- Canonical document source records remain authoritative. These tables store
-- source-grounded learning structure and learner-owned derived artifacts.

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

CREATE UNIQUE INDEX document_learning_plans_one_active_idx
ON document_learning_plans(document_id)
WHERE state = 'active';

CREATE TABLE document_learning_topics (
    id          TEXT PRIMARY KEY CHECK(length(id) = 32),
    document_id TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    anchor_hash TEXT NOT NULL CHECK(length(anchor_hash) = 64),
    created_at  INTEGER NOT NULL,
    UNIQUE(document_id, anchor_hash)
) STRICT;

CREATE TABLE document_learning_plan_topics (
    plan_id                     TEXT NOT NULL REFERENCES document_learning_plans(id) ON DELETE CASCADE,
    topic_id                    TEXT NOT NULL REFERENCES document_learning_topics(id) ON DELETE CASCADE,
    parent_topic_id             TEXT REFERENCES document_learning_topics(id) ON DELETE RESTRICT,
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

CREATE INDEX document_learning_plan_topics_topic_idx
ON document_learning_plan_topics(topic_id, plan_id);

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

CREATE INDEX document_artifact_jobs_document_idx
ON document_artifact_jobs(document_id, artifact_kind, created_at DESC);

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

CREATE UNIQUE INDEX document_study_guides_one_active_idx
ON document_study_guides(document_id)
WHERE state = 'active';

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

CREATE INDEX document_study_guide_overlays_scope_idx
ON document_study_guide_overlays(document_id, guide_id, topic_id, state);

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

CREATE UNIQUE INDEX document_note_topics_one_primary_idx
ON document_note_topics(note_id)
WHERE relationship_kind = 'primary';

CREATE INDEX document_note_topics_topic_idx
ON document_note_topics(document_id, topic_id, note_id);

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
