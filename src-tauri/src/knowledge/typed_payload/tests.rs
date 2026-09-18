use super::*;
use serde_json::json;

fn phrase(text: &str) -> Value {
    json!({"text":text,"occurrence":0})
}

fn quantity(operator: &str, number: &str) -> Value {
    json!({"kind":"quantity","subject":phrase("v"),"unit":phrase("m/s"),
        "bounds":[{"label":phrase(operator),"value":phrase(number)}]})
}

#[test]
fn numeric_operators_units_and_exceptions_remain_literal() {
    let statement = "Only while moving, v ≤ −2.050e1 m/s; not applicable at rest.";
    let payload =
        TypedPayload::admit(quantity("≤", "−2.050e1"), statement, KnowledgeKind::Fact).unwrap();
    let persisted = serde_json::to_string(&payload).unwrap();
    assert!(!persisted.contains("2.050"));
    let restored: TypedPayload = serde_json::from_str(&persisted).unwrap();
    restored.validate(statement, KnowledgeKind::Fact).unwrap();
    let prompt = restored.prompt(statement).unwrap();
    assert_eq!(prompt["bounds"][0]["label"], "≤");
    assert_eq!(prompt["bounds"][0]["value"], "−2.050e1");
    assert_eq!(prompt["unit"], "m/s");
    assert!(
        TypedPayload::admit(quantity("<", "−2.050e1"), statement, KnowledgeKind::Fact).is_err()
    );
    assert!(TypedPayload::admit(quantity("≤", "2.050e1"), statement, KnowledgeKind::Fact).is_err());
}

#[test]
fn unsupported_versions_invalid_utf8_and_out_of_range_spans_fail_closed() {
    let statement = "v ≤ 2 m/s";
    let mut payload =
        TypedPayload::admit(quantity("≤", "2"), statement, KnowledgeKind::Fact).unwrap();
    payload.version = 2;
    assert!(payload.validate(statement, KnowledgeKind::Fact).is_err());
    payload.version = 1;
    if let Shape::Quantity { bounds, .. } = &mut payload.shape {
        bounds[0].label.start = 3;
    }
    assert!(payload.validate(statement, KnowledgeKind::Fact).is_err());
    assert!(payload.prompt(statement).is_err());
    let mut proposal = quantity("≤", "2");
    proposal["bounds"][0]["value"]["occurrence"] = json!(65);
    assert!(TypedPayload::admit(proposal, statement, KnowledgeKind::Fact).is_err());
}

#[test]
fn repeated_phrases_resolve_only_the_selected_literal_occurrence() {
    let statement = "v = 2 m/s, or v = 2 m/s after the reset";
    let mut proposal = quantity("=", "2");
    proposal["bounds"][0]["value"]["occurrence"] = json!(1);
    let payload = TypedPayload::admit(proposal, statement, KnowledgeKind::Fact).unwrap();
    let Shape::Quantity { bounds, .. } = payload.shape else {
        panic!("quantity expected")
    };
    assert_eq!(bounds[0].value.start, statement.rfind('2').unwrap());
}

#[test]
fn process_dependencies_preserve_branches_without_inventing_a_total_order() {
    let statement = "Prepare first. Then heat and stir concurrently. Finally measure.";
    let proposal = json!({"kind":"process","steps":[phrase("Prepare"),phrase("heat"),phrase("stir"),phrase("measure")],
        "dependencies":[{"before":0,"after":1},{"before":0,"after":2},{"before":1,"after":3},{"before":2,"after":3}]});
    let payload = TypedPayload::admit(proposal.clone(), statement, KnowledgeKind::Process).unwrap();
    let prompt = payload.prompt(statement).unwrap();
    assert_eq!(prompt["dependencies"].as_array().unwrap().len(), 4);
    assert!(!prompt["dependencies"].as_array().unwrap().contains(&json!({"before":1,"after":2})));
    for bad in [
        json!({"before":3,"after":0}),
        json!({"before":9,"after":0}),
        json!({"before":0,"after":1}),
    ] {
        let mut invalid = proposal.clone();
        invalid["dependencies"].as_array_mut().unwrap().push(bad);
        assert!(TypedPayload::admit(invalid, statement, KnowledgeKind::Process).is_err());
    }
    let mut unordered = proposal;
    unordered["dependencies"] = json!([]);
    let payload = TypedPayload::admit(unordered, statement, KnowledgeKind::Process).unwrap();
    assert_eq!(payload.prompt(statement).unwrap()["dependencies"], json!([]));
}

#[test]
fn comparison_criteria_and_overlapping_classifications_remain_explicit() {
    let statement = "By speed, A is slow and B is fast.";
    let comparison = json!({"kind":"comparison","criterion":phrase("speed"),"items":[
        {"label":phrase("A"),"value":phrase("slow")},{"label":phrase("B"),"value":phrase("fast")}]});
    let payload = TypedPayload::admit(comparison, statement, KnowledgeKind::Comparison).unwrap();
    assert_eq!(payload.prompt(statement).unwrap()["criterion"], "speed");
    let statement = "By properties, red and round both include apple; green excludes apple.";
    let classification = json!({"kind":"classification","criterion":phrase("properties"),
        "categories":[{"label":phrase("red"),"members":[phrase("apple")]},
        {"label":phrase("round"),"members":[phrase("apple")]}],"exclusion":phrase("green excludes apple")});
    let payload =
        TypedPayload::admit(classification, statement, KnowledgeKind::Definition).unwrap();
    let prompt = payload.prompt(statement).unwrap();
    assert_eq!(prompt["categories"][0]["members"], prompt["categories"][1]["members"]);
    assert_eq!(prompt["exclusion"], "green excludes apple");
    assert!(prompt.get("mutuallyExclusive").is_none());
}

#[test]
fn malformed_and_oversized_structure_is_not_silently_treated_as_absent() {
    let statement = "v = 2 m/s";
    let mut proposal = quantity("=", "2");
    proposal["bounds"] = json!(vec![proposal["bounds"][0].clone(); 3]);
    assert!(TypedPayload::admit(proposal, statement, KnowledgeKind::Fact).is_err());
    assert!(
        TypedPayload::admit(json!({"kind":"quantity","unknown":true}), statement, KnowledgeKind::Fact).is_err()
    );
    assert!(TypedPayload::admit(quantity("=", "NaN"), "v = NaN m/s", KnowledgeKind::Fact).is_err());
}

#[test]
fn interval_bounds_retain_sides_and_inclusive_operators() {
    let statement = "−2 ≤ v < 3 m/s";
    let proposal = json!({"kind":"quantity","subject":phrase("v"),"unit":phrase("m/s"),
        "bounds":[{"label":phrase("≤"),"value":phrase("−2")},
            {"label":phrase("<"),"value":phrase("3")}]});
    let payload = TypedPayload::admit(proposal, statement, KnowledgeKind::Fact).unwrap();
    let prompt = payload.prompt(statement).unwrap();
    assert_eq!(prompt["bounds"][0]["subjectOnLeft"], false);
    assert_eq!(prompt["bounds"][1]["subjectOnLeft"], true);
    assert_eq!(prompt["bounds"][0]["label"], "≤");
    assert_eq!(prompt["bounds"][1]["label"], "<");
    assert!(TypedPayload::admit(quantity("<", "2"), "v <= 2 m/s", KnowledgeKind::Fact).is_err());
}
