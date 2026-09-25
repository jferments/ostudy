//! The provider boundary is injected; admission and SQLite checkpoints are real.
use super::*;
use crate::{
    knowledge::{analysis_store::Checkpoint, authoring::KnowledgeCatalog, repository},
    task_cancellation::CancellationSignal,
};

fn proposal() -> Value {
    json!({"title":"Triangle","kind":"fact","statement":"A triangle has three sides.",
        "qualifications":["In plane geometry."],"topic":"Geometry","importance":"core",
        "evidenceIds":["E0"],"details":{"privateDraft":"INVALID ANNOTATION"}})
}

#[tokio::test]
async fn document_annotation_failure_keeps_core_and_never_buys_a_whole_window_retry() {
    let (database, source, _, _) = crate::knowledge::authoring::tests::fixture();
    let sources = [source];
    let catalog = EvidenceCatalog::build(&sources, false).unwrap();
    let contract = hash("independent annotation admission");
    let signal = CancellationSignal::new();
    let calls = std::cell::Cell::new(0);
    let units = super::super::run_window(
        &database,
        &sources,
        &contract,
        &catalog,
        &signal,
        false,
        |_| {
            calls.set(calls.get() + 1);
            async { Ok(json!({"units":[proposal()]})) }
        },
    )
    .await
    .unwrap();
    assert_eq!(calls.get(), 1);
    assert_eq!(units[&sources[0].id].len(), 1);
    let checkpoint = Checkpoint::open(&database, &sources, &contract).unwrap();
    let receipt = checkpoint.load().unwrap().unwrap();
    assert_eq!(
        receipt.accepted[0].content.statement,
        proposal()["statement"].as_str().unwrap()
    );
    assert_eq!(
        receipt.accepted[0].content.qualifications,
        vec!["In plane geometry."]
    );
    assert!(receipt.rejected.is_empty());
    assert!(!receipt.is_complete());
    assert!(!receipt.requires_paid_repair(&sources));
    let grounded = receipt.accepted[0].content.grounding.as_ref().unwrap();
    assert!(grounded.is_visible_in(&sources));
    let authoring = KnowledgeCatalog {
        units: units[&sources[0].id].clone(),
        ..KnowledgeCatalog::default()
    };
    let prompt = authoring.prompt().unwrap();
    assert!(prompt.contains("A triangle has three sides."));
    assert!(prompt.contains("In plane geometry."));
    assert!(prompt.contains("gradedBindingsAllowed"));
    assert!(!prompt.contains("INVALID ANNOTATION"));
    for retry_partial in [false, true] {
        let reused = super::super::run_window(
            &database,
            &sources,
            &contract,
            &catalog,
            &signal,
            retry_partial,
            |_| async { panic!("annotation debt must not buy another whole-window request") },
        )
        .await
        .unwrap();
        assert_eq!(
            serde_json::to_value(&reused).unwrap(),
            serde_json::to_value(&units).unwrap()
        );
    }
}

#[test]
fn document_annotation_deferral_does_not_admit_invalid_core_or_evidence() {
    let (_, source, _, _) = crate::knowledge::authoring::tests::fixture();
    let sources = [source];
    let catalog = EvidenceCatalog::build(&sources, false).unwrap();
    for (field, value, rule) in [
        ("title", json!(""), "invalid_unit_content"),
        ("statement", json!(""), "invalid_unit_content"),
        ("qualifications", json!([""]), "invalid_unit_content"),
        ("evidenceIds", json!(["E999"]), "invalid_evidence_selection"),
    ] {
        let mut bad = proposal();
        bad[field] = value;
        let report = initial(json!({"units":[bad]}), &catalog, &sources).unwrap();
        assert!(report.accepted.is_empty());
        assert!(report.deferred_annotations().is_empty());
        assert_eq!(report.rejected[0].code, rule);
        assert!(report.requires_paid_repair(&sources));
    }
}

#[tokio::test]
async fn document_annotation_legacy_rejections_recover_locally_without_erasing_history() {
    let (database, source, _, _) = crate::knowledge::authoring::tests::fixture();
    let sources = [source];
    let catalog = EvidenceCatalog::build(&sources, false).unwrap();
    let contract = hash("legacy annotation recovery");
    let mut old = AnalysisReport::default();
    old.rejected.push(RejectedUnit {
        slot: 0,
        proposal: proposal(),
        code: "invalid_qualified_structure".into(),
    });
    old.refresh_coverage(&sources);
    Checkpoint::open(&database, &sources, &contract)
        .unwrap()
        .save(&old)
        .unwrap();
    let recovered = super::super::run_window(
        &database,
        &sources,
        &contract,
        &catalog,
        &CancellationSignal::new(),
        false,
        |_| async { panic!("retained paid draft must be re-admitted locally") },
    )
    .await
    .unwrap();
    assert_eq!(recovered[&sources[0].id].len(), 1);
    let saved = Checkpoint::open(&database, &sources, &contract)
        .unwrap()
        .load()
        .unwrap()
        .unwrap();
    assert!(saved.rejected.is_empty());
    assert!(!saved.is_complete());
    assert_eq!(
        serde_json::to_value(&saved.accepted[0].content.details).unwrap()["draft"],
        proposal()["details"]
    );
    let mut invalid = old;
    invalid.rejected[0].proposal["evidenceIds"] = json!(["E999"]);
    let denied = recover_capacity(&invalid, &catalog, &sources);
    assert!(denied.accepted.is_empty());
    assert_eq!(denied.rejected[0].code, "invalid_evidence_selection");
}

#[test]
fn document_annotation_state_tampering_fails_checkpoint_validation_without_writes() {
    let (database, source, _, _) = crate::knowledge::authoring::tests::fixture();
    let sources = [source];
    let catalog = EvidenceCatalog::build(&sources, false).unwrap();
    let report = initial(json!({"units":[proposal()]}), &catalog, &sources).unwrap();
    let contract = hash("annotation state tampering");
    let mut checkpoint = Checkpoint::open(&database, &sources, &contract).unwrap();
    checkpoint.save(&report).unwrap();
    let mut stale = report.clone();
    stale.accepted[0].content.statement = "A different claim.".into();
    assert!(checkpoint.save(&stale).is_err());
    let restored = checkpoint.load().unwrap().unwrap();
    assert_eq!(
        serde_json::to_value(&restored).unwrap(),
        serde_json::to_value(&report).unwrap()
    );
    database
        .with_connection(|connection| {
            let before = connection.total_changes();
            assert_eq!(repository::load_units(connection, &sources[0].id)?.len(), 1);
            assert_eq!(before, connection.total_changes());
            Ok(())
        })
        .unwrap();
}
