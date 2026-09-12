-- An outer source UPSERT can override an inner trigger's OR IGNORE policy.
-- Do not attempt duplicate eviction insertion at all. SQLite serializes the
-- write transaction, so the existence predicate and insert are one operation.
-- Keep this additive: already-installed schema 11 databases need the repair too.

DROP TRIGGER knowledge_source_invalidated;
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

DROP TRIGGER knowledge_source_deleted;
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
