//! Release-v6 admission for subjects.
//!
//! Migration 6 is not additive: it reshapes decks, documents and
//! document_collections, drops the deck-folder and collection-membership
//! tables, and adds card_merges, document_study_ranges and
//! subject_filing_reviews with their indexes and triggers. Admission compares
//! every schema object the migration adds, changes or drops with the bundled
//! schema at the stored version, so a library that half-applied the migration,
//! or applied a different one, is refused before anything writes to it.

use std::sync::OnceLock;

use rusqlite::Connection;

use super::object_delta::{self, Delta};

const VERSION: i64 = 6;

pub(super) fn validate_schema(connection: &Connection, version: i64) -> Result<(), String> {
    static DELTA: OnceLock<Result<Delta, String>> = OnceLock::new();
    object_delta::validate(connection, version, VERSION, "subjects", &DELTA)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::release::{
        apply_migrations, apply_migrations_through, migration_sql, preservation_receipt,
        subjects_fixture::{
            release_library, seed, SubjectsLibrary, GEOGRAPHY_SUBJECT, LEGACY_ALGEBRA_FOLDER, T0,
        },
        validate_backup_input, validate_input, validate_supported_input, SCHEMA_VERSION,
    };
    use rusqlite::{types::Value, OptionalExtension};
    use serde_json::json;

    /// The fixture library at `version`, seeded, then upgraded to schema 6.
    fn upgraded_from(version: i64) -> (Connection, SubjectsLibrary) {
        let mut connection = release_library(version);
        let library = seed(&connection);
        assert_eq!(validate_supported_input(&connection).unwrap(), version);
        assert!(apply_migrations(&mut connection).unwrap());
        validate_input(&connection).unwrap();
        (connection, library)
    }

    fn rows(connection: &Connection, sql: &str) -> Vec<Vec<Value>> {
        let mut statement = connection.prepare(sql).unwrap();
        let width = statement.column_count();
        let rows = statement
            .query_map([], |row| {
                (0..width)
                    .map(|index| row.get::<_, Value>(index))
                    .collect::<Result<Vec<_>, _>>()
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        rows
    }

    fn text(value: &str) -> Value {
        Value::Text(value.to_string())
    }

    fn subject_of(connection: &Connection, table: &str, id: &str) -> Option<String> {
        connection
            .query_row(
                &format!("SELECT subject_id FROM {table} WHERE id = ?1"),
                [id],
                |row| row.get(0),
            )
            .unwrap()
    }

    #[test]
    fn folders_and_collections_become_one_ordered_list_of_subjects() {
        let (connection, l) = upgraded_from(5);
        let subjects = rows(
            &connection,
            "SELECT id, name, sort_order, created_at, revision
             FROM document_collections ORDER BY sort_order",
        );
        let expected = [
            // Biology merged with " biology": the collection keeps its id and name.
            (l.c_bio.as_str(), " biology", T0 + 1, 3),
            (l.f_chem.as_str(), "Chemistry", T0, 2),
            // Two folders share the name, so neither merged with the collection.
            (l.f_hist_upper.as_str(), "History", T0, 2),
            (l.f_hist_lower.as_str(), "history", T0, 2),
            // The legacy folder "  Algebra " merged with "algebra" by name.
            (l.c_algebra.as_str(), "algebra", T0 + 1, 3),
            // A folder id that is not 32 characters maps to a derived one.
            (GEOGRAPHY_SUBJECT, "Geography", T0, 2),
            (l.c_phys.as_str(), "Physics", T0 + 1, 3),
            (l.c_hist.as_str(), "History", T0 + 1, 3),
            (l.c_unused.as_str(), "Unused", T0 + 1, 3),
        ];
        assert_eq!(subjects.len(), expected.len());
        for (position, (row, (id, name, created_at, revision))) in
            subjects.iter().zip(expected).enumerate()
        {
            assert_eq!(
                row,
                &vec![
                    text(id),
                    text(name),
                    Value::Integer(1000 * (position as i64 + 1)),
                    Value::Integer(created_at),
                    Value::Integer(revision),
                ],
                "subject {position}"
            );
        }
        // Folders that merged by name leave no subject of their own.
        for merged in [l.f_bio.as_str(), LEGACY_ALGEBRA_FOLDER] {
            let own: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM document_collections WHERE id = ?1",
                    [merged],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(own, 0, "folder {merged}");
        }
    }

    #[test]
    fn every_deck_and_document_is_filed_in_at_most_one_subject() {
        let (connection, l) = upgraded_from(5);
        for (deck, subject) in [
            (&l.k_bio, Some(l.c_bio.as_str())),
            // Generated for Mechanics, which stayed in Physics.
            (&l.k_mech, Some(l.c_phys.as_str())),
            (&l.k_acids, Some(l.f_chem.as_str())),
            // An unfiled deck follows its only document's subject.
            (&l.k_optics, Some(l.c_phys.as_str())),
            (&l.k_dates, Some(l.f_hist_upper.as_str())),
            (&l.k_misc, None),
            // Two documents in two subjects: the deck stays in its folder's one.
            (&l.k_merged, Some(l.c_bio.as_str())),
            (&l.k_geo, Some(GEOGRAPHY_SUBJECT)),
            (&l.k_algebra, Some(l.c_algebra.as_str())),
            (&l.k_deleted, Some(l.c_hist.as_str())),
            (&l.k_arch, Some(l.c_bio.as_str())),
            // Study bundles go to their collection's subject.
            (&l.b_bio, Some(l.c_bio.as_str())),
            (&l.b_phys, Some(l.c_phys.as_str())),
            (&l.b_unused, Some(l.c_unused.as_str())),
            (&l.b_shared, Some(l.c_hist.as_str())),
        ] {
            assert_eq!(
                subject_of(&connection, "decks", deck).as_deref(),
                subject,
                "deck {deck}"
            );
        }
        for (document, subject) in [
            (&l.d_cells, Some(l.c_bio.as_str())),
            // Decision A: the collection it joined first.
            (&l.d_mech, Some(l.c_phys.as_str())),
            // In no collection: follows its generated deck's folder.
            (&l.d_acids, Some(l.f_chem.as_str())),
            (&l.d_optics, Some(l.c_phys.as_str())),
            (&l.d_loose, Some(l.c_hist.as_str())),
            (&l.d_unfiled, None),
        ] {
            assert_eq!(
                subject_of(&connection, "documents", document).as_deref(),
                subject,
                "document {document}"
            );
        }
        // Decision C: ranges from the collection the document stays in, ids kept.
        let ranges = rows(
            &connection,
            "SELECT id, document_id, start_unit, end_unit FROM document_study_ranges
             ORDER BY document_id, start_unit",
        );
        // Ids grow in creation order, so Cells sorts before Mechanics.
        let expected = vec![
            vec![
                text(&l.cells_range),
                text(&l.d_cells),
                Value::Integer(1),
                Value::Integer(4),
            ],
            vec![
                text(&l.kept_ranges[0]),
                text(&l.d_mech),
                Value::Integer(1),
                Value::Integer(3),
            ],
            vec![
                text(&l.kept_ranges[1]),
                text(&l.d_mech),
                Value::Integer(5),
                Value::Integer(6),
            ],
        ];
        assert_eq!(ranges, expected);
        // The range set in Biology, which lost Mechanics, is not kept.
        assert!(ranges
            .iter()
            .all(|range| range[0] != text(&l.dropped_range)));
        for gone in [
            "deck_folders",
            "document_collection_memberships",
            "document_collection_ranges",
        ] {
            let present: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_schema WHERE name = ?1",
                    [gone],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(present, 0, "{gone}");
        }
        let parent_column: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('document_collections')
                 WHERE name = 'parent_id'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(parent_column, 0);
    }

    fn library_revision(connection: &Connection) -> (i64, i64) {
        connection
            .query_row(
                "SELECT revision, updated_at FROM library_organization",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap()
    }

    #[test]
    fn collection_copies_retire_into_their_originals() {
        let mut connection = release_library(5);
        let l = seed(&connection);
        let (revision, updated_at) = library_revision(&connection);
        assert!(apply_migrations(&mut connection).unwrap());
        let merges = rows(
            &connection,
            "SELECT retired_card_id, surviving_card_id FROM card_merges
             ORDER BY retired_card_id",
        );
        let expected: Vec<Vec<Value>> = l
            .expected_merges()
            .into_iter()
            .map(|(retired, surviving)| vec![text(&retired), text(&surviving)])
            .collect();
        assert_eq!(merges, expected);

        // Retired copies keep their history and move into their original's
        // deck, retired at their own last update.
        for (copy, deck) in [
            (&l.cb[0], &l.k_bio),
            (&l.cb[1], &l.k_bio),
            (&l.cq, &l.k_bio),
            // Moved to Misc by the learner, merged all the same.
            (&l.co[0], &l.k_optics),
            (&l.cm_bio[0], &l.k_mech),
            (&l.cm_phys[2], &l.k_mech),
        ] {
            let note: (String, Option<i64>, i64, i64) = connection
                .query_row(
                    "SELECT deck_id, deleted_at, updated_at, revision FROM notes WHERE id = ?1",
                    [&copy.id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .unwrap();
            assert_eq!(note.0, *deck);
            assert_eq!(note.1, Some(note.2));
            assert_eq!(note.3, 2);
            let card: (String, Option<i64>, i64) = connection
                .query_row(
                    "SELECT deck_id, deleted_at, updated_at FROM cards WHERE id = ?1",
                    [copy.card()],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .unwrap();
            assert_eq!((card.0.as_str(), card.1), (deck.as_str(), Some(card.2)));
        }
        for note in l.retired_notes() {
            let retired: (Option<i64>, i64, i64) = connection
                .query_row(
                    "SELECT deleted_at, updated_at, revision FROM notes WHERE id = ?1",
                    [&note.id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .unwrap();
            assert_eq!(retired.0, Some(retired.1), "retired copy {}", note.id);
            assert_eq!(retired.2, 2, "retired copy {}", note.id);
        }
        // Decision B and the other kept copies stay live in their bundle.
        for kept in [&l.cb[2], &l.cb[3], &l.cb[4], &l.ghost, &l.carch, &l.amb] {
            let deleted: Option<i64> = connection
                .query_row(
                    "SELECT deleted_at FROM notes WHERE id = ?1",
                    [&kept.id],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(deleted, None, "kept copy {}", kept.id);
        }
        // The deleted copy is untouched: still in the bundle, not merged.
        let deleted_copy: (String, i64) = connection
            .query_row(
                "SELECT deck_id, revision FROM notes WHERE id = ?1",
                [&l.co[1].id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(deleted_copy, (l.b_phys.clone(), 1));

        // A tag added to an unedited copy carries over to its original.
        let tagged: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM note_tags WHERE note_id = ?1 AND tag_id = ?2",
                [&l.o_bio[1].id, &l.exam_tag],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(tagged, 1);

        // The bundle the merge emptied is archived; one that still holds kept
        // copies, and one that never had copies, are not.
        let archived = rows(
            &connection,
            "SELECT id, archived_at IS NOT NULL FROM decks
             WHERE id IN (SELECT deck_id FROM study_targets) ORDER BY id",
        );
        let expected = vec![
            vec![text(&l.b_bio), Value::Integer(0)],
            vec![text(&l.b_phys), Value::Integer(1)],
            vec![text(&l.b_unused), Value::Integer(0)],
            vec![text(&l.b_shared), Value::Integer(0)],
        ];
        assert_eq!(archived, expected);
        // Archiving it is one deck-library change, as the trigger records it.
        assert_eq!(library_revision(&connection), (revision + 1, updated_at));
    }

    #[test]
    fn a_survivor_keeps_the_schedule_of_the_card_studied_last() {
        let mut source = release_library(5);
        let l = seed(&source);
        let schedule = |connection: &Connection, card: &str| {
            rows(
                connection,
                &format!(
                    "SELECT state, due_at, stability, difficulty, elapsed_days, scheduled_days,
                            reps, lapses, last_review_at, scheduler_name, scheduler_version,
                            updated_at
                     FROM card_schedules WHERE card_id = '{card}'"
                ),
            )
        };
        let revision = |connection: &Connection, card: &str| -> i64 {
            connection
                .query_row(
                    "SELECT revision FROM card_schedules WHERE card_id = ?1",
                    [card],
                    |row| row.get(0),
                )
                .unwrap()
        };
        let before: Vec<(String, Vec<Vec<Value>>, i64)> = l
            .reviews
            .iter()
            .map(|(card, _)| card.clone())
            .chain(
                [
                    &l.o_bio[1],
                    &l.o_mech[1],
                    &l.o_mech[2],
                    &l.o_opt[0],
                    &l.q_orig,
                ]
                .iter()
                .map(|note| note.card().to_string()),
            )
            .map(|card| {
                (
                    card.clone(),
                    schedule(&source, &card),
                    revision(&source, &card),
                )
            })
            .collect();
        let old = |card: &str| {
            before
                .iter()
                .find(|(id, _, _)| id == card)
                .map(|(_, schedule, revision)| (schedule.clone(), *revision))
                .unwrap()
        };
        assert!(apply_migrations(&mut source).unwrap());
        let connection = source;
        for (survivor, winner, members) in [
            // The copy was studied a minute after the original.
            (&l.o_bio[0], &l.cb[0], vec![&l.o_bio[0], &l.cb[0]]),
            // The Physics copy was studied last of three.
            (
                &l.o_mech[0],
                &l.cm_phys[0],
                vec![&l.o_mech[0], &l.cm_bio[0], &l.cm_phys[0]],
            ),
            (&l.o_opt[0], &l.co[0], vec![&l.o_opt[0], &l.co[0]]),
            (&l.q_orig, &l.cq, vec![&l.q_orig, &l.cq]),
        ] {
            assert_eq!(
                schedule(&connection, survivor.card()),
                old(winner.card()).0,
                "survivor {}",
                survivor.id
            );
            let highest = members
                .iter()
                .map(|member| old(member.card()).1)
                .max()
                .unwrap();
            assert_eq!(revision(&connection, survivor.card()), highest + 1);
        }
        // The original was studied last, was studied before the copies were
        // made (although their new schedules say otherwise), or was reset
        // after the copy's last review: its schedule stays as it was.
        for original in [&l.o_bio[1], &l.o_mech[1], &l.o_mech[2]] {
            assert_eq!(
                (
                    schedule(&connection, original.card()),
                    revision(&connection, original.card())
                ),
                old(original.card()),
                "original {}",
                original.id
            );
        }
        // A copy the upgrade kept keeps its own schedule and history.
        assert_eq!(schedule(&connection, l.cb[2].card()), old(l.cb[2].card()).0);
    }

    #[test]
    fn every_choice_the_upgrade_made_is_on_the_check_these_list() {
        let (connection, l) = upgraded_from(5);
        let listed: Vec<(String, String, String, serde_json::Value, Option<i64>)> = {
            let mut statement = connection
                .prepare(
                    "SELECT entity_kind, entity_id, reason, detail_json, dismissed_at
                     FROM subject_filing_reviews ORDER BY entity_kind, entity_id",
                )
                .unwrap();
            let listed = statement
                .query_map([], |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        serde_json::from_str(&row.get::<_, String>(3)?).unwrap(),
                        row.get(4)?,
                    ))
                })
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            listed
        };
        let mut expected = vec![
            (
                "subject",
                &l.c_hist,
                "same_name_not_merged",
                json!({"origin": "collection"}),
            ),
            (
                "subject",
                &l.f_hist_upper,
                "same_name_not_merged",
                json!({"origin": "folder"}),
            ),
            (
                "subject",
                &l.f_hist_lower,
                "same_name_not_merged",
                json!({"origin": "folder"}),
            ),
            (
                "deck",
                &l.k_mech,
                "moved_to_document_subject",
                json!({"folderSubjectId": l.f_chem, "subjectIds": [l.c_phys]}),
            ),
            (
                "deck",
                &l.k_merged,
                "documents_in_several_subjects",
                json!({"folderSubjectId": l.c_bio, "subjectIds": [l.c_bio, l.c_phys]}),
            ),
            (
                "deck",
                &l.b_phys,
                "moved_to_collection_subject",
                json!({"folderSubjectId": l.f_chem, "subjectIds": [l.c_phys]}),
            ),
            (
                "deck",
                &l.b_shared,
                "bundle_of_several_collections",
                json!({"folderSubjectId": null, "subjectIds": [l.c_hist, l.c_algebra]}),
            ),
            (
                "document",
                &l.d_mech,
                "in_several_collections",
                json!({"droppedSubjectIds": [l.c_bio], "droppedRangeCount": 1}),
            ),
            (
                "document",
                &l.d_acids,
                "followed_generated_deck",
                json!({"deckId": l.k_acids}),
            ),
            (
                "note",
                &l.cb[2].id,
                "copy_edited",
                json!({"originalNoteId": l.o_bio[2].id}),
            ),
            (
                "note",
                &l.cb[3].id,
                "original_missing",
                json!({"originalNoteId": l.o_bio[3].id}),
            ),
            (
                "note",
                &l.cb[4].id,
                "card_mismatch",
                json!({"originalNoteId": l.o_bio[4].id}),
            ),
            (
                "note",
                &l.ghost.id,
                "original_missing",
                json!({"originalNoteId": "9".repeat(32)}),
            ),
            (
                "note",
                &l.carch.id,
                "original_archived",
                json!({"originalNoteId": l.o_arch.id}),
            ),
            (
                "note",
                &l.amb.id,
                "ambiguous_origin",
                json!({"originalNoteId": l.o_dates.id.clone().min(l.o_misc.id.clone())}),
            ),
        ];
        expected.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));
        let expected: Vec<_> = expected
            .into_iter()
            .map(|(kind, id, reason, detail)| {
                (
                    kind.to_string(),
                    id.clone(),
                    reason.to_string(),
                    detail,
                    None,
                )
            })
            .collect();
        assert_eq!(listed, expected);
    }

    #[test]
    fn review_evidence_and_derived_caches_come_through_as_the_contract_expects() {
        let mut connection = release_library(5);
        let l = seed(&connection);
        let protected = [
            "review_events",
            "review_event_attributions",
            "review_event_tag_attributions",
            "review_event_scheduler_context",
            "question_attempts",
            "question_review_links",
            "practice_session_cards",
            "practice_session_card_snapshots",
            "schedule_action_events",
            "study_target_card_origins",
            "study_targets",
        ];
        let before: Vec<_> = protected
            .iter()
            .map(|table| rows(&connection, &format!("SELECT * FROM {table} ORDER BY 1, 2")))
            .collect();
        assert!(before[0].len() >= 12);
        assert!(apply_migrations(&mut connection).unwrap());
        for (table, retained) in protected.iter().zip(before) {
            assert_eq!(
                rows(&connection, &format!("SELECT * FROM {table} ORDER BY 1, 2")),
                retained,
                "{table}"
            );
        }
        // The copy's question attempt still names the copy's card and session:
        // history counts under the original through card_merges, unmoved.
        assert_eq!(
            rows(
                &connection,
                &format!(
                    "SELECT session_id, card_id FROM question_attempts WHERE instance_id = '{}'",
                    l.attempt
                ),
            ),
            vec![vec![text(&l.session), text(l.cq.card())]]
        );
        // Retiring the indexed copy deactivates its Tutor source and drops the
        // guide checkpoint built on it; the original's stay.
        let sources = rows(
            &connection,
            "SELECT id, active, extracted_hash, extraction_contract FROM knowledge_sources
             ORDER BY id",
        );
        let expected = vec![
            vec![text(&l.copy_source), Value::Integer(0), text(""), text("")],
            vec![
                text(&l.original_source),
                Value::Integer(1),
                text(&"3".repeat(64)),
                text(&"2".repeat(64)),
            ],
        ];
        assert_eq!(sources, expected);
        assert_eq!(
            rows(
                &connection,
                "SELECT source_id FROM knowledge_cache_evictions"
            ),
            vec![vec![text(&l.copy_source)]]
        );
        assert_eq!(
            rows(
                &connection,
                "SELECT cache_key FROM collection_guide_checkpoints"
            ),
            vec![vec![text(&l.original_checkpoint)]]
        );
        assert_eq!(
            rows(
                &connection,
                "SELECT COUNT(*) FROM knowledge_collection_views WHERE stale = 0"
            ),
            vec![vec![Value::Integer(0)]]
        );
    }

    #[test]
    fn two_devices_upgrading_the_same_rows_reach_the_same_library() {
        let (first, _) = upgraded_from(5);
        let (second, _) = upgraded_from(5);
        assert_eq!(
            preservation_receipt(&first).unwrap(),
            preservation_receipt(&second).unwrap()
        );
        // A schema-4 library takes both remaining releases to the same place.
        let (older, _) = upgraded_from(4);
        assert_eq!(
            preservation_receipt(&older).unwrap(),
            preservation_receipt(&first).unwrap()
        );
    }

    #[test]
    fn a_library_without_folders_collections_or_copies_files_nothing() {
        let mut connection = release_library(5);
        connection
            .execute_batch(
                "INSERT INTO decks(id, name, created_at, updated_at)
                 VALUES ('11111111111111111111111111111111', 'Unfiled', 10, 20);",
            )
            .unwrap();
        assert!(apply_migrations(&mut connection).unwrap());
        validate_input(&connection).unwrap();
        for table in [
            "document_collections",
            "card_merges",
            "subject_filing_reviews",
            "document_study_ranges",
        ] {
            assert_eq!(
                rows(&connection, &format!("SELECT COUNT(*) FROM {table}")),
                vec![vec![Value::Integer(0)]],
                "{table}"
            );
        }
        assert_eq!(
            rows(&connection, "SELECT name, subject_id FROM decks"),
            vec![vec![text("Unfiled"), Value::Null]]
        );
    }

    #[test]
    fn interrupted_sixth_migration_rolls_back_to_schema_five_and_retries() {
        let mut connection = release_library(5);
        seed(&connection);
        let before = preservation_receipt(&connection).unwrap();
        connection
            .execute_batch(
                "CREATE TEMP TRIGGER reject_subjects_migration
                 BEFORE INSERT ON schema_migrations
                 WHEN NEW.version = 6
                 BEGIN SELECT RAISE(ABORT, 'injected failure'); END;",
            )
            .unwrap();
        assert!(apply_migrations(&mut connection).is_err());
        assert_eq!(validate_supported_input(&connection).unwrap(), 5);
        assert_eq!(preservation_receipt(&connection).unwrap(), before);
        validate_schema(&connection, 5).unwrap();
        let leftovers: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM temp.sqlite_schema WHERE name LIKE 'subject_%'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(leftovers, 0, "the failed attempt left temporary tables");

        connection
            .execute_batch("DROP TRIGGER reject_subjects_migration;")
            .unwrap();
        assert!(apply_migrations(&mut connection).unwrap());
        let changes = connection.total_changes();
        assert!(!apply_migrations(&mut connection).unwrap());
        assert_eq!(connection.total_changes(), changes);
        validate_input(&connection).unwrap();
        assert_eq!(
            validate_supported_input(&connection).unwrap(),
            SCHEMA_VERSION
        );
    }

    #[test]
    fn unregistered_missing_and_substituted_subject_objects_fail_read_only() {
        let mut connection = release_library(5);
        seed(&connection);
        {
            let transaction = connection.transaction().unwrap();
            transaction.execute_batch(migration_sql(6)).unwrap();
            let changes = transaction.total_changes();
            assert!(validate_supported_input(&transaction).is_err());
            assert_eq!(transaction.total_changes(), changes);
            transaction.rollback().unwrap();
        }
        {
            // Half of the change, at schema 5, is not schema 5 either.
            let transaction = connection.transaction().unwrap();
            transaction
                .execute_batch("ALTER TABLE decks ADD COLUMN subject_id TEXT;")
                .unwrap();
            assert!(validate_supported_input(&transaction).is_err());
            transaction.rollback().unwrap();
        }

        assert!(apply_migrations_through(&mut connection, SCHEMA_VERSION).unwrap());
        for damage in [
            "DROP TABLE subject_filing_reviews;",
            "DROP TRIGGER card_merges_one_level_insert;",
            "DROP INDEX decks_subject_sort;",
            "CREATE TABLE deck_folders(id TEXT PRIMARY KEY);",
            "CREATE INDEX document_collections_parent_order ON document_collections(name);",
            // Same columns without the checks is not the registered table.
            "DROP TABLE card_merges;
             CREATE TABLE card_merges(
                 retired_card_id TEXT PRIMARY KEY, surviving_card_id TEXT NOT NULL,
                 retired_note_id TEXT NOT NULL, surviving_note_id TEXT NOT NULL,
                 reason TEXT NOT NULL, copied_at INTEGER NOT NULL
             ) STRICT;",
        ] {
            let transaction = connection.transaction().unwrap();
            transaction.execute_batch(damage).unwrap();
            let changes = transaction.total_changes();
            assert!(validate_backup_input(&transaction).is_err(), "{damage}");
            assert_eq!(transaction.total_changes(), changes);
            transaction.rollback().unwrap();
        }
        validate_input(&connection).unwrap();
    }

    #[test]
    fn card_merges_stay_one_level_and_name_the_notes_that_own_their_cards() {
        let (connection, l) = upgraded_from(5);
        let insert = |retired: &str, surviving: &str, retired_note: &str, surviving_note: &str| {
            connection
                .execute(
                    "INSERT INTO card_merges(retired_card_id, surviving_card_id,
                         retired_note_id, surviving_note_id, reason, copied_at)
                     VALUES (?1, ?2, ?3, ?4, 'collection_copy', 1)",
                    [retired, surviving, retired_note, surviving_note],
                )
                .unwrap_err()
                .to_string()
        };
        // A retired card cannot survive another merge.
        let chained = insert(l.cb[2].card(), l.cb[0].card(), &l.cb[2].id, &l.cb[0].id);
        assert!(
            chained.contains("card_merges_must_be_one_level"),
            "{chained}"
        );
        // A survivor cannot be retired.
        let retired_survivor = insert(
            l.o_bio[0].card(),
            l.cb[2].card(),
            &l.o_bio[0].id,
            &l.cb[2].id,
        );
        assert!(
            retired_survivor.contains("card_merges_must_be_one_level"),
            "{retired_survivor}"
        );
        // The notes must own the cards.
        let mismatched = insert(
            l.cb[2].card(),
            l.o_bio[2].card(),
            &l.cb[2].id,
            &l.o_bio[3].id,
        );
        assert!(
            mismatched.contains("card_merges_identity_mismatch"),
            "{mismatched}"
        );
        let edited = connection
            .execute(
                "UPDATE card_merges SET surviving_card_id = ?1, surviving_note_id = ?2
                 WHERE retired_card_id = ?3",
                [l.cb[1].card(), l.cb[1].id.as_str(), l.cm_bio[0].card()],
            )
            .unwrap_err()
            .to_string();
        assert!(edited.contains("card_merges_must_be_one_level"), "{edited}");
    }

    #[test]
    fn leaving_a_subject_clears_study_ranges_and_marks_its_views_stale() {
        let (connection, l) = upgraded_from(5);
        let ranges = |document: &str| -> i64 {
            connection
                .query_row(
                    "SELECT COUNT(*) FROM document_study_ranges WHERE document_id = ?1",
                    [document],
                    |row| row.get(0),
                )
                .unwrap()
        };
        let stale = |subject: &str| -> i64 {
            connection
                .query_row(
                    "SELECT stale FROM knowledge_collection_views WHERE collection_id = ?1",
                    [subject],
                    |row| row.get(0),
                )
                .unwrap()
        };
        let fresh = || {
            connection
                .execute("UPDATE knowledge_collection_views SET stale = 0", [])
                .unwrap();
        };
        let file = |document: &str, subject: Option<&str>| {
            connection
                .execute(
                    "UPDATE documents SET subject_id = ?1 WHERE id = ?2",
                    rusqlite::params![subject, document],
                )
                .unwrap();
        };

        // Moving Mechanics from Physics to Biology keeps its ranges, and both
        // subjects' views go stale.
        fresh();
        file(&l.d_mech, Some(&l.c_bio));
        assert_eq!(ranges(&l.d_mech), 2);
        assert_eq!((stale(&l.c_bio), stale(&l.c_phys)), (1, 1));

        // Leaving every subject clears them.
        fresh();
        file(&l.d_mech, None);
        assert_eq!(ranges(&l.d_mech), 0);
        assert_eq!((stale(&l.c_bio), stale(&l.c_phys)), (1, 0));

        // A deleted document leaves its subject's view stale.
        fresh();
        connection
            .execute("DELETE FROM documents WHERE id = ?1", [&l.d_optics])
            .unwrap();
        assert_eq!((stale(&l.c_bio), stale(&l.c_phys)), (0, 1));

        // Deleting a subject sends its decks and documents to No subject, and
        // its documents lose their ranges.
        assert_eq!(ranges(&l.d_cells), 1);
        connection
            .execute("DELETE FROM document_collections WHERE id = ?1", [&l.c_bio])
            .unwrap();
        assert_eq!(subject_of(&connection, "documents", &l.d_cells), None);
        assert_eq!(subject_of(&connection, "decks", &l.k_bio), None);
        assert_eq!(ranges(&l.d_cells), 0);
        assert_eq!(ranges(&l.d_mech), 0);
        let integrity: String = connection
            .query_row("PRAGMA foreign_key_check", [], |row| row.get(0))
            .optional()
            .unwrap()
            .unwrap_or_else(|| "ok".to_string());
        assert_eq!(integrity, "ok");
    }
}
