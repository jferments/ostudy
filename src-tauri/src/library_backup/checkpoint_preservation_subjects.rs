//! What release migration 6 (subjects) may change, and the proof for each
//! change. Folders and collection memberships become subject filing, collection
//! copies retire into their originals, and the derived knowledge and Tutor
//! caches that cited a retired copy are invalidated by their own triggers.
//! Review evidence stays under `ALWAYS_IDENTICAL`; nothing here can excuse it.
//!
//! Each proof restates the rule it checks from the retained library
//! (`source`) and the upgraded copy (`target`), independently of the migration
//! SQL, and returns the rows that break it.

use super::{Projection, Rule};

/// Views every proof below shares. `retired_*` come from the upgraded
/// library's merge ledger, whose own rows are proved by the `card_merges` rule.
/// `folder_subject` is the subject each folder must become: its own id, or
/// the collection whose name it shares when that name is unambiguous on both
/// sides. `invalidated` is everything a retired copy can invalidate through
/// the knowledge triggers, followed from source to citing turn to that turn's
/// conversation sources.
pub(super) const SETUP: &str = r"
CREATE TEMP VIEW retired_note(id, survivor_id) AS
    SELECT DISTINCT retired_note_id, surviving_note_id FROM target.card_merges;
CREATE TEMP VIEW retired_card(id, survivor_id) AS
    SELECT retired_card_id, surviving_card_id FROM target.card_merges;
CREATE TEMP VIEW folder_subject(folder_id, subject_id, merged) AS
    WITH folder_key AS (
        SELECT id,
               CASE WHEN length(id) = 32 THEN id
                    ELSE substr(lower(hex(id)) || '00000000000000000000000000000000', 1, 32)
               END AS own_id,
               lower(trim(name)) AS name_key
        FROM source.deck_folders),
    collection_key AS (
        SELECT id, lower(trim(name)) AS name_key FROM source.document_collections),
    unique_key AS (
        SELECT name_key FROM folder_key GROUP BY name_key HAVING COUNT(*) = 1
        INTERSECT
        SELECT name_key FROM collection_key GROUP BY name_key HAVING COUNT(*) = 1)
    SELECT f.id, COALESCE(c.id, f.own_id), c.id IS NOT NULL
    FROM folder_key f
    LEFT JOIN collection_key c
           ON c.name_key = f.name_key AND f.name_key IN (SELECT name_key FROM unique_key);
CREATE TEMP VIEW deactivated_source(id) AS
    SELECT s.id FROM source.knowledge_sources s
    JOIN target.knowledge_sources t ON t.id = s.id
    WHERE s.active = 1 AND t.active = 0;
CREATE TEMP VIEW cleared_source(id) AS
    SELECT id FROM deactivated_source
    UNION
    SELECT a.source_id FROM source.knowledge_analysis_sources a
    JOIN source.knowledge_analysis_sources w ON w.window_id = a.window_id
    WHERE w.source_id IN (SELECT id FROM deactivated_source)
    UNION
    SELECT u.source_id FROM source.knowledge_units u
    JOIN source.knowledge_unit_evidence e ON e.unit_id = u.id
    WHERE e.source_id IN (SELECT id FROM deactivated_source);
CREATE TEMP VIEW invalidated(kind, id) AS
    WITH RECURSIVE reached(kind, id) AS (
        SELECT 'source', id FROM source.knowledge_sources
        WHERE note_id IN (SELECT id FROM retired_note)
           OR card_id IN (SELECT id FROM retired_card)
        UNION
        SELECT 'turn', l.turn_id FROM reached r
        JOIN source.tutor_turn_sources l ON l.source_id = r.id
        WHERE r.kind = 'source'
        UNION
        SELECT 'source', k.id FROM reached r
        JOIN source.knowledge_sources k ON k.turn_id = r.id
        WHERE r.kind = 'turn')
    SELECT kind, id FROM reached;
";

/// Every folder becomes the subject it maps to, keeping its name (trimmed),
/// times and revision unless it merged into a collection of the same name.
const FOLDERS_BECOME_SUBJECTS: &str = r"
SELECT f.id FROM source.deck_folders f
JOIN folder_subject m ON m.folder_id = f.id
LEFT JOIN target.document_collections s ON s.id = m.subject_id
WHERE s.id IS NULL
   OR (m.merged = 0 AND (s.name IS NOT trim(f.name) OR s.created_at IS NOT f.created_at
                         OR s.updated_at IS NOT f.updated_at OR s.revision IS NOT f.revision))
";

/// Each membership is the document's subject now, or is named on the
/// document's "Check these" row as a collection it left (decision A).
const MEMBERSHIPS_BECOME_FILING: &str = r"
SELECT m.id FROM source.document_collection_memberships m
JOIN target.documents d ON d.id = m.document_id
WHERE d.subject_id IS NOT m.collection_id
  AND NOT EXISTS (
      SELECT 1 FROM target.subject_filing_reviews r,
                    json_each(r.detail_json, '$.droppedSubjectIds') j
      WHERE r.entity_kind = 'document' AND r.entity_id = m.document_id
        AND r.reason = 'in_several_collections' AND j.value = m.collection_id)
";

/// Ranges set in the collection a document stays in become its study ranges,
/// value for value; ranges set elsewhere are counted on its "Check these" row
/// (decision C).
const RANGES_KEPT_OR_COUNTED: &str = r"
SELECT r.id FROM source.document_collection_ranges r
JOIN source.document_collection_memberships m ON m.id = r.membership_id
JOIN target.documents d ON d.id = m.document_id AND d.subject_id = m.collection_id
WHERE NOT EXISTS (
    SELECT 1 FROM target.document_study_ranges s
    WHERE s.id = r.id AND s.document_id = m.document_id AND s.unit_kind = r.unit_kind
      AND s.start_unit = r.start_unit AND s.end_unit = r.end_unit
      AND s.source_sha256 = r.source_sha256 AND s.created_at = r.created_at)
UNION ALL
SELECT x.document_id FROM (
    SELECT m.document_id, COUNT(r.id) AS ranges
    FROM source.document_collection_memberships m
    JOIN target.documents d ON d.id = m.document_id
    LEFT JOIN source.document_collection_ranges r ON r.membership_id = m.id
    WHERE d.subject_id IS NOT m.collection_id
    GROUP BY m.document_id) x
LEFT JOIN target.subject_filing_reviews f
       ON f.entity_kind = 'document' AND f.entity_id = x.document_id
      AND f.reason = 'in_several_collections'
WHERE f.entity_id IS NULL
   OR json_extract(f.detail_json, '$.droppedRangeCount') IS NOT x.ranges
";

/// A study range is a range the document had in the collection it stays in.
const STUDY_RANGES_FROM_KEPT_MEMBERSHIP: &str = r"
SELECT s.id FROM target.document_study_ranges s
WHERE NOT EXISTS (
    SELECT 1 FROM source.document_collection_ranges r
    JOIN source.document_collection_memberships m ON m.id = r.membership_id
    JOIN target.documents d ON d.id = m.document_id AND d.subject_id = m.collection_id
    WHERE r.id = s.id AND m.document_id = s.document_id AND r.unit_kind = s.unit_kind
      AND r.start_unit = s.start_unit AND r.end_unit = s.end_unit
      AND r.source_sha256 = s.source_sha256 AND r.created_at = s.created_at)
";

/// A merge retires a live card of an unedited collection copy with exactly one
/// origin into the matching live card of that origin, whose deck is neither
/// deleted nor archived while the copy is studied. A note merges whole.
const MERGES_FOLLOW_COPY_ORIGINS: &str = r"
WITH origin AS (
    SELECT target_note_id, MIN(source_note_id) AS original_note_id,
           COUNT(DISTINCT source_note_id) AS origins, MIN(created_at) AS copied_at
    FROM source.study_target_card_origins
    GROUP BY target_note_id)
SELECT x.retired_card_id FROM target.card_merges x
LEFT JOIN origin o ON o.target_note_id = x.retired_note_id
LEFT JOIN source.notes copy ON copy.id = x.retired_note_id
LEFT JOIN source.decks copy_deck ON copy_deck.id = copy.deck_id
LEFT JOIN source.notes original ON original.id = x.surviving_note_id
LEFT JOIN source.decks original_deck ON original_deck.id = original.deck_id
LEFT JOIN source.cards retired ON retired.id = x.retired_card_id
LEFT JOIN source.cards survivor ON survivor.id = x.surviving_card_id
WHERE x.reason IS NOT 'collection_copy'
   OR o.target_note_id IS NULL OR o.origins <> 1
   OR o.original_note_id IS NOT x.surviving_note_id OR o.copied_at IS NOT x.copied_at
   OR x.surviving_note_id IN (SELECT target_note_id FROM source.study_target_card_origins)
   OR copy.revision IS NOT 1 OR copy.deleted_at IS NOT NULL
   OR copy_deck.deleted_at IS NOT NULL
   OR original.id IS NULL OR original.deleted_at IS NOT NULL
   OR original_deck.deleted_at IS NOT NULL
   OR (original_deck.archived_at IS NOT NULL AND copy_deck.archived_at IS NULL)
   OR retired.note_id IS NOT x.retired_note_id OR retired.deleted_at IS NOT NULL
   OR survivor.note_id IS NOT x.surviving_note_id OR survivor.deleted_at IS NOT NULL
   OR survivor.direction IS NOT retired.direction
   OR survivor.template_ordinal IS NOT retired.template_ordinal
UNION ALL
SELECT c.id FROM source.cards c
WHERE c.deleted_at IS NULL AND c.note_id IN (SELECT id FROM retired_note)
  AND c.id NOT IN (SELECT id FROM retired_card)
";

/// The upgrade lists only rows that exist, never dismissed, and never in
/// the reasons device sync reserves for itself.
const FILING_REVIEWS_NAME_REAL_ROWS: &str = r"
SELECT f.entity_id FROM target.subject_filing_reviews f
WHERE f.dismissed_at IS NOT NULL
   OR f.reason IN ('sync_filing_conflict', 'sync_edited_retired_copy')
   OR (f.entity_kind = 'subject'
       AND NOT EXISTS (SELECT 1 FROM target.document_collections WHERE id = f.entity_id))
   OR (f.entity_kind = 'deck'
       AND NOT EXISTS (SELECT 1 FROM target.decks WHERE id = f.entity_id))
   OR (f.entity_kind = 'document'
       AND NOT EXISTS (SELECT 1 FROM target.documents WHERE id = f.entity_id))
   OR (f.entity_kind = 'note'
       AND NOT EXISTS (SELECT 1 FROM target.notes WHERE id = f.entity_id))
";

/// Collections never had a parent (schema 1 refused one), and subjects keep
/// the library's order: folders first, then collections no folder merged with.
const SUBJECTS_KEEP_LIBRARY_ORDER: &str = r"
SELECT id FROM source.document_collections WHERE parent_id IS NOT NULL
UNION ALL
SELECT expected.subject_id FROM (
    SELECT subject_id,
           ROW_NUMBER() OVER (ORDER BY grp, sort_order, name COLLATE NOCASE, created_at, tie)
               AS position
    FROM (SELECT m.subject_id, 0 AS grp, f.sort_order, f.name, f.created_at, f.id AS tie
          FROM source.deck_folders f JOIN folder_subject m ON m.folder_id = f.id
          UNION ALL
          SELECT c.id, 1, c.sort_order, c.name, c.created_at, c.id
          FROM source.document_collections c
          WHERE c.id NOT IN (SELECT subject_id FROM folder_subject))) expected
LEFT JOIN (SELECT id, ROW_NUMBER() OVER (ORDER BY sort_order, id) AS position
           FROM target.document_collections) actual ON actual.id = expected.subject_id
WHERE actual.position IS NOT expected.position
";

/// A study-bundle deck the merge emptied is archived the way deck merging
/// archives an emptied source. A deck is filed under its folder's subject, a
/// collection it bundles, or the subject of a document linked to it, and a
/// live deck that left its folder's subject is on the "Check these" list.
const DECKS_ARCHIVED_AND_FILED: &str = r"
SELECT k.id FROM target.decks k JOIN source.decks s ON s.id = k.id
WHERE (k.archived_at IS NOT s.archived_at OR k.revision IS NOT s.revision)
  AND (s.archived_at IS NOT NULL OR s.deleted_at IS NOT NULL
       OR k.archived_at IS NOT s.updated_at OR k.revision IS NOT s.revision + 1
       OR NOT EXISTS (SELECT 1 FROM source.study_targets x
                      WHERE x.target_kind = 'collection' AND x.deck_id = k.id)
       OR EXISTS (SELECT 1 FROM target.notes n
                  WHERE n.deck_id = k.id AND n.deleted_at IS NULL))
UNION ALL
SELECT k.id FROM target.decks k JOIN source.decks s ON s.id = k.id
LEFT JOIN folder_subject f ON f.folder_id = s.folder_id
WHERE k.subject_id IS NOT f.subject_id
  AND NOT EXISTS (SELECT 1 FROM source.study_targets x
                  WHERE x.target_kind = 'collection' AND x.deck_id = k.id
                    AND x.collection_id = k.subject_id)
  AND NOT EXISTS (SELECT 1 FROM target.documents d
                  WHERE d.subject_id = k.subject_id
                    AND (d.generated_deck_id = k.id
                         OR d.id IN (SELECT document_id FROM source.document_decks
                                     WHERE deck_id = k.id)))
UNION ALL
SELECT k.id FROM target.decks k JOIN source.decks s ON s.id = k.id
JOIN folder_subject f ON f.folder_id = s.folder_id
WHERE k.deleted_at IS NULL AND k.subject_id IS NOT f.subject_id
  AND NOT EXISTS (SELECT 1 FROM target.subject_filing_reviews r
                  WHERE r.entity_kind = 'deck' AND r.entity_id = k.id)
";

/// A document in collections stays in the one it joined first (decision A).
/// A document in none is unfiled, or follows its generated deck's folder and
/// is on the "Check these" list.
const DOCUMENTS_FILED_BY_FIRST_MEMBERSHIP: &str = r"
SELECT d.id FROM target.documents d
JOIN (SELECT document_id, collection_id FROM (
          SELECT document_id, collection_id,
                 ROW_NUMBER() OVER (PARTITION BY document_id ORDER BY created_at, id)
                     AS position
          FROM source.document_collection_memberships)
      WHERE position = 1) first ON first.document_id = d.id
WHERE d.subject_id IS NOT first.collection_id
UNION ALL
SELECT d.id FROM target.documents d
WHERE d.subject_id IS NOT NULL
  AND d.id NOT IN (SELECT document_id FROM source.document_collection_memberships)
  AND NOT (EXISTS (SELECT 1 FROM source.decks k
                   JOIN folder_subject f ON f.folder_id = k.folder_id
                   WHERE k.id = d.generated_deck_id AND k.deleted_at IS NULL
                     AND f.subject_id = d.subject_id)
           AND EXISTS (SELECT 1 FROM target.subject_filing_reviews r
                       WHERE r.entity_kind = 'document' AND r.entity_id = d.id
                         AND r.reason = 'followed_generated_deck'))
";

/// A retired copy note was live and unedited. It is deleted as of its own last
/// update, its revision moves on by one, and it joins its original's deck.
const RETIRED_NOTES_JOIN_THEIR_ORIGINAL: &str = r"
SELECT n.id FROM target.notes n
JOIN source.notes s ON s.id = n.id
JOIN retired_note r ON r.id = n.id
LEFT JOIN target.notes survivor ON survivor.id = r.survivor_id
WHERE s.deleted_at IS NOT NULL OR s.revision IS NOT 1
   OR n.deleted_at IS NOT s.updated_at OR n.revision IS NOT s.revision + 1
   OR survivor.id IS NULL OR n.deck_id IS NOT survivor.deck_id
";

/// Cards follow their retired note into the original's deck. A retired card
/// was live and is deleted as of its own last update; a card the learner had
/// already deleted stays as it was.
const RETIRED_CARDS_FOLLOW_THEIR_NOTE: &str = r"
SELECT c.id FROM target.cards c
JOIN source.cards s ON s.id = c.id
JOIN target.notes n ON n.id = c.note_id
WHERE c.note_id IN (SELECT id FROM retired_note)
  AND (c.deck_id IS NOT n.deck_id
       OR CASE WHEN c.id IN (SELECT id FROM retired_card)
               THEN s.deleted_at IS NOT NULL OR c.deleted_at IS NOT s.updated_at
               ELSE c.deleted_at IS NOT s.deleted_at
          END)
";

/// A survivor keeps the whole schedule of the pair member studied, reset or
/// rescheduled last according to the review and schedule-action ledgers, with
/// ties to the original. Its revision moves past every member's when the
/// schedule came from a copy.
const SURVIVOR_KEEPS_LATEST_SCHEDULE: &str = r"
WITH member AS (
    SELECT DISTINCT survivor_id, survivor_id AS card_id, 0 AS is_copy FROM retired_card
    UNION ALL
    SELECT survivor_id, id, 1 FROM retired_card),
activity AS (
    SELECT card_id, MAX(at) AS at FROM (
        SELECT card_id, reviewed_at AS at FROM source.review_events
        WHERE card_id IN (SELECT card_id FROM member)
        UNION ALL
        SELECT card_id, occurred_at FROM source.schedule_action_events
        WHERE action IN ('reset', 'reschedule')
          AND card_id IN (SELECT card_id FROM member))
    GROUP BY card_id),
ranked AS (
    SELECT m.survivor_id, m.card_id,
           ROW_NUMBER() OVER (PARTITION BY m.survivor_id
                              ORDER BY COALESCE(a.at, -1) DESC, m.is_copy, m.card_id)
               AS position,
           1 + MAX(s.revision) OVER (PARTITION BY m.survivor_id) AS next_revision
    FROM member m
    JOIN source.card_schedules s ON s.card_id = m.card_id
    LEFT JOIN activity a ON a.card_id = m.card_id)
SELECT r.survivor_id FROM ranked r
JOIN source.card_schedules w ON w.card_id = r.card_id
JOIN target.card_schedules t ON t.card_id = r.survivor_id
WHERE r.position = 1
  AND (t.state IS NOT w.state OR t.due_at IS NOT w.due_at
       OR t.stability IS NOT w.stability OR t.difficulty IS NOT w.difficulty
       OR t.elapsed_days IS NOT w.elapsed_days OR t.scheduled_days IS NOT w.scheduled_days
       OR t.reps IS NOT w.reps OR t.lapses IS NOT w.lapses
       OR t.last_review_at IS NOT w.last_review_at
       OR t.scheduler_name IS NOT w.scheduler_name
       OR t.scheduler_version IS NOT w.scheduler_version
       OR t.updated_at IS NOT w.updated_at
       OR t.revision IS NOT CASE WHEN r.card_id = r.survivor_id THEN w.revision
                                 ELSE r.next_revision END)
";

/// Each deck archived by the upgrade moves the deck library revision on once,
/// as the visibility trigger does.
const LIBRARY_REVISION_COUNTS_ARCHIVED_DECKS: &str = r"
WITH visibility AS (
    SELECT k.updated_at FROM target.decks k JOIN source.decks s ON s.id = k.id
    WHERE k.archived_at IS NOT s.archived_at OR k.deleted_at IS NOT s.deleted_at)
SELECT t.singleton_id FROM target.library_organization t
JOIN source.library_organization s ON s.singleton_id = t.singleton_id
WHERE t.revision IS NOT s.revision + (SELECT COUNT(*) FROM visibility)
   OR t.updated_at IS NOT MAX(s.updated_at,
                              COALESCE((SELECT MAX(updated_at) FROM visibility), s.updated_at))
";

/// A source is deactivated only when a retired copy invalidated it, and only
/// sources the knowledge triggers clear lose their extraction hashes.
const SOURCES_INVALIDATED_BY_RETIRED_COPIES: &str = r"
SELECT t.id FROM target.knowledge_sources t
JOIN source.knowledge_sources s ON s.id = t.id
WHERE (t.active IS NOT s.active
       AND (t.active <> 0
            OR NOT EXISTS (SELECT 1 FROM invalidated i
                           WHERE i.kind = 'source' AND i.id = t.id)))
   OR ((t.extraction_contract IS NOT s.extraction_contract
        OR t.extracted_hash IS NOT s.extracted_hash)
       AND (t.extraction_contract <> '' OR t.extracted_hash <> ''))
";

/// Collection views can only become stale.
const COLLECTION_VIEWS_ONLY_GO_STALE: &str = r"
SELECT t.collection_id FROM target.knowledge_collection_views t
JOIN source.knowledge_collection_views s ON s.collection_id = t.collection_id
WHERE t.stale IS NOT s.stale AND t.stale <> 1
";

/// A Tutor turn's evidence can only go out of date.
const TURN_EVIDENCE_ONLY_GOES_STALE: &str = r"
SELECT t.id FROM target.tutor_turns t
JOIN source.tutor_turns s ON s.id = t.id
WHERE t.evidence_current IS NOT s.evidence_current AND t.evidence_current <> 0
";

/// A conversation summary that cited invalidated evidence is cleared back to
/// the conversation's memory start.
const SUMMARIES_ONLY_CLEAR: &str = r"
SELECT t.id FROM target.tutor_conversations t
JOIN source.tutor_conversations s ON s.id = t.id
WHERE (t.summary_text IS NOT s.summary_text
       OR t.summary_through_sequence IS NOT s.summary_through_sequence)
  AND (t.summary_text <> '' OR t.summary_through_sequence IS NOT t.memory_start_sequence)
";

const NO_PROOF: &str = "";

pub(super) const RULES: &[Rule] = &[
    Rule::Dropped {
        table: "deck_folders",
        description: "deck folders",
        proof: FOLDERS_BECOME_SUBJECTS,
    },
    Rule::Dropped {
        table: "document_collection_memberships",
        description: "collection memberships",
        proof: MEMBERSHIPS_BECOME_FILING,
    },
    Rule::Dropped {
        table: "document_collection_ranges",
        description: "collection page ranges",
        proof: RANGES_KEPT_OR_COUNTED,
    },
    Rule::Added {
        table: "document_study_ranges",
        description: "document study ranges",
        proof: STUDY_RANGES_FROM_KEPT_MEMBERSHIP,
    },
    Rule::Added {
        table: "card_merges",
        description: "merged collection copies",
        proof: MERGES_FOLLOW_COPY_ORIGINS,
    },
    Rule::Added {
        table: "subject_filing_reviews",
        description: "the \"Check these\" list",
        proof: FILING_REVIEWS_NAME_REAL_ROWS,
    },
    Rule::Projected(Projection {
        table: "document_collections",
        description: "subjects",
        key: &["id"],
        ignored: &["parent_id"],
        columns: &["sort_order"],
        changed: "SELECT id FROM source.document_collections",
        added: "SELECT subject_id FROM folder_subject WHERE merged = 0",
        removed: NO_PROOF,
        proof: SUBJECTS_KEEP_LIBRARY_ORDER,
    }),
    Rule::Projected(Projection {
        table: "decks",
        description: "decks",
        key: &["id"],
        ignored: &["folder_id", "subject_id"],
        columns: &["archived_at", "revision"],
        changed: "SELECT DISTINCT n.deck_id FROM source.notes n
                  JOIN retired_note r ON r.id = n.id",
        added: NO_PROOF,
        removed: NO_PROOF,
        proof: DECKS_ARCHIVED_AND_FILED,
    }),
    Rule::Projected(Projection {
        table: "documents",
        description: "documents",
        key: &["id"],
        ignored: &["subject_id"],
        columns: &[],
        changed: NO_PROOF,
        added: NO_PROOF,
        removed: NO_PROOF,
        proof: DOCUMENTS_FILED_BY_FIRST_MEMBERSHIP,
    }),
    Rule::Projected(Projection {
        table: "notes",
        description: "notes",
        key: &["id"],
        ignored: &[],
        columns: &["deleted_at", "revision", "deck_id"],
        changed: "SELECT id FROM retired_note",
        added: NO_PROOF,
        removed: NO_PROOF,
        proof: RETIRED_NOTES_JOIN_THEIR_ORIGINAL,
    }),
    Rule::Projected(Projection {
        table: "cards",
        description: "cards",
        key: &["id"],
        ignored: &[],
        columns: &["deleted_at", "deck_id"],
        changed: "SELECT id FROM source.cards WHERE note_id IN (SELECT id FROM retired_note)",
        added: NO_PROOF,
        removed: NO_PROOF,
        proof: RETIRED_CARDS_FOLLOW_THEIR_NOTE,
    }),
    Rule::Projected(Projection {
        table: "card_schedules",
        description: "card schedules",
        key: &["card_id"],
        ignored: &[],
        columns: &[
            "state",
            "due_at",
            "stability",
            "difficulty",
            "elapsed_days",
            "scheduled_days",
            "reps",
            "lapses",
            "last_review_at",
            "scheduler_name",
            "scheduler_version",
            "updated_at",
            "revision",
        ],
        changed: "SELECT DISTINCT survivor_id FROM retired_card",
        added: NO_PROOF,
        removed: NO_PROOF,
        proof: SURVIVOR_KEEPS_LATEST_SCHEDULE,
    }),
    // Tags a learner added to an unedited copy carry over to the original.
    Rule::Projected(Projection {
        table: "note_tags",
        description: "note tags",
        key: &["note_id", "tag_id"],
        ignored: &[],
        columns: &[],
        changed: NO_PROOF,
        added: "SELECT DISTINCT r.survivor_id, t.tag_id FROM retired_note r
                JOIN source.note_tags t ON t.note_id = r.id",
        removed: NO_PROOF,
        proof: NO_PROOF,
    }),
    Rule::Projected(Projection {
        table: "library_organization",
        description: "the deck library order",
        key: &["singleton_id"],
        ignored: &[],
        columns: &["revision", "updated_at"],
        changed: "SELECT singleton_id FROM source.library_organization",
        added: NO_PROOF,
        removed: NO_PROOF,
        proof: LIBRARY_REVISION_COUNTS_ARCHIVED_DECKS,
    }),
    // Derived caches. Retiring a copy fires the knowledge triggers exactly as
    // deleting a note does at runtime; these rules admit that and nothing else.
    Rule::Projected(Projection {
        table: "knowledge_sources",
        description: "Tutor knowledge sources",
        key: &["id"],
        ignored: &[],
        columns: &["active", "extraction_contract", "extracted_hash"],
        changed: "SELECT id FROM cleared_source",
        added: NO_PROOF,
        removed: NO_PROOF,
        proof: SOURCES_INVALIDATED_BY_RETIRED_COPIES,
    }),
    Rule::Projected(Projection {
        table: "knowledge_cache_evictions",
        description: "Tutor cache evictions",
        key: &["source_id"],
        ignored: &[],
        columns: &[],
        changed: NO_PROOF,
        added: "SELECT id FROM deactivated_source
                UNION
                SELECT u.source_id FROM source.knowledge_units u
                JOIN source.knowledge_unit_evidence e ON e.unit_id = u.id
                WHERE e.source_id IN (SELECT id FROM deactivated_source)",
        removed: NO_PROOF,
        proof: NO_PROOF,
    }),
    Rule::Projected(Projection {
        table: "knowledge_units",
        description: "Tutor knowledge units",
        key: &["id"],
        ignored: &[],
        columns: &[],
        changed: NO_PROOF,
        added: NO_PROOF,
        removed: "SELECT id FROM source.knowledge_units
                  WHERE source_id IN (SELECT id FROM deactivated_source)
                  UNION
                  SELECT unit_id FROM source.knowledge_unit_evidence
                  WHERE source_id IN (SELECT id FROM deactivated_source)",
        proof: NO_PROOF,
    }),
    Rule::Projected(Projection {
        table: "knowledge_unit_evidence",
        description: "Tutor knowledge evidence",
        key: &["unit_id", "evidence_id"],
        ignored: &[],
        columns: &[],
        changed: NO_PROOF,
        added: NO_PROOF,
        removed: "SELECT unit_id, evidence_id FROM source.knowledge_unit_evidence
                  WHERE unit_id NOT IN (SELECT id FROM target.knowledge_units)",
        proof: NO_PROOF,
    }),
    Rule::Projected(Projection {
        table: "knowledge_artifact_links",
        description: "Tutor knowledge links",
        key: &["source_id", "unit_id"],
        ignored: &[],
        columns: &[],
        changed: NO_PROOF,
        added: NO_PROOF,
        removed: "SELECT source_id, unit_id FROM source.knowledge_artifact_links
                  WHERE source_id IN (SELECT id FROM deactivated_source)
                     OR unit_id NOT IN (SELECT id FROM target.knowledge_units)",
        proof: NO_PROOF,
    }),
    Rule::Projected(Projection {
        table: "knowledge_analysis_windows",
        description: "Tutor analysis windows",
        key: &["id"],
        ignored: &[],
        columns: &[],
        changed: NO_PROOF,
        added: NO_PROOF,
        removed: "SELECT window_id FROM source.knowledge_analysis_sources
                  WHERE source_id IN (SELECT id FROM deactivated_source)",
        proof: NO_PROOF,
    }),
    Rule::Projected(Projection {
        table: "knowledge_analysis_sources",
        description: "Tutor analysis sources",
        key: &["window_id", "source_id"],
        ignored: &[],
        columns: &[],
        changed: NO_PROOF,
        added: NO_PROOF,
        removed: "SELECT window_id, source_id FROM source.knowledge_analysis_sources
                  WHERE window_id NOT IN (SELECT id FROM target.knowledge_analysis_windows)",
        proof: NO_PROOF,
    }),
    Rule::Projected(Projection {
        table: "knowledge_document_views",
        description: "Tutor document views",
        key: &["document_id"],
        ignored: &[],
        columns: &[],
        changed: NO_PROOF,
        added: NO_PROOF,
        removed: "SELECT k.document_id FROM source.knowledge_units u
                  JOIN source.knowledge_sources k ON k.id = u.source_id
                  WHERE k.document_id IS NOT NULL
                    AND u.id NOT IN (SELECT id FROM target.knowledge_units)
                  UNION
                  SELECT document_id FROM source.knowledge_analysis_windows
                  WHERE document_id IS NOT NULL
                    AND id NOT IN (SELECT id FROM target.knowledge_analysis_windows)",
        proof: NO_PROOF,
    }),
    Rule::Projected(Projection {
        table: "knowledge_collection_views",
        description: "Tutor collection views",
        key: &["collection_id"],
        ignored: &[],
        columns: &["stale"],
        changed: "SELECT collection_id FROM source.knowledge_collection_views",
        added: NO_PROOF,
        removed: NO_PROOF,
        proof: COLLECTION_VIEWS_ONLY_GO_STALE,
    }),
    Rule::Projected(Projection {
        table: "collection_guide_checkpoints",
        description: "collection guide checkpoints",
        key: &["cache_key"],
        ignored: &[],
        columns: &[],
        changed: NO_PROOF,
        added: NO_PROOF,
        removed: "SELECT c.cache_key FROM source.collection_guide_checkpoint_sources c
                  JOIN source.knowledge_sources s ON s.id = c.source_id
                  JOIN target.knowledge_sources t ON t.id = s.id
                  WHERE s.active IS NOT t.active
                     OR s.extraction_contract IS NOT t.extraction_contract",
        proof: NO_PROOF,
    }),
    Rule::Projected(Projection {
        table: "collection_guide_checkpoint_sources",
        description: "collection guide checkpoint sources",
        key: &["cache_key", "source_id"],
        ignored: &[],
        columns: &[],
        changed: NO_PROOF,
        added: NO_PROOF,
        removed: "SELECT cache_key, source_id FROM source.collection_guide_checkpoint_sources
                  WHERE cache_key NOT IN (SELECT cache_key FROM target.collection_guide_checkpoints)",
        proof: NO_PROOF,
    }),
    Rule::Projected(Projection {
        table: "tutor_turns",
        description: "Tutor turns",
        key: &["id"],
        ignored: &[],
        columns: &["evidence_current"],
        changed: "SELECT turn_id FROM source.tutor_turn_sources
                  WHERE source_id IN (SELECT id FROM deactivated_source)",
        added: NO_PROOF,
        removed: NO_PROOF,
        proof: TURN_EVIDENCE_ONLY_GOES_STALE,
    }),
    Rule::Projected(Projection {
        table: "tutor_conversations",
        description: "Tutor conversations",
        key: &["id"],
        ignored: &[],
        columns: &["summary_text", "summary_through_sequence"],
        changed: "SELECT t.conversation_id FROM source.tutor_turns t
                  JOIN source.tutor_turn_sources l ON l.turn_id = t.id
                  WHERE l.source_id IN (SELECT id FROM deactivated_source)",
        added: NO_PROOF,
        removed: NO_PROOF,
        proof: SUMMARIES_ONLY_CLEAR,
    }),
];
