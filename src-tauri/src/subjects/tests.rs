use serde_json::json;

use super::*;
use crate::{
    collections::test_support::seed_document,
    database::subjects_fixture::{advance_release_library, release_library, seed},
    decks::{create_deck_in_connection, CreateDeckInput},
};

fn id(byte: char) -> String {
    byte.to_string().repeat(32)
}

fn deck(connection: &mut Connection, name: &str) -> String {
    create_deck_in_connection(
        connection,
        CreateDeckInput {
            name: name.to_string(),
            description: None,
        },
    )
    .unwrap()
    .id
}

fn subject(connection: &mut Connection, name: &str) -> SubjectLibraryDto {
    let expected_library_revision = load_library(connection).unwrap().library_revision;
    create(
        connection,
        CreateSubjectInput {
            name: name.to_string(),
            expected_library_revision,
        },
    )
    .unwrap()
}

fn subject_id(library: &SubjectLibraryDto, name: &str) -> String {
    library
        .subjects
        .iter()
        .find(|subject| subject.name == name)
        .unwrap()
        .id
        .clone()
}

fn decks_in(library: &SubjectLibraryDto, subject_id: Option<&str>) -> Vec<String> {
    library
        .decks
        .iter()
        .filter(|placement| placement.subject_id.as_deref() == subject_id)
        .map(|placement| placement.deck_id.clone())
        .collect()
}

fn file_deck_input(
    library: &SubjectLibraryDto,
    deck_id: &str,
    subject_id: Option<&str>,
    target_index: Option<i64>,
) -> MoveDeckToSubjectInput {
    MoveDeckToSubjectInput {
        deck_id: deck_id.to_string(),
        subject_id: subject_id.map(str::to_string),
        target_index,
        expected_library_revision: library.library_revision,
    }
}

fn file_document_input(
    library: &SubjectLibraryDto,
    document_id: &str,
    subject_id: Option<&str>,
) -> MoveDocumentToSubjectInput {
    MoveDocumentToSubjectInput {
        document_id: document_id.to_string(),
        subject_id: subject_id.map(str::to_string),
        expected_document_filing_revision: library.document_filing_revision,
    }
}

fn page_range(start_unit: i64, end_unit: i64) -> SourceRangeInput {
    SourceRangeInput {
        unit_kind: "page".to_string(),
        start_unit,
        end_unit,
    }
}

#[test]
fn decks_and_documents_are_filed_under_separate_revisions() {
    let database = Database::open_in_memory().unwrap();
    database
        .with_connection(|connection| {
            let pharmacology = deck(connection, "Pharmacology");
            let anatomy = deck(connection, "Anatomy");
            let document = id('d');
            seed_document(connection, &document, 4, 'a')?;
            let start = load_library(connection)?;

            let created = subject(connection, "  Nursing ");
            let nursing = subject_id(&created, "Nursing");
            assert_eq!(created.subjects.len(), 1);
            assert_eq!(created.library_revision, start.library_revision + 1);
            assert_eq!(
                created.document_filing_revision,
                start.document_filing_revision
            );

            // Filing decks, at the end or at a position, moves only the library
            // revision.
            let filed = move_deck(
                connection,
                file_deck_input(&created, &anatomy, Some(&nursing), None),
            )?;
            let filed = move_deck(
                connection,
                file_deck_input(&filed, &pharmacology, Some(&nursing), Some(0)),
            )?;
            assert_eq!(
                decks_in(&filed, Some(&nursing)),
                vec![pharmacology.clone(), anatomy.clone()]
            );
            assert!(decks_in(&filed, None).is_empty());
            assert_eq!(filed.library_revision, created.library_revision + 2);
            assert_eq!(
                filed.document_filing_revision,
                start.document_filing_revision
            );

            // Filing a document moves only the document filing revision, and
            // filing it where it already is changes nothing.
            let with_document = move_document(
                connection,
                file_document_input(&filed, &document, Some(&nursing)),
            )?;
            assert_eq!(
                with_document.documents,
                vec![SubjectDocumentPlacementDto {
                    document_id: document.clone(),
                    subject_id: Some(nursing.clone()),
                }]
            );
            assert_eq!(with_document.library_revision, filed.library_revision);
            assert_eq!(
                with_document.document_filing_revision,
                filed.document_filing_revision + 1
            );
            let unchanged = move_document(
                connection,
                file_document_input(&with_document, &document, Some(&nursing)),
            )?;
            assert_eq!(unchanged, with_document);

            // Back to No subject, in the position asked for.
            let unfiled = move_deck(
                connection,
                file_deck_input(&unchanged, &anatomy, None, Some(0)),
            )?;
            assert_eq!(decks_in(&unfiled, None), vec![anatomy.clone()]);
            assert_eq!(decks_in(&unfiled, Some(&nursing)), vec![pharmacology]);
            Ok(())
        })
        .unwrap();
}

#[test]
fn subjects_are_renamed_and_reordered_by_position() {
    let database = Database::open_in_memory().unwrap();
    database
        .with_connection(|connection| {
            subject(connection, "Anatomy");
            subject(connection, "Biology");
            let library = subject(connection, "Chemistry");
            let chemistry = subject_id(&library, "Chemistry");

            let moved = reorder(
                connection,
                MoveSubjectInput {
                    subject_id: chemistry.clone(),
                    target_index: 0,
                    expected_library_revision: library.library_revision,
                },
            )?;
            let names = |library: &SubjectLibraryDto| {
                library
                    .subjects
                    .iter()
                    .map(|subject| subject.name.clone())
                    .collect::<Vec<_>>()
            };
            assert_eq!(names(&moved), ["Chemistry", "Anatomy", "Biology"]);
            assert_eq!(
                moved.document_filing_revision,
                library.document_filing_revision
            );

            let before = moved
                .subjects
                .iter()
                .find(|subject| subject.id == chemistry)
                .unwrap()
                .clone();
            let renamed = rename(
                connection,
                RenameSubjectInput {
                    subject_id: chemistry.clone(),
                    name: "Organic chemistry".to_string(),
                    revision: before.revision,
                    expected_library_revision: moved.library_revision,
                },
            )?;
            let after = renamed
                .subjects
                .iter()
                .find(|subject| subject.id == chemistry)
                .unwrap();
            assert_eq!(after.name, "Organic chemistry");
            assert_eq!(after.revision, before.revision + 1);
            assert_eq!(after.sort_order, before.sort_order);
            assert_eq!(renamed.library_revision, moved.library_revision + 1);
            Ok(())
        })
        .unwrap();
}

#[test]
fn stale_revisions_missing_rows_and_bad_positions_are_refused() {
    let database = Database::open_in_memory().unwrap();
    database
        .with_connection(|connection| {
            let anatomy_deck = deck(connection, "Anatomy");
            let document = id('d');
            seed_document(connection, &document, 2, 'a')?;
            let library = subject(connection, "Anatomy");
            let anatomy = subject_id(&library, "Anatomy");
            let code = |result: Result<SubjectLibraryDto, CommandError>| result.unwrap_err().code;

            let stale_library = CreateSubjectInput {
                name: "Biology".to_string(),
                expected_library_revision: library.library_revision + 1,
            };
            assert_eq!(
                code(create(connection, stale_library)),
                "library_organization_changed"
            );
            let stale_subject = RenameSubjectInput {
                subject_id: anatomy.clone(),
                name: "Human anatomy".to_string(),
                revision: 2,
                expected_library_revision: library.library_revision,
            };
            assert_eq!(code(rename(connection, stale_subject)), "subject_changed");
            let missing = DeleteSubjectInput {
                subject_id: id('9'),
                revision: 1,
                expected_library_revision: library.library_revision,
            };
            assert_eq!(code(delete(connection, missing)), "subject_not_found");
            let too_far = MoveSubjectInput {
                subject_id: anatomy.clone(),
                target_index: 2,
                expected_library_revision: library.library_revision,
            };
            assert_eq!(code(reorder(connection, too_far)), "invalid_library_target");
            let negative = file_deck_input(&library, &anatomy_deck, Some(&anatomy), Some(-1));
            assert_eq!(
                code(move_deck(connection, negative)),
                "invalid_library_target"
            );
            let nowhere = file_deck_input(&library, &anatomy_deck, Some(&id('9')), None);
            assert_eq!(code(move_deck(connection, nowhere)), "subject_not_found");
            let no_deck = file_deck_input(&library, &id('8'), Some(&anatomy), None);
            assert_eq!(code(move_deck(connection, no_deck)), "deck_not_found");
            let mut stale_filing = file_document_input(&library, &document, Some(&anatomy));
            stale_filing.expected_document_filing_revision += 1;
            assert_eq!(
                code(move_document(connection, stale_filing)),
                "document_filing_changed"
            );
            let no_document = file_document_input(&library, &id('7'), Some(&anatomy));
            assert_eq!(
                code(move_document(connection, no_document)),
                "document_not_found"
            );

            // Nothing above was written.
            assert_eq!(load_library(connection)?, library);
            Ok(())
        })
        .unwrap();
}

#[test]
fn deleting_a_subject_sends_its_decks_and_documents_to_no_subject() {
    let database = Database::open_in_memory().unwrap();
    database
        .with_connection(|connection| {
            let loose = deck(connection, "Loose");
            let first = deck(connection, "First");
            let second = deck(connection, "Second");
            let document = id('d');
            seed_document(connection, &document, 3, 'a')?;
            let library = subject(connection, "Nursing");
            let nursing = subject_id(&library, "Nursing");
            let library = move_deck(
                connection,
                file_deck_input(&library, &first, Some(&nursing), None),
            )?;
            let library = move_deck(
                connection,
                file_deck_input(&library, &second, Some(&nursing), None),
            )?;
            let library = move_document(
                connection,
                file_document_input(&library, &document, Some(&nursing)),
            )?;
            let library = set_ranges(
                connection,
                SetDocumentStudyRangesInput {
                    document_id: document.clone(),
                    ranges: vec![page_range(2, 3)],
                    expected_document_filing_revision: library.document_filing_revision,
                },
            )?;
            assert_eq!(library.study_ranges.len(), 1);
            connection
                .execute(
                    "INSERT INTO study_targets(id, target_kind, collection_id, created_at,
                                               updated_at)
                     VALUES (?1, 'collection', ?2, 1, 1)",
                    params![id('5'), nursing],
                )
                .map_err(CommandError::database)?;

            let nursing_revision = library.subjects[0].revision;
            let deleted = delete(
                connection,
                DeleteSubjectInput {
                    subject_id: nursing.clone(),
                    revision: nursing_revision,
                    expected_library_revision: library.library_revision,
                },
            )?;
            assert!(deleted.subjects.is_empty());
            assert_eq!(decks_in(&deleted, None), vec![loose, first, second]);
            assert_eq!(
                deleted.documents,
                vec![SubjectDocumentPlacementDto {
                    document_id: document,
                    subject_id: None,
                }]
            );
            assert!(deleted.study_ranges.is_empty());
            assert_eq!(deleted.library_revision, library.library_revision + 1);
            assert_eq!(
                deleted.document_filing_revision,
                library.document_filing_revision + 1
            );
            let targets: i64 = connection
                .query_row("SELECT COUNT(*) FROM study_targets", [], |row| row.get(0))
                .map_err(CommandError::database)?;
            assert_eq!(targets, 0);
            Ok(())
        })
        .unwrap();
}

#[test]
fn a_document_s_generated_deck_goes_with_it_unless_it_was_filed_elsewhere() {
    let database = Database::open_in_memory().unwrap();
    database
        .with_connection(|connection| {
            let generated = deck(connection, "Generated");
            let document = id('d');
            let other = id('e');
            seed_document(connection, &document, 3, 'a')?;
            seed_document(connection, &other, 3, 'b')?;
            let generate = |connection: &Connection, document_id: &str| {
                connection
                    .execute(
                        "UPDATE documents SET generated_deck_id = ?2 WHERE id = ?1",
                        params![document_id, generated],
                    )
                    .map_err(CommandError::database)
            };
            generate(connection, &document)?;
            subject(connection, "Physics");
            let library = subject(connection, "Biology");
            let physics = subject_id(&library, "Physics");
            let biology = subject_id(&library, "Biology");

            // From No subject the deck joins its document, which also moves
            // the library revision.
            let filed = move_document(
                connection,
                file_document_input(&library, &document, Some(&physics)),
            )?;
            assert_eq!(decks_in(&filed, Some(&physics)), vec![generated.clone()]);
            assert_eq!(filed.library_revision, library.library_revision + 1);
            // It follows the document to another subject and back to No subject.
            let moved = move_document(
                connection,
                file_document_input(&filed, &document, Some(&biology)),
            )?;
            assert_eq!(decks_in(&moved, Some(&biology)), vec![generated.clone()]);
            let unfiled = move_document(connection, file_document_input(&moved, &document, None))?;
            assert!(decks_in(&unfiled, Some(&biology)).is_empty());
            assert!(decks_in(&unfiled, None).contains(&generated));

            // A deck the learner filed somewhere else stays there.
            let elsewhere = move_deck(
                connection,
                file_deck_input(&unfiled, &generated, Some(&physics), None),
            )?;
            let kept = move_document(
                connection,
                file_document_input(&elsewhere, &document, Some(&biology)),
            )?;
            assert_eq!(decks_in(&kept, Some(&physics)), vec![generated.clone()]);
            assert_eq!(kept.library_revision, elsewhere.library_revision);

            // So does a deck that another document in its subject also uses.
            let together = move_document(
                connection,
                file_document_input(&kept, &document, Some(&physics)),
            )?;
            generate(connection, &other)?;
            let shared = move_document(
                connection,
                file_document_input(&together, &other, Some(&physics)),
            )?;
            let left = move_document(
                connection,
                file_document_input(&shared, &document, Some(&biology)),
            )?;
            assert_eq!(decks_in(&left, Some(&physics)), vec![generated.clone()]);

            // A newly generated deck is filed with its document.
            let fresh = deck(connection, "Fresh");
            connection
                .execute(
                    "UPDATE documents SET generated_deck_id = ?2 WHERE id = ?1",
                    params![document, fresh],
                )
                .map_err(CommandError::database)?;
            let tx = immediate(connection)?;
            file_generated_deck_with_document(&tx, &document, None, 9)?;
            tx.commit().map_err(CommandError::database)?;
            assert_eq!(
                decks_in(&load_library(connection)?, Some(&biology)),
                vec![fresh]
            );
            Ok(())
        })
        .unwrap();
}

#[test]
fn study_ranges_need_a_subject_and_follow_the_document_between_subjects() {
    let database = Database::open_in_memory().unwrap();
    database
        .with_connection(|connection| {
            let document = id('d');
            seed_document(connection, &document, 6, 'a')?;
            subject(connection, "Physics");
            let library = subject(connection, "Biology");
            let physics = subject_id(&library, "Physics");
            let biology = subject_id(&library, "Biology");
            let ranges = |library: &SubjectLibraryDto, ranges: Vec<SourceRangeInput>| {
                SetDocumentStudyRangesInput {
                    document_id: document.clone(),
                    ranges,
                    expected_document_filing_revision: library.document_filing_revision,
                }
            };

            let unfiled = set_ranges(connection, ranges(&library, vec![page_range(1, 2)]));
            assert_eq!(unfiled.unwrap_err().code, "document_not_in_subject");

            let library = move_document(
                connection,
                file_document_input(&library, &document, Some(&physics)),
            )?;
            let library = set_ranges(
                connection,
                ranges(&library, vec![page_range(1, 2), page_range(2, 4)]),
            )?;
            let spans = |library: &SubjectLibraryDto| {
                library
                    .study_ranges
                    .iter()
                    .map(|range| (range.start_unit, range.end_unit))
                    .collect::<Vec<_>>()
            };
            assert_eq!(spans(&library), [(1, 4)]);
            assert_eq!(library.study_ranges[0].source_sha256, "a".repeat(64));

            let outside = set_ranges(connection, ranges(&library, vec![page_range(5, 7)]));
            assert_eq!(outside.unwrap_err().code, "invalid_document_collection");

            let moved = move_document(
                connection,
                file_document_input(&library, &document, Some(&biology)),
            )?;
            assert_eq!(spans(&moved), [(1, 4)]);

            let whole = set_ranges(connection, ranges(&moved, Vec::new()))?;
            assert!(whole.study_ranges.is_empty());
            let narrowed = set_ranges(connection, ranges(&whole, vec![page_range(3, 3)]))?;
            let unfiled =
                move_document(connection, file_document_input(&narrowed, &document, None))?;
            assert!(unfiled.study_ranges.is_empty());
            Ok(())
        })
        .unwrap();
}

#[test]
fn the_check_these_list_names_live_items_and_dismisses_idempotently() {
    let mut connection = release_library(5);
    let l = seed(&connection);
    advance_release_library(&connection, 5, 6);

    let listed = load_filing_reviews(&connection).unwrap();
    assert_eq!(listed.len(), 15);
    assert_eq!(check_these_count(&connection).unwrap(), 15);
    let kind_order = |kind: &str| {
        ["subject", "deck", "document", "note"]
            .iter()
            .position(|k| *k == kind)
    };
    assert!(listed
        .windows(2)
        .all(|pair| kind_order(&pair[0].entity_kind) <= kind_order(&pair[1].entity_kind)));
    let name = |sql: &str, id: &str| -> String {
        connection.query_row(sql, [id], |row| row.get(0)).unwrap()
    };
    let mechanics = listed
        .iter()
        .find(|review| review.entity_id == l.k_mech)
        .unwrap();
    assert_eq!(mechanics.entity_kind, "deck");
    assert_eq!(mechanics.reason, "moved_to_document_subject");
    assert_eq!(
        mechanics.detail,
        json!({"folderSubjectId": l.f_chem, "subjectIds": [l.c_phys]})
    );
    assert_eq!(
        mechanics.label,
        name("SELECT name FROM decks WHERE id = ?1", &l.k_mech)
    );
    let history = listed
        .iter()
        .find(|review| review.entity_id == l.c_hist)
        .unwrap();
    assert_eq!(
        history.label,
        name(
            "SELECT name FROM document_collections WHERE id = ?1",
            &l.c_hist
        )
    );
    let edited = listed
        .iter()
        .find(|review| review.entity_id == l.cb[2].id)
        .unwrap();
    assert_eq!(edited.label, "edited copy");

    // A deck deleted since the upgrade leaves the list.
    connection
        .execute(
            "UPDATE decks SET deleted_at = updated_at WHERE id = ?1",
            [&l.k_mech],
        )
        .unwrap();
    let dismissal = || DismissSubjectFilingReviewInput {
        entity_kind: "document".to_string(),
        entity_id: l.d_acids.clone(),
    };
    let after = dismiss(&mut connection, dismissal()).unwrap();
    assert_eq!(after.len(), 13);
    assert!(after
        .iter()
        .all(|review| review.entity_id != l.k_mech && review.entity_id != l.d_acids));
    assert_eq!(dismiss(&mut connection, dismissal()).unwrap(), after);
    assert_eq!(check_these_count(&connection).unwrap(), 13);
    let dismissed_at: Option<i64> = connection
        .query_row(
            "SELECT dismissed_at FROM subject_filing_reviews
             WHERE entity_kind = 'document' AND entity_id = ?1",
            [&l.d_acids],
            |row| row.get(0),
        )
        .unwrap();
    assert!(dismissed_at.is_some());
}
