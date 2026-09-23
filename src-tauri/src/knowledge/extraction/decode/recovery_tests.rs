//! Exercise admission, revision-bound receipts and the real checkpoint loop.
use super::*;
use crate::knowledge::authoring::tests::fixture;
use crate::task_cancellation::CancellationSignal;

fn sources() -> Vec<SourceRecord> {
    let (_, first, _, _) = fixture();
    let mut second = first.clone();
    second.segment_key = "1:0:0".into();
    second.text = "Its interior angles sum to 180 degrees.".into();
    second.refresh_identity().unwrap();
    vec![first, second]
}

fn unit(evidence_ids: Value) -> Value {
    json!({
        "title": "Triangle", "kind": "fact",
        "statement": "A triangle has three sides and interior angles summing to 180 degrees.",
        "qualifications": [], "topic": "Geometry", "importance": "core",
        "evidenceIds": evidence_ids
    })
}

#[test]
fn document_supporting_sources_do_not_create_phantom_repair_obligations() {
    let sources = sources();
    let catalog = EvidenceCatalog::build(&sources, false).unwrap();
    let mut report = initial(
        json!({"units": [unit(json!(["E0", "E1"]))]}),
        &catalog,
        &sources,
    )
    .unwrap();
    assert!(report.is_complete());
    assert!(!report.requires_paid_repair(&sources));
    assert_eq!(report.accepted[0].source_id, sources[0].id);
    // Older receipts without region accounting must also count supporting refs.
    report.coverage = None;
    report.refresh_coverage(&sources);
    assert!(report.is_complete());
    assert!(!report.requires_paid_repair(&sources));
}

#[test]
fn document_region_dispositions_and_recovery_agree_without_whole_source_labels() {
    let sources = sources();
    let catalog = EvidenceCatalog::build(&sources, false).unwrap();
    for disposition in ["retained_metadata", "no_learning", "unreadable"] {
        let report = initial(
            json!({
                "units": [], "noLearningSourceIds": [],
                "regionDispositions": [
                    {"evidenceId": "E0", "disposition": disposition},
                    {"evidenceId": "E1", "disposition": disposition}
                ]
            }),
            &catalog,
            &sources,
        )
        .unwrap();
        assert!(report.is_complete());
        assert!(report.accepted.is_empty());
        assert!(!report.requires_paid_repair(&sources));
    }
}

#[test]
fn document_secondary_source_gaps_are_not_mistaken_for_missing_primary_claims() {
    let mut sources = sources();
    sources[1].text = "Its interior angles sum to 180 degrees. ".repeat(40);
    sources[1].refresh_identity().unwrap();
    let catalog = EvidenceCatalog::build(&sources, false).unwrap();
    assert!(catalog.entries.len() > 2);
    let report = initial(
        json!({"units": [unit(json!(["E0", "E1"]))]}),
        &catalog,
        &sources,
    )
    .unwrap();
    assert!(!report.is_complete());
    assert_eq!(report.missing_sources, vec![sources[1].id.clone()]);
    assert!(!report.requires_paid_repair(&sources));
}

#[test]
fn document_real_missing_sources_and_rejected_proposals_still_need_repair() {
    let sources = sources();
    let catalog = EvidenceCatalog::build(&sources, false).unwrap();
    let missing = initial(json!({"units": []}), &catalog, &sources).unwrap();
    assert!(!missing.is_complete());
    assert!(missing.requires_paid_repair(&sources));
    let rejected = initial(
        json!({"units": [unit(json!(["E0", "E1"])), unit(json!(["E999"]))]}),
        &catalog,
        &sources,
    )
    .unwrap();
    assert_eq!(rejected.accepted.len(), 1);
    assert_eq!(rejected.rejected.len(), 1);
    assert!(rejected.requires_paid_repair(&sources));
}

#[tokio::test]
async fn document_complete_cross_source_receipt_uses_one_provider_call_and_reuses_checkpoint() {
    let (database, _, _, _) = fixture();
    let sources = sources();
    database
        .with_connection(|connection| {
            crate::knowledge::repository::publish_source(connection, &sources[1], "0", 1)
        })
        .unwrap();
    let catalog = EvidenceCatalog::build(&sources, false).unwrap();
    let contract = hash("cross-source recovery regression");
    let mut calls = 0;
    let result = super::super::run_window(
        &database,
        &sources,
        &contract,
        &catalog,
        &CancellationSignal::new(),
        false,
        |_| {
            calls += 1;
            assert_eq!(calls, 1, "a complete receipt must not buy repair");
            std::future::ready(Ok(json!({"units": [unit(json!(["E0", "E1"]))]})))
        },
    )
    .await
    .unwrap();
    assert_eq!(calls, 1);
    assert_eq!(result.values().map(Vec::len).sum::<usize>(), 1);
    let reused = super::super::run_window(
        &database,
        &sources,
        &contract,
        &catalog,
        &CancellationSignal::new(),
        true,
        |_| async { panic!("the complete durable receipt must be reused") },
    )
    .await
    .unwrap();
    assert_eq!(
        serde_json::to_value(result).unwrap(),
        serde_json::to_value(reused).unwrap()
    );
}

#[tokio::test]
async fn document_region_only_receipt_does_not_buy_an_empty_repair() {
    let (database, source, _, _) = fixture();
    let sources = [source];
    let catalog = EvidenceCatalog::build(&sources, false).unwrap();
    let mut calls = 0;
    let result = super::super::run_window(
        &database,
        &sources,
        &hash("region-only recovery regression"),
        &catalog,
        &CancellationSignal::new(),
        false,
        |_| {
            calls += 1;
            assert_eq!(
                calls, 1,
                "region disposition already accounts for the source"
            );
            std::future::ready(Ok(json!({
                "units": [], "noLearningSourceIds": [],
                "regionDispositions": [{"evidenceId": "E0", "disposition": "retained_metadata"}]
            })))
        },
    )
    .await
    .unwrap();
    assert_eq!(calls, 1);
    assert_eq!(result.values().map(Vec::len).sum::<usize>(), 0);
}
