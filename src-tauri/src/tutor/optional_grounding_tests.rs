//! Regression cases for optional grounding and the duplicate question fields.
use super::*;
use crate::{knowledge::contracts::StudyScope, tutor::history::HistorySnapshot};

fn context() -> StudyContext {
    StudyContext {
        question_hint: None,
        sources: Vec::new(),
        units: Vec::new(),
        history: HistorySnapshot::default(),
        inherited_turns: Vec::new(),
        request_text: String::new(),
        notice: String::new(),
        images: Vec::new(),
        completion_tokens: 100,
    }
}

fn question() -> Value {
    json!({
        "text":"Let's begin. How many sides does a triangle have?",
        "sourceIds":[], "feedback":null,
        "quiz":{
            "question":"How many sides does a triangle have?",
            "expectedAnswer":"Three", "criteria":["States the number of sides."],
            "explanation":"A triangle is a polygon with three sides.",
            "sourceIds":[], "knowledgeUnitIds":[]
        }
    })
}

#[test]
fn reference_free_questions_work_in_every_scope_through_send_and_legacy_quiz() {
    for value in [
        json!({"kind":"general","id":null}),
        json!({"kind":"library","id":null}),
        json!({"kind":"deck","id":"a".repeat(32)}),
        json!({"kind":"document","id":"a".repeat(32)}),
        json!({"kind":"guide","id":"a".repeat(32)}),
        json!({"kind":"selection","id":null,"members":[{"kind":"deck","id":"a".repeat(32)}]}),
    ] {
        let scope: StudyScope = serde_json::from_value(value).unwrap();
        scope.validate().unwrap();
        for mode in [TutorMode::Explain, TutorMode::Quiz] {
            let result = validate(question(), mode, &scope, &context(), None).unwrap();
            assert_eq!(result.text, "How many sides does a triangle have?");
            let quiz = result.quiz.unwrap();
            assert!(quiz.source_ids.is_empty());
            assert!(quiz.knowledge_unit_ids.is_empty());
            assert_eq!(quiz.expected_answer, "Three");
        }
    }
}

#[test]
fn redundant_public_prose_cannot_disclose_a_private_answer() {
    let scope: StudyScope = serde_json::from_value(json!({"kind":"general"})).unwrap();
    let mut value = question();
    value["text"] = json!("The answer is Three. How many sides does a triangle have?");
    let result = validate(value, TutorMode::Explain, &scope, &context(), None).unwrap();
    assert_eq!(result.text, "How many sides does a triangle have?");
    assert!(!result.text.contains("Three"));
}

#[test]
fn optional_grounding_never_admits_fabricated_references_or_answer_spoilers() {
    let scope: StudyScope = serde_json::from_value(json!({"kind":"general"})).unwrap();
    for field in ["sourceIds", "knowledgeUnitIds"] {
        let mut value = question();
        value["quiz"][field] = json!(["f".repeat(32)]);
        assert!(validate(value, TutorMode::Explain, &scope, &context(), None).is_err());
    }
    let mut value = question();
    value["quiz"]["question"] = json!("Is Three the answer?");
    assert!(validate(value, TutorMode::Explain, &scope, &context(), None).is_err());
}

#[test]
fn reference_free_question_can_be_evaluated_against_its_saved_rubric() {
    let scope: StudyScope =
        serde_json::from_value(json!({"kind":"deck","id":"a".repeat(32)})).unwrap();
    let question = validate(question(), TutorMode::Explain, &scope, &context(), None).unwrap();
    let fixed = question.quiz.unwrap();
    let result = validate(
        json!({
            "text":"Feedback", "sourceIds":[], "quiz":null,
            "feedback":{"outcome":"correct","explanation":"That is correct.","criteriaFeedback":["The number is correct."]}
        }),
        TutorMode::Answer,
        &scope,
        &context(),
        Some(&fixed),
    )
    .unwrap();
    assert!(result.text.contains("Expected answer: Three"));
    assert!(result.source_ids.is_empty());
}

#[test]
fn prompt_distinguishes_general_knowledge_from_explicit_source_only_requests() {
    let prompt = system_prompt(TutorMode::Explain);
    assert!(prompt.contains("Sources are optional additional grounding"));
    assert!(prompt.contains("instead of silently substituting general knowledge"));
    assert!(prompt.contains("Never invent an ID"));
}
