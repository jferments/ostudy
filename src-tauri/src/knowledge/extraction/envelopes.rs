//! A provider may submit several independent tool calls for one semantic batch.
//! Combine only the named additive proposal collections, before ordinary native
//! admission. Never merge scalar fields, assign evidence, or pick one result.
use serde_json::{json, Value};

use super::{decode, EvidenceCatalog, SourceRecord};
use crate::error::CommandError;

pub(super) fn initial(
    value: Value,
    catalog: &EvidenceCatalog,
    sources: &[SourceRecord],
) -> Result<decode::AnalysisReport, CommandError> {
    decode::initial(
        combine(value, &["units", "noLearningSourceIds"], "units")?,
        catalog,
        sources,
    )
}

pub(super) fn repair(
    report: &decode::AnalysisReport,
    value: Value,
    catalog: &EvidenceCatalog,
    sources: &[SourceRecord],
) -> Result<decode::AnalysisReport, CommandError> {
    decode::repair(
        report,
        combine(
            value,
            &["repairs", "additionalUnits", "noLearningSourceIds"],
            "repairs",
        )?,
        catalog,
        sources,
    )
}

fn combine(value: Value, fields: &[&str], primary: &str) -> Result<Value, CommandError> {
    let Some(envelopes) = value.as_array().filter(|values| {
        !values.is_empty() && values.iter().any(|value| value.get(primary).is_some())
    }) else {
        return Ok(value);
    };
    if envelopes.len() > decode::MAX_WINDOW_UNITS
        || envelopes
            .iter()
            .any(|value| !value.get(primary).is_some_and(Value::is_array))
    {
        return Err(super::invalid(
            "The semantic batch contains ambiguous or oversized result envelopes.",
        ));
    }
    let mut combined = json!({});
    for field in fields {
        let mut items = Vec::new();
        for envelope in envelopes {
            if let Some(values) = envelope.get(field).and_then(Value::as_array) {
                if items.len() + values.len() > decode::MAX_WINDOW_UNITS {
                    return Err(super::invalid(
                        "The combined semantic proposals exceed the window limit.",
                    ));
                }
                items.extend(values.iter().cloned());
            }
        }
        combined[*field] = Value::Array(items);
    }
    crate::logging::debug_target(
        "ai.knowledge.extraction",
        "knowledge_result_envelopes_combined",
        "Combined independent proposal collections before native evidence validation.",
        json!({"envelopes":envelopes.len(),"collection":primary}),
    );
    Ok(combined)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(evidence: &str) -> Value {
        json!({"title":"Sides","kind":"fact","statement":"A triangle has three sides.",
            "qualifications":[],"topic":"Geometry","importance":"core","evidenceIds":[evidence]})
    }

    #[test]
    fn multiple_envelopes_keep_valid_siblings_and_quarantine_invalid_evidence() {
        let (_, source, _, _) = crate::knowledge::authoring::tests::fixture();
        let sources = [source];
        let catalog = EvidenceCatalog::build(&sources, false).unwrap();
        let report = initial(
            json!([
                {"units":[unit("E0")],"noLearningSourceIds":[]},
                {"units":[unit("E404")],"noLearningSourceIds":[]}
            ]),
            &catalog,
            &sources,
        )
        .unwrap();
        assert_eq!(report.accepted.len(), 1);
        assert_eq!(report.rejected.len(), 1);
        assert!(!report.is_complete());
        assert_eq!(
            report.accepted[0].content.supporting_excerpt,
            sources[0].text
        );
        let fixed = repair(
            &report,
            json!([
                {"repairs":[{"slot":1,"unit":unit("E0")}]},
                {"repairs":[],"additionalUnits":[],"noLearningSourceIds":[]}
            ]),
            &catalog,
            &sources,
        )
        .unwrap();
        assert!(fixed.is_complete());
        assert_eq!(fixed.accepted.len(), 1);
    }

    #[test]
    fn bare_unit_arrays_still_work_and_non_learning_dispositions_survive() {
        let (_, source, _, _) = crate::knowledge::authoring::tests::fixture();
        let sources = [source];
        let catalog = EvidenceCatalog::build(&sources, false).unwrap();
        assert!(initial(json!([unit("E0")]), &catalog, &sources)
            .unwrap()
            .is_complete());
        let report = initial(
            json!([
                {"units":[],"noLearningSourceIds":[sources[0].id]},
                {"units":[]}
            ]),
            &catalog,
            &sources,
        )
        .unwrap();
        assert!(report.is_complete());
        assert!(report.accepted.is_empty());
    }

    #[test]
    fn combined_limits_and_ambiguous_results_are_not_silently_truncated() {
        assert!(combine(json!([{"units":[]},{"units":null}]), &["units"], "units").is_err());
        assert!(combine(
            json!([{"units":vec![unit("E0");100]},
            {"units":vec![unit("E0");100]}]),
            &["units"],
            "units"
        )
        .is_err());
    }
}
