use super::*;
use crate::knowledge::{analysis_store::Checkpoint, authoring::KnowledgeCatalog, repository};

fn proposal(statement: &str) -> Value {
    let phrase = |text: &str| json!({"text":text,"occurrence":0});
    json!({"title":"Velocity rule","kind":"fact","statement":statement,
        "qualifications":["Not applicable at rest."],"topic":"Physics","importance":"core","evidenceIds":["E0"],
        "details":{"kind":"quantity","subject":phrase("v"),"bounds":[{"label":phrase("≤"),"value":phrase("−2.050e1")}],"unit":phrase("m/s")}})
}

#[test]
fn qualified_units_round_trip_through_decoder_receipt_storage_and_authoring_prompt() {
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
fn invalid_qualified_sibling_is_retained_for_repair_without_erasing_valid_content() {
    let (_, mut source, _, _) = crate::knowledge::authoring::tests::fixture();
    source.text = "v ≤ −2.050e1 m/s".into();
    source.refresh_identity().unwrap();
    let sources = [source];
    let evidence = EvidenceCatalog::build(&sources, false).unwrap();
    let good = proposal(&sources[0].text);
    let mut bad = good.clone();
    bad["details"]["bounds"][0]["value"]["text"] = json!("invented value");
    let report = initial(json!({"units":[good,bad]}), &evidence, &sources).unwrap();
    assert_eq!(report.accepted.len(), 1);
    assert_eq!(report.rejected.len(), 1);
    assert_eq!(report.rejected[0].code, "invalid_qualified_structure");
    assert!(!report.is_complete());
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
fn legacy_unit_serialization_is_unchanged_when_no_structure_was_supplied() {
    let value = json!({"title":"Triangle","kind":"definition","statement":"Three sides.",
        "qualifications":[],"topic":"Geometry","supportingExcerpt":"Three sides."});
    let unit: KnowledgeUnitInput = serde_json::from_value(value.clone()).unwrap();
    assert!(unit.details.is_none());
    assert_eq!(serde_json::to_value(unit).unwrap(), value);
}
