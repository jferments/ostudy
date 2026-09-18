use super::*;
use crate::knowledge::{
    analysis_store::Checkpoint, authoring::tests::fixture, contracts::EvidenceImage,
    extraction::decode, repository,
};

fn proposal(evidence: Vec<String>) -> Value {
    json!({"title":"Conditions","kind":"fact","statement":"The stated condition applies.",
        "qualifications":[],"topic":"Conditions","importance":"core","evidenceIds":evidence})
}

#[test]
fn one_supported_region_does_not_complete_a_large_source_and_repair_preserves_siblings() {
    let (db, mut source, _, _) = fixture();
    source.text = format!(
        "{}\n\n{}",
        "First condition. ".repeat(32),
        "Second condition. ".repeat(32)
    );
    source.refresh_identity().unwrap();
    db.with_connection(|c| repository::publish_source(c, &source, "0", 2))
        .unwrap();
    let sources = [source];
    let catalog = EvidenceCatalog::build(&sources, false).unwrap();
    assert!(catalog.entries.len() > 2);
    let initial = decode::initial(
        json!({"units":[proposal(vec!["E0".into()])]}),
        &catalog,
        &sources,
    )
    .unwrap();
    assert!(!initial.is_complete());
    assert_eq!(initial.missing_sources, vec![sources[0].id.clone()]);
    let accepted = serde_json::to_value(&initial.accepted).unwrap();
    let missing = initial
        .coverage
        .as_ref()
        .unwrap()
        .unresolved_aliases(&catalog);
    assert!(!missing.contains(&"E0".to_string()));
    let contract = hash("coverage-window");
    let mut checkpoint = Checkpoint::open(&db, &sources, &contract).unwrap();
    checkpoint.save(&initial).unwrap();
    let before = db.with_connection(|c| Ok(c.total_changes())).unwrap();
    assert!(!checkpoint.load().unwrap().unwrap().is_complete());
    assert_eq!(
        db.with_connection(|c| Ok(c.total_changes())).unwrap(),
        before
    );
    let repaired = decode::repair(
        &initial,
        json!({"additionalUnits":[proposal(missing)]}),
        &catalog,
        &sources,
    )
    .unwrap();
    assert!(repaired.is_complete());
    assert_eq!(
        serde_json::to_value(&repaired.accepted[..1]).unwrap(),
        accepted
    );
    checkpoint.save(&repaired).unwrap();
    assert!(checkpoint.load().unwrap().unwrap().is_complete());
}

#[test]
fn dispositions_are_explicit_bounded_and_cannot_filter_or_merge_evidence() {
    let (_, source, _, _) = fixture();
    let sources = [source];
    let catalog = EvidenceCatalog::build(&sources, false).unwrap();
    for invalid in [
        "filtered_interface",
        "verified_duplicate",
        "interpreted",
        "invented",
    ] {
        let report = decode::initial(
            json!({"units":[],"regionDispositions":[{"evidenceId":"E0","disposition":invalid}]}),
            &catalog,
            &sources,
        )
        .unwrap();
        assert!(!report.is_complete());
    }
    for valid in ["retained_metadata", "no_learning", "unreadable"] {
        let report = decode::initial(
            json!({"units":[],"regionDispositions":[{"evidenceId":"E0","disposition":valid}]}),
            &catalog,
            &sources,
        )
        .unwrap();
        assert!(report.is_complete());
        let summary = report.coverage.unwrap().diagnostic();
        assert_eq!(summary["interpreted"], 0);
        assert_eq!(summary["exhaustiveConceptCoverage"], false);
    }
    let duplicate = decode::initial(
        json!({"units":[],"regionDispositions":[
            {"evidenceId":"E0","disposition":"no_learning"},
            {"evidenceId":"E0","disposition":"unreadable"}
        ]}),
        &catalog,
        &sources,
    )
    .unwrap();
    assert!(!duplicate.is_complete());
}

#[test]
fn unexposed_image_remains_unresolved_even_when_all_text_is_non_learning() {
    let (_, mut source, _, _) = fixture();
    source.images.push(EvidenceImage::DocumentAsset {
        document_id: source.owner_id.clone(),
        asset_id: "diagram".to_string(),
    });
    source.refresh_identity().unwrap();
    let sources = [source];
    let catalog = EvidenceCatalog::build(&sources, false).unwrap();
    let report = decode::initial(
        json!({"units":[],"noLearningSourceIds":[sources[0].id]}),
        &catalog,
        &sources,
    )
    .unwrap();
    assert!(!report.is_complete());
    let summary = report.coverage.unwrap().diagnostic();
    assert_eq!(summary["regions"], 2);
    assert_eq!(summary["noLearning"], 1);
    assert_eq!(summary["unresolved"], 1);
}

#[test]
fn forged_or_stale_region_receipts_are_rejected_before_any_publication() {
    let (db, source, _, _) = fixture();
    let sources = [source];
    let catalog = EvidenceCatalog::build(&sources, false).unwrap();
    let report = decode::initial(
        json!({"units":[proposal(vec!["E0".into()])]}),
        &catalog,
        &sources,
    )
    .unwrap();
    let contract = hash("coverage-integrity");
    for (key, value) in [
        ("version", json!(999)),
        ("planHash", json!("0".repeat(64))),
        ("regions", json!([])),
    ] {
        let mut serialized = serde_json::to_value(&report).unwrap();
        serialized["coverage"][key] = value;
        let bad: decode::AnalysisReport = serde_json::from_value(serialized).unwrap();
        let before = db.with_connection(|c| Ok(c.total_changes())).unwrap();
        let mut checkpoint = Checkpoint::open(&db, &sources, &contract).unwrap();
        assert!(checkpoint.save(&bad).is_err());
        assert_eq!(
            db.with_connection(|c| Ok(c.total_changes())).unwrap(),
            before
        );
    }
    let mut changed = sources[0].clone();
    changed.text.push_str(" New condition");
    changed.refresh_identity().unwrap();
    assert!(report
        .coverage
        .as_ref()
        .unwrap()
        .validate(&[changed], &report.accepted, &report.no_learning)
        .is_err());
}

#[test]
fn multi_result_transport_retains_region_declarations_and_legacy_receipts_stay_read_only() {
    let (db, source, _, _) = fixture();
    let sources = [source];
    let catalog = EvidenceCatalog::build(&sources, false).unwrap();
    let report = decode::initial(
        json!([
            {"units":[]},
            {"units":[],"regionDispositions":[{"evidenceId":"E0","disposition":"retained_metadata"}]}
        ]),
        &catalog,
        &sources,
    )
    .unwrap();
    assert!(report.is_complete());
    assert_eq!(report.coverage.unwrap().diagnostic()["retainedMetadata"], 1);
    let old: decode::AnalysisReport = serde_json::from_value(json!({
        "accepted":[],"rejected":[],"noLearning":[sources[0].id],"missingSources":[]
    }))
    .unwrap();
    let contract = hash("retained-legacy-receipt");
    let mut checkpoint = Checkpoint::open(&db, &sources, &contract).unwrap();
    checkpoint.save(&old).unwrap();
    let before = db.with_connection(|c| Ok(c.total_changes())).unwrap();
    assert!(checkpoint.load().unwrap().unwrap().coverage.is_none());
    assert_eq!(
        db.with_connection(|c| Ok(c.total_changes())).unwrap(),
        before
    );
}

#[test]
fn oversized_or_malformed_declarations_cannot_certify_missing_regions() {
    let (_, source, _, _) = fixture();
    let sources = [source];
    let catalog = EvidenceCatalog::build(&sources, false).unwrap();
    for declarations in [
        json!([{"evidenceId":"E0","disposition":"no_learning","extra":true}]),
        json!([{"evidenceId":"E999","disposition":"no_learning"}]),
        json!(vec![
            json!({"evidenceId":"E0","disposition":"no_learning"});
            MAX_REGIONS + 1
        ]),
    ] {
        let report = decode::initial(
            json!({"units":[],"regionDispositions":declarations}),
            &catalog,
            &sources,
        )
        .unwrap();
        assert!(!report.is_complete());
    }
}
