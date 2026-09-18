use serde_json::{json, Value};

use super::*;
use crate::questions::{
    DisplayOrder, InstanceIdentity, QuestionDefinition, ValidatedQuestion, Verdict,
};

fn id(number: u32) -> String {
    format!("{number:032x}")
}

fn rich(text: &str) -> Value {
    json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":text}]}]})
}

fn body(kind: &str) -> Value {
    let options = json!([
        {"id":id(1),"content":rich("First"),"rationale":rich("PRIVATE RATIONALE")},
        {"id":id(2),"content":rich("Second"),"rationale":null}
    ]);
    match kind {
        "single_choice" => json!({
            "kind":kind,"prompt":rich("Choose one"),"options":options,
            "correctOptionId":id(2),"explanation":rich("PRIVATE EXPLANATION")
        }),
        "select_all" => json!({
            "kind":kind,"prompt":rich("Select all"),"options":options,
            "correctOptionIds":[],"explanation":null
        }),
        "true_false" => json!({
            "kind":kind,"statement":rich("A proposition"),"correct":false,
            "explanation":null,"correctedStatement":rich("PRIVATE CORRECTION")
        }),
        "ordering" => json!({
            "kind":kind,"prompt":rich("Order these"),"explanation":null,
            "items":[{"id":id(1),"content":rich("First")},
                     {"id":id(2),"content":rich("Second")}]
        }),
        "matching" => json!({
            "kind":kind,"prompt":rich("Match these"),"explanation":null,
            "pairs":[
                {"left":{"id":id(1),"content":rich("A")},
                 "right":{"id":id(3),"content":rich("C")},"rationale":null},
                {"left":{"id":id(2),"content":rich("B")},
                 "right":{"id":id(4),"content":rich("D")},"rationale":null}
            ]
        }),
        _ => panic!("unsupported fixture kind"),
    }
}

fn captured(body: Value) -> QuestionInstance {
    let kind = body["kind"].as_str().unwrap();
    let order = if kind == "true_false" {
        DisplayOrder::default()
    } else {
        DisplayOrder {
            item_ids: vec![id(2), id(1)],
            target_ids: if kind == "matching" {
                vec![id(4), id(3)]
            } else {
                vec![]
            },
        }
    };
    let definition: QuestionDefinition =
        serde_json::from_value(json!({"schemaVersion":1,"question":body})).unwrap();
    QuestionInstance::capture(
        InstanceIdentity {
            instance_id: id(10),
            note_id: id(11),
            card_id: id(12),
            note_revision: 1,
        },
        ValidatedQuestion::validate(definition).unwrap(),
        "main".to_string(),
        order,
    )
    .unwrap()
}

fn attempt(kind: &str) -> QuestionAttempt {
    QuestionAttempt::start(captured(body(kind))).unwrap()
}

fn answer(response: Value) -> LearnerAnswer {
    serde_json::from_value(json!({"schemaVersion":1,"instanceId":id(10),"response":response}))
        .unwrap()
}

fn single(selected: Option<u32>) -> LearnerAnswer {
    answer(json!({"kind":"single_choice","selectedOptionId":selected.map(id)}))
}

fn save(revision: i64, mutation: u32, answer: LearnerAnswer) -> SaveAnswerRequest {
    SaveAnswerRequest {
        mutation_id: id(mutation),
        expected_revision: revision,
        answer,
    }
}

fn check(revision: i64, check: u32, answer: LearnerAnswer) -> CheckAnswerRequest {
    CheckAnswerRequest {
        check_id: id(check),
        expected_revision: revision,
        answer,
        response_time_ms: 800,
    }
}

fn view(attempt: &QuestionAttempt) -> Value {
    serde_json::to_value(attempt.view().unwrap()).unwrap()
}

fn restore(attempt: &QuestionAttempt) -> QuestionAttempt {
    QuestionAttempt::restore(&attempt.to_json().unwrap()).unwrap()
}

#[test]
fn initial_projection_and_restoration_have_no_key_feedback_or_submission() {
    let original = attempt("single_choice");
    let public = view(&original);
    assert_eq!(public["revision"], 1);
    assert_eq!(public["progress"]["phase"], "answering");
    assert!(public["progress"]["savedAnswer"].is_null());
    for hidden in ["PRIVATE", "correctOptionId", "evaluation", "rationale"] {
        assert!(!public.to_string().contains(hidden));
    }
    assert_eq!(view(&restore(&original)), public);
    assert!(original.to_json().unwrap().contains("PRIVATE"));
}

#[test]
fn saved_answers_restore_without_checking_or_changing_display_order() {
    let mut original = attempt("single_choice");
    let before = view(&original)["question"].clone();
    assert_eq!(
        original.save_answer(save(1, 20, single(Some(1)))).unwrap(),
        2
    );
    let reopened = restore(&original);
    let public = view(&reopened);
    assert_eq!(public["question"], before);
    assert_eq!(public["progress"]["phase"], "answering");
    assert_eq!(
        public["progress"]["savedAnswer"]["response"]["selectedOptionId"],
        id(1)
    );
    assert!(!public.to_string().contains("PRIVATE"));
}

#[test]
fn duplicate_save_replays_but_changed_identity_and_stale_save_cannot_overwrite() {
    let mut attempt = attempt("single_choice");
    let first = save(1, 20, single(Some(1)));
    attempt.save_answer(first.clone()).unwrap();
    let stored = attempt.to_json().unwrap();
    assert_eq!(attempt.save_answer(first.clone()).unwrap(), 2);
    assert_eq!(attempt.to_json().unwrap(), stored);
    let mut conflict = first.clone();
    conflict.answer = single(Some(2));
    assert_eq!(
        attempt.save_answer(conflict).unwrap_err().code,
        "question_operation_conflict"
    );
    assert_eq!(attempt.to_json().unwrap(), stored);
    attempt.save_answer(save(2, 21, single(Some(2)))).unwrap();
    let newer = attempt.to_json().unwrap();
    assert_eq!(
        attempt.save_answer(first).unwrap_err().code,
        "question_attempt_conflict"
    );
    assert_eq!(attempt.to_json().unwrap(), newer);
}

#[test]
fn checking_is_separate_from_saving_and_freezes_the_first_answer() {
    let mut attempt = attempt("single_choice");
    attempt.save_answer(save(1, 20, single(Some(1)))).unwrap();
    attempt
        .check_answer(check(2, 30, single(Some(2))), 900)
        .unwrap();
    let public = view(&attempt);
    assert_eq!(public["revision"], 3);
    assert_eq!(public["progress"]["evaluation"]["verdict"], "correct");
    assert_eq!(
        public["progress"]["firstAnswer"]["response"]["selectedOptionId"],
        id(2)
    );
    let stored = attempt.to_json().unwrap();
    assert!(attempt.save_answer(save(3, 21, single(Some(1)))).is_err());
    assert!(attempt.mark_assisted().is_err());
    assert_eq!(attempt.to_json().unwrap(), stored);
    assert_eq!(view(&restore(&attempt)), public);
}

#[test]
fn identical_check_retry_after_restart_keeps_time_result_and_revision() {
    let mut attempt = attempt("single_choice");
    let request = check(1, 30, single(Some(1)));
    attempt.check_answer(request.clone(), 900).unwrap();
    let mut reopened = restore(&attempt);
    let stored = reopened.to_json().unwrap();
    reopened.check_answer(request, 20_000).unwrap();
    assert_eq!(reopened.to_json().unwrap(), stored);
    assert_eq!(view(&reopened)["progress"]["checkedAtMs"], 900);
    assert_eq!(
        view(&reopened)["progress"]["evaluation"]["verdict"],
        "incorrect"
    );
}

#[test]
fn check_identity_cannot_be_reused_for_changed_content_timing_or_revision() {
    let mut attempt = attempt("single_choice");
    let request = check(1, 30, single(Some(1)));
    attempt.check_answer(request.clone(), 900).unwrap();
    let stored = attempt.to_json().unwrap();
    for index in 0..4 {
        let mut changed = request.clone();
        match index {
            0 => changed.answer = single(Some(2)),
            1 => changed.response_time_ms += 1,
            2 => changed.expected_revision += 1,
            _ => changed.check_id = id(31),
        }
        assert!(attempt.check_answer(changed, 1000).is_err());
        assert_eq!(attempt.to_json().unwrap(), stored);
    }
}

#[test]
fn empty_select_all_is_unsubmitted_until_explicitly_checked() {
    let mut attempt = attempt("select_all");
    let empty = answer(json!({"kind":"select_all","selectedOptionIds":[]}));
    attempt.save_answer(save(1, 20, empty.clone())).unwrap();
    assert_eq!(view(&attempt)["progress"]["phase"], "answering");
    attempt.check_answer(check(2, 30, empty), 900).unwrap();
    let result = &view(&attempt)["progress"]["evaluation"];
    assert_eq!(result["verdict"], "correct");
    assert_eq!(result["matchedCount"], 0);
    assert_eq!(result["requiredCount"], 0);
}

#[test]
fn unanswered_boolean_never_becomes_false_during_save_or_restore() {
    for (selected, expected) in [(None, "incomplete"), (Some(false), "correct")] {
        let mut attempt = attempt("true_false");
        let response = answer(json!({"kind":"true_false","selected":selected}));
        attempt.save_answer(save(1, 20, response.clone())).unwrap();
        let mut reopened = restore(&attempt);
        reopened.check_answer(check(2, 30, response), 900).unwrap();
        assert_eq!(
            view(&reopened)["progress"]["evaluation"]["verdict"],
            expected
        );
    }
}

#[test]
fn equivalent_sets_replay_but_duplicates_are_not_silently_normalized() {
    let mut attempt = attempt("select_all");
    let response = answer(json!({"kind":"select_all","selectedOptionIds":[id(2),id(1)]}));
    attempt.check_answer(check(1, 30, response), 900).unwrap();
    let sorted = answer(json!({"kind":"select_all","selectedOptionIds":[id(1),id(2)]}));
    let stored = attempt.to_json().unwrap();
    attempt.check_answer(check(1, 30, sorted), 1000).unwrap();
    assert_eq!(attempt.to_json().unwrap(), stored);
    let duplicate = answer(json!({"kind":"select_all","selectedOptionIds":[id(1),id(1)]}));
    assert!(attempt.check_answer(check(1, 30, duplicate), 1000).is_err());
    assert_eq!(attempt.to_json().unwrap(), stored);
}

#[test]
fn assignment_order_is_normalized_but_ordering_answers_are_not() {
    let mut matching = attempt("matching");
    let entries = json!([{"itemId":id(2),"targetId":id(4)},
                         {"itemId":id(1),"targetId":id(3)}]);
    let first = answer(json!({"kind":"matching","assignments":entries}));
    matching
        .check_answer(check(1, 30, first.clone()), 900)
        .unwrap();
    let mut reordered = first;
    if let QuestionResponse::Matching { assignments } = &mut reordered.response {
        assignments.reverse();
    }
    matching
        .check_answer(check(1, 30, reordered), 1000)
        .unwrap();
    assert_eq!(
        view(&matching)["progress"]["evaluation"]["verdict"],
        "correct"
    );
    let mut ordering = attempt("ordering");
    let first = answer(json!({"kind":"ordering","orderedItemIds":[id(1),id(2)]}));
    ordering.check_answer(check(1, 30, first), 900).unwrap();
    let reversed = answer(json!({"kind":"ordering","orderedItemIds":[id(2),id(1)]}));
    assert!(ordering.check_answer(check(1, 30, reversed), 1000).is_err());
}

#[test]
fn exposed_assistance_is_sticky_and_invalidates_unacknowledged_changes() {
    let mut attempt = attempt("single_choice");
    let saved = save(1, 20, single(Some(1)));
    attempt.save_answer(saved.clone()).unwrap();
    assert_eq!(attempt.mark_assisted().unwrap(), 3);
    assert_eq!(attempt.mark_assisted().unwrap(), 3);
    assert_eq!(attempt.save_answer(saved).unwrap(), 2);
    let mut reopened = restore(&attempt);
    assert!(reopened
        .check_answer(check(2, 30, single(Some(2))), 900)
        .is_err());
    reopened
        .check_answer(check(3, 30, single(Some(2))), 900)
        .unwrap();
    assert_eq!(view(&reopened)["assisted"], true);
    assert_eq!(
        view(&reopened)["progress"]["evaluation"]["verdict"],
        "correct"
    );
}

#[test]
fn malformed_requests_and_time_bounds_do_not_mutate_attempts() {
    let mut attempt = attempt("single_choice");
    let stored = attempt.to_json().unwrap();
    let mut wrong_instance = single(Some(1));
    wrong_instance.instance_id = id(99);
    assert!(attempt.save_answer(save(1, 20, wrong_instance)).is_err());
    assert!(attempt.save_answer(save(1, 20, single(Some(99)))).is_err());
    for elapsed in [-1, MAX_RESPONSE_TIME_MS + 1] {
        let mut request = check(1, 30, single(Some(2)));
        request.response_time_ms = elapsed;
        assert!(attempt.check_answer(request, 900).is_err());
    }
    assert!(attempt
        .check_answer(check(1, 30, single(Some(2))), -1)
        .is_err());
    assert_eq!(attempt.to_json().unwrap(), stored);
}

#[test]
fn corrupted_restored_evaluation_identity_and_versions_fail_closed() {
    let mut attempt = attempt("single_choice");
    attempt
        .check_answer(check(1, 30, single(Some(1))), 900)
        .unwrap();
    let original: Value = serde_json::from_str(&attempt.to_json().unwrap()).unwrap();
    for index in 0..6 {
        let mut changed = original.clone();
        match index {
            0 => changed["attemptVersion"] = json!(99),
            1 => changed["revision"] = json!(0),
            2 => changed["state"]["record"]["evaluation"]["verdict"] = json!("correct"),
            3 => changed["state"]["record"]["request"]["answer"]["instanceId"] = json!(id(99)),
            4 => changed["state"]["record"]["checkedAtMs"] = json!(-1),
            _ => changed["state"]["record"]["evaluation"]["requiredCount"] = json!(10),
        }
        assert!(QuestionAttempt::restore(&changed.to_string()).is_err());
    }
    assert!(QuestionAttempt::restore(&" ".repeat(MAX_ATTEMPT_BYTES + 1)).is_err());
    assert!(QuestionAttempt::restore("not JSON").is_err());
}

#[test]
fn invalid_unchecked_revisions_and_noncanonical_saved_sets_are_rejected() {
    let mut attempt = attempt("select_all");
    let response = answer(json!({"kind":"select_all","selectedOptionIds":[id(1),id(2)]}));
    attempt.save_answer(save(1, 20, response)).unwrap();
    let original: Value = serde_json::from_str(&attempt.to_json().unwrap()).unwrap();
    let mut changed = original.clone();
    changed["state"]["saved"]["answer"]["response"]["selectedOptionIds"] = json!([id(2), id(1)]);
    assert!(QuestionAttempt::restore(&changed.to_string()).is_err());
    let mut changed = original;
    changed["revision"] = json!(3);
    assert!(QuestionAttempt::restore(&changed.to_string()).is_err());
}

#[test]
fn overflow_and_unknown_payload_fields_fail_without_erasing_the_response() {
    let mut attempt = attempt("single_choice");
    attempt.save_answer(save(1, 20, single(Some(1)))).unwrap();
    let mut value: Value = serde_json::from_str(&attempt.to_json().unwrap()).unwrap();
    value["revision"] = json!(i64::MAX);
    value["state"]["saved"]["expectedRevision"] = json!(i64::MAX - 1);
    let mut maximal = QuestionAttempt::restore(&value.to_string()).unwrap();
    let stored = maximal.to_json().unwrap();
    assert!(maximal
        .save_answer(save(i64::MAX, 21, single(Some(2))))
        .is_err());
    assert!(maximal
        .check_answer(check(i64::MAX, 30, single(Some(2))), 900)
        .is_err());
    assert_eq!(maximal.to_json().unwrap(), stored);
    value["unknown"] = json!("PRIVATE INJECTED FIELD");
    let error = QuestionAttempt::restore(&value.to_string()).err().unwrap();
    assert!(!error.message.contains("PRIVATE"));
}

#[test]
fn feedback_assets_are_admitted_only_after_checking() {
    let mut source = body("single_choice");
    source["options"][0]["rationale"] = json!({
        "type":"doc","attrs":{"schemaVersion":3},"content":[{
            "type":"image","attrs":{"assetId":id(40),"src":format!("study-asset://{}",id(40))}
        }]
    });
    let mut attempt = QuestionAttempt::start(captured(source)).unwrap();
    assert_eq!(attempt.instance().asset_ids(), &[id(40)]);
    assert!(attempt.view().unwrap().asset_ids().unwrap().is_empty());
    attempt
        .check_answer(check(1, 30, single(Some(2))), 900)
        .unwrap();
    assert_eq!(attempt.view().unwrap().asset_ids().unwrap(), vec![id(40)]);
    let AttemptProgress::Checked { evaluation, .. } = attempt.view().unwrap().progress else {
        panic!("checked view required");
    };
    assert_eq!(evaluation.verdict, Verdict::Correct);
}
