use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    thread,
};

use rusqlite::params;

use super::*;
use crate::{
    document_domain::{SourceLocator, SourceUnitKind},
    document_learning::{
        ensure_document_learning_plan, DocumentLearningPlanInput, LearningEvidenceChunk,
        LearningTopicInput,
    },
    knowledge::collection_view::tests::{fixture, install_reducer_profile},
};

const OPERATION: &str = "0123456789abcdef0123456789abcdef";

/// Tests run in parallel and share the registry of running writers, so each
/// one uses its own operation.
fn operation(marker: char) -> String {
    marker.to_string().repeat(32)
}

/// A local completion server that gives every request the same answer and
/// counts the requests it served.
fn spawn_provider(content: String) -> (String, Arc<AtomicUsize>, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let served = Arc::new(AtomicUsize::new(0));
    let bodies = Arc::new(Mutex::new(Vec::new()));
    let (count, seen) = (served.clone(), bodies.clone());
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            let mut request = Vec::new();
            let mut buffer = [0_u8; 8192];
            let header_end = loop {
                let read = stream.read(&mut buffer).unwrap_or(0);
                if read == 0 {
                    break None;
                }
                request.extend_from_slice(&buffer[..read]);
                if let Some(index) = request.windows(4).position(|window| window == b"\r\n\r\n") {
                    break Some(index + 4);
                }
            };
            let Some(header_end) = header_end else {
                continue;
            };
            let headers = String::from_utf8_lossy(&request[..header_end]).to_string();
            let length = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().unwrap())
                })
                .unwrap_or(0);
            while request.len() < header_end + length {
                let read = stream.read(&mut buffer).unwrap_or(0);
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..read]);
            }
            seen.lock()
                .unwrap()
                .push(String::from_utf8_lossy(&request[header_end..]).to_string());
            count.fetch_add(1, Ordering::SeqCst);
            let event =
                json!({"choices":[{"index":0,"delta":{"content":content},"finish_reason":"stop"}]});
            let body = format!("data: {event}\n\ndata: [DONE]\n\n");
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });
    (format!("http://{address}/"), served, bodies)
}

/// Makes the reducer test profile the default document model.
fn use_profile(database: &Database, base_url: &str, marker: char) {
    let task = install_reducer_profile(database, base_url, marker);
    database
        .with_connection(|connection| {
            connection
                .execute(
                    "INSERT OR REPLACE INTO ai_task_defaults(
                         task_kind, profile_id, model_id, reasoning_effort, updated_at
                     ) VALUES ('flashcard_generation', ?1, 'fixture', 'none', 1)",
                    params![task.profile_id],
                )
                .map_err(CommandError::database)
        })
        .unwrap();
}

/// Gives each document a one-topic learning plan over its one page.
fn plan(database: &Database, documents: &[&str]) {
    let locator = SourceLocator::new(SourceUnitKind::Page, vec![1], None).unwrap();
    for document in documents {
        ensure_document_learning_plan(
            database,
            document,
            DocumentLearningPlanInput {
                summary: "Plan".into(),
                learning_objectives: Vec::new(),
                key_concepts: Vec::new(),
                topics: vec![LearningTopicInput {
                    parent_index: None,
                    title: "Hemodynamics".into(),
                    summary: "What cardiac output depends on.".into(),
                    learning_objectives: vec!["Explain cardiac output.".into()],
                    key_concepts: Vec::new(),
                    source_locator: locator.clone(),
                }],
            },
            &[LearningEvidenceChunk {
                chunk_index: 0,
                source_locator: locator.clone(),
                content_hash: "f".repeat(64),
            }],
        )
        .unwrap();
    }
}

fn lesson_answer() -> String {
    json!({
        "title": "Cardiac output",
        "summary": "What cardiac output depends on.",
        "objectives": ["Say what cardiac output depends on."],
        "steps": [{
            "title": "Heart rate",
            "blocks": [{"kind": "explanation", "heading": null, "body": "Cardiac output depends on heart rate."}],
            "narration": "Cardiac output depends on how fast the heart beats.",
            "slidePoints": ["Output depends on heart rate"],
            "check": {
                "timing": "after",
                "style": "recall",
                "aboutStep": 1,
                "prompt": "What does cardiac output depend on?",
                "options": ["Heart rate", "Hair colour"],
                "correctOption": 0,
                "explanation": "Cardiac output depends on heart rate."
            },
            "knowledgeUnitIds": ["K0"]
        }]
    })
    .to_string()
}

/// The collection fixture gives every test the same subject id, and one
/// writer runs per subject, so tests that write a course take turns.
static WRITING: Mutex<()> = Mutex::new(());

fn write(
    database: &Database,
    subject_id: &str,
    operation_id: &str,
) -> Result<SubjectLessonsDto, CommandError> {
    run_course(database, subject_id, operation_id, false)
}

fn rewrite(
    database: &Database,
    subject_id: &str,
    operation_id: &str,
) -> Result<SubjectLessonsDto, CommandError> {
    run_course(database, subject_id, operation_id, true)
}

fn run_course(
    database: &Database,
    subject_id: &str,
    operation_id: &str,
    rewrite: bool,
) -> Result<SubjectLessonsDto, CommandError> {
    let _turn = WRITING
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    tauri::async_runtime::block_on(write_course(
        database,
        &runtime(),
        WriteSubjectCourseInput {
            subject_id: subject_id.to_string(),
            operation_id: operation_id.to_string(),
            rewrite,
        },
    ))
}

fn runtime() -> AiRuntimeState {
    AiRuntimeState::new(crate::platform::unavailable_secret_store()).unwrap()
}

fn count(database: &Database, sql: &str) -> i64 {
    database
        .with_connection(|connection| {
            connection
                .query_row(sql, [], |row| row.get(0))
                .map_err(CommandError::database)
        })
        .unwrap()
}

#[test]
fn a_course_is_written_one_lesson_per_document_topic_and_saved_with_its_provenance() {
    let (database, subject, first, second, _) = fixture().unwrap();
    let (base_url, served, bodies) = spawn_provider(lesson_answer());
    use_profile(&database, &base_url, '7');
    plan(&database, &[&first, &second]);

    let course = write(&database, &subject, &operation('2')).unwrap();

    // One topic in each of the two documents.
    let lessons: i64 = 2;
    assert_eq!(course.lessons.len(), 2);
    for lesson in &course.lessons {
        let written = lesson.written.as_ref().unwrap();
        assert_eq!(written.origin, "generated");
        assert_eq!(written.step_count, 1);
        assert_eq!(written.check_count, 1);
    }
    let provenance: Value = serde_json::from_str(
        &database
            .with_connection(|connection| {
                connection
                    .query_row(
                        "SELECT provenance_json FROM lesson_revisions LIMIT 1",
                        [],
                        |row| row.get::<_, String>(0),
                    )
                    .map_err(CommandError::database)
            })
            .unwrap(),
    )
    .unwrap();
    assert_eq!(provenance["promptVersion"], LESSON_PROMPT_VERSION);
    assert_eq!(provenance["model"], "fixture");
    assert_eq!(provenance["operationId"], operation('2'));
    assert_eq!(provenance["topicIds"].as_array().unwrap().len(), 1);
    assert!(provenance["learningPlanId"].is_string());
    // Each lesson records the unit it explains, stamped with its content.
    assert_eq!(
        count(&database, "SELECT COUNT(*) FROM lesson_revision_units"),
        lessons
    );
    let lesson_requests = bodies
        .lock()
        .unwrap()
        .iter()
        .filter(|body| body.contains("LESSON author"))
        .count();
    assert_eq!(lesson_requests, 2);
    assert_eq!(served.load(Ordering::SeqCst), lesson_requests);
    assert_eq!(
        count(
            &database,
            "SELECT COUNT(*) FROM generation_requests WHERE stage = 'lesson_authoring'"
        ),
        lessons
    );

    // Writing the course again changes nothing and asks the provider nothing.
    let before = served.load(Ordering::SeqCst);
    assert_eq!(
        write(&database, &subject, &operation('2'))
            .err()
            .unwrap()
            .code,
        "lesson_course_exists"
    );
    assert_eq!(served.load(Ordering::SeqCst), before);
    assert_eq!(count(&database, "SELECT COUNT(*) FROM lessons"), lessons);
}

#[test]
fn a_course_with_an_unwritable_lesson_saves_nothing() {
    let (database, subject, first, second, _) = fixture().unwrap();
    let (base_url, served, _) = spawn_provider("{}".to_string());
    use_profile(&database, &base_url, '6');
    plan(&database, &[&first, &second]);

    let error = write(&database, &subject, &operation('3')).err().unwrap();

    assert_eq!(error.code, "ai_document_output_invalid");
    assert!(served.load(Ordering::SeqCst) >= 2);
    assert_eq!(count(&database, "SELECT COUNT(*) FROM lessons"), 0);
    assert_eq!(count(&database, "SELECT COUNT(*) FROM lesson_revisions"), 0);
}

#[test]
fn a_subject_without_knowledge_is_refused_before_any_provider_work() {
    let (database, _, _, _, _) = fixture().unwrap();
    let (base_url, served, _) = spawn_provider(lesson_answer());
    use_profile(&database, &base_url, '5');
    let empty = "e".repeat(32);
    database
        .with_connection(|connection| {
            connection
                .execute(
                    "INSERT INTO document_collections(id,name,sort_order,created_at,updated_at,revision)
                     VALUES(?1,'Empty',2000,1,1,1)",
                    [&empty],
                )
                .map_err(CommandError::database)
        })
        .unwrap();

    assert_eq!(
        write(&database, &empty, &operation('4'))
            .err()
            .unwrap()
            .code,
        "lesson_course_empty"
    );
    assert_eq!(
        write(&database, &"f".repeat(32), &operation('4'))
            .err()
            .unwrap()
            .code,
        "subject_not_found"
    );
    assert_eq!(served.load(Ordering::SeqCst), 0);
}

#[test]
fn a_subject_with_an_unprocessed_document_is_refused_before_any_provider_work() {
    let (database, subject, first, _, _) = fixture().unwrap();
    let (base_url, served, _) = spawn_provider(lesson_answer());
    use_profile(&database, &base_url, '8');
    plan(&database, &[&first]);

    assert_eq!(
        write(&database, &subject, &operation('9'))
            .err()
            .unwrap()
            .code,
        "lesson_documents_unprepared"
    );
    assert_eq!(served.load(Ordering::SeqCst), 0);
}

#[test]
fn one_writer_per_subject_and_stopping_an_unknown_run_says_so() {
    let subject = "d".repeat(32);
    let guard = Guard::begin(&subject, OPERATION).unwrap();
    assert_eq!(
        Guard::begin(&subject, &"1".repeat(32)).err().unwrap().code,
        "lesson_writing_already_running"
    );
    cancel_subject_course_writing(CancelSubjectCourseWritingInput {
        operation_id: OPERATION.to_string(),
    })
    .unwrap();
    assert!(guard.signal().is_canceled());
    assert!(ensure_running(&guard.signal()).is_err());
    drop(guard);
    assert_eq!(
        cancel_subject_course_writing(CancelSubjectCourseWritingInput {
            operation_id: OPERATION.to_string(),
        })
        .err()
        .unwrap()
        .code,
        "lesson_writing_not_running"
    );
}

#[test]
fn a_running_course_reports_lessons_planned_and_written() {
    let subject = "e".repeat(32);
    // Not operation('2'), which the course-writing tests run beside this one.
    let operation = "7".repeat(32);
    let progress = |operation: &str| {
        get_subject_course_writing_progress(GetSubjectCourseWritingProgressInput {
            operation_id: operation.to_string(),
        })
    };
    let guard = Guard::begin(&subject, &operation).unwrap();
    assert_eq!(
        progress(&operation).unwrap(),
        CourseWritingProgressDto {
            planned: None,
            written: 0
        }
    );
    guard.progress().plan(3);
    guard.progress().wrote_one();
    assert_eq!(
        progress(&operation).unwrap(),
        CourseWritingProgressDto {
            planned: Some(3),
            written: 1
        }
    );
    drop(guard);
    assert_eq!(
        progress(&operation).err().unwrap().code,
        "lesson_writing_not_running"
    );
}

fn part(title: &str, summary: String) -> CoursePart {
    CoursePart {
        document_id: String::new(),
        plan_id: String::new(),
        topic_ids: vec![title.to_string()],
        title: title.to_string(),
        summary,
        objectives: String::new(),
        catalog: crate::knowledge::authoring::tests::catalog_fixture(),
    }
}

#[test]
fn every_lesson_sees_the_whole_course_with_itself_marked() {
    let parts = vec![
        part("First part", "Heart rate".to_string()),
        part(
            "Second part",
            format!("Long   context\n{}", "x".repeat(500)),
        ),
    ];
    let outline = course_outline(&parts, 1);
    let lines = outline.lines().collect::<Vec<_>>();
    assert_eq!(lines, [lines[0], lines[1]]);
    assert_eq!(lines[0], "1. First part: Heart rate");
    assert!(lines[1].starts_with("2. Second part: Long context x"));
    assert!(lines[1].ends_with("<- this lesson"));
    assert!(lines[1].len() < 300);
}

#[test]
fn subtopics_join_their_top_level_topic_in_plan_order() {
    let topic = |id: char, parent: Option<char>, sort_index: usize| LearningTopic {
        id: id.to_string().repeat(32),
        parent_topic_id: parent.map(|parent| parent.to_string().repeat(32)),
        sort_index,
        title: format!("Topic {id}"),
        summary: String::new(),
        learning_objectives: Vec::new(),
        key_concepts: Vec::new(),
        source_locator: SourceLocator::new(SourceUnitKind::Page, vec![1], None).unwrap(),
        evidence_chunk_indices: vec![0],
        evidence_hashes: Vec::new(),
        evidence_hash: String::new(),
        anchor_hash: String::new(),
    };
    let plan = DocumentLearningPlan {
        id: "1".repeat(32),
        document_id: "2".repeat(32),
        revision: 1,
        compiler_version: 1,
        outline_fingerprint: String::new(),
        plan_fingerprint: String::new(),
        summary: String::new(),
        learning_objectives: Vec::new(),
        key_concepts: Vec::new(),
        // A grandchild listed before its parent still joins its root.
        topics: vec![
            topic('a', None, 0),
            topic('d', Some('c'), 2),
            topic('c', Some('a'), 1),
            topic('b', None, 3),
            topic('e', Some('b'), 4),
        ],
    };
    let groups = topic_groups(&plan)
        .iter()
        .map(|topics| {
            topics
                .iter()
                .map(|topic| topic.title.as_str())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        groups,
        [
            vec!["Topic a", "Topic c", "Topic d"],
            vec!["Topic b", "Topic e"]
        ]
    );
}

/// A teacher's lecture over every slide, asking one question on the step.
fn lecture_answer(lecture: &crate::lessons::lecture::LectureDto) -> String {
    json!({"slides": lecture.slides.iter().map(|slide| json!({
        "slideKey": slide.key,
        "talk": format!("Look at {} with me, because it matters.", slide.title),
        "question": if slide.key == "step:step-1" {
            json!({
                "timing": "before",
                "style": "predict",
                "prompt": "If the heart beats faster, what happens to cardiac output?",
                "options": [
                    {"text": "It rises", "reply": "Right. Output depends on heart rate, so it rises."},
                    {"text": "It falls", "reply": "Not quite. Output depends on heart rate, so it rises."}
                ],
                "correctOption": 0
            })
        } else {
            Value::Null
        },
    })).collect::<Vec<_>>()})
    .to_string()
}

#[test]
fn a_teachers_lecture_is_written_once_kept_and_answered_without_the_provider() {
    use crate::ai::document_intelligence::LECTURE_SCRIPT_PROMPT_VERSION;
    use crate::lessons::{
        lecture::read_lecture,
        teaching::{self, AnswerLectureQuestionInput, WriteLectureScriptInput},
        GetLessonInput,
    };

    let (database, subject, first, second, _) = fixture().unwrap();
    let (base_url, _, _) = spawn_provider(lesson_answer());
    use_profile(&database, &base_url, 'b');
    plan(&database, &[&first, &second]);
    let course = write(&database, &subject, &operation('c')).unwrap();
    let lesson_id = course.lessons[0].id.clone();
    let lecture = database
        .with_connection(|connection| {
            read_lecture(
                connection,
                GetLessonInput {
                    lesson_id: lesson_id.clone(),
                    revision_number: Some(1),
                },
            )
        })
        .unwrap();

    let (lecture_url, served, bodies) = spawn_provider(lecture_answer(&lecture));
    database
        .with_connection(|connection| {
            connection
                .execute(
                    "UPDATE ai_provider_profiles SET base_url = ?1",
                    [&lecture_url],
                )
                .map_err(CommandError::database)
        })
        .unwrap();
    let script = tauri::async_runtime::block_on(teaching::write(
        &database,
        &runtime(),
        WriteLectureScriptInput {
            lesson_id: lesson_id.clone(),
            revision_number: 1,
            operation_id: operation('d'),
        },
    ))
    .unwrap();
    assert_eq!(served.load(Ordering::SeqCst), 1);
    assert!(bodies.lock().unwrap()[0].contains("LECTURE writer"));
    assert_eq!(script.slides.len(), lecture.slides.len());
    let asked = script.slides[1].question.as_ref().unwrap();
    // The learner sees the question but not its answer or the replies.
    let shown = serde_json::to_string(&script).unwrap();
    assert!(!shown.contains("correctOptionId"));
    assert!(!shown.contains("Not quite"));
    assert!(shown.contains("\"timing\":\"before\""));
    assert_eq!(
        count(
            &database,
            "SELECT COUNT(*) FROM generation_requests WHERE stage = 'lecture_authoring'"
        ),
        1
    );
    let provenance: Value = serde_json::from_str(
        &database
            .with_connection(|connection| {
                connection
                    .query_row("SELECT provenance_json FROM lecture_scripts", [], |row| {
                        row.get::<_, String>(0)
                    })
                    .map_err(CommandError::database)
            })
            .unwrap(),
    )
    .unwrap();
    assert_eq!(provenance["promptVersion"], LECTURE_SCRIPT_PROMPT_VERSION);

    // Answering is graded from the kept script, with no provider call and no
    // review written.
    let reviews = count(&database, "SELECT COUNT(*) FROM review_events");
    let answer = |option: usize| {
        database
            .with_connection(|connection| {
                teaching::answer(
                    connection,
                    &AnswerLectureQuestionInput {
                        lesson_id: lesson_id.clone(),
                        revision_number: 1,
                        slide_key: "step:step-1".into(),
                        option_id: asked.options[option].id.clone(),
                    },
                )
            })
            .unwrap()
    };
    let right = answer(0);
    assert!(right.correct);
    assert!(right.reply.starts_with("Right."));
    let wrong = answer(1);
    assert!(!wrong.correct);
    assert_eq!(wrong.correct_option_id, asked.options[0].id);
    assert!(wrong.reply.starts_with("Not quite."));
    assert_eq!(served.load(Ordering::SeqCst), 1);
    assert_eq!(
        count(&database, "SELECT COUNT(*) FROM review_events"),
        reviews
    );

    // The teacher speaks only the kept text.
    let reply = database
        .with_connection(|connection| {
            teaching::spoken_text(
                connection,
                &lesson_id,
                1,
                "step:step-1",
                teaching::SpokenPart::Reply(&asked.options[1].id),
            )
        })
        .unwrap();
    assert!(reply.starts_with("Not quite."));
    // A question asked before the talk is spoken as written.
    let asking = database
        .with_connection(|connection| {
            teaching::spoken_text(
                connection,
                &lesson_id,
                1,
                "step:step-1",
                teaching::SpokenPart::Question,
            )
        })
        .unwrap();
    assert_eq!(
        asking,
        "If the heart beats faster, what happens to cardiac output?"
    );

    // Writing again replaces the revision's script; deleting the lesson
    // deletes it.
    tauri::async_runtime::block_on(teaching::write(
        &database,
        &runtime(),
        WriteLectureScriptInput {
            lesson_id: lesson_id.clone(),
            revision_number: 1,
            operation_id: operation('e'),
        },
    ))
    .unwrap();
    assert_eq!(count(&database, "SELECT COUNT(*) FROM lecture_scripts"), 1);
    database
        .with_connection(|connection| {
            connection
                .execute("DELETE FROM lessons WHERE id = ?1", [&lesson_id])
                .map_err(CommandError::database)
        })
        .unwrap();
    assert_eq!(count(&database, "SELECT COUNT(*) FROM lecture_scripts"), 0);
}

#[test]
fn a_podcast_is_written_from_the_picked_lessons_and_kept_with_its_hosts_voices() {
    use crate::ai::document_intelligence::PODCAST_SCRIPT_PROMPT_VERSION;
    use crate::lessons::podcasts::{self, WritePodcastInput};

    use crate::lessons::{authoring::create_authored_lesson, fixture::loop_of_henle};

    // Authored lessons, so no course writer runs beside the other tests'.
    let (database, subject, _, _, _) = fixture().unwrap();
    let mut picked = database
        .with_connection(|connection| {
            (0..2)
                .map(|_| {
                    Ok(create_authored_lesson(connection, &subject, loop_of_henle(&[]), 1)?.id)
                })
                .collect::<Result<Vec<_>, CommandError>>()
        })
        .unwrap();
    picked.reverse();

    let answer = json!({
        "title": "Why the heart speeds up",
        "chapters": (1..=picked.len()).map(|index| json!({
            "lessonKey": format!("lesson-{index}"),
            "turns": [
                {"host": "Ada", "text": "Cardiac output depends on heart rate."},
                {"host": "Ben", "text": "So a faster heart pumps more?"},
            ],
        })).collect::<Vec<_>>(),
    })
    .to_string();
    let (podcast_url, served, bodies) = spawn_provider(answer);
    use_profile(&database, &podcast_url, 'b');
    let podcast = tauri::async_runtime::block_on(podcasts::write(
        &database,
        &runtime(),
        WritePodcastInput {
            subject_id: subject.clone(),
            lesson_ids: picked.clone(),
            target_minutes: 10,
            voices: None,
            operation_id: operation('a'),
        },
    ))
    .unwrap();
    assert_eq!(served.load(Ordering::SeqCst), 1);
    let sent = &bodies.lock().unwrap()[0];
    assert!(sent.contains("PODCAST writer"));
    assert!(sent.contains("about 1500 words"));
    assert_eq!(podcast.title, "Why the heart speeds up");
    assert_eq!(podcast.target_minutes, 10);
    // Chapters follow the learner's order, not the course's.
    assert_eq!(
        podcast
            .chapters
            .iter()
            .map(|chapter| chapter.lesson_id.clone())
            .collect::<Vec<_>>(),
        picked
    );
    assert_eq!(podcast.hosts[0].voice, "builtin:calm-older-woman");
    assert_eq!(podcast.hosts[1].voice, "builtin:relaxed-man");
    assert_eq!(
        count(
            &database,
            "SELECT COUNT(*) FROM generation_requests WHERE stage = 'podcast_authoring'"
        ),
        1
    );
    let provenance: Value = serde_json::from_str(
        &database
            .with_connection(|connection| {
                connection
                    .query_row("SELECT provenance_json FROM podcasts", [], |row| {
                        row.get::<_, String>(0)
                    })
                    .map_err(CommandError::database)
            })
            .unwrap(),
    )
    .unwrap();
    assert_eq!(provenance["promptVersion"], PODCAST_SCRIPT_PROMPT_VERSION);
    let listed = database
        .with_connection(|connection| podcasts::list(connection, &subject))
        .unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, podcast.id);
}

fn update(
    database: &Database,
    lesson_id: &str,
    operation_id: &str,
) -> Result<SubjectLessonsDto, CommandError> {
    let _turn = WRITING
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    tauri::async_runtime::block_on(update_lesson(
        database,
        &runtime(),
        UpdateLessonInput {
            lesson_id: lesson_id.to_string(),
            operation_id: operation_id.to_string(),
        },
    ))
}

fn execute(database: &Database, sql: &str, lesson_id: &str) {
    database
        .with_connection(|connection| {
            connection
                .execute(sql, [lesson_id])
                .map_err(CommandError::database)
        })
        .unwrap();
}

#[test]
fn a_lesson_whose_source_changed_is_written_again_as_its_next_revision() {
    let (database, subject, first, second, _) = fixture().unwrap();
    let (base_url, served, bodies) = spawn_provider(lesson_answer());
    use_profile(&database, &base_url, 'a');
    plan(&database, &[&first, &second]);
    let course = write(&database, &subject, &operation('a')).unwrap();
    let lesson = &course.lessons[1];
    // The knowledge the lesson explains changes.
    execute(
        &database,
        "UPDATE knowledge_units SET content_hash = 'e' || substr(content_hash, 2)
         WHERE id IN (SELECT unit_id FROM lesson_revision_units WHERE lesson_id = ?1)",
        &lesson.id,
    );
    let before = served.load(Ordering::SeqCst);
    let listed = list_lessons_now(&database, &subject);
    assert_eq!(
        listed.lessons[1].written.as_ref().unwrap().sources,
        crate::lessons::SourceStatus::Changed
    );

    let updated = update(&database, &lesson.id, &operation('b')).unwrap();

    // One request, for this lesson only, which knows the rest of the course.
    assert_eq!(served.load(Ordering::SeqCst), before + 1);
    assert!(bodies
        .lock()
        .unwrap()
        .last()
        .unwrap()
        .contains("this lesson"));
    assert_eq!(updated.lessons.len(), 2);
    assert_eq!(updated.lessons[1].id, lesson.id);
    let written = updated.lessons[1].written.as_ref().unwrap();
    assert_eq!(written.revision_number, 2);
    assert_eq!(written.origin, "generated");
    assert_eq!(written.sources, crate::lessons::SourceStatus::Current);
    // The other lesson and the first revision stay as they were.
    assert_eq!(
        updated.lessons[0].written.as_ref().unwrap().revision_number,
        1
    );
    assert_eq!(count(&database, "SELECT COUNT(*) FROM lesson_revisions"), 3);
    let provenance: Value = serde_json::from_str(
        &database
            .with_connection(|connection| {
                connection
                    .query_row(
                        "SELECT provenance_json FROM lesson_revisions
                         WHERE lesson_id = ?1 AND revision_number = 2",
                        [&lesson.id],
                        |row| row.get::<_, String>(0),
                    )
                    .map_err(CommandError::database)
            })
            .unwrap(),
    )
    .unwrap();
    assert_eq!(provenance["operationId"], operation('b'));
}

#[test]
fn a_lesson_is_not_written_again_without_its_part_or_its_provenance() {
    let (database, subject, first, second, _) = fixture().unwrap();
    let (base_url, served, _) = spawn_provider(lesson_answer());
    use_profile(&database, &base_url, 'c');
    plan(&database, &[&first, &second]);
    let course = write(&database, &subject, &operation('c')).unwrap();
    let lesson = course.lessons[0].id.clone();
    let before = served.load(Ordering::SeqCst);

    // The topic it taught is no longer in the plan.
    execute(
        &database,
        "UPDATE lesson_revisions
         SET provenance_json = json_set(provenance_json, '$.topicIds[0]', 'gone')
         WHERE lesson_id = ?1",
        &lesson,
    );
    assert_eq!(
        update(&database, &lesson, &operation('d'))
            .err()
            .unwrap()
            .code,
        "lesson_part_gone"
    );
    // A lesson written by hand has no sources to write it from.
    execute(
        &database,
        "UPDATE lesson_revisions SET origin = 'authored', provenance_json = NULL
         WHERE lesson_id = ?1",
        &lesson,
    );
    assert_eq!(
        update(&database, &lesson, &operation('d'))
            .err()
            .unwrap()
            .code,
        "lesson_not_generated"
    );
    assert_eq!(
        update(&database, &"9".repeat(32), &operation('d'))
            .err()
            .unwrap()
            .code,
        "lesson_not_found"
    );
    assert_eq!(served.load(Ordering::SeqCst), before);
    assert_eq!(count(&database, "SELECT COUNT(*) FROM lesson_revisions"), 2);
}

fn list_lessons_now(database: &Database, subject: &str) -> SubjectLessonsDto {
    database
        .with_connection(|connection| list_lessons(connection, subject))
        .unwrap()
}

#[test]
fn a_rewritten_course_revises_lessons_by_part_adds_new_parts_and_keeps_the_rest() {
    let (database, subject, first, second, _) = fixture().unwrap();
    let (base_url, served, _) = spawn_provider(lesson_answer());
    use_profile(&database, &base_url, 'e');
    plan(&database, &[&first, &second]);
    let course = write(&database, &subject, &operation('e')).unwrap();
    let (old_first, kept) = (course.lessons[0].id.clone(), course.lessons[1].id.clone());
    // The first lesson's part is no longer in the plan.
    execute(
        &database,
        "UPDATE lesson_revisions
         SET provenance_json = json_set(provenance_json, '$.topicIds[0]', 'gone')
         WHERE lesson_id = ?1",
        &old_first,
    );
    let before = served.load(Ordering::SeqCst);

    let rewritten = rewrite(&database, &subject, &operation('f')).unwrap();

    assert_eq!(served.load(Ordering::SeqCst), before + 2);
    let ids = rewritten
        .lessons
        .iter()
        .map(|lesson| lesson.id.clone())
        .collect::<Vec<_>>();
    // A new lesson for the first part, the second lesson's next revision,
    // then the lesson whose part went, unchanged.
    assert_eq!(ids.len(), 3);
    assert!(ids[0] != old_first && ids[0] != kept);
    assert_eq!(ids[1..], [kept.clone(), old_first.clone()]);
    let revision = |index: usize| {
        rewritten.lessons[index]
            .written
            .as_ref()
            .unwrap()
            .revision_number
    };
    assert_eq!((revision(0), revision(1), revision(2)), (1, 2, 1));
    // The kept lesson is marked as no longer in the study plan.
    let in_plan = rewritten
        .lessons
        .iter()
        .map(|lesson| lesson.written.as_ref().unwrap().in_study_plan)
        .collect::<Vec<_>>();
    assert_eq!(in_plan, [true, true, false]);
}
