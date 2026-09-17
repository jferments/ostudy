use super::policy::GenerationPolicy;
use super::*;
use crate::knowledge::authoring::{card_support, tests::catalog_fixture, without_card_support};

fn examples() -> Vec<Value> {
    vec![
        json!({"kind":"single_choice","prompt":"Which has three sides?",
            "options":[{"id":"triangle","content":"Triangle"},
                {"id":"square","content":"Square","rationale":"A square has four sides, not three."}],
            "correctOptionId":"triangle"}),
        json!({"kind":"select_all","prompt":"Select the polygons.",
            "options":[{"id":"triangle","content":"Triangle"},
                {"id":"square","content":"Square"},
                {"id":"circle","content":"Circle","rationale":"A circle has a curved boundary."}],
            "correctOptionIds":["triangle","square"]}),
        json!({"kind":"true_false","statement":"A triangle has four sides.","correct":false,
            "explanation":"A triangle has three sides.","correctedStatement":"A triangle has three sides."}),
        json!({"kind":"matching","prompt":"Match shapes to side counts.",
            "pairs":[{"id":"triangle","left":"Triangle","right":"Three"},
                {"id":"square","left":"Square","right":"Four"}]}),
        json!({"kind":"grouping","prompt":"Group by whether the boundary contains a curve.",
            "categories":[{"id":"straight","content":"Straight edges only"},
                {"id":"curved","content":"Curved boundary"}],
            "items":[{"id":"triangle","content":"Triangle","categoryId":"straight"},
                {"id":"circle","content":"Circle","categoryId":"curved"}]}),
        json!({"kind":"ordering","prompt":"Arrange in increasing numerical order.",
            "items":[{"id":"one","content":"1"},{"id":"two","content":"2"},
                {"id":"three","content":"3"}]}),
    ]
}

fn proposal(index: usize) -> AssessmentProposal {
    use EvidenceStance::{Contradicts as C, Supports as S};
    let requirements = match index {
        0 => vec![("prompt", S), ("option:triangle", S), ("option:square", C)],
        1 => vec![
            ("prompt", S),
            ("option:triangle", S),
            ("option:square", S),
            ("option:circle", C),
        ],
        2 => vec![("prompt", S), ("statement", C), ("correction", S)],
        3 => vec![("prompt", S), ("pair:triangle", S), ("pair:square", S)],
        4 => vec![
            ("prompt", S),
            ("category:straight", S),
            ("category:curved", S),
            ("item:triangle", S),
            ("item:circle", S),
        ],
        5 => vec![
            ("prompt", S),
            ("item:one", S),
            ("item:two", S),
            ("item:three", S),
            ("before:one:two", S),
            ("before:two:three", S),
        ],
        _ => panic!("unknown fixture"),
    };
    AssessmentProposal {
        criterion: "Use the explicit relation stated by the question.".to_string(),
        ambiguous: false,
        components: requirements
            .into_iter()
            .map(|(key, stance)| ComponentProposal {
                key: key.to_string(),
                stance,
                knowledge_unit_ids: vec!["a".repeat(32)],
            })
            .collect(),
    }
}

fn raw(index: usize) -> Value {
    json!({"type":"interactive","question":examples()[index]})
}
fn admit(index: usize) -> Value {
    admit_card(
        raw(index),
        proposal(index),
        &catalog_fixture(),
        &policy::GenerationPolicy::default(),
    )
    .unwrap()
}

#[test]
fn all_six_types_preserve_native_keys_and_component_addresses() {
    // These fixtures test native contracts, not a claim that ID selection proves
    // the proposition true. Factual model quality has a separate acceptance gate.
    for index in 0..6 {
        let card = admit(index);
        let record = record_from_card(&card).unwrap().unwrap();
        assert_eq!(record.components.len(), proposal(index).components.len());
        assert_eq!(card_support(&card).unwrap(), record.support().unwrap());
        record.validate_card(&card).unwrap();
        assert!(validates_checkpoint(
            &card,
            &catalog_fixture(),
            &GenerationPolicy::default()
        ));
        let public = without_card_support(card);
        assert!(public.get(ASSESSMENT_KEY).is_none());
        assert!(public.get(CARD_SUPPORT_KEY).is_none());
        assert_eq!(public["type"], "interactive");
    }
}

#[test]
fn no_relation_is_invented_to_fill_a_quota() {
    let catalog = catalog_fixture();
    let policy = policy::GenerationPolicy::default();
    let mut ambiguous = proposal(5);
    ambiguous.ambiguous = true;
    assert!(admit_card(raw(5), ambiguous, &catalog, &policy).is_err());
    let mut missing_edge = proposal(5);
    missing_edge.components.pop();
    assert!(admit_card(raw(5), missing_edge, &catalog, &policy).is_err());
    assert!(policy::select_cards(Vec::new(), 20, &catalog, &policy)
        .unwrap()
        .is_empty());
}

#[test]
fn fabricated_missing_duplicate_and_wrong_role_evidence_fail_closed() {
    let catalog = catalog_fixture();
    let policy = policy::GenerationPolicy::default();
    let mut unknown = proposal(0);
    unknown.components[0].knowledge_unit_ids = vec!["f".repeat(32)];
    assert!(admit_card(raw(0), unknown, &catalog, &policy).is_err());
    let mut duplicate = proposal(0);
    duplicate.components[1] = duplicate.components[0].clone();
    assert!(admit_card(raw(0), duplicate, &catalog, &policy).is_err());
    let mut wrong_role = proposal(0);
    wrong_role.components[2].stance = EvidenceStance::Supports;
    assert!(admit_card(raw(0), wrong_role, &catalog, &policy).is_err());
    let mut excessive = proposal(0);
    excessive.components[0].knowledge_unit_ids = vec!["a".repeat(32); 3];
    assert!(admit_card(raw(0), excessive, &catalog, &policy).is_err());
}

#[test]
fn changed_keys_and_source_revisions_cannot_reuse_a_checkpoint() {
    let card = admit(0);
    let mut changed_key = card.clone();
    changed_key["question"]["correctOptionId"] = json!("square");
    assert!(record_from_card(&changed_key).is_err());
    let mut changed_catalog = catalog_fixture();
    changed_catalog.units[0].source_hash = "b".repeat(64);
    assert!(!validates_checkpoint(
        &card,
        &changed_catalog,
        &GenerationPolicy::default()
    ));
    let mut missing = card.clone();
    missing.as_object_mut().unwrap().remove(ASSESSMENT_KEY);
    assert!(card_support(&missing).is_err());
    let mut wrong_union = card;
    wrong_union[CARD_SUPPORT_KEY][0]["sourceHash"] = json!("b".repeat(64));
    assert!(card_support(&wrong_union).is_err());
}

#[test]
fn compound_support_can_cover_more_than_two_targets_but_recall_cannot() {
    let mut catalog = catalog_fixture();
    for name in ["b", "c"] {
        let mut unit = catalog.units[0].clone();
        unit.id = name.repeat(32);
        catalog.units.push(unit);
    }
    let mut bindings = proposal(5);
    bindings.components[1].knowledge_unit_ids = vec!["b".repeat(32)];
    bindings.components[2].knowledge_unit_ids = vec!["c".repeat(32)];
    let card = admit_card(
        raw(5),
        bindings,
        &catalog,
        &policy::GenerationPolicy::default(),
    )
    .unwrap();
    let support = card_support(&card).unwrap();
    assert_eq!(support.len(), 3);
    assert!(validates_checkpoint(
        &card,
        &catalog,
        &GenerationPolicy::default()
    ));
    let recall =
        json!({"type":"basic","front":"Question","back":"Answer",CARD_SUPPORT_KEY:support});
    assert!(card_support(&recall).is_err());
}

#[test]
fn image_occlusion_remote_media_and_unexplained_distractors_are_not_generated() {
    let policy = policy::GenerationPolicy::default();
    let catalog = catalog_fixture();
    let mut image = raw(0);
    image["question"]["kind"] = json!("image_occlusion");
    assert!(admit_card(image, proposal(0), &catalog, &policy).is_err());
    let mut remote = raw(0);
    remote["question"]["options"][0]["content"] =
        json!("![answer](https://example.invalid/secret)");
    assert!(admit_card(remote, proposal(0), &catalog, &policy).is_err());
    let mut unexplained = raw(0);
    unexplained["question"]["options"][1]["rationale"] = Value::Null;
    assert!(admit_card(unexplained, proposal(0), &catalog, &policy).is_err());
}

#[test]
fn policy_is_bounded_canonical_and_enforced_without_filler() {
    use policy::{GenerationPolicy, QuestionFamily};
    let mut policy = GenerationPolicy::default();
    let fingerprint = policy.fingerprint().unwrap();
    policy.allowed_types.reverse();
    assert_eq!(fingerprint, policy.fingerprint().unwrap());
    policy.allowed_types = vec![QuestionFamily::Recall];
    assert!(admit_card(raw(0), proposal(0), &catalog_fixture(), &policy).is_err());
    assert!(!generation_schema(
        json!({"properties":{"cards":{"items":{"oneOf":[]}}}}),
        &policy
    )
    .to_string()
    .contains("single_choice"));
    policy.allowed_types.clear();
    assert!(policy.validate().is_err());
    assert!(serde_json::from_value::<GenerationPolicy>(
        json!({"allowedTypes":["image_occlusion"]})
    )
    .is_err());
}

#[test]
fn duplicate_questions_and_reverse_children_use_the_real_presentation_budget() {
    let catalog = catalog_fixture();
    let support = catalog.select(&["a".repeat(32)], 2).unwrap();
    let basic = |front: &str, reverse: bool| {
        json!({"type":"basic","front":front,"back":"Three",
        "reverseEnabled":reverse,CARD_SUPPORT_KEY:support})
    };
    let chosen = policy::select_cards(
        vec![basic("First", true), basic("Second", false)],
        1,
        &catalog,
        &policy::GenerationPolicy::default(),
    )
    .unwrap();
    assert_eq!(chosen.len(), 1);
    assert_eq!(chosen[0]["front"], "Second");
    let chosen = policy::select_cards(
        vec![admit(0), admit(0)],
        20,
        &catalog,
        &policy::GenerationPolicy::default(),
    )
    .unwrap();
    assert_eq!(chosen.len(), 1);
}

#[test]
fn native_preferences_reject_stale_writers_without_losing_the_saved_mix() {
    let database = crate::database::Database::open_in_memory().unwrap();
    let policy = policy::GenerationPolicy::default();
    let saved = settings::save(
        &database,
        settings::SaveGenerationPolicyInput {
            revision: 0,
            policy: policy.clone(),
        },
    )
    .unwrap();
    assert_eq!(saved.revision, 1);
    let error = settings::save(
        &database,
        settings::SaveGenerationPolicyInput {
            revision: 0,
            policy: policy.clone(),
        },
    )
    .unwrap_err();
    assert_eq!(error.code, "generation_policy_changed");
    database
        .with_connection(|connection| {
            let restored = settings::load(connection)?;
            assert_eq!(restored.revision, 1);
            assert_eq!(restored.policy, policy);
            Ok(())
        })
        .unwrap();
}
