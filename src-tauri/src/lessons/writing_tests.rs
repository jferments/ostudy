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
use crate::knowledge::collection_view::tests::{fixture, install_reducer_profile};

const OPERATION: &str = "0123456789abcdef0123456789abcdef";

/// Tests run in parallel and share the registry of running writers, so each
/// one uses its own operation.
fn operation(marker: char) -> String {
    marker.to_string().repeat(32)
}

/// A local completion server that gives every request the same answer and
/// counts the requests it served. The collection reducer refuses a lesson
/// as its answer and carries on with the native outline, so one answer
/// serves the whole run.
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
    let _turn = WRITING
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    tauri::async_runtime::block_on(write_course(
        database,
        &runtime(),
        WriteSubjectCourseInput {
            subject_id: subject_id.to_string(),
            operation_id: operation_id.to_string(),
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
fn a_course_is_written_one_lesson_per_outline_window_and_saved_with_its_provenance() {
    let (database, subject, _, _, _) = fixture().unwrap();
    let (base_url, served, bodies) = spawn_provider(lesson_answer());
    use_profile(&database, &base_url, '7');
    let native = collection_view::refresh_native(&database, &subject).unwrap();
    let windows =
        collection_view::guide_section_windows(&database, &subject, &native.synthesis).unwrap();
    assert!(!windows.is_empty());

    let course = write(&database, &subject, &operation('2')).unwrap();

    assert_eq!(course.lessons.len(), windows.len());
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
    assert_eq!(
        provenance["catalogFingerprint"],
        windows[0].catalog.fingerprint().unwrap()
    );
    // Each lesson records the unit it explains, stamped with its content.
    assert_eq!(
        count(&database, "SELECT COUNT(*) FROM lesson_revision_units"),
        i64::try_from(windows.len()).unwrap()
    );
    let lesson_requests = bodies
        .lock()
        .unwrap()
        .iter()
        .filter(|body| body.contains("LESSON author"))
        .count();
    assert_eq!(lesson_requests, windows.len());
    assert!(served.load(Ordering::SeqCst) >= lesson_requests);
    assert_eq!(
        count(
            &database,
            "SELECT COUNT(*) FROM generation_requests WHERE stage = 'lesson_authoring'"
        ),
        i64::try_from(windows.len()).unwrap()
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
    assert_eq!(
        count(&database, "SELECT COUNT(*) FROM lessons"),
        i64::try_from(windows.len()).unwrap()
    );
}

#[test]
fn a_course_with_an_unwritable_lesson_saves_nothing() {
    let (database, subject, _, _, _) = fixture().unwrap();
    let (base_url, served, _) = spawn_provider("{}".to_string());
    use_profile(&database, &base_url, '6');

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
fn every_lesson_sees_the_whole_course_with_itself_marked() {
    let window = |title: &str, context: String| CollectionGuideWindow {
        block_key: title.to_string(),
        title: title.to_string(),
        topic_ids: Vec::new(),
        topic_context: context,
        catalog: crate::knowledge::authoring::tests::catalog_fixture(),
    };
    let windows = vec![
        window("First part", "Heart rate".to_string()),
        window(
            "Second part",
            format!("Long   context\n{}", "x".repeat(500)),
        ),
    ];
    let outline = course_outline(&windows, 1);
    let lines = outline.lines().collect::<Vec<_>>();
    assert_eq!(lines, [lines[0], lines[1]]);
    assert_eq!(lines[0], "1. First part: Heart rate");
    assert!(lines[1].starts_with("2. Second part: Long context x"));
    assert!(lines[1].ends_with("<- this lesson"));
    assert!(lines[1].len() < 300);
}
