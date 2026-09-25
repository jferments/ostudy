-- Bind retained collection reducer output to the exact model/prompt request
-- that produced or admitted its native fallback. Existing schema-27 rows remain
-- readable but will be recomputed once before becoming reusable under this key.
ALTER TABLE knowledge_collection_views
ADD COLUMN reduction_request_key TEXT
CHECK(reduction_request_key IS NULL OR length(reduction_request_key) = 64);
