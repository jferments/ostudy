//! Repair retained representation debt before deciding whether any provider work
//! is needed. Full receipt validation and the existing optimistic transaction
//! remain authoritative; annotation-only changes are real checkpoint changes.
use super::*;

pub(super) fn restore(input: Window<'_>, signal: &CancellationSignal) -> Result<(), CommandError> {
    check_canceled(signal)?;
    let mut checkpoint =
        analysis_store::Checkpoint::open(input.database, input.sources, input.contract)?;
    let Some(report) = checkpoint.load()? else {
        return Ok(());
    };
    let Some(repaired) = repair(&report, input.sources)? else {
        return Ok(());
    };
    check_canceled(signal)?;
    checkpoint.save(&repaired)?;
    crate::logging::info_target(
        "ai.knowledge.extraction",
        "knowledge_annotation_checkpoint_repaired",
        "Committed local annotation repairs without a provider request or a core rewrite.",
        json!({
            "deferredAnnotationsBefore": report.deferred_annotations().len(),
            "deferredAnnotationsAfter": repaired.deferred_annotations().len(),
            "acceptedUnits": repaired.accepted.len(),
        }),
    );
    Ok(())
}

fn repair(
    report: &decode::AnalysisReport,
    sources: &[SourceRecord],
) -> Result<Option<decode::AnalysisReport>, CommandError> {
    // This checks every core, qualification, evidence dependency and retained
    // deferred binding, not just the one field that could be normalized.
    analysis_store::validate_report(sources, report)?;
    let mut result = report.clone();
    let mut changed = false;
    for unit in &mut result.accepted {
        let Some(details) = &unit.content.details else {
            continue;
        };
        if let Some(repaired) =
            details.repair_locally(&unit.content.statement, unit.content.kind)?
        {
            unit.content.details = Some(repaired);
            changed = true;
        }
    }
    if changed {
        deduplicate(&mut result)?;
    }
    Ok(changed.then_some(result))
}

/// Exact duplicates can converge when debt is resolved. Coalesce only fully
/// identical admitted units, including every qualification and evidence binding;
/// different claims or annotations remain distinct. No semantic guess is made.
pub(super) fn deduplicate(report: &mut decode::AnalysisReport) -> Result<(), CommandError> {
    let mut seen = HashSet::new();
    let mut accepted = Vec::with_capacity(report.accepted.len());
    for unit in &report.accepted {
        let identity = serde_json::to_string(unit).map_err(CommandError::serialization)?;
        if seen.insert(identity) {
            accepted.push(unit.clone());
        }
    }
    report.accepted = accepted;
    Ok(())
}

#[cfg(test)]
mod tests;
