-- Immutable replay receipts for an atomic deck merge. These rows own no cards,
-- decks, or source files; later edits/deletion must not turn retry into a write.
CREATE TABLE deck_merge_operations (
    operation_id      TEXT PRIMARY KEY CHECK(length(operation_id) = 32),
    request_sha256    TEXT NOT NULL CHECK(length(request_sha256) = 64),
    result_json       TEXT NOT NULL CHECK(
        json_valid(result_json) AND length(result_json) BETWEEN 2 AND 32768
    ),
    created_at        INTEGER NOT NULL CHECK(created_at >= 0)
) STRICT;
