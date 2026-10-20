use std::cell::RefCell;

use super::*;
use crate::lessons::lecture::LectureSlideDto;

fn slide(key: &str, kind: LectureSlideKind) -> LectureSlideDto {
    LectureSlideDto {
        key: key.into(),
        kind,
        step_key: key.strip_prefix("step:").map(str::to_string),
        title: key.into(),
        points: vec!["A point".into()],
        notes: String::new(),
    }
}

/// A title slide, `steps` step slides and a sources slide.
fn lecture(steps: usize) -> LectureDto {
    let mut slides = vec![slide("title", LectureSlideKind::Title)];
    slides.extend((1..=steps).map(|n| slide(&format!("step:step-{n}"), LectureSlideKind::Step)));
    slides.push(slide("sources", LectureSlideKind::Sources));
    LectureDto {
        lesson_id: "a".repeat(32),
        revision_number: 1,
        title: "Triangles".into(),
        slides,
    }
}

fn question() -> Value {
    json!({
        "prompt": "Which shape has three sides?",
        "options": [
            {"text": "A square", "reply": "Not quite: a square has four sides."},
            {"text": " A triangle ", "reply": "Right: three sides make a triangle."}
        ],
        "correctOption": 1
    })
}

fn wire(lecture: &LectureDto, question_on: impl Fn(&str) -> bool) -> Value {
    json!({"slides": lecture.slides.iter().map(|slide| json!({
        "slideKey": slide.key,
        "talk": format!("  Look at {} with me.  ", slide.key),
        "question": if question_on(&slide.key) { question() } else { Value::Null },
    })).collect::<Vec<_>>()})
}

#[test]
fn decoding_gives_options_native_identities_and_keeps_the_teachers_replies() {
    let lecture = lecture(2);
    let script = decode(wire(&lecture, |key| key == "step:step-2"), &lecture).unwrap();
    assert_eq!(script.schema_version, LECTURE_SCRIPT_VERSION);
    assert_eq!(script.slides.len(), 4);
    assert_eq!(script.slides[0].talk, "Look at title with me.");
    assert!(script.slides[1].question.is_none());
    let asked = script.slides[2].question.as_ref().unwrap();
    assert_eq!(asked.options[1].id, teaching::option_id("step:step-2", 1));
    assert_eq!(asked.correct_option_id, asked.options[1].id);
    assert_eq!(asked.options[1].text, "A triangle");
    assert_eq!(
        asked.options[0].reply,
        "Not quite: a square has four sides."
    );
    // The same answer always decodes to the same script.
    assert_eq!(
        decode(wire(&lecture, |key| key == "step:step-2"), &lecture).unwrap(),
        script
    );
}

#[test]
fn questions_the_lecture_cannot_hold_are_dropped_not_refused() {
    let lecture = lecture(6);
    let script = decode(wire(&lecture, |_| true), &lecture).unwrap();
    let asked = script
        .slides
        .iter()
        .map(|slide| slide.question.is_some())
        .collect::<Vec<_>>();
    // None on the title or sources slide, and only the first four steps ask.
    assert_eq!(asked, [false, true, true, true, true, false, false, false]);
}

#[test]
fn a_script_that_does_not_follow_the_slides_is_refused() {
    let lecture = lecture(2);
    let mut missing = wire(&lecture, |_| false);
    missing["slides"].as_array_mut().unwrap().pop();
    let mut extra = wire(&lecture, |_| false);
    let last = extra["slides"][3].clone();
    extra["slides"].as_array_mut().unwrap().push(last);
    let mut swapped = wire(&lecture, |_| false);
    swapped["slides"].as_array_mut().unwrap().swap(1, 2);
    let mut silent = wire(&lecture, |_| false);
    silent["slides"][1]["talk"] = json!("   ");
    let mut wrong_answer = wire(&lecture, |key| key == "step:step-1");
    wrong_answer["slides"][1]["question"]["correctOption"] = json!(2);
    let mut one_option = wire(&lecture, |key| key == "step:step-1");
    one_option["slides"][1]["question"]["options"]
        .as_array_mut()
        .unwrap()
        .pop();
    for value in [missing, extra, swapped, silent, wrong_answer, one_option] {
        let error = decode(value, &lecture).unwrap_err();
        assert!(retryable(&error), "{}", error.message);
    }
}

#[test]
fn the_schema_names_every_slide_once_in_order() {
    let lecture = lecture(3);
    let schema = schema(&lecture);
    let slides = &schema["properties"]["slides"];
    assert_eq!(slides["minItems"], 5);
    assert_eq!(slides["maxItems"], 5);
    assert_eq!(
        slides["items"]["properties"]["slideKey"]["enum"],
        json!([
            "title",
            "step:step-1",
            "step:step-2",
            "step:step-3",
            "sources"
        ])
    );
}

fn request() -> ScriptRequest {
    ScriptRequest {
        system: "system".into(),
        user: "user".into(),
        schema: schema(&lecture(2)),
    }
}

#[test]
fn one_repair_follows_an_invalid_answer() {
    let lecture = lecture(2);
    let mut broken = wire(&lecture, |_| false);
    broken["slides"][2]["talk"] = json!("");
    let answers = RefCell::new(vec![wire(&lecture, |_| false), broken]);
    let sent = RefCell::new(Vec::new());
    let script = tauri::async_runtime::block_on(run(
        request(),
        &lecture,
        |_| Ok(()),
        |_| true,
        |request| {
            sent.borrow_mut().push(request.user.clone());
            let answer = answers.borrow_mut().pop().unwrap();
            async move { Ok(answer) }
        },
    ))
    .unwrap();
    assert_eq!(script.slides.len(), 4);
    let sent = sent.into_inner();
    assert_eq!(sent.len(), 2);
    assert!(sent[1].contains("<invalid-lecture-candidate>"));
}

#[test]
fn provider_errors_and_refused_recovery_are_not_repaired() {
    let lecture = lecture(2);
    let calls = RefCell::new(0);
    let error = tauri::async_runtime::block_on(run(
        request(),
        &lecture,
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
        &lecture,
        |_| Ok(()),
        |_| false,
        |_| {
            *calls.borrow_mut() += 1;
            async { Ok(json!({"slides": []})) }
        },
    ))
    .unwrap_err();
    assert!(retryable(&error));
    assert_eq!(*calls.borrow(), 1);
}
