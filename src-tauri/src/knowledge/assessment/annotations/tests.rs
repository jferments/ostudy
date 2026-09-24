use super::*;
use crate::knowledge::{
    analysis_store::Checkpoint,
    assessment::{
        self, policy::GenerationPolicy, AssessmentProposal, ComponentProposal, EvidenceStance,
    },
    authoring::{self, tests::catalog_fixture, CARD_SUPPORT_KEY},
    contracts::hash,
    evidence::EvidenceCatalog,
    extraction::decode,
    typed_payload::TypedPayload,
};
use serde_json::{json, Value};

fn defer(catalog: &mut KnowledgeCatalog, index: usize) {
    let unit = &mut catalog.units[index];
    unit.content.details = Some(
        TypedPayload::admit_or_defer(json!({}), &unit.content.statement, unit.content.kind).unwrap(),
    );
}

fn raw() -> Value {
    json!({"type":"interactive","question":{"kind":"true_false",
        "statement":"A triangle has three sides.","correct":true,
        "explanation":"Its boundary consists of three sides."}})
}

fn proposal() -> AssessmentProposal {
    AssessmentProposal {
        criterion: "Use the explicitly supplied side count.".into(),
        ambiguous: false,
        components: vec![ComponentProposal {
            key: "statement".into(),
            stance: EvidenceStance::Supports,
            knowledge_unit_ids: vec!["K0".into()],
        }],
    }
}

#[test]
fn document_annotation_new_and_forged_cached_grading_reject_deferred_support() {
    let mut catalog = catalog_fixture();
    let policy = GenerationPolicy::default();
    let mut card = assessment::admit_card(raw(), proposal(), &catalog, &policy).unwrap();
    assert!(assessment::validates_checkpoint(&card, &catalog, &policy));
    defer(&mut catalog, 0);
    let error = assessment::admit_card(raw(), proposal(), &catalog, &policy).unwrap_err();
    assert_eq!(error.code, unavailable().code);
    assert_eq!(error.message, unavailable().message);
    // Rebind every hash to the new catalog, so rejection cannot be attributed
    // merely to a stale content hash. Annotation capability is independently checked.
    let mut record = assessment::record_from_card(&card).unwrap().unwrap();
    for component in &mut record.components {
        component.units = catalog.select(&["K0".into()], 2).unwrap();
    }
    record.opportunity_id = assessment::opportunity_id(
        &record.question_hash,
        &record.criterion,
        &record.components,
    )
    .unwrap();
    card[CARD_SUPPORT_KEY] = serde_json::to_value(record.support().unwrap()).unwrap();
    card[assessment::ASSESSMENT_KEY] = serde_json::to_value(&record).unwrap();
    record.validate_card(&card).unwrap();
    assert!(catalog.validates(&record.support().unwrap(), 128));
    assert!(!assessment::validates_checkpoint(&card, &catalog, &policy));
}

#[test]
fn document_annotation_schema_keeps_aliases_and_recall_but_excludes_deferred_grading() {
    let mut catalog = catalog_fixture();
    let mut second = catalog.units[0].clone();
    second.id = "b".repeat(32);
    catalog.units.push(second);
    defer(&mut catalog, 0);
    let schema = assessment::generation_schema(
        json!({"properties":{"cards":{"items":{"type":"object"}}}}),
        &GenerationPolicy::default(),
        &catalog,
    );
    let alternatives = schema["properties"]["cards"]["items"]["oneOf"]
        .as_array()
        .unwrap();
    let interactive = alternatives
        .iter()
        .find(|item| item["properties"].get("assessment").is_some())
        .unwrap();
    assert_eq!(
        interactive["properties"]["assessment"]["properties"]["components"]["items"]["properties"]["knowledgeUnitIds"]["items"]["enum"],
        json!(["K1"])
    );
    let recall = alternatives
        .iter()
        .find(|item| item["properties"].get("knowledgeUnitIds").is_some())
        .unwrap();
    assert!(recall["properties"]["knowledgeUnitIds"]["items"]
        .get("enum")
        .is_none());
    let support = catalog.select(&["K0".into()], 2).unwrap();
    let mut card = json!({"type":"basic","front":"How many sides?","back":"Three.",
        "_ostudyKnowledge":support});
    card[assessment::CARD_POLICY_KEY] = json!(GenerationPolicy::default().fingerprint().unwrap());
    assert!(assessment::validates_checkpoint(
        &card,
        &catalog,
        &GenerationPolicy::default()
    ));
    let prompt = catalog.prompt().unwrap();
    assert!(prompt.contains("gradedBindingsAllowed"));
    assert!(!prompt.contains("draftHash"));
}

#[test]
fn document_annotation_final_publication_rejects_deferred_support_without_writes() {
    let (database, source, _, _) = authoring::tests::fixture();
    let sources = [source];
    let evidence = EvidenceCatalog::build(&sources, false).unwrap();
    let report = decode::initial(
        json!({"units":[{"title":"Sides","kind":"fact","statement":"A triangle has three sides.",
            "qualifications":[],"topic":"Geometry","importance":"core","evidenceIds":["E0"],"details":{}}]}),
        &evidence,
        &sources,
    )
    .unwrap();
    let contract = hash("annotation publication gate");
    let units = Checkpoint::open(&database, &sources, &contract)
        .unwrap()
        .save(&report)
        .unwrap();
    let catalog = KnowledgeCatalog {
        units: units[&sources[0].id].clone(),
        ..KnowledgeCatalog::default()
    };
    let support = catalog.select(&["K0".into()], 2).unwrap();
    let policy = GenerationPolicy::default();
    let pristine = catalog_fixture();
    let valid = assessment::admit_card(raw(), proposal(), &pristine, &policy).unwrap();
    let mut record = assessment::record_from_card(&valid).unwrap().unwrap();
    record.components[0].units = support.clone();
    database
        .with_connection(|connection| {
            authoring::validate_current_support(connection, &sources[0].owner_id, &support)?;
            let before = connection.total_changes();
            let error = assessment::store::publish(
                connection,
                &"c".repeat(32),
                &sources[0].owner_id,
                &record,
                3,
            )
            .unwrap_err();
            assert_eq!(error.message, unavailable().message);
            assert_eq!(connection.total_changes(), before);
            Ok(())
        })
        .unwrap();
}
