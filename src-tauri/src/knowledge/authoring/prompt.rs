//! Request-only projection of preparation metadata. A collection's complete
//! evidence store is not the context of every individual authoring window.
//! Keep all structures for the selected claims' primary and supporting sources;
//! never mutate persisted evidence or clip the structure of an admitted table.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Value};

use super::KnowledgeCatalog;

fn selected_sources(catalog: &KnowledgeCatalog) -> BTreeSet<&str> {
    let mut sources = BTreeSet::new();
    for unit in &catalog.units {
        sources.insert(unit.source_id.as_str());
        if let Some(grounding) = &unit.content.grounding {
            sources.extend(
                grounding
                    .evidence
                    .iter()
                    .map(|reference| reference.source_id.as_str()),
            );
        }
    }
    sources
}

fn selected_structures(catalog: &KnowledgeCatalog) -> Vec<&Value> {
    let sources = selected_sources(catalog);
    catalog
        .preparation
        .evidence_structures
        .iter()
        .filter(|value| {
            value
                .get("sourceId")
                .and_then(Value::as_str)
                .is_some_and(|id| sources.contains(id))
        })
        .collect()
}

pub(super) fn preparation(
    catalog: &KnowledgeCatalog,
    source_aliases: &BTreeMap<String, String>,
) -> Value {
    let summary = &catalog.preparation;
    let structures = selected_structures(catalog)
        .into_iter()
        .map(|value| {
            let mut compact = value.clone();
            if let Some(object) = compact.as_object_mut() {
                if let Some(source_id) = object
                    .remove("sourceId")
                    .and_then(|value| value.as_str().map(str::to_string))
                {
                    if let Some(alias) = source_aliases.get(&source_id) {
                        object.insert("source".to_string(), json!(alias));
                    }
                }
                object.remove("occurrenceId");
                object.remove("payloadHash");
            }
            compact
        })
        .collect::<Vec<_>>();
    // The counters still describe preparation of the selected source set, not
    // a claim that the smaller authoring window covers the entire collection.
    json!({
        "unpreparedSources": summary.unprepared_sources,
        "unresolvedRegions": summary.unresolved_regions,
        "unreadableRegions": summary.unreadable_regions,
        "rejectedProposals": summary.rejected_proposals,
        "legacyCoverageSources": summary.legacy_coverage_sources,
        "omittedSources": summary.omitted_sources,
        "omittedUnits": summary.omitted_units,
        "evidenceStructures": structures,
    })
}

/// Stable identity for the evidence bytes/claims relevant to one authoring
/// window. Global preparation counters are diagnostics, not semantic inputs.
pub(super) fn request_identity(catalog: &KnowledgeCatalog) -> Value {
    let evidence_structures = selected_structures(catalog);
    let selected_ids = catalog
        .units
        .iter()
        .map(|unit| unit.id.as_str())
        .collect::<BTreeSet<_>>();
    let document_core_units = catalog
        .document_core_units
        .iter()
        .filter(|id| selected_ids.contains(id.as_str()))
        .collect::<Vec<_>>();
    json!({
        "units": &catalog.units,
        "documentCoreUnits": document_core_units,
        "evidenceStructures": evidence_structures,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge::{
        authoring::tests::catalog_fixture,
        contracts::ContentOrigin,
        evidence::{EvidenceReference, EvidenceSelector, Grounding, Importance, EVIDENCE_VERSION},
    };

    fn structure(source: &str, text: &str) -> Value {
        json!({
            "sourceId": source,
            "occurrenceId": format!("occurrence-{source}"),
            "payloadHash": "c".repeat(64),
            "payload": {"version": 1, "table": {
                "rows": 1, "columns": 2, "columnHeaders": ["Condition", "Outcome"],
                "cells": [{"row": 0, "column": 0, "rowSpan": 1, "columnSpan": 2,
                    "header": false, "text": text, "bounds": null}]
            }},
            "location": {"unitNumber": 1},
            "links": [{"kind": "footnote", "targetNode": 2}],
        })
    }

    #[test]
    fn provider_projection_uses_compact_aliases_without_losing_exact_evidence() {
        let catalog = catalog_fixture();
        let native_unit = catalog.units[0].id.clone();
        let native_source = catalog.units[0].source_id.clone();
        let native_hash = catalog.units[0].source_hash.clone();
        let value: Value = serde_json::from_str(&catalog.prompt().unwrap()).unwrap();

        assert_eq!(value["units"][0]["id"], "K0");
        assert_eq!(
            value["units"][0]["statement"],
            "A triangle has three sides."
        );
        assert_eq!(value["evidence"][0]["id"], "E0");
        assert_eq!(value["evidence"][0]["source"], "S0");
        assert_eq!(value["evidence"][0]["text"], "A triangle has three sides.");
        assert!(value["evidence"][0].get("selector").is_none());
        assert!(value["evidence"][0].get("locator").is_none());
        assert!(value["evidence"][0].get("sourceHash").is_none());
        assert!(!value["units"].to_string().contains(&native_unit));
        assert!(!value["evidence"].to_string().contains(&native_source));
        assert!(!value["evidence"].to_string().contains(&native_hash));

        let support = catalog.select(&["K0".to_string()], 2).unwrap();
        assert_eq!(support[0].unit_id, native_unit);
        assert_eq!(support[0].source_id, native_source);
        assert_eq!(support[0].source_hash, native_hash);
    }

    #[test]
    fn unrelated_collection_growth_does_not_inflate_any_authoring_request() {
        let mut catalog = catalog_fixture();
        catalog.preparation.evidence_structures = vec![structure(
            &catalog.units[0].source_id,
            "The qualification must remain complete.",
        )];
        catalog.preparation.unresolved_regions = 2;
        let before = catalog.prompt().unwrap();
        let unrelated = "Unselected source content. ".repeat(4_000);
        for index in 0..16 {
            catalog
                .preparation
                .evidence_structures
                .push(structure(&format!("unrelated-source-{index}"), &unrelated));
        }
        let native_fingerprint = catalog.fingerprint().unwrap();
        let after = catalog.prompt().unwrap();
        assert_eq!(after, before);
        assert_eq!(catalog.preparation.evidence_structures.len(), 17);
        assert_eq!(catalog.fingerprint().unwrap(), native_fingerprint);
        let mut old_payload: Value = serde_json::from_str(&after).unwrap();
        old_payload["preparation"] = serde_json::to_value(&catalog.preparation).unwrap();
        assert!(old_payload.to_string().len() > after.len() * 100);
        println!(
            "authoring payload regression: old={} bytes, scoped={} bytes",
            old_payload.to_string().len(),
            after.len()
        );
    }

    #[test]
    fn cross_source_support_and_complete_table_structure_survive_projection() {
        let mut catalog = catalog_fixture();
        let primary = catalog.units[0].source_id.clone();
        let dependency = "d".repeat(32);
        let excerpt = "Only when the source condition holds.";
        catalog.units[0].content.qualifications = vec![excerpt.to_string()];
        catalog.units[0].content.grounding = Some(Grounding {
            version: EVIDENCE_VERSION,
            importance: Importance::Core,
            evidence: [primary.clone(), dependency.clone()]
                .into_iter()
                .enumerate()
                .map(|(index, source_id)| EvidenceReference {
                    id: format!("evidence-{index}"),
                    source_id,
                    source_hash: "f".repeat(64),
                    selector: EvidenceSelector::Text {
                        start_byte: 0,
                        end_byte: excerpt.len(),
                    },
                    excerpt: excerpt.to_string(),
                    locator: None,
                    origin: ContentOrigin::SourceDocument,
                })
                .collect(),
        });
        let expected = vec![
            structure(&primary, excerpt),
            structure(&dependency, excerpt),
        ];
        catalog.preparation.evidence_structures = expected.clone();
        catalog.preparation.evidence_structures.push(structure(
            "unrelated",
            "This is not evidence for the selected claim.",
        ));
        let selected = catalog.select(&[catalog.units[0].id.clone()], 1).unwrap();
        let value: Value = serde_json::from_str(&catalog.prompt().unwrap()).unwrap();
        let projected = value["preparation"]["evidenceStructures"]
            .as_array()
            .unwrap();
        assert_eq!(projected.len(), 2);
        assert_eq!(projected[0]["source"], "S0");
        assert_eq!(projected[1]["source"], "S1");
        assert_eq!(projected[0]["payload"], expected[0]["payload"]);
        assert_eq!(projected[1]["payload"], expected[1]["payload"]);
        for structure in projected {
            assert!(structure.get("sourceId").is_none());
            assert!(structure.get("occurrenceId").is_none());
            assert!(structure.get("payloadHash").is_none());
        }
        assert_eq!(value["units"][0]["qualifications"], json!([excerpt]));
        assert_eq!(value["evidence"].as_array().unwrap().len(), 2);
        for evidence in value["evidence"].as_array().unwrap() {
            assert_eq!(evidence["text"], excerpt);
        }
        assert_eq!(
            catalog.select(&[catalog.units[0].id.clone()], 1).unwrap(),
            selected
        );
    }

    #[test]
    fn reducing_a_catalog_also_reduces_its_structural_payload() {
        let mut catalog = catalog_fixture();
        let mut second = catalog.units[0].clone();
        second.id = "2".repeat(32);
        second.source_id = "3".repeat(32);
        catalog.units.push(second);
        catalog.preparation.evidence_structures = catalog
            .units
            .iter()
            .map(|unit| structure(&unit.source_id, &unit.content.statement))
            .collect();
        let full: Value = serde_json::from_str(&catalog.prompt().unwrap()).unwrap();
        assert_eq!(
            full["preparation"]["evidenceStructures"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        catalog.units.truncate(1);
        let bounded: Value = serde_json::from_str(&catalog.prompt().unwrap()).unwrap();
        let structures = bounded["preparation"]["evidenceStructures"]
            .as_array()
            .unwrap();
        assert_eq!(structures.len(), 1);
        assert_eq!(structures[0]["source"], "S0");
        assert!(structures[0].get("sourceId").is_none());
        assert!(structures[0].get("occurrenceId").is_none());
        assert!(structures[0].get("payloadHash").is_none());
        assert!(bounded.to_string().len() < full.to_string().len());
        catalog.units.clear();
        assert_eq!(
            preparation(&catalog, &BTreeMap::new())["evidenceStructures"],
            json!([])
        );
        assert_eq!(catalog.preparation.evidence_structures.len(), 2);
    }

    #[test]
    fn preparation_counters_are_preserved_without_serializing_unrelated_structures() {
        let mut catalog = catalog_fixture();
        catalog.preparation.unprepared_sources = 1;
        catalog.preparation.unresolved_regions = 2;
        catalog.preparation.unreadable_regions = 3;
        catalog.preparation.rejected_proposals = 4;
        catalog.preparation.legacy_coverage_sources = 5;
        catalog.preparation.omitted_sources = 6;
        catalog.preparation.omitted_units = 7;
        let aliases = BTreeMap::from([(catalog.units[0].source_id.clone(), "S0".to_string())]);
        let projected = preparation(&catalog, &aliases);
        assert_eq!(projected["unpreparedSources"], 1);
        assert_eq!(projected["unresolvedRegions"], 2);
        assert_eq!(projected["unreadableRegions"], 3);
        assert_eq!(projected["rejectedProposals"], 4);
        assert_eq!(projected["legacyCoverageSources"], 5);
        assert_eq!(projected["omittedSources"], 6);
        assert_eq!(projected["omittedUnits"], 7);
        assert_eq!(projected["evidenceStructures"], json!([]));
    }
}
