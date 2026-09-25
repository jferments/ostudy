use super::*;
use crate::knowledge::{analysis_store::Checkpoint, authoring::KnowledgeCatalog, repository};

fn proposal(statement: &str) -> Value {
    let phrase = |text: &str| json!({"text":text,"occurrence":0});
    json!({"title":"Velocity rule","kind":"fact","statement":statement,
        "qualifications":["Not applicable at rest."],"topic":"Physics","importance":"core","evidenceIds":["E0"],
        "details":{"kind":"quantity","subject":phrase("v"),"bounds":[{"label":phrase("≤"),"value":phrase("−2.050e1")}],"unit":phrase("m/s")}})
}

#[test]
fn document_qualified_units_round_trip_through_decoder_receipt_storage_and_authoring_prompt() {
    let (database, mut source, _, _) = crate::knowledge::authoring::tests::fixture();
    source.text = "Only while moving, v ≤ −2.050e1 m/s. Not applicable at rest.".into();
    source.refresh_identity().unwrap();
    database
        .with_connection(|c| repository::publish_source(c, &source, "0", 2))
        .unwrap();
    let sources = [source];
    let evidence = EvidenceCatalog::build(&sources, false).unwrap();
    let report = initial(
        json!({"units":[proposal(&sources[0].text)]}),
        &evidence,
        &sources,
    )
    .unwrap();
    assert!(report.is_complete());
    let contract = hash("qualified-round-trip");
    Checkpoint::open(&database, &sources, &contract)
        .unwrap()
        .save(&report)
        .unwrap();
    database
        .with_connection(|c| {
            let before = c.total_changes();
            let units = repository::load_units(c, &sources[0].id)?;
            let catalog = KnowledgeCatalog {
                units,
                document_core_units: Default::default(),
                preparation: Default::default(),
            };
            let value: Value = serde_json::from_str(&catalog.prompt()?).unwrap();
            assert_eq!(value["units"][0]["statement"], sources[0].text);
            assert_eq!(
                value["units"][0]["qualifications"],
                json!(["Not applicable at rest."])
            );
            assert_eq!(
                value["units"][0]["details"]["bounds"][0]["value"],
                "−2.050e1"
            );
            assert_eq!(value["units"][0]["details"]["unit"], "m/s");
            assert_eq!(c.total_changes(), before);
            Ok(())
        })
        .unwrap();
    database.with_connection(|c| {
        c.execute("UPDATE knowledge_units SET record_json=json_set(record_json, '$.content.details.version', 999)", [])
            .map_err(CommandError::database)?;
        let before = c.total_changes();
        assert!(repository::load_units(c, &sources[0].id).is_err());
        assert_eq!(c.total_changes(), before);
        Ok(())
    }).unwrap();
}

#[test]
fn document_invalid_annotation_is_deferred_without_erasing_either_admitted_core() {
    let (_, mut source, _, _) = crate::knowledge::authoring::tests::fixture();
    source.text = "v ≤ −2.050e1 m/s".into();
    source.refresh_identity().unwrap();
    let sources = [source];
    let evidence = EvidenceCatalog::build(&sources, false).unwrap();
    let good = proposal(&sources[0].text);
    let mut bad = good.clone();
    bad["details"]["bounds"][0]["value"]["text"] = json!("invented value");
    let report = initial(json!({"units":[good,bad]}), &evidence, &sources).unwrap();
    assert_eq!(report.accepted.len(), 2);
    assert!(report.rejected.is_empty());
    assert_eq!(report.deferred_annotations().len(), 1);
    let details = report.accepted[1].content.details.as_ref().unwrap();
    assert_eq!(
        details.deferred_rule(),
        Some("knowledge_annotation_selection_absent")
    );
    assert!(!report.is_complete());
    assert!(!report.requires_paid_repair(&sources));
    let before = serde_json::to_value(&report.accepted).unwrap();
    let repaired = repair(
        &report,
        json!({"repairs":[{"slot":1,"unit":null}]}),
        &evidence,
        &sources,
    )
    .unwrap();
    assert_eq!(serde_json::to_value(&repaired.accepted).unwrap(), before);
    assert!(!repaired.is_complete());
}

#[test]
fn document_legacy_unit_serialization_is_unchanged_when_no_structure_was_supplied() {
    let value = json!({"title":"Triangle","kind":"definition","statement":"Three sides.",
        "qualifications":[],"topic":"Geometry","supportingExcerpt":"Three sides."});
    let unit: KnowledgeUnitInput = serde_json::from_value(value.clone()).unwrap();
    assert!(unit.details.is_none());
    assert_eq!(serde_json::to_value(unit).unwrap(), value);
}

fn diagnostic_proposal(statement: &str, operator: &str, number: &str) -> Value {
    let mut value = proposal(statement);
    value["details"]["bounds"][0]["label"]["text"] = json!(operator);
    value["details"]["bounds"][0]["value"]["text"] = json!(number);
    value
}

fn assert_rule(value: Value, expected: &str) {
    let (_, mut source, _, _) = crate::knowledge::authoring::tests::fixture();
    source.text = value["statement"].as_str().unwrap().to_string();
    source.refresh_identity().unwrap();
    let sources = [source];
    let catalog = EvidenceCatalog::build(&sources, false).unwrap();
    let report = initial(json!({"units":[value]}), &catalog, &sources).unwrap();
    assert_eq!(report.accepted.len(), 1);
    assert!(report.rejected.is_empty());
    assert!(!report.requires_paid_repair(&sources));
    assert!(!report.is_complete());
    let details = report.accepted[0].content.details.as_ref().unwrap();
    assert_eq!(details.deferred_rule(), Some(expected));
    let diagnostic = report_diagnostic(&report);
    assert_eq!(
        diagnostic["deferredAnnotations"][0]["annotation"]["rule"],
        expected
    );
    assert!(!diagnostic.to_string().contains(&sources[0].text));
}

#[test]
fn document_annotation_shape_and_selection_failures_have_distinct_rules() {
    // Repeated phrases cannot be normalized by guessing an occurrence. Unique
    // occurrences are tested separately through the production decoder below.
    let base = proposal("v ≤ −2.050e1 m/s; speed in m/s");
    let cases = [
        ("/details", json!({}), "knowledge_annotation_malformed"),
        (
            "/details/bounds",
            json!([]),
            "knowledge_annotation_structural_bounds",
        ),
        (
            "/details/unit/text",
            json!(""),
            "knowledge_annotation_selection_invalid",
        ),
        (
            "/details/unit/text",
            json!("kg"),
            "knowledge_annotation_selection_absent",
        ),
        (
            "/details/unit/occurrence",
            json!(2),
            "knowledge_annotation_occurrence_out_of_range",
        ),
        (
            "/details/unit/occurrence",
            json!(65),
            "knowledge_annotation_occurrence_out_of_range",
        ),
        (
            "/kind",
            json!("process"),
            "knowledge_annotation_kind_mismatch",
        ),
    ];
    for (path, replacement, expected) in cases {
        let mut value = base.clone();
        *value.pointer_mut(path).unwrap() = replacement;
        assert_rule(value, expected);
    }
}

#[test]
fn document_unique_occurrence_is_normalized_but_strict_admission_still_rejects_it() {
    let (_, mut source, _, _) = crate::knowledge::authoring::tests::fixture();
    source.text = "v ≤ −2.050e1 m/s".into();
    source.refresh_identity().unwrap();
    let sources = [source];
    let catalog = EvidenceCatalog::build(&sources, false).unwrap();
    for occurrence in [1, 65] {
        let mut value = proposal(&sources[0].text);
        value["details"]["unit"]["occurrence"] = json!(occurrence);
        assert!(crate::knowledge::typed_payload::TypedPayload::admit(
            value["details"].clone(),
            &sources[0].text,
            KnowledgeKind::Fact,
        )
        .is_err());
        let report = initial(json!({"units": [value]}), &catalog, &sources).unwrap();
        assert!(report.is_complete());
        assert!(!report.requires_paid_repair(&sources));
        assert_eq!(report.accepted[0].content.statement, sources[0].text);
        assert!(report.deferred_annotations().is_empty());
    }
}

#[test]
fn document_annotation_numeric_checks_preserve_sign_operator_and_finite_value() {
    for (statement, operator, number, expected) in [
        (
            "v <= 2 m/s",
            "<",
            "2",
            "knowledge_annotation_operator_token_split",
        ),
        (
            "v ≤ −2.050e1 m/s",
            "≤",
            "2.050e1",
            "knowledge_annotation_numeric_token_split",
        ),
        (
            "v ~ 2 m/s",
            "~",
            "2",
            "knowledge_annotation_quantity_operator",
        ),
        (
            "v = NaN m/s",
            "=",
            "NaN",
            "knowledge_annotation_quantity_literal",
        ),
    ] {
        assert_rule(diagnostic_proposal(statement, operator, number), expected);
    }
    let mut reversed = proposal("v ≤ −2.050e1 m/s");
    reversed["details"]["subject"]["text"] = json!("m/s");
    assert_rule(reversed, "knowledge_annotation_quantity_order");
}

#[test]
fn document_annotation_process_cycle_is_distinct_from_invalid_edge() {
    let phrase = |text: &str| json!({"text":text,"occurrence":0});
    for (edges, expected) in [
        (
            json!([{"before":0,"after":1},{"before":1,"after":0}]),
            "knowledge_annotation_dependency_cycle",
        ),
        (
            json!([{"before":0,"after":3}]),
            "knowledge_annotation_dependency_invalid",
        ),
        (
            json!([{"before":0,"after":1},{"before":0,"after":1}]),
            "knowledge_annotation_dependency_invalid",
        ),
    ] {
        let mut value = proposal("Prepare. Then heat.");
        value["kind"] = json!("process");
        value["details"] = json!({
            "kind":"process", "steps":[phrase("Prepare"),phrase("heat")],
            "dependencies":edges
        });
        assert_rule(value, expected);
    }
}

#[test]
fn document_annotation_draft_survives_checkpoint_but_is_not_a_whole_window_repair_target() {
    let (database, mut source, _, _) = crate::knowledge::authoring::tests::fixture();
    source.text = "Only while moving, v ≤ −2.050e1 m/s. Not applicable at rest.".into();
    source.refresh_identity().unwrap();
    database
        .with_connection(|c| repository::publish_source(c, &source, "0", 2))
        .unwrap();
    let sources = [source];
    let catalog = EvidenceCatalog::build(&sources, false).unwrap();
    let good = proposal(&sources[0].text);
    let mut bad = good.clone();
    bad["details"]["unit"]["text"] = json!("PRIVATE_MISSING_PHRASE");
    let draft = bad["details"].clone();
    let report = initial(json!({"units":[good,bad]}), &catalog, &sources).unwrap();
    let contract = hash("annotation-diagnostic-checkpoint");
    let mut checkpoint = Checkpoint::open(&database, &sources, &contract).unwrap();
    checkpoint.save(&report).unwrap();
    let restored = checkpoint.load().unwrap().unwrap();
    assert_eq!(
        serde_json::to_value(&restored).unwrap(),
        serde_json::to_value(&report).unwrap()
    );
    assert_eq!(restored.accepted.len(), 2);
    assert_eq!(restored.repair_prompt()["rejectedProposals"], json!([]));
    let details = serde_json::to_value(&restored.accepted[1].content.details).unwrap();
    assert_eq!(details["draft"], draft);
    let diagnostic = report_diagnostic(&restored);
    assert!(!diagnostic.to_string().contains("PRIVATE_MISSING_PHRASE"));
    assert_eq!(
        diagnostic["deferredAnnotations"][0]["annotation"]["draftFingerprint"],
        hash(&draft.to_string())
    );
    let repaired = repair(
        &restored,
        json!({"repairs":[{"slot":1,"unit":null}]}),
        &catalog,
        &sources,
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(&repaired).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
    assert!(!repaired.is_complete());
}

#[test]
fn document_annotation_legacy_diagnostics_and_valid_occurrences_are_preserved() {
    let legacy = json!({
        "accepted":[], "rejected":[{"slot":0,"proposal":{},"code":"invalid_qualified_structure"}],
        "noLearning":[], "missingSources":[]
    });
    let report: AnalysisReport = serde_json::from_value(legacy.clone()).unwrap();
    assert_eq!(serde_json::to_value(&report).unwrap(), legacy);
    assert_eq!(
        report_diagnostic(&report)["rejections"][0]["rule"],
        "invalid_qualified_structure"
    );
    let statement = "v = 2 m/s, then v = 2 m/s";
    let mut value = diagnostic_proposal(statement, "=", "2");
    for path in [
        "/details/subject/occurrence",
        "/details/bounds/0/label/occurrence",
        "/details/bounds/0/value/occurrence",
        "/details/unit/occurrence",
    ] {
        *value.pointer_mut(path).unwrap() = json!(1);
    }
    let payload = crate::knowledge::typed_payload::TypedPayload::admit(
        value["details"].clone(),
        statement,
        KnowledgeKind::Fact,
    )
    .unwrap();
    let crate::knowledge::typed_payload::TypedPayload::Validated(payload) = payload else {
        panic!("validated annotation expected");
    };
    let crate::knowledge::typed_payload::Shape::Quantity {
        subject, bounds, ..
    } = payload.shape
    else {
        panic!("quantity expected");
    };
    assert_eq!(subject.start, statement.rfind('v').unwrap());
    assert_eq!(bounds[0].value.start, statement.rfind('2').unwrap());
}
