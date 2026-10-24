use std::cell::RefCell;

use super::*;
use crate::lessons::fixture::loop_of_henle;

fn lessons(count: usize) -> Vec<PodcastLesson> {
    (0..count)
        .map(|index| PodcastLesson {
            lesson_id: format!("{index:x}").repeat(32),
            revision_number: 2,
            content: loop_of_henle(&[]),
        })
        .collect()
}

fn authoring(lessons: &[PodcastLesson]) -> PodcastScriptAuthoring<'_> {
    PodcastScriptAuthoring {
        lessons,
        target_minutes: 5,
    }
}

fn wire(chapters: usize) -> Value {
    json!({
        "title": "  From filtration to urine ",
        "chapters": (0..chapters).map(|index| json!({
            "lessonKey": format!("lesson-{}", index + 1),
            "turns": [
                {"host": "Ben", "text": " So what does the loop actually do? "},
                {"host": "Ada", "text": "It concentrates the urine, step by step."},
            ],
        })).collect::<Vec<_>>(),
    })
}

#[test]
fn decoding_maps_chapters_to_their_lessons_and_turns_to_their_hosts() {
    let lessons = lessons(2);
    let script = decode(wire(2), &authoring(&lessons)).unwrap();
    assert_eq!(script.schema_version, PODCAST_SCRIPT_VERSION);
    assert_eq!(script.title, "From filtration to urine");
    assert_eq!(script.chapters.len(), 2);
    let chapter = &script.chapters[1];
    assert_eq!(chapter.lesson_id, lessons[1].lesson_id);
    assert_eq!(chapter.revision_number, 2);
    assert_eq!(chapter.title, lessons[1].content.title);
    assert_eq!(chapter.turns[0].host, 1);
    assert_eq!(chapter.turns[0].text, "So what does the loop actually do?");
    assert_eq!(chapter.turns[1].host, 0);
}

#[test]
fn a_podcast_that_does_not_follow_the_lessons_is_refused() {
    let lessons = lessons(2);
    let mut missing = wire(2);
    missing["chapters"].as_array_mut().unwrap().pop();
    let mut swapped = wire(2);
    swapped["chapters"].as_array_mut().unwrap().swap(0, 1);
    let mut stranger = wire(2);
    stranger["chapters"][0]["turns"][0]["host"] = json!("Cy");
    let mut directed = wire(2);
    directed["chapters"][0]["turns"][0]["text"] = json!("[laughs] Right.");
    let mut lonely = wire(2);
    lonely["chapters"][1]["turns"].as_array_mut().unwrap().pop();
    let mut silent = wire(2);
    silent["chapters"][1]["turns"][1]["text"] = json!("  ");
    let mut endless = wire(2);
    endless["chapters"][0]["turns"] = json!((0..30)
        .map(|_| json!({"host": "Ada", "text": "word ".repeat(150)}))
        .collect::<Vec<_>>());
    for value in [
        missing, swapped, stranger, directed, lonely, silent, endless,
    ] {
        let error = decode(value, &authoring(&lessons)).unwrap_err();
        assert!(retryable(&error), "{}", error.message);
    }
}

#[test]
fn the_schema_names_every_lesson_once_in_order_and_both_hosts() {
    let schema = schema(3);
    let chapters = &schema["properties"]["chapters"];
    assert_eq!(chapters["minItems"], 3);
    assert_eq!(chapters["maxItems"], 3);
    assert_eq!(
        chapters["items"]["properties"]["lessonKey"]["enum"],
        json!(["lesson-1", "lesson-2", "lesson-3"])
    );
    assert_eq!(
        chapters["items"]["properties"]["turns"]["items"]["properties"]["host"]["enum"],
        json!(["Ada", "Ben"])
    );
}

#[test]
fn the_writer_sees_the_lessons_teaching_but_not_their_checks() {
    let lessons = lessons(1);
    let prompt = user_prompt(&authoring(&lessons)).unwrap();
    assert!(prompt.contains("\"lessonKey\":\"lesson-1\""));
    assert!(prompt.contains(&lessons[0].content.steps[0].narration));
    assert!(!prompt.contains("\"check\""));
    assert!(!prompt.contains("knowledgeUnitIds"));
    assert!(system_prompt(15).contains("about 2250 words"));
}

fn request() -> ScriptRequest {
    ScriptRequest {
        system: "system".into(),
        user: "user".into(),
        schema: schema(1),
    }
}

#[test]
fn one_repair_follows_an_invalid_answer() {
    let lessons = lessons(1);
    let mut broken = wire(1);
    broken["chapters"][0]["turns"][0]["text"] = json!("");
    let answers = RefCell::new(vec![wire(1), broken]);
    let sent = RefCell::new(Vec::new());
    let script = tauri::async_runtime::block_on(run(
        request(),
        &authoring(&lessons),
        |_| Ok(()),
        |_| true,
        |request| {
            sent.borrow_mut().push(request.user.clone());
            let answer = answers.borrow_mut().pop().unwrap();
            async move { Ok(answer) }
        },
    ))
    .unwrap();
    assert_eq!(script.chapters.len(), 1);
    let sent = sent.into_inner();
    assert_eq!(sent.len(), 2);
    assert!(sent[1].contains("<invalid-podcast-candidate>"));
}

#[test]
fn provider_errors_and_refused_recovery_are_not_repaired() {
    let lessons = lessons(1);
    let calls = RefCell::new(0);
    let error = tauri::async_runtime::block_on(run(
        request(),
        &authoring(&lessons),
        |_| Ok(()),
        |_| true,
        |_| {
            *calls.borrow_mut() += 1;
            async { Err(CommandError::new("ai_provider_unavailable", "offline")) }
        },
    ))
    .unwrap_err();
    assert_eq!(error.code, "ai_provider_unavailable");
    assert_eq!(*calls.borrow(), 1);

    *calls.borrow_mut() = 0;
    let error = tauri::async_runtime::block_on(run(
        request(),
        &authoring(&lessons),
        |_| Ok(()),
        |_| false,
        |_| {
            *calls.borrow_mut() += 1;
            async { Ok(json!({"title": "x", "chapters": []})) }
        },
    ))
    .unwrap_err();
    assert!(retryable(&error));
    assert_eq!(*calls.borrow(), 1);
}
