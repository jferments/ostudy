use serde_json::{json, Value};

use super::*;
use crate::cards::questions::{DisplayOrder, InstanceIdentity, QuestionKind, ValidatedQuestion};

fn id(value: u32) -> String {
    format!("{value:032x}")
}

fn rich(text: &str) -> Value {
    json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":text}]}]})
}

fn image(asset: u32) -> Value {
    json!({"type":"doc","content":[{"type":"image","attrs":{
        "assetId":id(asset),"src":format!("study-asset://{}",id(asset))}}]})
}

fn definition() -> Value {
    json!({"schemaVersion":1,"question":{
        "kind":"image_occlusion","imageAssetId":id(50),
        "masks":[
            {"id":id(1),"rect":{"x":0.1,"y":0.1,"width":0.2,"height":0.2}},
            {"id":id(2),"rect":{"x":0.6,"y":0.6,"width":0.2,"height":0.2}}
        ],
        "questions":[
            {"id":id(10),"prompt":rich("Identify the highlighted region."),
             "answer":rich("PRIVATE FIRST ANSWER"),"explanation":image(51),
             "maskIds":[id(1)],"mode":"hide_all"},
            {"id":id(11),"prompt":rich("Another prompt"),
             "answer":rich("PRIVATE OTHER ANSWER"),"explanation":null,
             "maskIds":[id(2)],"mode":"hide_target"}
        ]
    }})
}

fn instance(definition: Value, key: &str) -> QuestionInstance {
    QuestionInstance::capture(
        InstanceIdentity {
            instance_id: id(100),
            note_id: id(101),
            card_id: id(102),
            note_revision: 3,
        },
        ValidatedQuestion::parse(&definition.to_string()).unwrap(),
        key.to_string(),
        DisplayOrder::default(),
    )
    .unwrap()
}

fn attempt() -> QuestionAttempt {
    QuestionAttempt::start(instance(definition(), &id(10))).unwrap()
}

fn error_code<T>(result: Result<T, QuestionError>) -> String {
    result.err().expect("operation should fail").code
}

fn request() -> RevealAnswerRequest {
    RevealAnswerRequest {
        reveal_id: id(110),
        expected_revision: 1,
        response_time_ms: 234,
    }
}

#[test]
fn hidden_image_attempt_exposes_only_target_geometry_and_prompt() {
    let attempt = attempt();
    let view = attempt.view().unwrap();
    assert_eq!(attempt.instance().kind(), QuestionKind::ImageOcclusion);
    let value = serde_json::to_value(&view).unwrap();
    assert_eq!(value["question"]["kind"], "image_occlusion");
    assert_eq!(
        value["question"]["targetRectangles"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(value["progress"]["phase"], "answering");
    assert!(view.asset_ids().unwrap().is_empty());
    let serialized = value.to_string();
    for hidden in [
        id(50),
        id(51),
        "PRIVATE".to_string(),
        "imageAssetId".to_string(),
    ] {
        assert!(!serialized.contains(&hidden));
    }
    assert!(value["progress"].get("answer").is_none());
    assert!(value["progress"].get("evaluation").is_none());
    // A generic objective projection cannot accidentally provide a source image.
    assert!(attempt.instance().public_question().is_err());
}

#[test]
fn reveal_has_no_objective_verdict_and_exposes_only_its_frozen_target_answer() {
    let mut attempt = attempt();
    let view = attempt.reveal_answer(request(), 1000).unwrap();
    let value = serde_json::to_value(&view).unwrap();
    assert_eq!(view.revision, 2);
    assert_eq!(value["progress"]["phase"], "revealed");
    assert_eq!(value["progress"]["responseTimeMs"], 234);
    assert!(value.to_string().contains("PRIVATE FIRST ANSWER"));
    assert!(!value.to_string().contains("PRIVATE OTHER ANSWER"));
    assert!(value["progress"].get("evaluation").is_none());
    assert!(value["progress"].get("verdict").is_none());
    assert_eq!(view.asset_ids().unwrap(), vec![id(51)]);
}

#[test]
fn reveal_replays_the_original_receipt_after_checkpoint_restore() {
    let mut first = attempt();
    let original = serde_json::to_value(first.reveal_answer(request(), 1000).unwrap()).unwrap();
    let checkpoint = first.to_json().unwrap();
    let mut restored = QuestionAttempt::restore(&checkpoint).unwrap();
    let replay = restored.reveal_answer(request(), 9000).unwrap();
    assert_eq!(serde_json::to_value(replay).unwrap(), original);
    assert_eq!(restored.to_json().unwrap(), checkpoint);
    let mut changed = request();
    changed.response_time_ms += 1;
    assert_eq!(
        error_code(restored.reveal_answer(changed, 9000)),
        "question_operation_conflict"
    );
    let mut another = request();
    another.reveal_id = id(111);
    assert_eq!(
        error_code(restored.reveal_answer(another, 9000)),
        "question_already_revealed"
    );
    assert_eq!(restored.to_json().unwrap(), checkpoint);
}

#[test]
fn a_stale_reveal_or_invalid_elapsed_time_never_changes_the_attempt() {
    let mut attempt = attempt();
    let before = attempt.to_json().unwrap();
    let mut stale = request();
    stale.expected_revision = 2;
    assert_eq!(
        error_code(attempt.reveal_answer(stale, 1000)),
        "question_attempt_conflict"
    );
    let mut invalid = request();
    invalid.response_time_ms = -1;
    assert!(attempt.reveal_answer(invalid, 1000).is_err());
    assert!(attempt.reveal_answer(request(), -1).is_err());
    assert_eq!(attempt.to_json().unwrap(), before);
}

#[test]
fn self_assessed_questions_cannot_be_saved_or_checked_as_an_objective_response() {
    let mut attempt = attempt();
    let answer = LearnerAnswer {
        schema_version: 1,
        instance_id: id(100),
        response: QuestionResponse::TrueFalse {
            selected: Some(true),
        },
    };
    let before = attempt.to_json().unwrap();
    let save = SaveAnswerRequest {
        mutation_id: id(111),
        expected_revision: 1,
        answer: answer.clone(),
    };
    assert_eq!(
        error_code(attempt.save_answer(save)),
        "question_requires_self_rating"
    );
    let check = CheckAnswerRequest {
        check_id: id(112),
        expected_revision: 1,
        answer,
        response_time_ms: 10,
    };
    assert_eq!(
        error_code(attempt.check_answer(check, 100)),
        "question_requires_self_rating"
    );
    assert_eq!(attempt.to_json().unwrap(), before);
}

#[test]
fn an_objective_question_cannot_use_the_reveal_only_path() {
    let definition = json!({"schemaVersion":1,"question":{
        "kind":"true_false","statement":rich("A statement"),"correct":true,
        "explanation":null,"correctedStatement":null}});
    let mut attempt = QuestionAttempt::start(instance(definition, "main")).unwrap();
    let before = attempt.to_json().unwrap();
    assert!(attempt.reveal_answer(request(), 100).is_err());
    assert_eq!(attempt.to_json().unwrap(), before);
}

#[test]
fn assisted_reveal_preserves_assistance_and_cannot_rewrite_it_after_exposure() {
    let mut attempt = attempt();
    assert_eq!(attempt.mark_assisted().unwrap(), 2);
    let mut request = request();
    request.expected_revision = 2;
    let view = attempt.reveal_answer(request, 1000).unwrap();
    assert!(view.assisted);
    assert_eq!(view.revision, 3);
    let checkpoint = attempt.to_json().unwrap();
    let restored = QuestionAttempt::restore(&checkpoint).unwrap();
    assert!(restored.view().unwrap().assisted);
    assert!(attempt.mark_assisted().is_err());
    assert_eq!(attempt.to_json().unwrap(), checkpoint);
}

#[test]
fn forged_reveal_receipts_and_extra_objective_fields_fail_restore() {
    let mut attempt = attempt();
    attempt.reveal_answer(request(), 1000).unwrap();
    let original: Value = serde_json::from_str(&attempt.to_json().unwrap()).unwrap();
    let mut changed = original.clone();
    changed["revision"] = json!(100);
    changed["state"]["record"]["request"]["expectedRevision"] = json!(99);
    assert!(QuestionAttempt::restore(&changed.to_string()).is_err());
    let mut changed = original.clone();
    changed["state"]["record"]["revealedAtMs"] = json!(-1);
    assert!(QuestionAttempt::restore(&changed.to_string()).is_err());
    let mut changed = original;
    changed["state"]["record"]["evaluation"] = json!({"verdict":"correct"});
    assert!(QuestionAttempt::restore(&changed.to_string()).is_err());
}

#[test]
fn the_unmasked_source_cannot_be_smuggled_into_the_pre_answer_prompt() {
    let mut definition = definition();
    definition["question"]["questions"][0]["prompt"] = image(50);
    let instance = instance(definition, &id(10));
    assert_eq!(
        error_code(QuestionAttempt::start(instance)),
        "occlusion_source_in_prompt"
    );
}
