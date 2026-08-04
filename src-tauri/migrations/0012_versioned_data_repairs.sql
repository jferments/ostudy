-- Establish one-time data repair bookkeeping and retry-safe deck import identity.
--
-- Schema migrations remain immutable structural history. Data repairs are
-- versioned separately because some canonicalization requires validated Rust
-- transformations rather than raw SQL. Deck import operations retain their
-- request fingerprint and committed result so an ambiguous transport retry
-- cannot create a second imported deck.

CREATE TABLE data_repairs (
    version       INTEGER PRIMARY KEY,
    name          TEXT NOT NULL,
    checksum      TEXT NOT NULL,
    applied_at    INTEGER NOT NULL,
    changed_rows  INTEGER NOT NULL DEFAULT 0 CHECK(changed_rows >= 0)
) STRICT;

CREATE TABLE deck_import_operations (
    id                     TEXT PRIMARY KEY CHECK(length(id) = 32),
    request_fingerprint    TEXT NOT NULL CHECK(length(request_fingerprint) = 64),
    package_format         TEXT NOT NULL,
    package_version        INTEGER NOT NULL CHECK(package_version >= 1),
    status                 TEXT NOT NULL CHECK(status IN ('pending', 'completed', 'failed')),
    result_deck_id         TEXT REFERENCES decks(id) ON DELETE SET NULL,
    result_card_count      INTEGER CHECK(result_card_count IS NULL OR result_card_count >= 0),
    result_asset_count     INTEGER CHECK(result_asset_count IS NULL OR result_asset_count >= 0),
    error_code             TEXT,
    created_at             INTEGER NOT NULL,
    updated_at             INTEGER NOT NULL,
    completed_at           INTEGER,
    CHECK (
        (status = 'completed'
         AND result_deck_id IS NOT NULL
         AND result_card_count IS NOT NULL
         AND result_asset_count IS NOT NULL
         AND completed_at IS NOT NULL
         AND error_code IS NULL)
        OR
        (status = 'pending'
         AND result_deck_id IS NULL
         AND result_card_count IS NULL
         AND result_asset_count IS NULL
         AND completed_at IS NULL
         AND error_code IS NULL)
        OR
        (status = 'failed'
         AND result_deck_id IS NULL
         AND result_card_count IS NULL
         AND result_asset_count IS NULL
         AND completed_at IS NULL
         AND error_code IS NOT NULL)
    )
) STRICT;

CREATE INDEX deck_import_operations_status_idx
    ON deck_import_operations(status, updated_at);
