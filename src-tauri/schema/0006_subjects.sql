-- Release migration 6: subjects.
--
-- A subject is one course: it holds the course's documents, guides and decks.
-- Every deck and every document belongs to at most one subject (otherwise it is
-- under "No subject"), and subjects do not nest. Subjects reuse the
-- document_collections table and its ids, because study targets, collection
-- guides, knowledge views and generation operations already reference it.
--
-- The migration turns deck folders and document collections into subjects,
-- files every deck and document, and folds collection study-bundle copies back
-- into their original cards. A copy is retired (deleted_at) and recorded in
-- card_merges; its review history is never moved or rewritten, and readers
-- count it under the surviving card. Every choice the upgrade had to make is
-- written to subject_filing_reviews, the Library's "Check these" list.
--
-- Deterministic: no clock reads and no random ids. Every value written is a
-- function of rows that existed before the migration ran, so two devices that
-- upgrade the same synchronized rows reach the same result.

-- 1. Subjects are document_collections without the unused parent_id. SQLite
--    refuses to drop a column that an index or trigger names, so those go first.
DROP TRIGGER document_collections_one_level_insert;
DROP TRIGGER document_collections_one_level_update;
DROP INDEX document_collections_parent_order;
ALTER TABLE document_collections DROP COLUMN parent_id;

-- 2. Every deck folder becomes a subject. A folder merges into the collection
--    with the same name only when the match is unambiguous: case and
--    surrounding spaces are ignored, and the name is used by exactly one folder
--    and exactly one collection. A folder keeps its own id as the subject id;
--    an id that is not 32 characters long (none is expected) is mapped to a
--    hex-derived one deterministically.
CREATE TEMP TABLE subject_folder_map (
    folder_id  TEXT PRIMARY KEY,
    subject_id TEXT NOT NULL,
    merged     INTEGER NOT NULL
);
INSERT INTO subject_folder_map(folder_id, subject_id, merged)
WITH folder_key AS (
    SELECT id,
           CASE WHEN length(id) = 32 THEN id
                ELSE substr(lower(hex(id)) || '00000000000000000000000000000000', 1, 32)
           END AS mapped_id,
           lower(trim(name)) AS name_key
    FROM deck_folders),
collection_key AS (
    SELECT id, lower(trim(name)) AS name_key FROM document_collections),
unique_folder_key AS (
    SELECT name_key FROM folder_key GROUP BY name_key HAVING COUNT(*) = 1),
unique_collection_key AS (
    SELECT name_key FROM collection_key GROUP BY name_key HAVING COUNT(*) = 1)
SELECT f.id, COALESCE(c.id, f.mapped_id), c.id IS NOT NULL
FROM folder_key f
LEFT JOIN collection_key c
       ON c.name_key = f.name_key
      AND f.name_key IN (SELECT name_key FROM unique_folder_key)
      AND c.name_key IN (SELECT name_key FROM unique_collection_key);

-- Subject order: folders in their library order, then the collections that no
-- folder merged with, in their own order.
CREATE TEMP TABLE subject_order (
    subject_id TEXT PRIMARY KEY,
    sort_order INTEGER NOT NULL
);
INSERT INTO subject_order(subject_id, sort_order)
SELECT subject_id,
       ROW_NUMBER() OVER (ORDER BY grp, sort_order, name COLLATE NOCASE, created_at, tie) * 1000
FROM (
    SELECT m.subject_id, 0 AS grp, f.sort_order, f.name, f.created_at, f.id AS tie
    FROM deck_folders f JOIN subject_folder_map m ON m.folder_id = f.id
    UNION ALL
    SELECT c.id, 1, c.sort_order, c.name, c.created_at, c.id
    FROM document_collections c
    WHERE c.id NOT IN (SELECT subject_id FROM subject_folder_map)
);

INSERT INTO document_collections(id, name, sort_order, created_at, updated_at, revision)
SELECT m.subject_id, trim(f.name), o.sort_order, f.created_at, f.updated_at, f.revision
FROM deck_folders f
JOIN subject_folder_map m ON m.folder_id = f.id AND m.merged = 0
JOIN subject_order o ON o.subject_id = m.subject_id
ORDER BY o.sort_order;

UPDATE document_collections
SET sort_order = (SELECT o.sort_order FROM subject_order o
                  WHERE o.subject_id = document_collections.id)
WHERE sort_order IS NOT (SELECT o.sort_order FROM subject_order o
                         WHERE o.subject_id = document_collections.id);

-- 3. Documents. A document in several collections stays in the one it joined
--    first; the others lose it. A document in no collection follows the folder
--    of its generated deck.
ALTER TABLE documents ADD COLUMN subject_id TEXT
    REFERENCES document_collections(id) ON DELETE SET NULL;

CREATE TEMP TABLE subject_document_map (
    document_id      TEXT PRIMARY KEY,
    subject_id       TEXT NOT NULL,
    membership_count INTEGER NOT NULL,
    placed_by        TEXT NOT NULL
);
INSERT INTO subject_document_map(document_id, subject_id, membership_count, placed_by)
SELECT document_id, collection_id, membership_count, 'membership'
FROM (
    SELECT document_id, collection_id,
           ROW_NUMBER() OVER (PARTITION BY document_id ORDER BY created_at, id) AS position,
           COUNT(*) OVER (PARTITION BY document_id) AS membership_count
    FROM document_collection_memberships)
WHERE position = 1;
INSERT INTO subject_document_map(document_id, subject_id, membership_count, placed_by)
SELECT d.id, m.subject_id, 0, 'generated_deck'
FROM documents d
JOIN decks k ON k.id = d.generated_deck_id AND k.deleted_at IS NULL
JOIN subject_folder_map m ON m.folder_id = k.folder_id
WHERE d.id NOT IN (SELECT document_id FROM subject_document_map);

UPDATE documents
SET subject_id = (SELECT s.subject_id FROM subject_document_map s
                  WHERE s.document_id = documents.id)
WHERE id IN (SELECT document_id FROM subject_document_map);

-- 4. Page ranges (decision C): a document keeps the ranges it had in the
--    collection it stays in, as a "study only these pages" option. Range ids are
--    kept. Ranges set in the collections that lost the document are dropped and
--    counted on its filing row below.
CREATE TABLE document_study_ranges (
    id            TEXT PRIMARY KEY CHECK(length(id) = 32),
    document_id   TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    unit_kind     TEXT NOT NULL CHECK(unit_kind IN (
                      'page', 'slide', 'chapter', 'section', 'segment', 'image', 'time_segment'
                  )),
    start_unit    INTEGER NOT NULL CHECK(start_unit >= 1),
    end_unit      INTEGER NOT NULL CHECK(end_unit >= start_unit),
    source_sha256 TEXT NOT NULL CHECK(length(source_sha256) = 64),
    created_at    INTEGER NOT NULL,
    UNIQUE(document_id, unit_kind, start_unit, end_unit)
) STRICT;
INSERT INTO document_study_ranges(id, document_id, unit_kind, start_unit, end_unit,
                                  source_sha256, created_at)
SELECT r.id, m.document_id, r.unit_kind, r.start_unit, r.end_unit, r.source_sha256, r.created_at
FROM document_collection_ranges r
JOIN document_collection_memberships m ON m.id = r.membership_id
JOIN documents d ON d.id = m.document_id AND d.subject_id = m.collection_id
ORDER BY r.id;

-- 5. Decks. A study-bundle deck goes to its collection's subject. A deck that
--    documents link to (generated or merged into it) goes to their subject; when
--    they are in several subjects it stays in its folder's subject if that is
--    one of them, and otherwise follows the document linked first. Every other
--    deck stays in its folder's subject. Decks that left their folder's subject
--    are listed.
ALTER TABLE decks ADD COLUMN subject_id TEXT
    REFERENCES document_collections(id) ON DELETE SET NULL;

CREATE TEMP TABLE subject_deck_link (
    deck_id    TEXT NOT NULL,
    subject_id TEXT NOT NULL,
    linked_at  INTEGER NOT NULL,
    PRIMARY KEY(deck_id, subject_id)
);
INSERT INTO subject_deck_link(deck_id, subject_id, linked_at)
SELECT deck_id, subject_id, MIN(linked_at) FROM (
    SELECT d.generated_deck_id AS deck_id, d.subject_id, d.created_at AS linked_at
    FROM documents d
    WHERE d.generated_deck_id IS NOT NULL AND d.subject_id IS NOT NULL
    UNION ALL
    SELECT dd.deck_id, d.subject_id, dd.created_at
    FROM document_decks dd JOIN documents d ON d.id = dd.document_id
    WHERE d.subject_id IS NOT NULL)
GROUP BY deck_id, subject_id;

CREATE TEMP TABLE subject_deck_map (
    deck_id           TEXT PRIMARY KEY,
    subject_id        TEXT,
    folder_subject_id TEXT,
    reason            TEXT
);
INSERT INTO subject_deck_map(deck_id, subject_id, folder_subject_id, reason)
WITH folder AS (
    SELECT k.id AS deck_id, m.subject_id
    FROM decks k JOIN subject_folder_map m ON m.folder_id = k.folder_id),
bundle AS (
    SELECT deck_id, collection_id AS subject_id,
           ROW_NUMBER() OVER (PARTITION BY deck_id ORDER BY created_at, id) AS position,
           COUNT(*) OVER (PARTITION BY deck_id) AS bundle_count
    FROM study_targets
    WHERE target_kind = 'collection' AND deck_id IS NOT NULL),
link AS (
    SELECT deck_id, subject_id,
           ROW_NUMBER() OVER (PARTITION BY deck_id ORDER BY linked_at, subject_id) AS position,
           COUNT(*) OVER (PARTITION BY deck_id) AS subject_count
    FROM subject_deck_link)
SELECT k.id,
       CASE
           WHEN b.deck_id IS NOT NULL THEN b.subject_id
           WHEN l.subject_count = 1 THEN l.subject_id
           WHEN l.subject_count > 1
                AND EXISTS (SELECT 1 FROM subject_deck_link x
                            WHERE x.deck_id = k.id AND x.subject_id = f.subject_id)
                THEN f.subject_id
           WHEN l.subject_count > 1 THEN l.subject_id
           ELSE f.subject_id
       END,
       f.subject_id,
       CASE
           WHEN b.bundle_count > 1 THEN 'bundle_of_several_collections'
           WHEN b.deck_id IS NOT NULL AND f.subject_id IS NOT NULL
                AND f.subject_id <> b.subject_id THEN 'moved_to_collection_subject'
           WHEN b.deck_id IS NOT NULL THEN NULL
           WHEN l.subject_count > 1 THEN 'documents_in_several_subjects'
           WHEN l.subject_count = 1 AND f.subject_id IS NOT NULL
                AND f.subject_id <> l.subject_id THEN 'moved_to_document_subject'
           ELSE NULL
       END
FROM decks k
LEFT JOIN folder f ON f.deck_id = k.id
LEFT JOIN bundle b ON b.deck_id = k.id AND b.position = 1
LEFT JOIN link l ON l.deck_id = k.id AND l.position = 1;

UPDATE decks
SET subject_id = (SELECT m.subject_id FROM subject_deck_map m WHERE m.deck_id = decks.id)
WHERE (SELECT m.subject_id FROM subject_deck_map m WHERE m.deck_id = decks.id) IS NOT NULL;

-- 6. Collection copies fold back into their original cards. A copy note merges
--    only when it has exactly one origin, is live and unedited (revision 1), its
--    original is live in a deck that is neither deleted nor archived while the
--    copy is studied, and every live copy card pairs with a live original card
--    by direction and template ordinal. Other copies stay as the subject's own
--    cards (decision B) and are listed.
CREATE TABLE card_merges (
    retired_card_id   TEXT PRIMARY KEY REFERENCES cards(id) ON DELETE CASCADE
                      CHECK(length(retired_card_id) = 32),
    surviving_card_id TEXT NOT NULL REFERENCES cards(id)
                      CHECK(length(surviving_card_id) = 32),
    retired_note_id   TEXT NOT NULL CHECK(length(retired_note_id) = 32),
    surviving_note_id TEXT NOT NULL CHECK(length(surviving_note_id) = 32),
    reason            TEXT NOT NULL CHECK(reason IN ('collection_copy')),
    copied_at         INTEGER NOT NULL,
    CHECK(retired_card_id <> surviving_card_id),
    CHECK(retired_note_id <> surviving_note_id)
) STRICT;
CREATE INDEX card_merges_surviving ON card_merges(surviving_card_id);

-- Merges are one level deep: a retired card never survives another merge and a
-- survivor is never retired. Each row names the notes that own its two cards.
CREATE TRIGGER card_merges_one_level_insert BEFORE INSERT ON card_merges
WHEN EXISTS (SELECT 1 FROM card_merges WHERE retired_card_id = NEW.surviving_card_id)
  OR EXISTS (SELECT 1 FROM card_merges WHERE surviving_card_id = NEW.retired_card_id)
BEGIN
    SELECT RAISE(ABORT, 'card_merges_must_be_one_level');
END;
CREATE TRIGGER card_merges_one_level_update BEFORE UPDATE ON card_merges
WHEN EXISTS (SELECT 1 FROM card_merges
             WHERE retired_card_id = NEW.surviving_card_id
               AND retired_card_id <> OLD.retired_card_id)
  OR EXISTS (SELECT 1 FROM card_merges
             WHERE surviving_card_id = NEW.retired_card_id
               AND retired_card_id <> OLD.retired_card_id)
BEGIN
    SELECT RAISE(ABORT, 'card_merges_must_be_one_level');
END;
CREATE TRIGGER card_merges_identity_insert BEFORE INSERT ON card_merges
WHEN NOT EXISTS (SELECT 1 FROM cards
                 WHERE id = NEW.retired_card_id AND note_id = NEW.retired_note_id)
  OR NOT EXISTS (SELECT 1 FROM cards
                 WHERE id = NEW.surviving_card_id AND note_id = NEW.surviving_note_id)
BEGIN
    SELECT RAISE(ABORT, 'card_merges_identity_mismatch');
END;
CREATE TRIGGER card_merges_identity_update BEFORE UPDATE ON card_merges
WHEN NOT EXISTS (SELECT 1 FROM cards
                 WHERE id = NEW.retired_card_id AND note_id = NEW.retired_note_id)
  OR NOT EXISTS (SELECT 1 FROM cards
                 WHERE id = NEW.surviving_card_id AND note_id = NEW.surviving_note_id)
BEGIN
    SELECT RAISE(ABORT, 'card_merges_identity_mismatch');
END;

CREATE TEMP TABLE subject_copy_origin (
    copy_note_id     TEXT PRIMARY KEY,
    original_note_id TEXT NOT NULL,
    origin_count     INTEGER NOT NULL,
    copied_at        INTEGER NOT NULL
);
INSERT INTO subject_copy_origin(copy_note_id, original_note_id, origin_count, copied_at)
SELECT target_note_id, MIN(source_note_id), COUNT(DISTINCT source_note_id), MIN(created_at)
FROM study_target_card_origins
GROUP BY target_note_id;

CREATE TEMP TABLE subject_copy_note (
    copy_note_id     TEXT PRIMARY KEY,
    original_note_id TEXT NOT NULL,
    copied_at        INTEGER NOT NULL,
    copy_deck_id     TEXT NOT NULL,
    original_deck_id TEXT,
    disposition      TEXT NOT NULL
);
INSERT INTO subject_copy_note(copy_note_id, original_note_id, copied_at, copy_deck_id,
                              original_deck_id, disposition)
SELECT x.copy_note_id, x.original_note_id, x.copied_at, copy.deck_id, original.deck_id,
       CASE
           WHEN x.origin_count > 1
                OR x.original_note_id IN (SELECT copy_note_id FROM subject_copy_origin)
                THEN 'ambiguous_origin'
           WHEN copy.deleted_at IS NOT NULL OR copy_deck.deleted_at IS NOT NULL
                THEN 'copy_deleted'
           WHEN copy.revision > 1 THEN 'copy_edited'
           WHEN original.id IS NULL OR original.deleted_at IS NOT NULL
                OR original_deck.deleted_at IS NOT NULL
                THEN 'original_missing'
           WHEN original_deck.archived_at IS NOT NULL AND copy_deck.archived_at IS NULL
                THEN 'original_archived'
           WHEN NOT EXISTS (SELECT 1 FROM cards c
                            WHERE c.note_id = copy.id AND c.deleted_at IS NULL)
                OR EXISTS (
                    SELECT 1 FROM cards c
                    WHERE c.note_id = copy.id AND c.deleted_at IS NULL
                      AND NOT EXISTS (
                          SELECT 1 FROM cards o
                          WHERE o.note_id = original.id AND o.deleted_at IS NULL
                            AND o.direction = c.direction
                            AND o.template_ordinal = c.template_ordinal))
                THEN 'card_mismatch'
           ELSE 'merge'
       END
FROM subject_copy_origin x
JOIN notes copy ON copy.id = x.copy_note_id
JOIN decks copy_deck ON copy_deck.id = copy.deck_id
LEFT JOIN notes original ON original.id = x.original_note_id
LEFT JOIN decks original_deck ON original_deck.id = original.deck_id;

CREATE TEMP TABLE subject_card_merge (
    retired_card_id   TEXT PRIMARY KEY,
    surviving_card_id TEXT NOT NULL,
    retired_note_id   TEXT NOT NULL,
    surviving_note_id TEXT NOT NULL,
    copied_at         INTEGER NOT NULL
);
CREATE INDEX subject_card_merge_surviving ON subject_card_merge(surviving_card_id);
INSERT INTO subject_card_merge(retired_card_id, surviving_card_id, retired_note_id,
                               surviving_note_id, copied_at)
SELECT copy_card.id, original_card.id, n.copy_note_id, n.original_note_id, n.copied_at
FROM subject_copy_note n
JOIN cards copy_card
  ON copy_card.note_id = n.copy_note_id AND copy_card.deleted_at IS NULL
JOIN cards original_card
  ON original_card.note_id = n.original_note_id AND original_card.deleted_at IS NULL
 AND original_card.direction = copy_card.direction
 AND original_card.template_ordinal = copy_card.template_ordinal
WHERE n.disposition = 'merge';

-- The survivor keeps the whole schedule of the pair member that was studied,
-- reset or rescheduled most recently, from the review and schedule-action
-- ledgers. card_schedules.last_review_at is not used: a new schedule stores
-- its creation time there. Ties go to the original card.
CREATE TEMP TABLE subject_card_activity (
    card_id     TEXT PRIMARY KEY,
    activity_at INTEGER NOT NULL
);
INSERT INTO subject_card_activity(card_id, activity_at)
SELECT card_id, MAX(activity_at) FROM (
    SELECT card_id, reviewed_at AS activity_at FROM review_events
    WHERE card_id IN (SELECT retired_card_id FROM subject_card_merge
                      UNION SELECT surviving_card_id FROM subject_card_merge)
    UNION ALL
    SELECT card_id, occurred_at FROM schedule_action_events
    WHERE action IN ('reset', 'reschedule')
      AND card_id IN (SELECT retired_card_id FROM subject_card_merge
                      UNION SELECT surviving_card_id FROM subject_card_merge))
GROUP BY card_id;

-- The survivor's schedule revision moves past every member's, so a client
-- holding any member's revision sees the change.
CREATE TEMP TABLE subject_schedule_winner (
    survivor_id   TEXT PRIMARY KEY,
    winner_id     TEXT NOT NULL,
    next_revision INTEGER NOT NULL
);
INSERT INTO subject_schedule_winner(survivor_id, winner_id, next_revision)
WITH candidate AS (
    SELECT DISTINCT surviving_card_id AS survivor_id, surviving_card_id AS card_id,
           0 AS is_copy
    FROM subject_card_merge
    UNION ALL
    SELECT surviving_card_id, retired_card_id, 1 FROM subject_card_merge)
SELECT survivor_id, card_id, next_revision FROM (
    SELECT c.survivor_id, c.card_id,
           ROW_NUMBER() OVER (PARTITION BY c.survivor_id
                              ORDER BY COALESCE(a.activity_at, -1) DESC, c.is_copy, c.card_id)
               AS position,
           1 + MAX(s.revision) OVER (PARTITION BY c.survivor_id) AS next_revision
    FROM candidate c
    JOIN card_schedules s ON s.card_id = c.card_id
    LEFT JOIN subject_card_activity a ON a.card_id = c.card_id)
WHERE position = 1;

UPDATE card_schedules
SET (state, due_at, stability, difficulty, elapsed_days, scheduled_days, reps, lapses,
     last_review_at, scheduler_name, scheduler_version, updated_at, revision) = (
        SELECT w.state, w.due_at, w.stability, w.difficulty, w.elapsed_days,
               w.scheduled_days, w.reps, w.lapses, w.last_review_at,
               w.scheduler_name, w.scheduler_version, w.updated_at, x.next_revision
        FROM subject_schedule_winner x
        JOIN card_schedules w ON w.card_id = x.winner_id
        WHERE x.survivor_id = card_schedules.card_id)
WHERE card_id IN (SELECT survivor_id FROM subject_schedule_winner
                  WHERE winner_id <> survivor_id);

INSERT INTO card_merges(retired_card_id, surviving_card_id, retired_note_id,
                        surviving_note_id, reason, copied_at)
SELECT retired_card_id, surviving_card_id, retired_note_id, surviving_note_id,
       'collection_copy', copied_at
FROM subject_card_merge
ORDER BY retired_card_id;

-- Tags a learner added to an unedited copy (tagging does not change a note's
-- revision) carry over to the surviving note.
INSERT OR IGNORE INTO note_tags(note_id, tag_id)
SELECT DISTINCT n.original_note_id, t.tag_id
FROM subject_copy_note n
JOIN note_tags t ON t.note_id = n.copy_note_id
WHERE n.disposition = 'merge'
ORDER BY n.original_note_id, t.tag_id;

-- A retired copy keeps its review history and moves into its original's deck,
-- so purging the emptied study-bundle deck later cannot delete history that now
-- counts under the original. Each retire time is the row's own last update, so
-- it is deterministic. A copy card that was already deleted moves with its note
-- and stays as it was.
UPDATE cards
SET deleted_at = CASE WHEN id IN (SELECT retired_card_id FROM subject_card_merge)
                      THEN updated_at ELSE deleted_at END,
    deck_id = (SELECT n.original_deck_id FROM subject_copy_note n
               WHERE n.copy_note_id = cards.note_id)
WHERE note_id IN (SELECT copy_note_id FROM subject_copy_note WHERE disposition = 'merge');
UPDATE notes
SET deleted_at = updated_at, revision = revision + 1,
    deck_id = (SELECT n.original_deck_id FROM subject_copy_note n
               WHERE n.copy_note_id = notes.id)
WHERE id IN (SELECT copy_note_id FROM subject_copy_note WHERE disposition = 'merge');

-- A study-bundle deck that the merge left without live notes is archived, as
-- deck merging does with an emptied source deck.
UPDATE decks SET archived_at = updated_at, revision = revision + 1
WHERE id IN (SELECT deck_id FROM study_targets
             WHERE target_kind = 'collection' AND deck_id IS NOT NULL)
  AND archived_at IS NULL AND deleted_at IS NULL
  AND EXISTS (SELECT 1 FROM subject_copy_note n
              WHERE n.disposition = 'merge' AND n.copy_deck_id = decks.id)
  AND NOT EXISTS (SELECT 1 FROM notes x WHERE x.deck_id = decks.id AND x.deleted_at IS NULL);

-- 7. The "Check these" list: every subject, deck, document and kept copy where
--    the upgrade had to choose. Device sync adds the filing and copy conflicts
--    it settles. The list stays on this device and is never synced.
CREATE TABLE subject_filing_reviews (
    entity_kind  TEXT NOT NULL CHECK(entity_kind IN ('subject', 'deck', 'document', 'note')),
    entity_id    TEXT NOT NULL CHECK(length(entity_id) BETWEEN 1 AND 128),
    reason       TEXT NOT NULL CHECK(reason IN (
                     'same_name_not_merged',
                     'bundle_of_several_collections',
                     'moved_to_collection_subject',
                     'documents_in_several_subjects',
                     'moved_to_document_subject',
                     'in_several_collections',
                     'followed_generated_deck',
                     'copy_edited',
                     'original_missing',
                     'original_archived',
                     'card_mismatch',
                     'ambiguous_origin',
                     'sync_filing_conflict',
                     'sync_edited_retired_copy'
                 )),
    detail_json  TEXT NOT NULL DEFAULT '{}'
                 CHECK(json_valid(detail_json) AND json_type(detail_json) = 'object'
                       AND length(detail_json) <= 65536),
    dismissed_at INTEGER,
    PRIMARY KEY(entity_kind, entity_id)
) STRICT;

-- A folder and a collection share a name but did not merge, because a folder
-- or a collection used the name twice.
INSERT INTO subject_filing_reviews(entity_kind, entity_id, reason, detail_json)
WITH origin AS (
    SELECT c.id, lower(trim(c.name)) AS name_key,
           CASE WHEN c.id IN (SELECT subject_id FROM subject_folder_map WHERE merged = 0)
                THEN 'folder' ELSE 'collection' END AS origin
    FROM document_collections c),
ambiguous AS (
    SELECT name_key FROM origin GROUP BY name_key
    HAVING SUM(origin = 'folder') > 0 AND SUM(origin = 'collection') > 0)
SELECT 'subject', id, 'same_name_not_merged', json_object('origin', origin)
FROM origin
WHERE name_key IN (SELECT name_key FROM ambiguous)
ORDER BY id;

-- A live deck that left its folder's subject, or whose subject was one of
-- several candidates, names its folder's subject and the candidates in order.
INSERT INTO subject_filing_reviews(entity_kind, entity_id, reason, detail_json)
SELECT 'deck', m.deck_id, m.reason,
       json_object(
           'folderSubjectId', m.folder_subject_id,
           'subjectIds', json(CASE m.reason
               WHEN 'bundle_of_several_collections' THEN (
                   SELECT json_group_array(t.collection_id ORDER BY t.created_at, t.id)
                   FROM study_targets t
                   WHERE t.target_kind = 'collection' AND t.deck_id = m.deck_id)
               WHEN 'documents_in_several_subjects' THEN (
                   SELECT json_group_array(l.subject_id ORDER BY l.linked_at, l.subject_id)
                   FROM subject_deck_link l WHERE l.deck_id = m.deck_id)
               ELSE json_array(m.subject_id)
           END))
FROM subject_deck_map m JOIN decks k ON k.id = m.deck_id
WHERE m.reason IS NOT NULL AND k.deleted_at IS NULL
ORDER BY m.deck_id;

-- A document that lost collections names them in the order it joined them,
-- with the number of page ranges set there that were dropped.
INSERT INTO subject_filing_reviews(entity_kind, entity_id, reason, detail_json)
SELECT 'document', s.document_id, 'in_several_collections',
       json_object(
           'droppedSubjectIds', json((
               SELECT json_group_array(m.collection_id ORDER BY m.created_at, m.id)
               FROM document_collection_memberships m
               WHERE m.document_id = s.document_id AND m.collection_id <> s.subject_id)),
           'droppedRangeCount', (
               SELECT COUNT(*) FROM document_collection_ranges r
               JOIN document_collection_memberships m ON m.id = r.membership_id
               WHERE m.document_id = s.document_id AND m.collection_id <> s.subject_id))
FROM subject_document_map s
WHERE s.membership_count > 1
ORDER BY s.document_id;

INSERT INTO subject_filing_reviews(entity_kind, entity_id, reason, detail_json)
SELECT 'document', s.document_id, 'followed_generated_deck',
       json_object('deckId', d.generated_deck_id)
FROM subject_document_map s JOIN documents d ON d.id = s.document_id
WHERE s.placed_by = 'generated_deck'
ORDER BY s.document_id;

INSERT INTO subject_filing_reviews(entity_kind, entity_id, reason, detail_json)
SELECT 'note', copy_note_id, disposition, json_object('originalNoteId', original_note_id)
FROM subject_copy_note
WHERE disposition IN ('copy_edited', 'original_missing', 'original_archived',
                      'card_mismatch', 'ambiguous_origin')
ORDER BY copy_note_id;

-- 8. Folder and membership structures are fully carried over. A dropped table
--    fires no triggers, so collection knowledge views are marked stale here:
--    every collection's source set has a new identity.
DROP INDEX decks_folder_sort;
ALTER TABLE decks DROP COLUMN folder_id;
DROP TABLE deck_folders;
DROP TABLE document_collection_ranges;
DROP TABLE document_collection_memberships;
UPDATE knowledge_collection_views SET stale = 1 WHERE stale = 0;

CREATE INDEX decks_subject_sort
    ON decks(deleted_at, archived_at, subject_id, sort_order, name COLLATE NOCASE);
CREATE INDEX documents_subject ON documents(subject_id, title COLLATE NOCASE, id);

-- Filing changes replace the membership and range staleness triggers. A
-- deleted document leaves its subject the way a deleted membership did.
CREATE TRIGGER knowledge_collection_view_document_subject_change
AFTER UPDATE OF subject_id ON documents
WHEN OLD.subject_id IS NOT NEW.subject_id
BEGIN
    UPDATE knowledge_collection_views SET stale = 1
    WHERE collection_id IN (OLD.subject_id, NEW.subject_id);
END;
CREATE TRIGGER knowledge_collection_view_document_delete
BEFORE DELETE ON documents WHEN OLD.subject_id IS NOT NULL
BEGIN
    UPDATE knowledge_collection_views SET stale = 1 WHERE collection_id = OLD.subject_id;
END;
-- Study ranges narrow a document inside its subject, so a document that
-- leaves every subject, including by its subject's deletion, loses them.
CREATE TRIGGER document_study_ranges_follow_filing
AFTER UPDATE OF subject_id ON documents
WHEN OLD.subject_id IS NOT NULL AND NEW.subject_id IS NULL
BEGIN
    DELETE FROM document_study_ranges WHERE document_id = NEW.id;
END;
CREATE TRIGGER knowledge_collection_view_study_range_insert
AFTER INSERT ON document_study_ranges BEGIN
    UPDATE knowledge_collection_views SET stale = 1
    WHERE collection_id = (SELECT subject_id FROM documents WHERE id = NEW.document_id);
END;
CREATE TRIGGER knowledge_collection_view_study_range_update
AFTER UPDATE ON document_study_ranges BEGIN
    UPDATE knowledge_collection_views SET stale = 1
    WHERE collection_id IN (SELECT subject_id FROM documents
                            WHERE id IN (OLD.document_id, NEW.document_id));
END;
CREATE TRIGGER knowledge_collection_view_study_range_delete
BEFORE DELETE ON document_study_ranges BEGIN
    UPDATE knowledge_collection_views SET stale = 1
    WHERE collection_id = (SELECT subject_id FROM documents WHERE id = OLD.document_id);
END;

DROP TABLE subject_folder_map;
DROP TABLE subject_order;
DROP TABLE subject_document_map;
DROP TABLE subject_deck_link;
DROP TABLE subject_deck_map;
DROP TABLE subject_copy_origin;
DROP TABLE subject_copy_note;
DROP TABLE subject_card_merge;
DROP TABLE subject_card_activity;
DROP TABLE subject_schedule_winner;
