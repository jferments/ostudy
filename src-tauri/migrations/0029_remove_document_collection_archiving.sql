-- Document collections are always active; remove the obsolete archive state.
-- Existing archived collections remain intact and become ordinary visible collections.
UPDATE document_collections
SET archived_at = NULL
WHERE archived_at IS NOT NULL;

ALTER TABLE document_collections DROP COLUMN archived_at;
