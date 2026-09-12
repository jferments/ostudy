-- Shared semantic content and conversational tutoring. This migration does not
-- modify cards, schedules, review outcomes, or learner analytics.

ALTER TABLE ai_profile_operations ADD COLUMN task_kind TEXT
    CHECK(task_kind IS NULL OR task_kind IN ('flashcard_generation', 'tutor', 'embeddings'));

CREATE TABLE ai_task_defaults_next (
    task_kind TEXT PRIMARY KEY CHECK(task_kind IN ('flashcard_generation', 'tutor', 'embeddings')),
    profile_id TEXT REFERENCES ai_provider_profiles(id) ON DELETE SET NULL,
    updated_at INTEGER NOT NULL
) STRICT;
INSERT INTO ai_task_defaults_next SELECT task_kind, profile_id, updated_at FROM ai_task_defaults;
DROP TABLE ai_task_defaults;
ALTER TABLE ai_task_defaults_next RENAME TO ai_task_defaults;

CREATE TABLE ai_embedding_options (
    profile_id TEXT PRIMARY KEY REFERENCES ai_provider_profiles(id) ON DELETE CASCADE,
    dimensions INTEGER CHECK(dimensions IS NULL OR dimensions BETWEEN 32 AND 4096),
    input_policy TEXT NOT NULL DEFAULT 'automatic'
        CHECK(input_policy IN ('automatic', 'symmetric', 'qwen_retrieval', 'search_types')),
    revision INTEGER NOT NULL DEFAULT 1 CHECK(revision >= 1),
    updated_at INTEGER NOT NULL
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
) STRICT;
CREATE INDEX tutor_conversations_updated_idx ON tutor_conversations(updated_at DESC, id);

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
    updated_at INTEGER NOT NULL,
    UNIQUE(conversation_id, sequence)
) STRICT;
CREATE UNIQUE INDEX tutor_turns_one_running_idx
    ON tutor_turns(conversation_id) WHERE state = 'running';
CREATE UNIQUE INDEX tutor_turns_one_answer_idx ON tutor_turns(answered_question_id)
    WHERE answered_question_id IS NOT NULL AND state IN ('running', 'complete');
CREATE INDEX tutor_turns_history_idx ON tutor_turns(conversation_id, sequence);

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
CREATE INDEX knowledge_sources_document_idx ON knowledge_sources(document_id, kind, active);
CREATE INDEX knowledge_sources_note_idx ON knowledge_sources(note_id, active);
CREATE INDEX knowledge_sources_card_idx ON knowledge_sources(card_id, active);
CREATE INDEX knowledge_sources_guide_idx ON knowledge_sources(guide_id, parent_key, active);
CREATE INDEX knowledge_sources_conversation_idx ON knowledge_sources(conversation_id, active);
CREATE INDEX knowledge_sources_turn_idx ON knowledge_sources(turn_id);
CREATE INDEX knowledge_sources_preparation_idx ON knowledge_sources(active, extracted_hash);

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
CREATE INDEX knowledge_units_source_idx ON knowledge_units(source_id, source_hash);

CREATE TABLE knowledge_artifact_links (
    source_id TEXT NOT NULL REFERENCES knowledge_sources(id) ON DELETE CASCADE,
    unit_id TEXT NOT NULL REFERENCES knowledge_units(id) ON DELETE CASCADE,
    source_hash TEXT NOT NULL CHECK(length(source_hash) = 64),
    relationship TEXT NOT NULL CHECK(relationship IN ('assesses', 'explains', 'mentions')),
    created_at INTEGER NOT NULL,
    PRIMARY KEY(source_id, unit_id)
) STRICT;
CREATE INDEX knowledge_artifact_links_unit_idx ON knowledge_artifact_links(unit_id, source_id);

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
CREATE INDEX knowledge_preparations_scope_idx ON knowledge_preparations(scope_key, updated_at DESC);
CREATE UNIQUE INDEX knowledge_preparations_one_running_idx
    ON knowledge_preparations(scope_key) WHERE state = 'running';

CREATE TABLE tutor_turn_sources (
    turn_id TEXT NOT NULL REFERENCES tutor_turns(id) ON DELETE CASCADE,
    source_id TEXT NOT NULL REFERENCES knowledge_sources(id) ON DELETE CASCADE,
    content_hash TEXT NOT NULL CHECK(length(content_hash) = 64),
    PRIMARY KEY(turn_id, source_id)
) STRICT;
CREATE INDEX tutor_turn_sources_source_idx ON tutor_turn_sources(source_id, turn_id);

-- Historical messages remain readable, but invalidated evidence is excluded
-- from retrieval and from the conversation's current rolling summary.
CREATE TRIGGER knowledge_source_invalidated AFTER UPDATE OF active, content_hash ON knowledge_sources
WHEN NEW.active = 0 OR OLD.content_hash <> NEW.content_hash
BEGIN
    UPDATE tutor_conversations SET summary_text = '', summary_through_sequence = 0
    WHERE id IN (SELECT conversation_id FROM tutor_turns WHERE id IN
        (SELECT turn_id FROM tutor_turn_sources WHERE source_id = OLD.id));
    UPDATE tutor_turns SET evidence_current = 0
    WHERE id IN (SELECT turn_id FROM tutor_turn_sources WHERE source_id = OLD.id);
    DELETE FROM knowledge_artifact_links WHERE source_id = OLD.id;
END;
CREATE TRIGGER knowledge_source_deleted BEFORE DELETE ON knowledge_sources
BEGIN
    UPDATE tutor_conversations SET summary_text = '', summary_through_sequence = 0
    WHERE id IN (SELECT conversation_id FROM tutor_turns WHERE id IN
        (SELECT turn_id FROM tutor_turn_sources WHERE source_id = OLD.id));
    UPDATE tutor_turns SET evidence_current = 0
    WHERE id IN (SELECT turn_id FROM tutor_turn_sources WHERE source_id = OLD.id);
END;
CREATE TRIGGER knowledge_turn_evidence_invalidated AFTER UPDATE OF evidence_current ON tutor_turns
WHEN NEW.evidence_current = 0
BEGIN
    UPDATE knowledge_sources SET active = 0 WHERE turn_id = NEW.id AND active = 1;
END;

CREATE TRIGGER knowledge_note_changed
AFTER UPDATE OF front_doc, back_doc, cloze_source, cloze_mode, deleted_at, deck_id ON notes
WHEN OLD.front_doc <> NEW.front_doc OR OLD.back_doc <> NEW.back_doc
  OR OLD.cloze_source <> NEW.cloze_source OR OLD.cloze_mode <> NEW.cloze_mode
  OR OLD.deleted_at IS NOT NEW.deleted_at OR OLD.deck_id <> NEW.deck_id
BEGIN
    UPDATE knowledge_sources SET active = 0 WHERE note_id = NEW.id AND active = 1;
END;
CREATE TRIGGER knowledge_card_changed AFTER UPDATE OF direction, template_ordinal, deleted_at ON cards
WHEN OLD.direction <> NEW.direction OR OLD.template_ordinal <> NEW.template_ordinal
  OR OLD.deleted_at IS NOT NEW.deleted_at
BEGIN
    UPDATE knowledge_sources SET active = 0 WHERE card_id = NEW.id AND active = 1;
END;
CREATE TRIGGER knowledge_deck_deleted AFTER UPDATE OF deleted_at ON decks
WHEN NEW.deleted_at IS NOT NULL
BEGIN
    UPDATE knowledge_sources SET active = 0 WHERE note_id IN
        (SELECT id FROM notes WHERE deck_id = NEW.id) AND active = 1;
END;
CREATE TRIGGER knowledge_document_chunk_changed AFTER UPDATE ON document_semantic_chunk_sources
BEGIN
    UPDATE knowledge_sources SET active = 0 WHERE kind = 'document'
        AND document_id = NEW.document_id AND parent_key = CAST(NEW.chunk_index AS TEXT) AND active = 1;
END;
CREATE TRIGGER knowledge_document_chunk_deleted AFTER DELETE ON document_semantic_chunk_sources
BEGIN
    DELETE FROM knowledge_sources WHERE kind = 'document'
        AND document_id = OLD.document_id AND parent_key = CAST(OLD.chunk_index AS TEXT);
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
CREATE TRIGGER knowledge_overlay_inserted AFTER INSERT ON document_study_guide_overlays
BEGIN
    UPDATE knowledge_sources SET active = 0 WHERE kind = 'guide'
        AND document_id = NEW.document_id AND active = 1;
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
