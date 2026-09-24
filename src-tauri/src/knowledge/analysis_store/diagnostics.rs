//! Project only bounded native counts/rule IDs, never rejected drafts or source
//! text, into the request ledger. Coverage is not a certificate of entailment.
use super::AnalysisReport;
use crate::ai::generation::outcomes::{self, DeferredAnnotations, Snapshot};

fn snapshot(report: &AnalysisReport) -> Snapshot {
    let mut rejection_rules = std::collections::BTreeMap::new();
    for rejected in &report.rejected {
        let rule = &rejected.code;
        let safe = !rule.is_empty()
            && rule.len() <= 80
            && rule
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_');
        let rule = if safe {
            rule.as_str()
        } else {
            "unclassified_native_rejection"
        };
        *rejection_rules.entry(rule.to_string()).or_insert(0) += 1;
    }
    let mut deferred = DeferredAnnotations::default();
    for unit in &report.accepted {
        if let Some(rule) = unit
            .content
            .details
            .as_ref()
            .and_then(|details| details.deferred_rule())
        {
            // The saved payload already passed strict state/rule validation.
            deferred.count += 1;
            *deferred.rules.entry(rule.to_string()).or_insert(0) += 1;
        }
    }
    Snapshot {
        accepted_units: report.accepted.len() as u32,
        rejected_proposals: report.rejected.len() as u32,
        unaccounted_sources: report.missing_sources.len() as u32,
        unresolved_regions: report
            .coverage
            .as_ref()
            .map(|coverage| coverage.unresolved() as u32),
        protocol_complete: report.is_complete(),
        rejection_rules,
        deferred_annotations: Some(deferred),
    }
}

pub(super) fn baseline(report: &AnalysisReport) {
    outcomes::baseline(snapshot(report));
}

pub(super) fn committed(report: &AnalysisReport) {
    outcomes::checkpoint(snapshot(report));
}

impl super::Checkpoint<'_> {
    /// Call only after a successful revision-checked local reconciliation.
    pub(crate) fn observe_reconciled(&self, report: &AnalysisReport) {
        outcomes::checkpoint_reconciled(snapshot(report));
    }
}
