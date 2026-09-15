use serde_json::{json, Value};

use super::*;

fn id(number: u32) -> String {
    format!("{number:032x}")
}

fn rich(text: &str) -> Value {
    json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":text}]}]})
}

fn item(number: u32) -> RichItem {
    RichItem {
        id: id(number),
        content: rich(&format!("Item {number}")),
    }
}

fn definition(question: QuestionBody) -> QuestionDefinition {
    QuestionDefinition {
        schema_version: QUESTION_SCHEMA_VERSION,
        question,
    }
}

fn options() -> Vec<ChoiceOption> {
    (1..=3)
        .map(|number| ChoiceOption {
            id: id(number),
            content: rich(&format!("Option {number}")),
            rationale: Some(rich(&format!("Private rationale {number}"))),
        })
        .collect()
}

fn single() -> QuestionDefinition {
    definition(QuestionBody::SingleChoice {
        prompt: rich("Choose the best answer."),
        options: options(),
        correct_option_id: id(2),
        explanation: Some(rich("Private explanation")),
    })
}

fn select_all(correct_option_ids: Vec<String>) -> QuestionDefinition {
    definition(QuestionBody::SelectAll {
        prompt: rich("Select every applicable option."),
        options: options(),
        correct_option_ids,
        explanation: Some(rich("Private explanation")),
    })
}

fn true_false(correct: bool) -> QuestionDefinition {
    definition(QuestionBody::TrueFalse {
        statement: rich("A single proposition."),
        correct,
        explanation: Some(rich("Private explanation")),
        corrected_statement: Some(rich("Private correction")),
    })
}

fn matching() -> QuestionDefinition {
    definition(QuestionBody::Matching {
        prompt: rich("Match each item."),
        pairs: (1..=3)
            .map(|number| MatchingPair {
                left: item(number),
                right: item(number + 3),
                rationale: Some(rich("Private pair rationale")),
            })
            .collect(),
        explanation: None,
    })
}

fn grouping() -> QuestionDefinition {
    definition(QuestionBody::Grouping {
        prompt: rich("Group each item by its specified property."),
        categories: vec![item(4), item(5)],
        items: (1..=3)
            .map(|number| GroupedItem {
                item: item(number),
                category_id: id(if number < 3 { 4 } else { 5 }),
                rationale: Some(rich("Private membership rationale")),
            })
            .collect(),
        explanation: None,
    })
}

fn ordering() -> QuestionDefinition {
    definition(QuestionBody::Ordering {
        prompt: rich("Arrange the explicit process from first to last."),
        items: (1..=3).map(item).collect(),
        explanation: Some(rich("Private order explanation")),
    })
}

fn occlusion(mode: OcclusionMode) -> QuestionDefinition {
    definition(QuestionBody::ImageOcclusion {
        image_asset_id: id(20),
        masks: (1..=2)
            .map(|number| ImageMask {
                id: id(number),
                rect: NormalizedRect {
                    x: 0.2 * f64::from(number),
                    y: 0.1,
                    width: 0.1,
                    height: 0.2,
                },
            })
            .collect(),
        questions: vec![
            OcclusionQuestion {
                id: id(10),
                prompt: rich("Identify the target."),
                answer: rich("Private identification"),
                explanation: None,
                mask_ids: vec![id(1)],
                mode,
            },
            OcclusionQuestion {
                id: id(11),
                prompt: rich("Compare both targets."),
                answer: rich("Private comparison"),
                explanation: None,
                mask_ids: vec![id(1), id(2)],
                mode,
            },
        ],
    })
}

fn display_order(definition: &QuestionDefinition) -> DisplayOrder {
    let mut order = match &definition.question {
        QuestionBody::SingleChoice { options, .. } | QuestionBody::SelectAll { options, .. } => {
            DisplayOrder {
                item_ids: options.iter().map(|item| item.id.clone()).collect(),
                target_ids: vec![],
            }
        }
        QuestionBody::Matching { pairs, .. } => DisplayOrder {
            item_ids: pairs.iter().map(|pair| pair.left.id.clone()).collect(),
            target_ids: pairs.iter().map(|pair| pair.right.id.clone()).collect(),
        },
        QuestionBody::Grouping {
            items, categories, ..
        } => DisplayOrder {
            item_ids: items.iter().map(|item| item.item.id.clone()).collect(),
            target_ids: categories.iter().map(|item| item.id.clone()).collect(),
        },
        QuestionBody::Ordering { items, .. } => DisplayOrder {
            item_ids: items.iter().map(|item| item.id.clone()).collect(),
            target_ids: vec![],
        },
        _ => DisplayOrder::default(),
    };
    order.item_ids.reverse();
    let target_shift = usize::from(!order.target_ids.is_empty());
    order.target_ids.rotate_left(target_shift);
    order
}

fn capture(definition: QuestionDefinition) -> QuestionInstance {
    let order = display_order(&definition);
    let question = ValidatedQuestion::validate(definition).unwrap();
    let key = question.presentation_keys()[0].clone();
    QuestionInstance::capture(
        InstanceIdentity {
            instance_id: id(100),
            note_id: id(101),
            card_id: id(102),
            note_revision: 4,
        },
        question,
        key,
        order,
    )
    .unwrap()
}

fn answer(response: QuestionResponse) -> LearnerAnswer {
    LearnerAnswer {
        schema_version: QUESTION_SCHEMA_VERSION,
        instance_id: id(100),
        response,
    }
}

fn assignments(pairs: &[(u32, u32)]) -> Vec<Assignment> {
    pairs
        .iter()
        .map(|(item, target)| Assignment {
            item_id: id(*item),
            target_id: id(*target),
        })
        .collect()
}

fn single_response(selected: Option<u32>) -> QuestionResponse {
    QuestionResponse::SingleChoice {
        selected_option_id: selected.map(id),
    }
}

fn multi_response(selected: &[u32]) -> QuestionResponse {
    QuestionResponse::SelectAll {
        selected_option_ids: selected.iter().copied().map(id).collect(),
    }
}

fn verdict(instance: &QuestionInstance, response: QuestionResponse) -> Verdict {
    instance.evaluate(&answer(response)).unwrap().verdict
}

fn assert_rejected(definition: QuestionDefinition) {
    assert!(ValidatedQuestion::validate(definition).is_err());
}

#[test]
fn every_kind_round_trips_without_changing_rich_source_or_identity() {
    for definition in [
        single(),
        select_all(vec![id(1), id(3)]),
        true_false(false),
        matching(),
        grouping(),
        ordering(),
        occlusion(OcclusionMode::HideAll),
    ] {
        let original = definition.clone();
        let admitted = ValidatedQuestion::validate(definition).unwrap();
        let restored = ValidatedQuestion::parse(&admitted.to_json().unwrap()).unwrap();
        assert!(restored.definition() == &original);
        assert_eq!(restored.presentation_keys(), admitted.presentation_keys());
    }
}

#[test]
fn unknown_versions_kinds_fields_and_duplicate_json_members_fail_closed() {
    let original = serde_json::to_value(single()).unwrap();
    let mut cases = Vec::new();
    let mut changed = original.clone();
    changed["schemaVersion"] = json!(2);
    cases.push(changed);
    let mut changed = original.clone();
    changed["question"]["kind"] = json!("basic");
    cases.push(changed);
    let mut changed = original.clone();
    changed["question"]["answer"] = json!("SECRET UNTRUSTED PROPERTY");
    cases.push(changed);
    let mut changed = original.clone();
    changed["extra"] = json!(true);
    cases.push(changed);
    let mut changed = original;
    changed["question"]["options"][0]["script"] = json!("SECRET SCRIPT");
    cases.push(changed);
    for case in cases {
        let error = ValidatedQuestion::parse(&case.to_string()).err().unwrap();
        assert!(!error.message.contains("SECRET"));
    }
    assert!(ValidatedQuestion::parse("{\"schemaVersion\":1,\"schemaVersion\":1,\"question\":{}}").is_err());
    assert!(ValidatedQuestion::parse("not JSON").is_err());
}

#[test]
fn definitions_reject_empty_fields_duplicate_ids_and_unknown_keys() {
    let mut d = single();
    if let QuestionBody::SingleChoice { prompt, .. } = &mut d.question {
        *prompt = rich("   ");
    }
    assert_rejected(d);
    let mut d = single();
    if let QuestionBody::SingleChoice { options, .. } = &mut d.question {
        options[1].id = options[0].id.clone();
    }
    assert_rejected(d);
    let mut d = single();
    if let QuestionBody::SingleChoice { correct_option_id, .. } = &mut d.question {
        *correct_option_id = id(999);
    }
    assert_rejected(d);
    assert_rejected(select_all(vec![id(1), id(1)]));
    assert_rejected(select_all(vec![id(99)]));
    let mut d = single();
    if let QuestionBody::SingleChoice { options, .. } = &mut d.question {
        options[1].content = options[0].content.clone();
    }
    assert_rejected(d);
}

#[test]
fn matching_and_grouping_definitions_admit_only_their_structural_relations() {
    let mut d = matching();
    if let QuestionBody::Matching { pairs, .. } = &mut d.question {
        pairs[1].right.id = pairs[0].right.id.clone();
    }
    assert_rejected(d);
    let mut d = matching();
    if let QuestionBody::Matching { pairs, .. } = &mut d.question {
        pairs[1].right.content = pairs[0].right.content.clone();
    }
    assert_rejected(d);
    let mut d = grouping();
    if let QuestionBody::Grouping { items, .. } = &mut d.question {
        items[0].category_id = id(88);
    }
    assert_rejected(d);
    let mut d = grouping();
    if let QuestionBody::Grouping { categories, .. } = &mut d.question {
        categories.truncate(1);
    }
    assert_rejected(d);
}

#[test]
fn math_image_and_media_fields_keep_canonical_atoms_and_all_private_asset_owners() {
    let mut d = single();
    if let QuestionBody::SingleChoice { prompt, options, .. } = &mut d.question {
        *prompt = json!({"type":"doc","attrs":{"schemaVersion":3},"content":[{"type":"blockMath","attrs":{"latex":"\\frac{x_1}{x_2}"}}]});
        options[0].content = json!({"type":"doc","attrs":{"schemaVersion":3},"content":[{"type":"audio","attrs":{"assetId":id(40),"src":format!("study-asset://{}",id(40))}}]});
        options[0].rationale = Some(json!({"type":"doc","attrs":{"schemaVersion":3},"content":[{"type":"video","attrs":{"assetId":id(41),"src":format!("study-asset://{}",id(41))}}]}));
        options[1].content = json!({"type":"doc","attrs":{"schemaVersion":3},"content":[{"type":"image","attrs":{"assetId":id(42),"src":format!("study-asset://{}",id(42))}}]});
    }
    let instance = capture(d);
    assert_eq!(instance.asset_ids(), &[id(40), id(41), id(42)]);
    let public = instance.public_question().unwrap();
    assert_eq!(public.asset_ids().unwrap(), vec![id(40), id(42)]);
    let json = serde_json::to_string(&public).unwrap();
    assert!(json.contains("\\\\frac{x_1}{x_2}"));
    assert!(!json.contains(&id(41)));
    let restored = QuestionInstance::restore(&instance.to_json().unwrap()).unwrap();
    assert_eq!(restored.asset_ids(), instance.asset_ids());
}

#[test]
fn rich_fields_reuse_the_native_active_content_and_remote_asset_rejections() {
    let mut d = single();
    if let QuestionBody::SingleChoice { prompt, .. } = &mut d.question {
        *prompt = json!({"type":"doc","content":[{"type":"script","text":"SECRET SCRIPT"}]});
    }
    assert_rejected(d);
    let mut d = single();
    if let QuestionBody::SingleChoice { prompt, .. } = &mut d.question {
        *prompt = json!({"type":"doc","attrs":{"schemaVersion":3},"content":[{"type":"audio","attrs":{"assetId":id(40),"src":"https://example.invalid/media"}}]});
    }
    assert_rejected(d);
}

#[test]
fn both_per_field_and_aggregate_limits_are_enforced() {
    let mut d = single();
    if let QuestionBody::SingleChoice { prompt, .. } = &mut d.question {
        *prompt = rich(&"x".repeat(128_000));
    }
    assert_rejected(d);
    let mut d = single();
    if let QuestionBody::SingleChoice { options, .. } = &mut d.question {
        *options = (1..=17)
            .map(|n| ChoiceOption {
                id: id(n),
                content: rich(&format!("{n}{}", "x".repeat(64_000))),
                rationale: None,
            })
            .collect();
    }
    assert_rejected(d);
    assert!(ValidatedQuestion::parse(&" ".repeat(MAX_DEFINITION_BYTES + 1)).is_err());
}

#[test]
fn excessive_nesting_fails_before_snapshot_serialization() {
    let mut d = single();
    let mut nested = rich("text")["content"][0].clone();
    for _ in 0..40 {
        nested = json!({"type":"blockquote","content":[nested]});
    }
    if let QuestionBody::SingleChoice { prompt, .. } = &mut d.question {
        *prompt = json!({"type":"doc","content":[nested]});
    }
    assert_rejected(d);
}

#[test]
fn public_projections_drop_keys_rationales_pairs_and_memberships() {
    for d in [
        single(),
        select_all(vec![id(2)]),
        true_false(false),
        matching(),
        grouping(),
        ordering(),
    ] {
        let public = capture(d).public_question().unwrap();
        let json = serde_json::to_string(&public).unwrap();
        for forbidden in [
            "Private", "correctOption", "correctedStatement", "explanation", "rationale",
            "categoryId", "pairs",
        ] {
            assert!(!json.contains(forbidden), "public projection leaked {forbidden}");
        }
    }
}

#[test]
fn single_choice_distinguishes_correct_incorrect_and_unanswered() {
    let instance = capture(single());
    for (selected, expected) in [
        (Some(2), Verdict::Correct),
        (Some(1), Verdict::Incorrect),
        (None, Verdict::Incomplete),
    ] {
        let result = instance.evaluate(&answer(single_response(selected))).unwrap();
        assert_eq!(result.verdict, expected);
        assert_eq!(result.required_count, 1);
        assert_eq!(result.components.len(), 3);
    }
    assert!(instance.evaluate(&answer(single_response(Some(9)))).is_err());
    assert!(instance.evaluate(&answer(multi_response(&[2]))).is_err());
}

#[test]
fn select_all_checks_exact_sets_without_credit_for_unselected_distractors() {
    let instance = capture(select_all(vec![id(1), id(3)]));
    for bits in 0_u8..8 {
        let selected: Vec<_> = (1..=3).filter(|n| bits & (1 << (n - 1)) != 0).collect();
        let result = instance.evaluate(&answer(multi_response(&selected))).unwrap();
        let expected = if bits == 5 { Verdict::Correct } else { Verdict::Incorrect };
        assert_eq!(result.verdict, expected);
        assert_eq!(result.matched_count, (bits & 5).count_ones() as usize);
        assert_eq!(result.required_count, 2);
    }
    assert_eq!(verdict(&instance, multi_response(&[3, 1])), Verdict::Correct);
    assert!(instance.evaluate(&answer(multi_response(&[1, 1]))).is_err());
}

#[test]
fn empty_select_all_key_and_explicit_empty_response_are_valid() {
    let instance = capture(select_all(vec![]));
    let result = instance.evaluate(&answer(multi_response(&[]))).unwrap();
    assert_eq!(result.verdict, Verdict::Correct);
    assert_eq!((result.matched_count, result.required_count), (0, 0));
    assert_eq!(verdict(&instance, multi_response(&[1])), Verdict::Incorrect);
}

#[test]
fn true_false_never_confuses_an_unanswered_response_with_false() {
    for correct in [false, true] {
        let instance = capture(true_false(correct));
        for (selected, expected) in [
            (Some(correct), Verdict::Correct),
            (Some(!correct), Verdict::Incorrect),
            (None, Verdict::Incomplete),
        ] {
            let response = QuestionResponse::TrueFalse { selected };
            assert_eq!(verdict(&instance, response), expected);
        }
    }
}

#[test]
fn matching_is_bijective_and_partial_answers_report_missing_components() {
    let instance = capture(matching());
    let response = QuestionResponse::Matching {
        assignments: assignments(&[(3, 6), (1, 4), (2, 5)]),
    };
    assert_eq!(verdict(&instance, response), Verdict::Correct);
    let response = QuestionResponse::Matching {
        assignments: assignments(&[(1, 4), (2, 6)]),
    };
    let result = instance.evaluate(&answer(response)).unwrap();
    assert_eq!(result.verdict, Verdict::Incomplete);
    assert_eq!((result.matched_count, result.required_count), (1, 3));
    for pairs in [vec![(1, 4), (2, 4)], vec![(1, 4), (1, 5)], vec![(1, 99)]] {
        let response = QuestionResponse::Matching {
            assignments: assignments(&pairs),
        };
        assert!(instance.evaluate(&answer(response)).is_err());
    }
}

#[test]
fn grouping_allows_many_items_in_one_category_but_not_duplicate_assignments() {
    let instance = capture(grouping());
    for (pairs, expected) in [
        (vec![(3, 5), (2, 4), (1, 4)], Verdict::Correct),
        (vec![(1, 5), (2, 4), (3, 5)], Verdict::Incorrect),
        (vec![], Verdict::Incomplete),
    ] {
        let response = QuestionResponse::Grouping {
            assignments: assignments(&pairs),
        };
        assert_eq!(verdict(&instance, response), expected);
    }
    let response = QuestionResponse::Grouping {
        assignments: assignments(&[(1, 4), (1, 5)]),
    };
    assert!(instance.evaluate(&answer(response)).is_err());
}

#[test]
fn ordering_requires_the_complete_total_order_independently_of_display_order() {
    let instance = capture(ordering());
    let PublicQuestion::Ordering { items, .. } = instance.public_question().unwrap() else {
        panic!("ordering projection");
    };
    let shown: Vec<_> = items.iter().map(|item| item.id.clone()).collect();
    assert_eq!(shown, vec![id(3), id(2), id(1)]);
    for permutation in [[1, 2, 3], [1, 3, 2], [2, 1, 3], [2, 3, 1], [3, 1, 2], [3, 2, 1]] {
        let response = QuestionResponse::Ordering {
            ordered_item_ids: permutation.into_iter().map(id).collect(),
        };
        let expected = if permutation == [1, 2, 3] { Verdict::Correct } else { Verdict::Incorrect };
        assert_eq!(verdict(&instance, response), expected);
    }
    let partial = QuestionResponse::Ordering {
        ordered_item_ids: vec![id(1)],
    };
    assert_eq!(verdict(&instance, partial), Verdict::Incomplete);
    let duplicate = QuestionResponse::Ordering {
        ordered_item_ids: vec![id(1), id(1), id(2)],
    };
    assert!(instance.evaluate(&answer(duplicate)).is_err());
}

#[test]
fn instance_round_trip_keeps_order_identity_fingerprint_and_frozen_key() {
    let mut d = single();
    let instance = capture(d.clone());
    if let QuestionBody::SingleChoice { correct_option_id, .. } = &mut d.question {
        *correct_option_id = id(1);
    }
    let changed = capture(d);
    let restored = QuestionInstance::restore(&instance.to_json().unwrap()).unwrap();
    assert_eq!(instance.identity(), restored.identity());
    assert_eq!(instance.fingerprint(), restored.fingerprint());
    assert_ne!(instance.fingerprint(), changed.fingerprint());
    assert!(instance.public_question().unwrap() == restored.public_question().unwrap());
    assert_eq!(verdict(&restored, single_response(Some(2))), Verdict::Correct);
}

#[test]
fn corrupt_snapshot_versions_orders_and_identities_are_not_normalized() {
    let original: Value = serde_json::from_str(&capture(matching()).to_json().unwrap()).unwrap();
    for field in ["snapshotVersion", "gradingPolicyVersion"] {
        let mut changed = original.clone();
        changed[field] = json!(99);
        assert!(QuestionInstance::restore(&changed.to_string()).is_err());
    }
    let mut changed = original.clone();
    changed["displayOrder"]["itemIds"] = json!([id(1), id(1), id(3)]);
    assert!(QuestionInstance::restore(&changed.to_string()).is_err());
    let mut changed = original.clone();
    changed["displayOrder"]["targetIds"] = json!([id(4), id(5)]);
    assert!(QuestionInstance::restore(&changed.to_string()).is_err());
    let mut changed = original.clone();
    changed["identity"]["noteRevision"] = json!(0);
    assert!(QuestionInstance::restore(&changed.to_string()).is_err());
    let mut changed = original;
    changed["presentationKey"] = json!(id(999));
    assert!(QuestionInstance::restore(&changed.to_string()).is_err());
}

#[test]
fn responses_from_other_instances_or_versions_are_rejected_without_disclosing_keys() {
    let instance = capture(single());
    let mut response = answer(single_response(Some(2)));
    response.instance_id = id(999);
    assert!(instance.evaluate(&response).is_err());
    response.instance_id = id(100);
    response.schema_version = 2;
    assert!(instance.evaluate(&response).is_err());
    let malformed = json!({"schemaVersion":1,"instanceId":id(100),"response":{"kind":"single_choice","selectedOptionId":id(2),"SECRET":"SECRET"}});
    let error = instance.evaluate_json(&malformed.to_string()).err().unwrap();
    assert!(!error.message.contains("SECRET"));
    assert!(instance.evaluate_json(&" ".repeat(MAX_RESPONSE_BYTES + 1)).is_err());
}

#[test]
fn occlusion_validates_geometry_targets_and_both_mask_modes_without_exposing_source_image() {
    for mode in [OcclusionMode::HideTarget, OcclusionMode::HideAll] {
        let instance = capture(occlusion(mode));
        let plan = instance.occlusion_mask_plan().unwrap();
        let expected_count = if mode == OcclusionMode::HideTarget { 1 } else { 2 };
        assert_eq!(plan.hidden_masks.len(), expected_count);
        assert_eq!(plan.image_asset_id, id(20));
        assert_eq!(plan.target_id, id(10));
        assert_eq!(plan.target_mask_ids, vec![id(1)]);
        assert!(instance.public_question().is_err());
        let error = instance.evaluate(&answer(single_response(Some(1)))).err().unwrap();
        assert_eq!(error.code, "question_requires_self_rating");
    }
    let validated = ValidatedQuestion::validate(occlusion(OcclusionMode::HideTarget)).unwrap();
    assert_eq!(validated.occlusion_mask_plan(&id(11)).unwrap().hidden_masks.len(), 2);
    let valid = NormalizedRect { x: 0.0, y: 0.0, width: 0.1, height: 0.1 };
    for rect in [
        NormalizedRect { x: -0.1, ..valid },
        NormalizedRect { x: 0.9, width: 0.2, ..valid },
        NormalizedRect { width: 0.0, ..valid },
        NormalizedRect { x: f64::NAN, ..valid },
        NormalizedRect { y: f64::INFINITY, ..valid },
    ] {
        let mut d = occlusion(OcclusionMode::HideTarget);
        if let QuestionBody::ImageOcclusion { masks, .. } = &mut d.question {
            masks[0].rect = rect;
        }
        assert_rejected(d);
    }
    let mut d = occlusion(OcclusionMode::HideTarget);
    if let QuestionBody::ImageOcclusion { questions, .. } = &mut d.question {
        questions[0].mask_ids = vec![id(99)];
    }
    assert_rejected(d);
}

#[test]
fn moving_masks_or_reordering_questions_never_renumbers_occlusion_presentations() {
    let mut d = occlusion(OcclusionMode::HideAll);
    let before = ValidatedQuestion::validate(d.clone()).unwrap().presentation_keys();
    if let QuestionBody::ImageOcclusion { masks, questions, .. } = &mut d.question {
        masks[0].rect.x = 0.1;
        masks.reverse();
        questions.reverse();
    }
    let after = ValidatedQuestion::validate(d).unwrap();
    let mut keys = after.presentation_keys();
    keys.reverse();
    assert_eq!(keys, before);
    assert_eq!(after.occlusion_mask_plan(&id(10)).unwrap().target_mask_ids, vec![id(1)]);
}
