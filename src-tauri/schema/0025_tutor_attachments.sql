-- Release migration 25: pictures and text files attached to a Tutor message.
--
-- A learner can attach up to four pictures and a few text files to a
-- question. Each one is a row here, in the order attached: a picture names
-- the managed asset that holds it, so the asset store keeps it while the
-- message exists, and a text file keeps its text. Deleting the message (or
-- its conversation) removes its rows, and a picture's asset is retired once
-- nothing else uses it.
--
-- The upgrade creates the table and writes no rows.
CREATE TABLE tutor_turn_attachments (
    turn_id   TEXT NOT NULL REFERENCES tutor_turns(id) ON DELETE CASCADE
              CHECK(length(turn_id) = 32),
    position  INTEGER NOT NULL CHECK(position BETWEEN 0 AND 7),
    kind      TEXT NOT NULL CHECK(kind IN ('image', 'text')),
    name      TEXT NOT NULL CHECK(length(name) BETWEEN 1 AND 255),
    asset_id  TEXT REFERENCES assets(id) ON DELETE RESTRICT,
    text      TEXT,
    PRIMARY KEY (turn_id, position),
    CHECK((kind = 'image' AND asset_id IS NOT NULL AND text IS NULL)
       OR (kind = 'text' AND asset_id IS NULL AND text IS NOT NULL))
) STRICT;

CREATE INDEX tutor_turn_attachments_by_asset ON tutor_turn_attachments(asset_id)
    WHERE asset_id IS NOT NULL;
