CREATE TABLE deck_folders (
    id              TEXT PRIMARY KEY NOT NULL,
    name            TEXT NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 120),
    sort_order      INTEGER NOT NULL DEFAULT 0,
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL,
    revision        INTEGER NOT NULL DEFAULT 1 CHECK(revision >= 1)
) STRICT;

ALTER TABLE decks ADD COLUMN folder_id TEXT REFERENCES deck_folders(id) ON DELETE SET NULL;

CREATE INDEX deck_folders_sort
    ON deck_folders(sort_order, name COLLATE NOCASE, created_at);

CREATE INDEX decks_folder_sort
    ON decks(deleted_at, archived_at, folder_id, sort_order, name COLLATE NOCASE);

CREATE TABLE library_organization (
    singleton_id    INTEGER PRIMARY KEY NOT NULL CHECK(singleton_id = 1),
    revision        INTEGER NOT NULL CHECK(revision >= 1),
    updated_at      INTEGER NOT NULL
) STRICT;

INSERT INTO library_organization (singleton_id, revision, updated_at)
SELECT 1, 1, COALESCE(MAX(updated_at), 0)
FROM decks;
