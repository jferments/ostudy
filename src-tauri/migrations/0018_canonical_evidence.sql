-- Native evidence-v1 backfill runs inside this migration transaction.
-- Semantic payload identity is independent of presentation order and occurrence.
CREATE TABLE document_evidence_payloads (
    sha256 TEXT PRIMARY KEY CHECK(length(sha256) = 64),
    payload_json TEXT NOT NULL CHECK(json_valid(payload_json))
) STRICT;
CREATE TRIGGER document_evidence_payload_immutable
BEFORE UPDATE ON document_evidence_payloads
BEGIN
    SELECT RAISE(ABORT, 'Canonical evidence payloads are immutable');
END;

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
CREATE INDEX document_evidence_payload_owners
ON document_evidence_occurrences(payload_sha256);
CREATE INDEX document_evidence_source_units
ON document_evidence_occurrences(document_id, source_unit_number);

-- Unresolved links retain the exact extractor-local target, not an invented
-- reference. Cross-document authority is never inferred from a shared payload.
CREATE TABLE document_evidence_links (
    occurrence_id TEXT NOT NULL REFERENCES document_evidence_occurrences(id) ON DELETE CASCADE,
    target_node INTEGER NOT NULL CHECK(target_node >= 0 AND target_node < 1000000),
    kind TEXT NOT NULL CHECK(kind IN ('footnote', 'citation', 'internal_link', 'caption', 'label', 'contents', 'cross_reference')),
    target_occurrence_id TEXT REFERENCES document_evidence_occurrences(id) ON DELETE SET NULL,
    PRIMARY KEY(occurrence_id, target_node, kind)
) STRICT;
CREATE INDEX document_evidence_link_targets ON document_evidence_links(target_occurrence_id);
CREATE TRIGGER document_evidence_link_authority
BEFORE INSERT ON document_evidence_links WHEN NEW.target_occurrence_id IS NOT NULL
BEGIN
    SELECT CASE WHEN
        (SELECT document_id FROM document_evidence_occurrences WHERE id = NEW.occurrence_id)
        IS NOT
        (SELECT document_id FROM document_evidence_occurrences WHERE id = NEW.target_occurrence_id)
    THEN RAISE(ABORT, 'Evidence links must retain their source document') END;
END;
CREATE TRIGGER document_evidence_payload_release
AFTER DELETE ON document_evidence_occurrences
BEGIN
    DELETE FROM document_evidence_payloads WHERE sha256 = OLD.payload_sha256
      AND NOT EXISTS (SELECT 1 FROM document_evidence_occurrences WHERE payload_sha256 = OLD.payload_sha256);
END;
