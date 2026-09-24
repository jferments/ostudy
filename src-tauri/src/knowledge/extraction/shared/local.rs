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
    Ok(changed.then_some(result))
}

#[cfg(test)]
mod tests;
