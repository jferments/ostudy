-- Release migration 26: notebook boards.
--
-- A board is a notebook page without paper edges: one large sheet the
-- learner pans and zooms around, for mind maps, diagrams and everything else
-- that outgrows a page. It is stored like any other page (its size is the
-- largest a page may be), so its marks, links, pictures and clips need
-- nothing new; this table only says which pages are boards. Deleting the
-- page deletes its row.
--
-- The upgrade creates the table and writes no rows, so every existing page
-- stays a page.
CREATE TABLE notebook_boards (
    page_id     TEXT PRIMARY KEY REFERENCES notebook_pages(id) ON DELETE CASCADE,
    created_at  INTEGER NOT NULL CHECK(created_at >= 0)
) STRICT;
