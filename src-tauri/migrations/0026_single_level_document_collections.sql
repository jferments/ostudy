-- Document folders are intentionally one level, matching deck organization.
-- Existing pre-release nested rows are flattened in place so schema-25 test
-- libraries migrate forward without rewriting released migration 22.
UPDATE document_collection_organization
SET revision = revision + 1
WHERE singleton_id = 1
  AND EXISTS (
      SELECT 1 FROM document_collections WHERE parent_id IS NOT NULL
  );

UPDATE document_collections
SET parent_id = NULL,
    revision = revision + 1
WHERE parent_id IS NOT NULL;

CREATE TRIGGER document_collections_one_level_insert
BEFORE INSERT ON document_collections
WHEN NEW.parent_id IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'document_collections_parent_must_be_null');
END;

CREATE TRIGGER document_collections_one_level_update
BEFORE UPDATE OF parent_id ON document_collections
WHEN NEW.parent_id IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'document_collections_parent_must_be_null');
END;
