use std::cell::RefCell;

use super::*;
use crate::{
    cards::questions::QuestionBody, knowledge::authoring::tests::catalog_fixture,
    lessons::content::CalloutKind,
};

fn wire_lesson() -> Value {
    json!({
        "title": "Triangles",
        "summary": "What makes a shape a triangle.",
        "objectives": ["Say how many sides a triangle has."],
        "steps": [
            {
                "title": "Three sides",
                "blocks": [
                    {"kind": "explanation", "heading": "", "body": "A triangle has three sides."},
                    {"kind": "callout", "callout": "key_idea", "title": "Count the sides", "body": "Three sides make a triangle."}
                ],
                "narration": "Look at any triangle and count its sides. There are always three.",
                "slidePoints": ["A triangle has three sides"],
                "check": {
                    "prompt": "How many sides does a triangle have?",
                    "options": ["Three", "Four"],
                    "correctOption": 0,
                    "explanation": "Every triangle has three sides."
                },
                "knowledgeUnitIds": ["K0", "K0"]
            },
            {
                "title": "Why it matters",
                "blocks": [{"kind": "keyPoints", "items": ["Counting sides names the shape."]}],
                "narration": "Counting sides is how we tell shapes apart.",
                "slidePoints": [],
                "check": null,
                "knowledgeUnitIds": []
            }
        ]
    })
}

fn request() -> LessonRequest {
    LessonRequest {
        system: "system".into(),
        user: "user".into(),
        schema: schema(&catalog_fixture()),
    }
}

#[test]
fn decoding_resolves_aliases_and_builds_keys_and_checks_natively() {
    let knowledge = catalog_fixture();
    let lesson = decode(wire_lesson(), &knowledge).unwrap();
    assert_eq!(lesson.schema_version, LESSON_CONTENT_VERSION);
    assert_eq!(
        lesson
            .steps
            .iter()
            .map(|step| step.key.as_str())
            .collect::<Vec<_>>(),
        ["step-1", "step-2"]
    );
    // The repeated alias names one unit, kept once.
    assert_eq!(
        lesson.steps[0].knowledge_unit_ids,
        vec![knowledge.units[0].id.clone()]
    );
    assert_eq!(
        lesson.steps[0].blocks[0],
        LessonBlock::Explanation {
            heading: None,
            body: "A triangle has three sides.".into()
        }
    );
    assert!(matches!(
        lesson.steps[0].blocks[1],
        LessonBlock::Callout {
            callout: CalloutKind::KeyIdea,
            ..
        }
    ));
    let Some(QuestionDefinition {
        question:
            QuestionBody::SingleChoice {
                options,
                correct_option_id,
                ..
            },
        ..
    }) = &lesson.steps[0].check
    else {
        panic!("the first step has a single-choice check");
    };
    assert_eq!(options.len(), 2);
    assert_eq!(*correct_option_id, options[0].id);
    assert_ne!(options[0].id, options[1].id);
    // The same answer decodes to the same lesson.
    assert!(decode(wire_lesson(), &knowledge).unwrap() == lesson);
    assert!(lesson.steps[1].check.is_none());
}

#[test]
fn too_many_or_too_long_slide_points_and_objectives_are_shaped_not_refused() {
    let knowledge = catalog_fixture();
    let long = format!("{} end", "word ".repeat(60));
    let mut wire = wire_lesson();
    wire["steps"][0]["slidePoints"] =
        json!(["one", " ", "two", "three", "four", "five", "six", "seven", long]);
    wire["objectives"] = json!(["", long, "a", "b", "c", "d", "e", "f"]);
    let lesson = decode(wire, &knowledge).unwrap();

    assert_eq!(
        lesson.steps[0].slide_points,
        ["one", "two", "three", "four", "five", "six"]
    );
    assert_eq!(lesson.objectives.len(), 6);
    let shortened = &lesson.objectives[0];
    assert!(shortened.chars().count() <= 260, "{shortened}");
    assert!(shortened.ends_with("word…"), "{shortened}");

    let mut one_long_point = wire_lesson();
    one_long_point["steps"][0]["slidePoints"] = json!([long]);
    let point = &decode(one_long_point, &knowledge).unwrap().steps[0].slide_points[0];
    assert!(
        point.chars().count() <= 200 && point.ends_with('…'),
        "{point}"
    );
}

#[test]
fn decoding_refuses_answers_that_are_not_grounded_or_well_formed() {
    let knowledge = catalog_fixture();
    let mut outside = wire_lesson();
    outside["steps"][0]["knowledgeUnitIds"] = json!(["K7"]);
    let mut ungrounded = wire_lesson();
    ungrounded["steps"][0]["knowledgeUnitIds"] = json!([]);
    let mut wrong_answer = wire_lesson();
    wrong_answer["steps"][0]["check"]["correctOption"] = json!(2);
    let mut unknown_block = wire_lesson();
    unknown_block["steps"][1]["blocks"] = json!([{"kind": "poem", "text": "x"}]);
    let mut empty_title = wire_lesson();
    empty_title["title"] = json!("  ");
    for value in [
        outside,
        ungrounded,
        wrong_answer,
        unknown_block,
        empty_title,
    ] {
        assert_eq!(
            decode(value, &knowledge).err().unwrap().code,
            "ai_document_output_invalid"
        );
    }
}

#[test]
fn the_schema_offers_only_the_catalog_aliases() {
    let mut knowledge = catalog_fixture();
    let mut second = knowledge.units[0].clone();
    second.id = "b".repeat(32);
    knowledge.units.push(second);
    let value = schema(&knowledge);
    let ids = &value["properties"]["steps"]["items"]["properties"]["knowledgeUnitIds"];
    assert_eq!(ids["items"]["enum"], json!(["K0", "K1"]));
    let blocks = &value["properties"]["steps"]["items"]["properties"]["blocks"]["items"]["anyOf"];
    assert_eq!(blocks.as_array().unwrap().len(), 9);
}

#[tokio::test]
async fn an_invalid_answer_gets_one_repair_with_its_candidate() {
    let knowledge = catalog_fixture();
    let sent = RefCell::new(Vec::<LessonRequest>::new());
    let reserved = RefCell::new(Vec::<String>::new());
    let mut invalid = wire_lesson();
    invalid["steps"][0]["knowledgeUnitIds"] = json!(["K9"]);
    let answers = RefCell::new(vec![Ok(wire_lesson()), Ok(invalid)]);
    let lesson = run(
        request(),
        &knowledge,
        |_| Ok(()),
        |error| {
            reserved.borrow_mut().push(error.code.clone());
            true
        },
        |owned| {
            sent.borrow_mut().push(owned);
            let answer = answers.borrow_mut().pop().unwrap();
            async move { answer }
        },
    )
    .await
    .unwrap();
    assert_eq!(lesson.title, "Triangles");
    assert_eq!(*reserved.borrow(), ["ai_document_output_invalid"]);
    let sent = sent.borrow();
    assert_eq!(sent.len(), 2);
    assert!(sent[1].user.contains("<invalid-lesson-candidate>"));
    assert!(sent[1].user.contains("K9"));
}

#[tokio::test]
async fn a_second_invalid_answer_fails_with_a_stable_error() {
    let knowledge = catalog_fixture();
    let calls = RefCell::new(0);
    let error = run(
        request(),
        &knowledge,
        |_| Ok(()),
        |_| true,
        |_| {
            *calls.borrow_mut() += 1;
            async { Ok(json!({"title": "No steps"})) }
        },
    )
    .await
    .err()
    .unwrap();
    assert_eq!(*calls.borrow(), 2);
    assert_eq!(error.code, "ai_document_output_invalid");
    assert!(error.message.contains("invalid lesson twice"));
}

#[tokio::test]
async fn provider_errors_and_spent_budgets_are_not_repaired() {
    let knowledge = catalog_fixture();
    for (code, budget) in [
        ("ai_request_canceled", true),
        ("ai_profile_changed", true),
        ("ai_document_output_invalid", false),
    ] {
        let calls = RefCell::new(0);
        let error = run(
            request(),
            &knowledge,
            |_| Ok(()),
            |_| budget,
            |_| {
                *calls.borrow_mut() += 1;
                async move { Err::<Value, _>(CommandError::new(code, "fixture")) }
            },
        )
        .await
        .err()
        .unwrap();
        assert_eq!(error.code, code);
        assert_eq!(*calls.borrow(), 1, "{code}");
    }
}

#[tokio::test]
async fn a_truncated_answer_is_retried_shorter_without_its_candidate() {
    let knowledge = catalog_fixture();
    let sent = RefCell::new(Vec::<LessonRequest>::new());
    let answers = RefCell::new(vec![
        Ok(wire_lesson()),
        Err(CommandError::new("ai_document_output_truncated", "fixture")),
    ]);
    run(
        request(),
        &knowledge,
        |_| Ok(()),
        |_| true,
        |owned| {
            sent.borrow_mut().push(owned);
            let answer = answers.borrow_mut().pop().unwrap();
            async move { answer }
        },
    )
    .await
    .unwrap();
    let sent = sent.borrow();
    assert!(sent[1].user.contains("at most four steps"));
    assert!(!sent[1].user.contains("<invalid-lesson-candidate>"));
}

#[tokio::test]
async fn a_request_that_does_not_fit_is_never_sent() {
    let knowledge = catalog_fixture();
    let calls = RefCell::new(0);
    let error = run(
        request(),
        &knowledge,
        |_| Err(CommandError::new("ai_document_context_budget", "fixture")),
        |_| true,
        |_| {
            *calls.borrow_mut() += 1;
            async { Ok(wire_lesson()) }
        },
    )
    .await
    .err()
    .unwrap();
    assert_eq!(error.code, "ai_document_context_budget");
    assert_eq!(*calls.borrow(), 0);
}
