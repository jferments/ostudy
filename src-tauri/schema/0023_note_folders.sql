-- Release migration 23: Notes folders, and where each notebook sits in them.
--
-- Notes is its own destination, organized the learner's way: folders that
-- may hold folders, and notebooks filed in a folder or left at the top, some
-- of them pinned. This is separate from the subject a notebook is filed in.
-- Deleting a folder deletes the folders inside it, and their notebooks go
-- back to the top; deleting a notebook deletes its place.
--
-- The upgrade creates the tables and writes no rows.
CREATE TABLE note_folders (
    id          TEXT PRIMARY KEY
                CHECK(length(id) = 32 AND id NOT GLOB '*[^0-9a-f]*'),
    parent_id   TEXT REFERENCES note_folders(id) ON DELETE CASCADE,
    name        TEXT NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 120),
    sort_order  INTEGER NOT NULL CHECK(sort_order >= 0),
    created_at  INTEGER NOT NULL CHECK(created_at >= 0),
    updated_at  INTEGER NOT NULL CHECK(updated_at >= created_at),
    CHECK(parent_id IS NULL OR parent_id <> id)
) STRICT;

CREATE INDEX note_folders_by_parent ON note_folders(parent_id, sort_order);

CREATE TABLE notebook_shelf (
    notebook_id  TEXT PRIMARY KEY REFERENCES notebooks(id) ON DELETE CASCADE,
    folder_id    TEXT REFERENCES note_folders(id) ON DELETE SET NULL,
    pinned_at    INTEGER CHECK(pinned_at IS NULL OR pinned_at >= 0),
    updated_at   INTEGER NOT NULL CHECK(updated_at >= 0)
) STRICT;

CREATE INDEX notebook_shelf_by_folder ON notebook_shelf(folder_id);
