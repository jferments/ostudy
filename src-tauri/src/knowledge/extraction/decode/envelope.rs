//! Only knowledge batches have additive multi-result semantics. The provider
//! decoder preserves independent tool calls; this domain joins their explicit
//! collections before per-unit validation. Scalar tasks must never pick a result
//! or merge unrelated objects, and ambiguous envelope/proposal mixtures fail.
use serde_json::{Map, Value};

use crate::{error::CommandError, knowledge::contracts::invalid};

pub(super) fn initial(value: Value, maximum: usize) -> Result<Value, CommandError> {
    let fields = ["units", "noLearningSourceIds"];
    if !value.as_array().is_some_and(|items| {
        items
            .iter()
            .any(|item| fields.iter().any(|key| item.get(*key).is_some()))
    }) {
        // The existing bare proposal-array protocol is distinct from a list of
        // complete tool result envelopes. Do not guess a data/results wrapper.
        return Ok(value);
    }
    combine(value, &fields, Some("units"), maximum)
}

pub(super) fn repair(value: Value, maximum: usize) -> Result<Value, CommandError> {
    if !value.is_array() {
        return Ok(value);
    }
    combine(
        value,
        &["repairs", "additionalUnits", "noLearningSourceIds"],
        None,
        maximum,
    )
}

fn combine(
    value: Value,
    fields: &[&str],
    required: Option<&str>,
    maximum: usize,
) -> Result<Value, CommandError> {
    let Value::Array(items) = value else {
        return Ok(value);
    };
    if items.is_empty() || items.len() > maximum {
        return Err(invalid(
            "The knowledge result collection is empty or exceeds its bound.",
        ));
    }
    let count = items.len();
    let mut collections = vec![Vec::new(); fields.len()];
    for item in items {
        let object = item.as_object().ok_or_else(|| {
            invalid("The knowledge result collection mixes envelopes and proposals.")
        })?;
        if !fields.iter().any(|field| object.contains_key(*field))
            || required.is_some_and(|field| !object.get(field).is_some_and(Value::is_array))
        {
            return Err(invalid(
                "Each knowledge result must contain the required response collections.",
            ));
        }
        for (index, field) in fields.iter().enumerate() {
            if let Some(values) = object.get(*field).and_then(Value::as_array) {
                let target = &mut collections[index];
                if values.len() > maximum.saturating_sub(target.len()) {
                    return Err(invalid(
                        "The combined knowledge results exceed their collection budget.",
                    ));
                }
                target.extend(values.iter().cloned());
            }
            // Invalid ancillary dispositions cannot certify coverage. Required
            // units were checked above; missing repair slots remain unresolved.
        }
    }
    let object: Map<String, Value> = fields
        .iter()
        .zip(collections)
        .map(|(field, values)| ((*field).to_string(), Value::Array(values)))
        .collect();
    crate::logging::debug_target(
        "ai.knowledge.extraction",
        "knowledge_result_envelopes_combined",
        "Combined independent proposal collections before native evidence validation.",
        serde_json::json!({"envelopes":count,"collection":required.unwrap_or("repair")}),
    );
    Ok(Value::Object(object))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn joins_only_explicit_knowledge_collections_without_losing_dispositions() {
        assert_eq!(
            initial(
                json!([
                    {"units":[{"title":"first"}],"noLearningSourceIds":["s1"]},
                    {"units":[{"title":"second"}],"noLearningSourceIds":null}
                ]),
                128
            )
            .unwrap(),
            json!({"units":[{"title":"first"},{"title":"second"}],"noLearningSourceIds":["s1"]})
        );
        let bare = json!([{"title":"not an envelope"}]);
        assert_eq!(initial(bare.clone(), 128).unwrap(), bare);
        assert!(initial(json!([{"units":[]},{"title":"mixed"}]), 128).is_err());
        assert!(initial(json!([{"units":null}]), 128).is_err());
        assert!(initial(json!([{"units":[1,2]},{"units":[3]}]), 2).is_err());
    }

    #[test]
    fn duplicate_repair_slots_are_preserved_for_conflict_detection() {
        let merged=repair(json!([
            {"repairs":[{"slot":2,"unit":null}]},
            {"repairs":[{"slot":2,"unit":{"statement":"conflict"}}],"noLearningSourceIds":["s1"]}
        ]),128).unwrap();
        assert_eq!(merged["repairs"].as_array().unwrap().len(), 2);
        assert_eq!(merged["noLearningSourceIds"], json!(["s1"]));
        assert!(repair(json!([{"unrelated":true}]), 128).is_err());
    }
}

#[cfg(test)]
mod admission_tests {
    use crate::knowledge::{
        evidence::EvidenceCatalog,
        extraction::decode::{initial, repair},
    };
    use serde_json::{json, Value};

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
}
