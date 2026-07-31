CREATE TABLE decks (
    id              TEXT PRIMARY KEY NOT NULL,
    name            TEXT NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 200),
    description     TEXT NOT NULL DEFAULT '' CHECK(length(description) <= 2000),
    sort_order      INTEGER NOT NULL DEFAULT 0,
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL,
    archived_at     INTEGER,
    deleted_at      INTEGER,
    revision        INTEGER NOT NULL DEFAULT 1 CHECK(revision >= 1)
) STRICT;

CREATE INDEX decks_active_sort
    ON decks(deleted_at, archived_at, sort_order, name COLLATE NOCASE);
