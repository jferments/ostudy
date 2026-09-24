use super::*;
use crate::knowledge::{
    authoring::tests::catalog_fixture,
    contracts::ContentOrigin,
    evidence::{EvidenceReference, Grounding, Importance, EVIDENCE_VERSION},
};
use serde_json::json;

fn catalog() -> KnowledgeCatalog {
    let mut catalog = catalog_fixture();
    let unit = &mut catalog.units[0];
    unit.content.qualifications = vec!["Only under the stated condition.".to_string()];
    unit.content.grounding = Some(Grounding {
        version: EVIDENCE_VERSION,
        importance: Importance::Core,
        evidence: vec![EvidenceReference {
            id: "e".repeat(32),
            source_id: unit.source_id.clone(),
            source_hash: unit.source_hash.clone(),
            selector: EvidenceSelector::Text {
                start_byte: 4,
                end_byte: 14,
            },
            excerpt: "condition.".to_string(),
            locator: None,
            origin: ContentOrigin::SourceDocument,
        }],
    });
    catalog
}

fn block(source: &str, id: &str, start: u64, end: u64) -> Value {
    json!({"sourceId":source,"occurrenceId":id,"payloadHash":"f".repeat(64),
        "sourceRange":{"version":1,"sourceHash":"c".repeat(64),
            "startByte":start,"endByte":end},
        "payload":{"version":1,"text":"Literal complete evidence."},
        "location":{"unitNumber":1},"links":[]})
}

fn projected(catalog: &KnowledgeCatalog) -> Vec<&Value> {
    super::super::selected_structures(catalog)
}

#[test]
fn document_authoring_range_projection_drops_only_unrelated_whole_blocks() {
    let mut catalog = catalog();
    let source = catalog.units[0].source_id.clone();
    let retained = block(&source, "supported", 0, 20);
    catalog.preparation.evidence_structures = vec![
        block(&source, "before", 0, 4),
        retained.clone(),
        block(&source, "after", 14, 100),
        block("not-selected", "other-source", 0, 20),
    ];
    let before = catalog.fingerprint().unwrap();
    let units = serde_json::to_value(&catalog.units).unwrap();
    assert_eq!(projected(&catalog), vec![&retained]);
    assert_eq!(catalog.fingerprint().unwrap(), before);
    assert_eq!(serde_json::to_value(&catalog.units).unwrap(), units);
    let prompt: Value = serde_json::from_str(&catalog.prompt().unwrap()).unwrap();
    let structure = &prompt["preparation"]["evidenceStructures"][0];
    assert_eq!(structure["payload"], retained["payload"]);
    assert!(structure.get("sourceRange").is_none());
    assert_eq!(prompt["evidence"][0]["text"], "condition.");
    assert_eq!(
        prompt["units"][0]["qualifications"],
        json!(catalog.units[0].content.qualifications)
    );
}

#[test]
fn document_authoring_range_projection_preserves_whole_table_and_linked_qualifiers() {
    let mut catalog = catalog();
    let source = catalog.units[0].source_id.clone();
    let mut table = block(&source, "table", 0, 20);
    table["payload"]["table"] = json!({"rows":1,"columns":2,
        "columnHeaders":["Condition","Result"],
        "cells":[{"row":0,"column":0,"rowSpan":1,"columnSpan":2,
            "text":"Only for x ≤ 4; the result is not valid otherwise."}]});
    table["links"] = json!([{"kind":"caption","targetOccurrenceId":"caption"}]);
    let caption = block(&source, "caption", 30, 50);
    let mut footnote = block(&source, "footnote", 70, 100);
    footnote["links"] = json!([{"kind":"footnote","targetOccurrenceId":"caption"}]);
    catalog.preparation.evidence_structures = vec![
        table.clone(),
        caption.clone(),
        footnote.clone(),
        block(&source, "unrelated", 110, 200),
    ];
    assert_eq!(projected(&catalog), vec![&table, &caption, &footnote]);
}

#[test]
fn document_authoring_range_projection_keeps_every_cross_source_dependency() {
    let mut catalog = catalog();
    let primary = catalog.units[0].source_id.clone();
    let secondary = "d".repeat(32);
    let references = &mut catalog.units[0]
        .content
        .grounding
        .as_mut()
        .unwrap()
        .evidence;
    let mut reference = references[0].clone();
    reference.id = "f".repeat(32);
    reference.source_id = secondary.clone();
    references.push(reference);
    let first = block(&primary, "first", 0, 20);
    let second = block(&secondary, "second", 0, 20);
    catalog.preparation.evidence_structures = vec![
        first.clone(),
        second.clone(),
        block(&secondary, "unrelated", 50, 90),
    ];
    assert_eq!(projected(&catalog), vec![&first, &second]);
    let prompt: Value = serde_json::from_str(&catalog.prompt().unwrap()).unwrap();
    assert_eq!(prompt["evidence"].as_array().unwrap().len(), 2);
    assert_eq!(
        prompt["preparation"]["evidenceStructures"][1]["source"],
        "S1"
    );
}

#[test]
fn document_authoring_range_projection_keeps_legacy_and_invalid_locations() {
    let mut catalog = catalog();
    let source = catalog.units[0].source_id.clone();
    for range in [
        Value::Null,
        json!({"version":2,"startByte":100,"endByte":120,"sourceHash":"c".repeat(64)}),
        json!({"version":1,"startByte":100,"endByte":120,"sourceHash":"changed"}),
        json!({"version":1,"startByte":100,"endByte":100,"sourceHash":"c".repeat(64)}),
    ] {
        let mut unknown = block(&source, "unknown", 100, 120);
        unknown["sourceRange"] = range;
        catalog.preparation.evidence_structures = vec![unknown.clone()];
        assert_eq!(projected(&catalog), vec![&unknown]);
    }
    catalog.units[0].content.grounding = None;
    let legacy = block(&source, "legacy", 100, 120);
    catalog.preparation.evidence_structures = vec![legacy.clone()];
    assert_eq!(projected(&catalog), vec![&legacy]);
}

#[test]
fn document_authoring_range_projection_does_not_guess_unresolved_links() {
    let mut catalog = catalog();
    let source = catalog.units[0].source_id.clone();
    for link in [
        json!({"kind":"footnote","targetNode":2}),
        json!({"kind":"caption","targetOccurrenceId":"not-in-admitted-context"}),
    ] {
        let mut selected = block(&source, "selected", 0, 20);
        selected["links"] = json!([link]);
        let possible_context = block(&source, "ambiguous", 100, 120);
        catalog.preparation.evidence_structures =
            vec![selected.clone(), possible_context.clone()];
        assert_eq!(projected(&catalog), vec![&selected, &possible_context]);
    }
}

#[test]
fn document_authoring_range_projection_terminates_on_cyclic_links_in_source_order() {
    let mut catalog = catalog();
    let source = catalog.units[0].source_id.clone();
    let mut first = block(&source, "first", 0, 20);
    let mut second = block(&source, "second", 100, 120);
    first["links"] = json!([{"targetOccurrenceId":"second"}]);
    second["links"] = json!([{"targetOccurrenceId":"first"}]);
    catalog.preparation.evidence_structures = vec![second.clone(), first.clone()];
    assert_eq!(projected(&catalog), vec![&second, &first]);
}

#[test]
fn document_authoring_request_identity_ignores_unselected_blocks_not_retained_evidence() {
    let mut catalog = catalog();
    let source = catalog.units[0].source_id.clone();
    catalog.preparation.evidence_structures = vec![block(&source, "selected", 0, 20)];
    let request = catalog.request_fingerprint().unwrap();
    let prompt = catalog.prompt().unwrap();
    let native = catalog.fingerprint().unwrap();
    let mut large = block(&source, "unrelated", 100, 120);
    large["payload"]["text"] = json!("Unrelated content ".repeat(10_000));
    catalog.preparation.evidence_structures.push(large);
    assert_eq!(catalog.prompt().unwrap(), prompt);
    assert_eq!(catalog.request_fingerprint().unwrap(), request);
    assert_ne!(catalog.fingerprint().unwrap(), native);
    catalog.preparation.evidence_structures[0]["payload"]["text"] =
        json!("Changed qualifier.");
    assert_ne!(catalog.request_fingerprint().unwrap(), request);
}
