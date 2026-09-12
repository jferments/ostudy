-- Content lifecycle and explicit conversation memory controls. No scheduler or
-- review tables are changed. Old messages remain readable after a memory reset.
ALTER TABLE tutor_conversations ADD COLUMN history_enabled INTEGER NOT NULL DEFAULT 1
    CHECK(history_enabled IN (0, 1));
ALTER TABLE tutor_conversations ADD COLUMN retrieval_enabled INTEGER NOT NULL DEFAULT 0
    CHECK(retrieval_enabled IN (0, 1));
ALTER TABLE tutor_conversations ADD COLUMN memory_start_sequence INTEGER NOT NULL DEFAULT 0
    CHECK(memory_start_sequence >= 0);
CREATE TABLE tutor_memory_operations (
    operation_id TEXT PRIMARY KEY CHECK(length(operation_id) = 32),
    conversation_id TEXT NOT NULL REFERENCES tutor_conversations(id) ON DELETE CASCADE,
    request_hash TEXT NOT NULL CHECK(length(request_hash) = 64),
    created_at INTEGER NOT NULL
) STRICT;

CREATE TABLE knowledge_cache_evictions (
    source_id TEXT PRIMARY KEY CHECK(length(source_id) = 32)
) STRICT;

-- Invalidation must discard interpretations even when the visible text hashes
-- identically (e.g. a card type changes, or an asset with the same ID changes).
-- Clearing only active, then republishing the same hash, could revive old units.
DROP TRIGGER knowledge_source_invalidated;
CREATE TRIGGER knowledge_source_invalidated AFTER UPDATE OF active, content_hash ON knowledge_sources
WHEN (NEW.active = 0 AND OLD.active = 1) OR OLD.content_hash <> NEW.content_hash
BEGIN
    UPDATE tutor_conversations SET summary_text = '', summary_through_sequence = memory_start_sequence
    WHERE id IN (SELECT conversation_id FROM tutor_turns WHERE id IN
        (SELECT turn_id FROM tutor_turn_sources WHERE source_id = OLD.id));
    UPDATE tutor_turns SET evidence_current = 0
    WHERE evidence_current = 1 AND id IN (SELECT turn_id FROM tutor_turn_sources WHERE source_id = OLD.id);
    INSERT OR IGNORE INTO knowledge_cache_evictions VALUES (OLD.id);
    DELETE FROM knowledge_artifact_links WHERE source_id = OLD.id;
    DELETE FROM knowledge_units WHERE source_id = OLD.id;
    UPDATE knowledge_sources SET extraction_contract = '', extracted_hash = '' WHERE id = NEW.id;
END;
DROP TRIGGER knowledge_source_deleted;
CREATE TRIGGER knowledge_source_deleted BEFORE DELETE ON knowledge_sources
BEGIN
    INSERT OR IGNORE INTO knowledge_cache_evictions VALUES (OLD.id);
    UPDATE tutor_conversations SET summary_text = '', summary_through_sequence = memory_start_sequence
    WHERE id IN (SELECT conversation_id FROM tutor_turns WHERE id IN
        (SELECT turn_id FROM tutor_turn_sources WHERE source_id = OLD.id));
    UPDATE tutor_turns SET evidence_current = 0
    WHERE evidence_current = 1 AND id IN (SELECT turn_id FROM tutor_turn_sources WHERE source_id = OLD.id);
END;
DROP TRIGGER knowledge_turn_evidence_invalidated;
CREATE TRIGGER knowledge_turn_evidence_invalidated AFTER UPDATE OF evidence_current ON tutor_turns
WHEN OLD.evidence_current = 1 AND NEW.evidence_current = 0
BEGIN
    UPDATE knowledge_sources SET active = 0 WHERE turn_id = NEW.id AND active = 1;
END;

DROP TRIGGER knowledge_note_changed;
CREATE TRIGGER knowledge_note_changed
AFTER UPDATE OF note_type, front_doc, back_doc, front_text, back_text, cloze_source, cloze_mode, deleted_at, deck_id ON notes
WHEN OLD.note_type <> NEW.note_type OR OLD.front_doc <> NEW.front_doc OR OLD.back_doc <> NEW.back_doc
  OR OLD.front_text <> NEW.front_text OR OLD.back_text <> NEW.back_text
  OR OLD.cloze_source <> NEW.cloze_source OR OLD.cloze_mode <> NEW.cloze_mode
  OR OLD.deleted_at IS NOT NEW.deleted_at OR OLD.deck_id <> NEW.deck_id
BEGIN
    UPDATE knowledge_sources SET active = 0 WHERE note_id = NEW.id AND active = 1;
END;
CREATE TRIGGER knowledge_deck_renamed AFTER UPDATE OF name ON decks
WHEN OLD.name <> NEW.name
BEGIN
    UPDATE knowledge_sources SET active = 0
    WHERE note_id IN (SELECT id FROM notes WHERE deck_id = NEW.id) AND active = 1;
END;
CREATE TRIGGER knowledge_document_renamed AFTER UPDATE OF title ON documents
WHEN OLD.title <> NEW.title
BEGIN
    UPDATE knowledge_sources SET active = 0 WHERE document_id = NEW.id AND active = 1;
END;
CREATE TRIGGER knowledge_guide_overview_changed AFTER UPDATE OF overview_text ON document_study_guides
WHEN OLD.overview_text <> NEW.overview_text
BEGIN
    UPDATE knowledge_sources SET active = 0 WHERE guide_id = NEW.id AND parent_key = 'overview' AND active = 1;
END;
CREATE TRIGGER knowledge_guide_topic_changed AFTER UPDATE OF title ON document_learning_plan_topics
WHEN OLD.title <> NEW.title
BEGIN
    UPDATE knowledge_sources SET active = 0 WHERE parent_key = NEW.topic_id AND active = 1
    AND guide_id IN (SELECT id FROM document_study_guides WHERE plan_id = NEW.plan_id);
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
CREATE TRIGGER knowledge_document_asset_changed AFTER UPDATE OF sha256 ON document_source_assets
WHEN OLD.sha256 <> NEW.sha256
BEGIN
    UPDATE knowledge_sources SET active = 0 WHERE document_id = NEW.document_id AND active = 1;
END;
CREATE TRIGGER knowledge_document_asset_deleted BEFORE DELETE ON document_source_assets
BEGIN
    UPDATE knowledge_sources SET active = 0 WHERE document_id = OLD.document_id AND active = 1;
END;

-- A privacy change revokes existing retrieval projections, including copies of
-- old messages. Re-enabling search is local consent, not implicit re-indexing.
CREATE TRIGGER tutor_memory_changed
AFTER UPDATE OF history_enabled, retrieval_enabled, memory_start_sequence ON tutor_conversations
WHEN OLD.history_enabled <> NEW.history_enabled OR OLD.retrieval_enabled <> NEW.retrieval_enabled
  OR OLD.memory_start_sequence <> NEW.memory_start_sequence
BEGIN
    UPDATE tutor_conversations SET summary_text = '', summary_through_sequence = NEW.memory_start_sequence
    WHERE id = NEW.id;
    DELETE FROM knowledge_sources WHERE conversation_id = NEW.id;
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
-- Existing chats require explicit consent under the new sharing contract.
UPDATE knowledge_sources SET active = 0 WHERE kind = 'conversation' AND active = 1;
