use super::*;
use crate::knowledge::authoring::tests::catalog_fixture;

fn candidate() -> Value {
    json!({"content":{
        "overview":"A triangle is a polygon with three sides. Its edges meet at three vertices.",
        "keyPoints":["A triangle has exactly three sides when considered in plane geometry."],
        "explanations":[{"heading":"Boundary","body":"Three straight edges form its boundary."}],
        "importantTerms":[],"processSteps":[],"comparisons":[],"commonMisconceptions":[],
        "callouts":[],"tables":[],"workedExamples":[],"retrievalChecks":[],"visualExplanations":[]
    },"knowledgeUnitIds":["K0"]})
}

#[test]
fn document_guide_targeted_repair_keeps_valid_qualified_siblings_unchanged() {
    let catalog = catalog_fixture();
    let mut value = candidate();
    value["content"]["explanations"][0]["body"] = json!({"untrusted":"wrong shape"});
    let repair = GuideRepair::prepare(&value, &catalog, false)
        .unwrap()
        .unwrap();
    assert_eq!(repair.fields, ["explanations"]);
    assert!(!repair.repair_support);
    assert_eq!(repair.schema["required"], json!(["fields"]));
    assert_eq!(
        repair.schema["properties"]["fields"]["required"],
        json!(["explanations"])
    );
    let repaired = repair
        .apply(
            json!({"fields":{"explanations":[{
                "heading":"Boundary","body":"Three straight edges form its boundary."
            }]}}),
            &catalog,
            false,
        )
        .unwrap();
    let content = serde_json::to_value(repaired.content).unwrap();
    for field in FIELDS.into_iter().filter(|field| *field != "explanations") {
        assert_eq!(content[field], value["content"][field], "{field}");
    }
    assert_eq!(repaired.knowledge_support[0].unit_id, catalog.units[0].id);
}

#[test]
fn document_guide_alias_only_repair_cannot_rewrite_valid_content() {
    let catalog = catalog_fixture();
    let mut value = candidate();
    value["knowledgeUnitIds"] = json!(["K999"]);
    let repair = GuideRepair::prepare(&value, &catalog, false)
        .unwrap()
        .unwrap();
    assert!(repair.fields.is_empty());
    assert!(repair.repair_support);
    assert!(repair.schema["properties"].get("fields").is_none());
    assert_eq!(
        repair.schema["properties"]["knowledgeUnitIds"]["items"]["enum"],
        json!(["K0"])
    );
    let repaired = repair
        .apply(json!({"knowledgeUnitIds":["K0"]}), &catalog, false)
        .unwrap();
    assert_eq!(
        serde_json::to_value(repaired.content).unwrap(),
        value["content"]
    );
    let repair = GuideRepair::prepare(&value, &catalog, false)
        .unwrap()
        .unwrap();
    assert!(repair
        .apply(
            json!({"knowledgeUnitIds":["K0"],"content":{}}),
            &catalog,
            false
        )
        .is_err());
}

#[test]
fn document_guide_repair_rejects_extra_missing_and_still_invalid_fields() {
    let catalog = catalog_fixture();
    let mut value = candidate();
    value["content"]["overview"] = json!("terms / more terms");
    for response in [
        json!({}),
        json!({"fields":{}}),
        json!({"fields":{"overview":"Still invalid."}}),
        json!({"fields":{"overview":candidate()["content"]["overview"],"keyPoints":[]}}),
        json!({"fields":{"overview":candidate()["content"]["overview"]},"knowledgeUnitIds":["K0"]}),
    ] {
        let repair = GuideRepair::prepare(&value, &catalog, false)
            .unwrap()
            .unwrap();
        assert!(repair.apply(response, &catalog, false).is_err());
    }
}

#[test]
fn document_guide_field_repair_scopes_evidence_without_renumbering_saved_support() {
    let mut catalog = catalog_fixture();
    let mut unrelated = catalog.units[0].clone();
    unrelated.id = "b".repeat(32);
    unrelated.content.statement = "UNSELECTED STATEMENT ".repeat(100);
    catalog.units.insert(0, unrelated);
    let mut value = candidate();
    value["knowledgeUnitIds"] = json!(["K1"]);
    value["content"]["overview"] = json!("terms / terms");
    let repair = GuideRepair::prepare(&value, &catalog, false)
        .unwrap()
        .unwrap();
    assert!(!repair.user.contains("UNSELECTED STATEMENT"));
    assert!(!repair.user.contains("internal validation scaffold"));
    assert!(repair.user.contains(&catalog.units[1].content.statement));
    assert_eq!(
        repair.candidate["knowledgeUnitIds"],
        json!([catalog.units[1].id])
    );
    let repaired = repair
        .apply(
            json!({"fields":{"overview":candidate()["content"]["overview"]}}),
            &catalog,
            false,
        )
        .unwrap();
    assert_eq!(repaired.knowledge_support[0].unit_id, catalog.units[1].id);
}

#[test]
fn document_guide_repair_does_not_mistake_valid_or_unisolated_content_for_targets() {
    let catalog = catalog_fixture();
    assert!(GuideRepair::prepare(&candidate(), &catalog, false)
        .unwrap()
        .is_none());
    for value in [
        Value::Null,
        json!({"content":[]}),
        json!({"content":{"unsupportedClaim":"Keep the unrecognized draft explicit."}}),
        json!({"content":{"overview":"x".repeat(MAX_CANDIDATE_BYTES)}}),
    ] {
        assert!(GuideRepair::prepare(&value, &catalog, false)
            .unwrap()
            .is_none());
    }
}

#[test]
fn document_guide_repair_keeps_hard_table_shape_and_numeric_literals() {
    let catalog = catalog_fixture();
    let mut value = candidate();
    value["content"]["tables"] = json!([{"title":"Bounds","columns":["Condition","Value"],
        "rows":[["v ≤ −2.050e1 m/s"]],"caption":"Only under the stated conditions."}]);
    let repair = GuideRepair::prepare(&value, &catalog, false)
        .unwrap()
        .unwrap();
    assert_eq!(repair.fields, ["tables"]);
    assert!(repair.user.contains("v ≤ −2.050e1 m/s"));
    let replacement = json!({"fields":{"tables":[{"title":"Bounds","columns":["Condition","Value"],
        "rows":[["Only under the stated conditions.","v ≤ −2.050e1 m/s"]],"caption":""}]}});
    let repaired = repair.apply(replacement, &catalog, false).unwrap();
    assert_eq!(repaired.content.tables[0].rows[0][1], "v ≤ −2.050e1 m/s");
    assert_eq!(
        repaired.content.key_points[0],
        value["content"]["keyPoints"][0]
    );
}

#[test]
fn document_guide_repair_treats_empty_optional_key_points_as_valid_but_repairs_support() {
    let catalog = catalog_fixture();
    let mut value = candidate();
    value["content"]["keyPoints"] = json!([]);
    value["knowledgeUnitIds"] = json!(["K999"]);
    let repair = GuideRepair::prepare(&value, &catalog, false)
        .unwrap()
        .unwrap();
    assert_eq!(repair.target_count(), 1);
    assert!(repair
        .apply(
            json!({"fields":{"keyPoints":candidate()["content"]["keyPoints"]},
        "knowledgeUnitIds":["K999"]}),
            &catalog,
            false
        )
        .is_err());
    let repair = GuideRepair::prepare(&value, &catalog, false)
        .unwrap()
        .unwrap();
    assert!(repair
        .apply(json!({"knowledgeUnitIds":["K0"]}), &catalog, false)
        .is_ok());
}

#[test]
fn document_guide_repair_does_not_request_unavailable_visual_category() {
    let catalog = catalog_fixture();
    let mut value = candidate();
    value["content"]["overview"] = json!("wrong / shape");
    value["content"]["visualExplanations"] =
        json!([{"title":"Not attached","explanation":"PRIVATE UNSEEN IMAGE"}]);
    let repair = GuideRepair::prepare(&value, &catalog, false)
        .unwrap()
        .unwrap();
    assert_eq!(repair.fields, ["overview"]);
    assert!(!repair.user.contains("PRIVATE UNSEEN IMAGE"));
    let repaired = repair
        .apply(
            json!({"fields":{"overview":candidate()["content"]["overview"]}}),
            &catalog,
            false,
        )
        .unwrap();
    assert!(repaired.content.visual_explanations.is_empty());
}
