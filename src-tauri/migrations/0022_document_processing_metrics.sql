-- Record bounded metrics for the most recent managed-document processing run.
-- Provider token totals are stored only when the provider reports usage; ostudy
-- never substitutes local token estimates for an API-reported total.

ALTER TABLE documents ADD COLUMN processing_started_at INTEGER;
ALTER TABLE documents ADD COLUMN processing_duration_ms INTEGER
    CHECK(processing_duration_ms IS NULL OR processing_duration_ms >= 0);
ALTER TABLE documents ADD COLUMN api_token_count INTEGER
    CHECK(api_token_count IS NULL OR api_token_count >= 0);
ALTER TABLE documents ADD COLUMN api_token_usage_complete INTEGER NOT NULL DEFAULT 0
    CHECK(api_token_usage_complete IN (0, 1));
