use rusqlite::{params, Connection};
use serde_json::json;

use super::{
    authoring::{add_authored_revision, create_authored_lesson, MAX_LESSONS_PER_SUBJECT},
    content::{canonical_json, LessonBlock, LessonContent, ValidatedLesson, MAX_STEPS},
    fixture::{loop_of_henle, CORRECT_OPTION},
    list_lessons, read_lesson, save_progress, GetLessonInput, SaveLessonProgressInput,
    SourceStatus,
};
use crate::{
    cards::questions::{
        ImageMask, NormalizedRect, OcclusionMode, OcclusionQuestion, QuestionBody,
        QuestionDefinition, QUESTION_SCHEMA_VERSION,
    },
    collections::test_support::seed_document,
    database::Database,
    error::CommandError,
};

const SUBJECT: &str = "55555555555555555555555555555555";
const OTHER_SUBJECT: &str = "66666666666666666666666666666666";
const DOCUMENT: &str = "dddddddddddddddddddddddddddddddd";
const UNIT: &str = "77777777777777777777777777777777";
const NOW: i64 = 1_700_000_000_000;

fn library() -> Database {
    let database = Database::open_in_memory().unwrap();
    database
        .with_connection(|connection| {
            for (id, order) in [(SUBJECT, 1000), (OTHER_SUBJECT, 2000)] {
                connection
                    .execute(
                        "INSERT INTO document_collections(
                             id, name, sort_order, created_at, updated_at, revision
                         ) VALUES (?1, ?1, ?2, 1, 1, 1)",
                        params![id, order],
                    )
                    .map_err(CommandError::database)?;
            }
            seed_document(connection, DOCUMENT, 1, 'a')?;
            connection
                .execute(
                    "INSERT INTO knowledge_sources(id, kind, owner_id, segment_key, parent_key,
                         owner_revision, content_hash, record_json, active, document_id,
                         created_at, updated_at)
                     VALUES (?1, 'document', ?2, 'document', 'document', '1', ?3, '{}', 1, ?2,
                             1, 1)",
                    params!["8".repeat(32), DOCUMENT, "8".repeat(64)],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "INSERT INTO knowledge_units(id, source_id, source_hash, content_hash,
                         record_json, extraction_contract, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, '{}', ?5, 1, 1)",
                    params![
                        UNIT,
                        "8".repeat(32),
                        "1".repeat(64),
                        "9".repeat(64),
                        "2".repeat(64)
                    ],
                )
                .map_err(CommandError::database)?;
            Ok(())
        })
        .unwrap();
    database
}

fn create(database: &Database, content: LessonContent) -> Result<String, CommandError> {
    database
        .with_connection(|connection| create_authored_lesson(connection, SUBJECT, content, NOW))
        .map(|lesson| lesson.id)
}

fn read(database: &Database, lesson_id: &str, revision_number: Option<i64>) -> super::LessonDto {
    database
        .with_connection(|connection| {
            read_lesson(
                connection,
                GetLessonInput {
                    lesson_id: lesson_id.to_string(),
                    revision_number,
                },
            )
        })
        .unwrap()
}

fn save(
    database: &Database,
    lesson_id: &str,
    revision_number: i64,
    current_step: i64,
    finished: bool,
    now: i64,
) -> Result<super::LessonProgressDto, CommandError> {
    database.with_connection(|connection| {
        save_progress(
            connection,
            SaveLessonProgressInput {
                lesson_id: lesson_id.to_string(),
                revision_number,
                current_step,
                finished,
            },
            now,
        )
    })
}

fn count(database: &Database, sql: &str) -> i64 {
    database
        .with_connection(|connection: &mut Connection| {
            connection
                .query_row(sql, [], |row| row.get(0))
                .map_err(CommandError::database)
        })
        .unwrap()
}

fn rejected(content: LessonContent) -> String {
    match ValidatedLesson::validate(content) {
        Ok(_) => panic!("the lesson should be refused"),
        Err(error) => error.message,
    }
}

#[test]
fn the_fixture_lesson_opens_with_its_steps_and_without_check_answers() {
    let database = library();
    let id = create(&database, loop_of_henle(&[UNIT])).unwrap();

    let course = database
        .with_connection(|connection| list_lessons(connection, SUBJECT))
        .unwrap();
    assert_eq!(course.lessons.len(), 1);
    let summary = &course.lessons[0];
    assert_eq!(summary.title, "Loop of Henle");
    let written = summary.written.as_ref().unwrap();
    assert_eq!(
        (
            written.revision_number,
            written.step_count,
            written.check_count
        ),
        (1, 3, 1)
    );
    assert_eq!(written.origin, "authored");
    assert_eq!(written.sources, SourceStatus::Current);
    assert_eq!(written.estimated_minutes, 1);
    assert!(summary.progress.is_none());

    let lesson = read(&database, &id, None);
    assert_eq!(lesson.revision_number, 1);
    assert_eq!(lesson.content.steps.len(), 3);
    assert_eq!(lesson.content.steps[0].knowledge_unit_ids, vec![UNIT]);
    assert!(lesson.content.steps[1].check_kind.is_some());
    let wire = serde_json::to_string(&lesson).unwrap();
    assert!(!wire.contains(CORRECT_OPTION), "{wire}");
    assert!(!wire.contains("correctOptionId"));
    assert!(wire.contains("\"kind\":\"comparison\""));

    // The other subject's course is empty, and an unknown subject is refused.
    database
        .with_connection(|connection| {
            assert!(list_lessons(connection, OTHER_SUBJECT)?.lessons.is_empty());
            assert_eq!(
                list_lessons(connection, &"4".repeat(32)).unwrap_err().code,
                "subject_not_found"
            );
            Ok(())
        })
        .unwrap();
}

#[test]
fn stored_content_round_trips_and_refuses_a_newer_version() {
    let validated = ValidatedLesson::validate(loop_of_henle(&[UNIT])).unwrap();
    let json = canonical_json(&validated.content).unwrap();
    let parsed = ValidatedLesson::parse(&json).unwrap();
    assert!(parsed.content == validated.content);
    assert_eq!(canonical_json(&parsed.content).unwrap(), json);

    let newer = json.replacen("\"schemaVersion\":1", "\"schemaVersion\":2", 1);
    assert_eq!(
        ValidatedLesson::parse(&newer).err().unwrap().code,
        "lesson_version_unsupported"
    );
    let unknown = json.replacen("\"summary\"", "\"extra\":1,\"summary\"", 1);
    assert_eq!(
        ValidatedLesson::parse(&unknown).err().unwrap().code,
        "lesson_invalid"
    );
}

#[test]
fn malformed_lessons_are_refused() {
    let mut blank_title = loop_of_henle(&[]);
    blank_title.title = "  ".to_string();
    assert!(rejected(blank_title).contains("cannot be empty"));

    let mut padded = loop_of_henle(&[]);
    padded.steps[0].title = " Two limbs".to_string();
    assert!(rejected(padded).contains("spaces"));

    let mut control = loop_of_henle(&[]);
    control.steps[0].narration = "Picture\u{7}".to_string();
    assert!(rejected(control).contains("control characters"));

    let mut duplicate_keys = loop_of_henle(&[]);
    duplicate_keys.steps[1].key = duplicate_keys.steps[0].key.clone();
    assert!(rejected(duplicate_keys).contains("same key"));

    let mut bad_key = loop_of_henle(&[]);
    bad_key.steps[0].key = "Two Limbs".to_string();
    assert!(rejected(bad_key).contains("step key"));

    let mut no_steps = loop_of_henle(&[]);
    no_steps.steps.clear();
    assert!(rejected(no_steps).contains("steps"));

    let mut too_many = loop_of_henle(&[]);
    let step = too_many.steps[0].clone();
    too_many.steps = (0..=MAX_STEPS)
        .map(|index| {
            let mut step = step.clone();
            step.key = format!("step-{index}");
            step
        })
        .collect();
    assert!(rejected(too_many).contains("steps"));

    let mut ragged = loop_of_henle(&[]);
    ragged.steps[0].blocks.push(LessonBlock::Table {
        title: "Limbs".to_string(),
        columns: vec!["Limb".to_string(), "Water".to_string()],
        rows: vec![vec!["Descending".to_string()]],
        caption: None,
    });
    assert!(rejected(ragged).contains("one cell per column"));

    let repeated_unit = loop_of_henle(&[UNIT, UNIT]);
    assert!(rejected(repeated_unit).contains("same knowledge unit"));

    let bad_unit = loop_of_henle(&["not-a-unit"]);
    assert_eq!(
        ValidatedLesson::validate(bad_unit).err().unwrap().code,
        "invalid_identifier"
    );

    // A check is a native question and is validated as one.
    let mut wrong_answer = loop_of_henle(&[]);
    if let Some(QuestionDefinition {
        question: QuestionBody::SingleChoice {
            correct_option_id, ..
        },
        ..
    }) = &mut wrong_answer.steps[1].check
    {
        *correct_option_id = "0c".repeat(16);
    }
    assert!(ValidatedLesson::validate(wrong_answer).is_err());

    let mut occlusion = loop_of_henle(&[]);
    occlusion.steps[1].check = Some(QuestionDefinition {
        schema_version: QUESTION_SCHEMA_VERSION,
        question: QuestionBody::ImageOcclusion {
            image_asset_id: "a".repeat(32),
            masks: vec![ImageMask {
                id: "b".repeat(32),
                rect: NormalizedRect {
                    x: 0.1,
                    y: 0.1,
                    width: 0.2,
                    height: 0.2,
                },
            }],
            questions: vec![OcclusionQuestion {
                id: "c".repeat(32),
                prompt: json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Name it"}]}]}),
                answer: json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Loop"}]}]}),
                explanation: None,
                mask_ids: vec!["b".repeat(32)],
                mode: OcclusionMode::HideTarget,
            }],
        },
    });
    assert!(rejected(occlusion).contains("image occlusion"));
}

#[test]
fn a_lesson_only_explains_knowledge_in_the_library_and_notices_when_it_changes() {
    let database = library();
    let missing = create(&database, loop_of_henle(&[&"3".repeat(32)])).unwrap_err();
    assert_eq!(missing.code, "lesson_unit_missing");
    assert_eq!(count(&database, "SELECT COUNT(*) FROM lessons"), 0);

    let id = create(&database, loop_of_henle(&[UNIT])).unwrap();
    assert_eq!(
        count(
            &database,
            "SELECT COUNT(*) FROM lesson_revision_units WHERE unit_content_hash = '9999999999999999999999999999999999999999999999999999999999999999'"
        ),
        1
    );
    database
        .with_connection(|connection| {
            connection
                .execute(
                    "UPDATE knowledge_units SET content_hash = ?1 WHERE id = ?2",
                    params!["6".repeat(64), UNIT],
                )
                .map_err(CommandError::database)
        })
        .unwrap();
    assert_eq!(read(&database, &id, None).sources, SourceStatus::Changed);

    database
        .with_connection(|connection| {
            connection
                .execute("DELETE FROM knowledge_units WHERE id = ?1", [UNIT])
                .map_err(CommandError::database)
        })
        .unwrap();
    // The lesson is still readable without the unit it was written from.
    let lesson = read(&database, &id, None);
    assert_eq!(lesson.sources, SourceStatus::Unavailable);
    assert_eq!(lesson.content.steps.len(), 3);
}

#[test]
fn rewriting_a_lesson_adds_a_revision_and_keeps_the_old_one_readable() {
    let database = library();
    let id = create(&database, loop_of_henle(&[])).unwrap();
    let again = database
        .with_connection(|connection| {
            add_authored_revision(connection, &id, loop_of_henle(&[]), NOW + 1)
        })
        .unwrap();
    assert_eq!(again.written.unwrap().revision_number, 1);
    assert_eq!(again.revision, 1);

    let mut rewritten = loop_of_henle(&[]);
    rewritten.title = "The loop of Henle".to_string();
    rewritten.steps.pop();
    let lesson = database
        .with_connection(|connection| add_authored_revision(connection, &id, rewritten, NOW + 2))
        .unwrap();
    let written = lesson.written.unwrap();
    assert_eq!((written.revision_number, written.step_count), (2, 2));
    assert_eq!(
        (lesson.title.as_str(), lesson.revision),
        ("The loop of Henle", 2)
    );

    let old = read(&database, &id, Some(1));
    assert_eq!((old.revision_number, old.content.steps.len()), (1, 3));
    assert_eq!(old.content.title, "Loop of Henle");
    let error = database
        .with_connection(|connection| {
            read_lesson(
                connection,
                GetLessonInput {
                    lesson_id: id.clone(),
                    revision_number: Some(3),
                },
            )
        })
        .unwrap_err();
    assert_eq!(error.code, "lesson_revision_missing");
}

#[test]
fn progress_resumes_repeats_harmlessly_and_never_touches_reviews() {
    let database = library();
    let id = create(&database, loop_of_henle(&[])).unwrap();

    let first = save(&database, &id, 1, 1, false, NOW).unwrap();
    assert_eq!(
        (
            first.current_step,
            first.furthest_step,
            first.revision,
            first.finished_at
        ),
        (1, 1, 1, None)
    );
    // Going back keeps the furthest step; the same state again changes nothing.
    let back = save(&database, &id, 1, 0, false, NOW + 1).unwrap();
    assert_eq!(
        (back.current_step, back.furthest_step, back.revision),
        (0, 1, 2)
    );
    assert_eq!(save(&database, &id, 1, 0, false, NOW + 2).unwrap(), back);

    let finished = save(&database, &id, 1, 2, true, NOW + 3).unwrap();
    assert_eq!(finished.finished_at, Some(NOW + 3));
    // Reading a finished lesson again leaves it finished.
    let reread = save(&database, &id, 1, 0, false, NOW + 4).unwrap();
    assert_eq!(reread.finished_at, Some(NOW + 3));
    assert_eq!(reread.started_at, NOW);

    assert_eq!(
        save(&database, &id, 1, 3, false, NOW).unwrap_err().code,
        "lesson_step_missing"
    );
    assert_eq!(
        save(&database, &id, 1, -1, false, NOW).unwrap_err().code,
        "lesson_step_missing"
    );
    assert_eq!(
        save(&database, &id, 2, 0, false, NOW).unwrap_err().code,
        "lesson_revision_missing"
    );
    assert_eq!(
        save(&database, &"4".repeat(32), 1, 0, false, NOW)
            .unwrap_err()
            .code,
        "lesson_not_found"
    );

    let summary = read(&database, &id, None).lesson;
    assert_eq!(summary.progress.unwrap(), reread);
    for table in ["review_events", "card_schedules", "question_attempts"] {
        assert_eq!(
            count(&database, &format!("SELECT COUNT(*) FROM {table}")),
            0
        );
    }
}

#[test]
fn progress_on_a_rewritten_lesson_starts_over_on_the_new_revision() {
    let database = library();
    let id = create(&database, loop_of_henle(&[])).unwrap();
    save(&database, &id, 1, 2, true, NOW).unwrap();
    let mut rewritten = loop_of_henle(&[]);
    rewritten.summary = "A shorter look at the loop.".to_string();
    database
        .with_connection(|connection| add_authored_revision(connection, &id, rewritten, NOW + 5))
        .unwrap();

    // Until the learner opens the new revision, progress still names the old one.
    let summary = read(&database, &id, None).lesson;
    assert_eq!(summary.progress.as_ref().unwrap().revision_number, 1);
    let restarted = save(&database, &id, 2, 0, false, NOW + 6).unwrap();
    assert_eq!(
        (
            restarted.revision_number,
            restarted.current_step,
            restarted.furthest_step,
            restarted.started_at,
            restarted.finished_at,
            restarted.revision
        ),
        (2, 0, 0, NOW + 6, None, 2)
    );
}

#[test]
fn a_clock_that_went_backwards_keeps_progress_valid() {
    let database = library();
    let id = create(&database, loop_of_henle(&[])).unwrap();
    save(&database, &id, 1, 0, false, NOW).unwrap();
    let saved = save(&database, &id, 1, 1, true, NOW - 10_000).unwrap();
    assert_eq!((saved.updated_at, saved.finished_at), (NOW, Some(NOW)));
}

#[test]
fn lessons_follow_course_order_and_go_with_their_subject() {
    let database = library();
    let first = create(&database, loop_of_henle(&[UNIT])).unwrap();
    let mut second_content = loop_of_henle(&[]);
    second_content.title = "Collecting duct".to_string();
    let second = create(&database, second_content).unwrap();
    save(&database, &first, 1, 1, false, NOW).unwrap();
    let course = database
        .with_connection(|connection| list_lessons(connection, SUBJECT))
        .unwrap();
    assert_eq!(
        course
            .lessons
            .iter()
            .map(|lesson| lesson.id.as_str())
            .collect::<Vec<_>>(),
        vec![first.as_str(), second.as_str()]
    );

    database
        .with_connection(|connection| {
            let transaction = connection.transaction().map_err(CommandError::database)?;
            let outcome = crate::subjects::delete_subject_row(&transaction, SUBJECT, 1, NOW)?;
            assert!(matches!(outcome, crate::subjects::Outcome::Done));
            transaction.commit().map_err(CommandError::database)
        })
        .unwrap();
    for table in [
        "lessons",
        "lesson_revisions",
        "lesson_revision_units",
        "lesson_progress",
    ] {
        assert_eq!(
            count(&database, &format!("SELECT COUNT(*) FROM {table}")),
            0
        );
    }
}

#[test]
fn a_course_holds_a_bounded_number_of_lessons() {
    let database = library();
    database
        .with_connection(|connection| {
            for index in 0..MAX_LESSONS_PER_SUBJECT {
                connection
                    .execute(
                        "INSERT INTO lessons(id, subject_id, sort_order, title, revision,
                             created_at, updated_at)
                         VALUES (?1, ?2, ?3, 'Planned', 1, 1, 1)",
                        params![format!("{index:032x}"), SUBJECT, index * 1000],
                    )
                    .map_err(CommandError::database)?;
            }
            Ok(())
        })
        .unwrap();
    assert_eq!(
        create(&database, loop_of_henle(&[])).unwrap_err().code,
        "lesson_limit"
    );
    // A planned lesson with no revision lists as unwritten and does not open.
    let course = database
        .with_connection(|connection| list_lessons(connection, SUBJECT))
        .unwrap();
    assert!(course.lessons.iter().all(|lesson| lesson.written.is_none()));
    let error = database
        .with_connection(|connection| {
            read_lesson(
                connection,
                GetLessonInput {
                    lesson_id: format!("{:032x}", 0),
                    revision_number: None,
                },
            )
        })
        .unwrap_err();
    assert_eq!(error.code, "lesson_not_written");
}
