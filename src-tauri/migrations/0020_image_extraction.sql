-- Immutable, application-owned OCR results. Content equivalence does not convey
-- source ownership: each retained image member has its own association.
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

CREATE TABLE document_image_extraction_owners (
    member_id TEXT NOT NULL REFERENCES document_image_members(id) ON DELETE CASCADE,
    extraction_key TEXT NOT NULL REFERENCES document_image_extractions(extraction_key) ON DELETE RESTRICT,
    PRIMARY KEY (member_id, extraction_key)
) STRICT;

CREATE INDEX document_image_extraction_owners_by_key
ON document_image_extraction_owners(extraction_key);

-- Removing one source must preserve another source's shared extraction. Removing
-- the last owner erases the retained OCR rather than leaving deleted source text.
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
