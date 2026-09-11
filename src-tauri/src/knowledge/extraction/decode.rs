//! Per-proposal admission and bounded repair. Invalid candidates remain explicit
//! local drafts, never evidence. A malformed sibling cannot erase valid repairs.
use super::super::{
    contracts::{
        hash, invalid, KnowledgeKind, KnowledgeUnitInput, SourceKind, SourceRecord,
        MAX_UNITS_PER_SOURCE,
    },
    evidence::{EvidenceCatalog, Grounding, Importance, EVIDENCE_VERSION},
};
use crate::{ai::structured_output, error::CommandError};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

pub(crate) const MAX_WINDOW_UNITS: usize = 48;
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct AcceptedUnit {
    pub source_id: String,
    pub content: KnowledgeUnitInput,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RejectedUnit {
    pub slot: usize,
    pub proposal: Value,
    pub code: String,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct AnalysisReport {
    pub(crate) accepted: Vec<AcceptedUnit>,
    pub(crate) rejected: Vec<RejectedUnit>,
    pub(crate) no_learning: Vec<String>,
    pub(crate) missing_sources: Vec<String>,
}
impl AnalysisReport {
    pub(crate) fn rejected_count(&self) -> usize {
        self.rejected.len() + self.missing_sources.len()
    }
    pub(crate) fn is_complete(&self) -> bool {
        self.rejected.is_empty() && self.missing_sources.is_empty()
    }
    pub(crate) fn refresh_coverage(&mut self, sources: &[SourceRecord]) {
        let used = self
            .accepted
            .iter()
            .flat_map(|unit| {
                unit.content
                    .grounding
                    .iter()
                    .flat_map(|g| g.evidence.iter().map(|e| e.source_id.as_str()))
            })
            .collect::<HashSet<_>>();
        self.no_learning.retain(|id| !used.contains(id.as_str()));
        self.missing_sources = sources
            .iter()
            .filter(|s| !used.contains(s.id.as_str()) && !self.no_learning.contains(&s.id))
            .map(|s| s.id.clone())
            .collect();
    }
    pub(crate) fn repair_prompt(&self) -> Value {
        json!({"rejectedProposals":self.rejected,"sourcesWithoutDisposition":self.missing_sources})
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Proposal {
    title: String,
    kind: KnowledgeKind,
    statement: String,
    qualifications: Vec<String>,
    topic: String,
    importance: Importance,
    evidence_ids: Vec<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Repair {
    slot: usize,
    unit: Value,
}

fn admit(
    value: Value,
    catalog: &EvidenceCatalog,
    sources: &[SourceRecord],
) -> Result<AcceptedUnit, CommandError> {
    let proposal: Proposal =
        serde_json::from_value(value).map_err(|_| invalid("malformed_unit"))?;
    let evidence = catalog
        .resolve(&proposal.evidence_ids)
        .map_err(|_| invalid("invalid_evidence_selection"))?;
    let primary_id = &evidence
        .first()
        .ok_or_else(|| invalid("missing_evidence"))?
        .source_id;
    let primary = sources
        .iter()
        .find(|s| &s.id == primary_id)
        .ok_or_else(|| invalid("unknown_source"))?;
    for reference in &evidence {
        let source = sources
            .iter()
            .find(|s| s.id == reference.source_id)
            .ok_or_else(|| invalid("unknown_source"))?;
        if source.origin != primary.origin
            || source.kind != primary.kind
            || (source.id != primary.id
                && !(source.kind == SourceKind::Document && source.owner_id == primary.owner_id))
        {
            return Err(invalid("mixed_source_authority"));
        }
        reference.validate(source)?;
    }
    let supporting_excerpt = evidence
        .iter()
        .find(|e| e.source_id == primary.id && !e.excerpt.is_empty())
        .map_or_else(String::new, |e| e.excerpt.clone());
    let content = KnowledgeUnitInput {
        title: proposal.title,
        kind: proposal.kind,
        statement: proposal.statement,
        qualifications: proposal.qualifications,
        topic: proposal.topic,
        supporting_excerpt,
        grounding: Some(Grounding {
            version: EVIDENCE_VERSION,
            importance: proposal.importance,
            evidence,
        }),
    };
    content
        .validate(primary)
        .map_err(|_| invalid("invalid_unit_content"))?;
    Ok(AcceptedUnit {
        source_id: primary.id.clone(),
        content,
    })
}
fn append_unit(
    report: &mut AnalysisReport,
    value: Value,
    slot: usize,
    catalog: &EvidenceCatalog,
    sources: &[SourceRecord],
) {
    let result = admit(value.clone(), catalog, sources).and_then(|unit| {
        let identity = serde_json::to_value(&unit).map_err(CommandError::serialization)?;
        if report
            .accepted
            .iter()
            .any(|u| serde_json::to_value(u).is_ok_and(|other| other == identity))
        {
            return Ok(None);
        }
        let count = report
            .accepted
            .iter()
            .filter(|u| u.source_id == unit.source_id)
            .count();
        if count >= MAX_UNITS_PER_SOURCE || report.accepted.len() >= MAX_WINDOW_UNITS {
            Err(invalid("unit_capacity"))
        } else {
            Ok(Some(unit))
        }
    });
    match result {
        Ok(Some(unit)) => report.accepted.push(unit),
        Ok(None) => {}
        Err(error) => {
            let proposal = if value.to_string().len() <= 12000 {
                value
            } else {
                json!({"oversizedDraft":true,"fingerprint":hash(&value.to_string())})
            };
            report.rejected.push(RejectedUnit {
                slot,
                proposal,
                code: error.message,
            });
        }
    }
}
fn add_dispositions(report: &mut AnalysisReport, ids: &[Value], sources: &[SourceRecord]) {
    for value in ids {
        if let Some(id) = value.as_str() {
            if sources.iter().any(|s| s.id == id)
                && !report.no_learning.iter().any(|existing| existing == id)
            {
                report.no_learning.push(id.to_string());
            }
        }
    }
}

/// Decode only the explicitly named collection. Ignoring unusable ancillary
/// dispositions never certifies a source: refresh_coverage keeps it uncovered.
/// Unlike native IPC/storage DTOs, a provider envelope is not an authority boundary.
fn ancillary<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
}

pub(crate) fn initial(
    value: Value,
    catalog: &EvidenceCatalog,
    sources: &[SourceRecord],
) -> Result<AnalysisReport, CommandError> {
    let schema = super::schema::initial(catalog, sources);
    structured_output::log_shape(&value, &schema, "knowledge_initial");
    // Only one documented lossless alternate envelope is supported: a bare
    // proposal array. No arbitrary data/results wrapper or invented field names.
    let units = value
        .get("units")
        .and_then(Value::as_array)
        .or_else(|| value.as_array())
        .ok_or_else(|| {
            invalid(format!(
                "The semantic response must contain a units array. {}",
                structured_output::shape_detail(&value, &schema)
            ))
        })?;
    let dispositions = ancillary(&value, "noLearningSourceIds");
    if units.len() > MAX_WINDOW_UNITS || dispositions.len() > MAX_WINDOW_UNITS {
        return Err(invalid("The semantic window exceeded its proposal limit."));
    }
    let mut report = AnalysisReport::default();
    add_dispositions(&mut report, dispositions, sources);
    for (slot, proposal) in units.iter().enumerate() {
        append_unit(&mut report, proposal.clone(), slot, catalog, sources);
    }
    report.refresh_coverage(sources);
    Ok(report)
}
pub(crate) fn repair(
    report: &AnalysisReport,
    value: Value,
    catalog: &EvidenceCatalog,
    sources: &[SourceRecord],
) -> Result<AnalysisReport, CommandError> {
    let schema = super::schema::repair(catalog, sources, report);
    structured_output::log_shape(&value, &schema, "knowledge_repair");
    if !value.is_object() {
        return Err(invalid(format!(
            "The targeted repair must be a JSON object. {}",
            structured_output::shape_detail(&value, &schema)
        )));
    }
    let repairs = ancillary(&value, "repairs");
    let additional_units = ancillary(&value, "additionalUnits");
    let dispositions = ancillary(&value, "noLearningSourceIds");
    if repairs.len() > MAX_WINDOW_UNITS
        || additional_units.len() > MAX_WINDOW_UNITS
        || dispositions.len() > MAX_WINDOW_UNITS
    {
        return Err(invalid("The targeted repair exceeded its bounds."));
    }
    let pending = report
        .rejected
        .iter()
        .map(|item| (item.slot, item))
        .collect::<HashMap<_, _>>();
    let mut counts = HashMap::new();
    for raw in repairs {
        if let Some(slot) = raw
            .get("slot")
            .and_then(Value::as_u64)
            .and_then(|v| usize::try_from(v).ok())
        {
            *counts.entry(slot).or_insert(0usize) += 1;
        }
    }
    let mut result = report.clone();
    for raw in repairs {
        let Ok(item) = Repair::deserialize(raw) else {
            continue;
        };
        if !pending.contains_key(&item.slot)
            || counts.get(&item.slot) != Some(&1)
            || item.unit.is_null()
        {
            continue;
        }
        result
            .rejected
            .retain(|original| original.slot != item.slot);
        append_unit(&mut result, item.unit, item.slot, catalog, sources);
    }
    let mut next_slot = report
        .rejected
        .iter()
        .map(|item| item.slot)
        .max()
        .unwrap_or(report.accepted.len())
        .saturating_add(1);
    for value in additional_units {
        if report.missing_sources.is_empty()
            || result.accepted.len() + result.rejected.len() >= MAX_WINDOW_UNITS
        {
            continue;
        }
        let addresses_missing = admit(value.clone(), catalog, sources).is_ok_and(|unit| {
            unit.content.grounding.is_some_and(|g| {
                g.evidence
                    .iter()
                    .any(|e| report.missing_sources.contains(&e.source_id))
            })
        });
        if addresses_missing {
            append_unit(&mut result, value.clone(), next_slot, catalog, sources);
            next_slot = next_slot.saturating_add(1);
        }
    }
    add_dispositions(&mut result, dispositions, sources);
    result.refresh_coverage(sources);
    Ok(result)
}
pub(crate) fn report_diagnostic(report: &AnalysisReport) -> Value {
    json!({"acceptedUnits":report.accepted.len(),"rejectedUnits":report.rejected.len(),"uncoveredSources":report.missing_sources,
        "complete":report.is_complete(),"rejections":report.rejected.iter().map(|r|json!({"slot":r.slot,"category":r.code,
        "draftFingerprint":hash(&r.proposal.to_string())})).collect::<Vec<_>>()})
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge::authoring::tests::fixture;
    fn unit(ids: Value) -> Value {
        json!({"title":"Sides","kind":"fact","statement":"A triangle has three sides.","qualifications":[],"topic":"Geometry","importance":"core","evidenceIds":ids})
    }
    #[test]
    fn bad_sibling_is_quarantined_and_repair_does_not_replace_good_sibling() {
        let (_, source, _, _) = fixture();
        let sources = [source];
        let catalog = EvidenceCatalog::build(&sources, false).unwrap();
        let report = initial(
            json!({"units":[unit(json!(["E0"])),unit(json!(["E999"]))],"noLearningSourceIds":[]}),
            &catalog,
            &sources,
        )
        .unwrap();
        assert_eq!(report.accepted.len(), 1);
        assert_eq!(report.rejected.len(), 1);
        let old = serde_json::to_value(&report.accepted[0]).unwrap();
        let repaired=repair(&report,json!({"repairs":[{"slot":1,"unit":null}],"additionalUnits":[],"noLearningSourceIds":[]}),&catalog,&sources).unwrap();
        assert_eq!(serde_json::to_value(&repaired.accepted[0]).unwrap(), old);
        assert!(!repaired.is_complete());
    }
    #[test]
    fn cross_page_claim_keeps_two_separate_native_evidence_spans() {
        let (_, first, _, _) = fixture();
        let mut second = first.clone();
        second.segment_key = "1:0:0".into();
        second.text = "Its interior angles sum to 180 degrees.".into();
        second.refresh_identity().unwrap();
        let sources = [first, second];
        let catalog = EvidenceCatalog::build(&sources, false).unwrap();
        let report = initial(
            json!({"units":[unit(json!(["E0","E1"]))],"noLearningSourceIds":[]}),
            &catalog,
            &sources,
        )
        .unwrap();
        assert!(report.is_complete());
        let grounding = report.accepted[0].content.grounding.as_ref().unwrap();
        assert_eq!(grounding.evidence.len(), 2);
        assert_ne!(
            grounding.evidence[0].source_id,
            grounding.evidence[1].source_id
        );
        assert!(grounding.is_visible_in(&sources));
        assert!(!grounding.is_visible_in(&sources[..1]));
    }
    #[test]
    fn empty_output_is_not_complete_without_explicit_non_learning_dispositions() {
        let (_, source, _, _) = fixture();
        let sources = [source];
        let catalog = EvidenceCatalog::build(&sources, false).unwrap();
        assert!(!initial(
            json!({"units":[],"noLearningSourceIds":[]}),
            &catalog,
            &sources
        )
        .unwrap()
        .is_complete());
        assert!(initial(
            json!({"units":[],"noLearningSourceIds":[sources[0].id]}),
            &catalog,
            &sources
        )
        .unwrap()
        .is_complete());
    }
    #[test]
    fn malformed_and_duplicate_repair_slots_do_not_discard_independent_success() {
        let (_, source, _, _) = fixture();
        let sources = [source];
        let catalog = EvidenceCatalog::build(&sources, false).unwrap();
        let report=initial(json!({"units":[unit(json!(["E0"])),unit(json!(["E404"])),unit(json!(["E405"]))],"noLearningSourceIds":[]}),&catalog,&sources).unwrap();
        let mut fixed = unit(json!(["E0"]));
        fixed["title"] = json!("Another target");
        for bad in [
            json!({"slot":"invalid","unit":null}),
            json!({"slot":2,"unexpected":true}),
            json!({"slot":999,"unit":null}),
        ] {
            let repaired=repair(&report,json!({"repairs":[{"slot":1,"unit":fixed},bad],"additionalUnits":[],"noLearningSourceIds":[]}),&catalog,&sources).unwrap();
            assert_eq!(repaired.accepted.len(), 2);
            assert_eq!(repaired.rejected.len(), 1);
            assert_eq!(repaired.rejected[0].slot, 2);
        }
        let repaired=repair(&report,json!({"repairs":[{"slot":1,"unit":null},{"slot":1,"unit":fixed},{"slot":2,"unit":fixed}],"additionalUnits":[],"noLearningSourceIds":[]}),&catalog,&sources).unwrap();
        assert_eq!(repaired.accepted.len(), 2);
        assert_eq!(repaired.rejected.len(), 1);
        assert_eq!(repaired.rejected[0].slot, 1);
    }
    #[test]
    fn malformed_disposition_does_not_poison_valid_unit_or_hide_an_uncovered_source() {
        let (_, first, _, _) = fixture();
        let mut second = first.clone();
        second.segment_key = "1:0:0".into();
        second.refresh_identity().unwrap();
        let sources = [first, second];
        let catalog = EvidenceCatalog::build(&sources, false).unwrap();
        let report = initial(
            json!({"units":[unit(json!(["E0"]))],"noLearningSourceIds":[123,"invented"]}),
            &catalog,
            &sources,
        )
        .unwrap();
        assert_eq!(report.accepted.len(), 1);
        assert_eq!(report.missing_sources, vec![sources[1].id.clone()]);
    }
    #[test]
    fn valid_reference_is_not_a_claim_of_factual_entailment() {
        let (_, source, _, _) = fixture();
        let sources = [source];
        let catalog = EvidenceCatalog::build(&sources, false).unwrap();
        let mut proposal = unit(json!(["E0"]));
        proposal["statement"] = json!("An unsupported interpretation.");
        let report = initial(
            json!({"units":[proposal],"noLearningSourceIds":[]}),
            &catalog,
            &sources,
        )
        .unwrap();
        assert_eq!(
            report.accepted[0].content.statement,
            "An unsupported interpretation."
        );
        assert_eq!(
            report.accepted[0].content.supporting_excerpt,
            sources[0].text
        );
    }
    #[test]
    fn envelope_variants_preserve_valid_units_and_leave_uncovered_sources_pending() {
        let (_, first, _, _) = fixture();
        let mut second = first.clone();
        second.segment_key = "1:0:0".into();
        second.refresh_identity().unwrap();
        let sources = [first, second];
        let catalog = EvidenceCatalog::build(&sources, false).unwrap();
        for value in [
            json!({"units":[unit(json!(["E0"]))]}),
            json!({"units":[unit(json!(["E0"]))],"noLearningSourceIds":null}),
            json!({"units":[unit(json!(["E0"]))],"noLearningSourceIds":"not a disposition","commentary":"unused"}),
            json!([unit(json!(["E0"]))]),
        ] {
            let report = initial(value, &catalog, &sources).unwrap();
            assert_eq!(report.accepted.len(), 1);
            assert_eq!(report.missing_sources, vec![sources[1].id.clone()]);
            assert!(!report.is_complete());
        }
    }
    #[test]
    fn ambiguous_wrappers_and_missing_primary_arrays_are_not_guessed() {
        let (_, source, _, _) = fixture();
        let sources = [source];
        let catalog = EvidenceCatalog::build(&sources, false).unwrap();
        for value in [
            json!({"data":{"units":[]}}),
            json!({"units":null}),
            json!({"units":"PRIVATE TEXT"}),
            json!({}),
        ] {
            let error = initial(value, &catalog, &sources).unwrap_err();
            assert!(error.message.contains("units"));
            assert!(!error.message.contains("PRIVATE TEXT"));
        }
    }
    #[test]
    fn malformed_ancillary_repair_collection_does_not_throw_away_valid_corrections() {
        let (_, source, _, _) = fixture();
        let sources = [source];
        let catalog = EvidenceCatalog::build(&sources, false).unwrap();
        let report = initial(json!({"units":[unit(json!(["E404"]))]}), &catalog, &sources).unwrap();
        let repaired = repair(
            &report,
            json!({"repairs":[{"slot":0,"unit":unit(json!(["E0"]))}],"additionalUnits":null}),
            &catalog,
            &sources,
        )
        .unwrap();
        assert!(repaired.is_complete());
        assert_eq!(repaired.accepted.len(), 1);
        let unchanged = repair(
            &report,
            json!({"repairs":"bad","additionalUnits":null}),
            &catalog,
            &sources,
        )
        .unwrap();
        assert!(!unchanged.is_complete());
    }
}
