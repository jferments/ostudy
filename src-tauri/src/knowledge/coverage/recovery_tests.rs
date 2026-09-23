//! Recovery eligibility is a projection of the same receipt as completeness.
use super::*;

fn receipt(states: &[(&str, bool, Disposition)]) -> CoverageReceipt {
    CoverageReceipt {
        version: VERSION,
        plan_hash: hash("recovery test"),
        images_attached: false,
        regions: states
            .iter()
            .enumerate()
            .map(|(index, (source, exposed, state))| Region {
                id: format!("region-{index}"),
                source_id: (*source).into(),
                exposed: *exposed,
                declared: None,
                state: *state,
            })
            .collect(),
    }
}

#[test]
fn document_recovery_does_not_reinterpret_resolved_dispositions() {
    for state in [
        Disposition::Interpreted,
        Disposition::RetainedMetadata,
        Disposition::NoLearning,
        Disposition::Unreadable,
    ] {
        let coverage = receipt(&[("source", true, state)]);
        assert_eq!(coverage.unresolved(), 0);
        assert!(coverage.same_input_repair_sources().is_empty());
        assert_eq!(coverage.diagnostic()["protocolComplete"], true);
        assert_eq!(coverage.diagnostic()["exhaustiveConceptCoverage"], false);
    }
}

#[test]
fn document_recovery_preserves_unexposed_and_interpreted_source_gaps() {
    let coverage = receipt(&[
        ("supported", true, Disposition::Interpreted),
        ("supported", true, Disposition::Unresolved),
        ("omitted-image", false, Disposition::Unresolved),
        ("unreadable", true, Disposition::Unreadable),
    ]);
    let before = coverage.clone();
    assert!(coverage.same_input_repair_sources().is_empty());
    assert_eq!(coverage.unresolved(), 2);
    assert_eq!(coverage.diagnostic()["protocolComplete"], false);
    assert_eq!(coverage.diagnostic()["unreadable"], 1);
    assert_eq!(coverage, before);
}

#[test]
fn document_recovery_targets_only_exposed_unaccounted_sources_in_stable_order() {
    let coverage = receipt(&[
        ("first", true, Disposition::Unresolved),
        ("first", true, Disposition::Unresolved),
        ("supported", true, Disposition::Unresolved),
        ("supported", true, Disposition::Interpreted),
        ("omitted", false, Disposition::Unresolved),
        ("second", true, Disposition::RetainedMetadata),
        ("second", true, Disposition::Unresolved),
    ]);
    assert_eq!(
        coverage.same_input_repair_sources(),
        vec!["first", "second"]
    );
    assert_eq!(coverage.unresolved(), 5);
    assert_eq!(
        coverage.diagnostic()["sameInputRepairSources"],
        json!(["first", "second"])
    );
}
