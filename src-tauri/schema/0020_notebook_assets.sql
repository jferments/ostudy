-- Release migration 20: notebook images.
--
-- A picture placed on a notebook page is an 'image' notebook_items row whose
-- payload names a managed asset. This table records that use, one row per
-- image item, so the managed asset store keeps the picture while the item
-- exists: deleting the item, its page or its notebook removes the row, and
-- the asset is retired once nothing else uses it.
--
-- The upgrade creates the table and writes no rows.
CREATE TABLE notebook_item_assets (
    item_id   TEXT PRIMARY KEY REFERENCES notebook_items(id) ON DELETE CASCADE,
    asset_id  TEXT NOT NULL REFERENCES assets(id) ON DELETE RESTRICT
) STRICT;

CREATE INDEX notebook_item_assets_by_asset ON notebook_item_assets(asset_id);
