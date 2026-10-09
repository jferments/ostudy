-- Release migration 7: the study activity ledger and review rollups.
--
-- Both tables count reviews by local wall-clock date and hour (the review's
-- stored instant plus its stored UTC offset), never by study date. A study date
-- is a wall-clock date and hour read under the "New day starts at" setting, so
-- changing that setting regroups past days exactly, the same way the live
-- history queries do (ADR 0054).
--
-- study_activity_retained is learner evidence. When a finalized deletion purges
-- review history, it keeps that history's totals here, so study days, streaks
-- and study time survive deleting the cards. Day totals are the live reviews
-- plus these rows. Rows are written once per purge and never change.
--
-- review_day_rollups is a rebuildable cache of the history queries' rows: the
-- reviews of each deck's current, undeleted cards (a retired collection copy
-- counts under its surviving card). Only decks listed in
-- review_day_rollup_built_decks have current rows. Triggers append each new
-- review to a built deck and unlist a deck whenever its history could change
-- in any other way; readers rebuild unlisted decks before reading. The
-- migration builds nothing, so every deck is rebuilt on first use.

CREATE TABLE study_activity_retained (
    operation_id      TEXT NOT NULL CHECK(length(operation_id) = 32),
    wall_date         TEXT NOT NULL
                      CHECK(wall_date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
    wall_hour         INTEGER NOT NULL CHECK(wall_hour BETWEEN 0 AND 23),
    review_count      INTEGER NOT NULL CHECK(review_count > 0),
    remembered_count  INTEGER NOT NULL CHECK(remembered_count BETWEEN 0 AND review_count),
    new_card_count    INTEGER NOT NULL CHECK(new_card_count BETWEEN 0 AND review_count),
    study_time_ms     INTEGER NOT NULL CHECK(study_time_ms >= 0),
    PRIMARY KEY(operation_id, wall_date, wall_hour)
) STRICT, WITHOUT ROWID;

CREATE TABLE review_day_rollups (
    deck_id                     TEXT NOT NULL REFERENCES decks(id) ON DELETE CASCADE,
    wall_date                   TEXT NOT NULL,
    wall_hour                   INTEGER NOT NULL,
    review_count                INTEGER NOT NULL,
    new_card_count              INTEGER NOT NULL,
    remembered_count            INTEGER NOT NULL,
    again_count                 INTEGER NOT NULL,
    hard_count                  INTEGER NOT NULL,
    good_count                  INTEGER NOT NULL,
    easy_count                  INTEGER NOT NULL,
    response_time_ms            INTEGER NOT NULL,
    retention_review_count      INTEGER NOT NULL,
    retention_remembered_count  INTEGER NOT NULL,
    PRIMARY KEY(deck_id, wall_date, wall_hour)
) STRICT, WITHOUT ROWID;

CREATE TABLE review_day_rollup_built_decks (
    deck_id  TEXT PRIMARY KEY REFERENCES decks(id) ON DELETE CASCADE
) STRICT, WITHOUT ROWID;

-- A new review of a current card in a built deck.
CREATE TRIGGER review_day_rollups_after_review
AFTER INSERT ON review_event_attributions
BEGIN
    INSERT INTO review_day_rollups(
        deck_id, wall_date, wall_hour, review_count, new_card_count, remembered_count,
        again_count, hard_count, good_count, easy_count, response_time_ms,
        retention_review_count, retention_remembered_count)
    SELECT c.deck_id,
           date((NEW.reviewed_at + NEW.utc_offset_minutes * 60000) / 1000, 'unixepoch'),
           (NEW.reviewed_at + NEW.utc_offset_minutes * 60000) / 3600000 % 24,
           1,
           r.state_before = 0,
           r.answer_correct,
           r.rating = 1, r.rating = 2, r.rating = 3, r.rating = 4,
           r.response_time_ms,
           r.state_before = 2 AND r.due_before <= r.reviewed_at,
           r.state_before = 2 AND r.due_before <= r.reviewed_at AND r.answer_correct = 1
    FROM review_events r
    CROSS JOIN cards c ON c.id = COALESCE(
        (SELECT m.surviving_card_id FROM card_merges m WHERE m.retired_card_id = r.card_id),
        r.card_id)
    CROSS JOIN notes n ON n.id = c.note_id
    WHERE r.id = NEW.review_event_id
      AND c.deleted_at IS NULL
      AND n.deleted_at IS NULL
      AND EXISTS (SELECT 1 FROM review_day_rollup_built_decks b WHERE b.deck_id = c.deck_id)
    ON CONFLICT(deck_id, wall_date, wall_hour) DO UPDATE SET
        review_count = review_count + excluded.review_count,
        new_card_count = new_card_count + excluded.new_card_count,
        remembered_count = remembered_count + excluded.remembered_count,
        again_count = again_count + excluded.again_count,
        hard_count = hard_count + excluded.hard_count,
        good_count = good_count + excluded.good_count,
        easy_count = easy_count + excluded.easy_count,
        response_time_ms = response_time_ms + excluded.response_time_ms,
        retention_review_count = retention_review_count + excluded.retention_review_count,
        retention_remembered_count = retention_remembered_count + excluded.retention_remembered_count;
END;

CREATE TRIGGER review_day_rollups_attribution_update
AFTER UPDATE ON review_event_attributions
BEGIN
    DELETE FROM review_day_rollup_built_decks WHERE deck_id IN (
        SELECT c.deck_id FROM cards c
        WHERE c.id IN (OLD.card_id, NEW.card_id)
           OR c.id IN (SELECT m.surviving_card_id FROM card_merges m
                       WHERE m.retired_card_id IN (OLD.card_id, NEW.card_id)));
END;

CREATE TRIGGER review_day_rollups_attribution_delete
AFTER DELETE ON review_event_attributions
BEGIN
    DELETE FROM review_day_rollup_built_decks WHERE deck_id IN (
        SELECT c.deck_id FROM cards c
        WHERE c.id = OLD.card_id
           OR c.id IN (SELECT m.surviving_card_id FROM card_merges m
                       WHERE m.retired_card_id = OLD.card_id));
END;

CREATE TRIGGER review_day_rollups_review_update
AFTER UPDATE ON review_events
BEGIN
    DELETE FROM review_day_rollup_built_decks WHERE deck_id IN (
        SELECT c.deck_id FROM cards c
        WHERE c.id IN (OLD.card_id, NEW.card_id)
           OR c.id IN (SELECT m.surviving_card_id FROM card_merges m
                       WHERE m.retired_card_id IN (OLD.card_id, NEW.card_id)));
END;

CREATE TRIGGER review_day_rollups_review_delete
AFTER DELETE ON review_events
BEGIN
    DELETE FROM review_day_rollup_built_decks WHERE deck_id IN (
        SELECT c.deck_id FROM cards c
        WHERE c.id = OLD.card_id
           OR c.id IN (SELECT m.surviving_card_id FROM card_merges m
                       WHERE m.retired_card_id = OLD.card_id));
END;

CREATE TRIGGER review_day_rollups_card_change
AFTER UPDATE OF deck_id, note_id, deleted_at ON cards
WHEN OLD.deck_id IS NOT NEW.deck_id
  OR OLD.note_id IS NOT NEW.note_id
  OR (OLD.deleted_at IS NULL) <> (NEW.deleted_at IS NULL)
BEGIN
    DELETE FROM review_day_rollup_built_decks WHERE deck_id IN (OLD.deck_id, NEW.deck_id);
END;

CREATE TRIGGER review_day_rollups_card_delete
AFTER DELETE ON cards
BEGIN
    DELETE FROM review_day_rollup_built_decks WHERE deck_id = OLD.deck_id;
END;

CREATE TRIGGER review_day_rollups_note_change
AFTER UPDATE OF deleted_at ON notes
WHEN (OLD.deleted_at IS NULL) <> (NEW.deleted_at IS NULL)
BEGIN
    DELETE FROM review_day_rollup_built_decks WHERE deck_id IN (
        SELECT c.deck_id FROM cards c WHERE c.note_id = NEW.id);
END;

CREATE TRIGGER review_day_rollups_merge_insert
AFTER INSERT ON card_merges
BEGIN
    DELETE FROM review_day_rollup_built_decks WHERE deck_id IN (
        SELECT c.deck_id FROM cards c
        WHERE c.id IN (NEW.retired_card_id, NEW.surviving_card_id));
END;

CREATE TRIGGER review_day_rollups_merge_update
AFTER UPDATE ON card_merges
BEGIN
    DELETE FROM review_day_rollup_built_decks WHERE deck_id IN (
        SELECT c.deck_id FROM cards c
        WHERE c.id IN (OLD.retired_card_id, OLD.surviving_card_id,
                       NEW.retired_card_id, NEW.surviving_card_id));
END;

CREATE TRIGGER review_day_rollups_merge_delete
AFTER DELETE ON card_merges
BEGIN
    DELETE FROM review_day_rollup_built_decks WHERE deck_id IN (
        SELECT c.deck_id FROM cards c
        WHERE c.id IN (OLD.retired_card_id, OLD.surviving_card_id));
END;
