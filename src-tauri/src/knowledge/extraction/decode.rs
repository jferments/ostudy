//! Per-proposal admission and bounded repair. Rejected proposals are local draft
//! data, never knowledge. Successful siblings are not regenerated during repair.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::error::CommandError;

use super::super::{
    contracts::{
        hash, invalid, KnowledgeKind, KnowledgeUnitInput, SourceKind, SourceRecord,
        MAX_UNITS_PER_SOURCE,
    },
    evidence::{EvidenceCatalog, Grounding, Importance, EVIDENCE_VERSION},
};

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
                unit.content.grounding.iter().flat_map(|grounding| {
                    grounding
                        .evidence
                        .iter()
                        .map(|evidence| evidence.source_id.as_str())
                })
            })
            .collect::<HashSet<_>>();
        self.no_learning.retain(|id| !used.contains(id.as_str()));
        self.missing_sources = sources
            .iter()
            .filter(|source| {
                !used.contains(source.id.as_str()) && !self.no_learning.contains(&source.id)
            })
            .map(|source| source.id.clone())
            .collect();
    }

    pub(crate) fn repair_prompt(&self) -> Value {
        json!({"rejectedProposals": self.rejected, "sourcesWithoutDisposition": self.missing_sources})
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
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WindowResponse {
    units: Vec<Value>,
    no_learning_source_ids: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RepairResponse {
    repairs: Vec<Repair>,
    additional_units: Vec<Value>,
    no_learning_source_ids: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Repair {
    slot: usize,
    unit: Option<Value>,
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
        .find(|source| &source.id == primary_id)
        .ok_or_else(|| invalid("unknown_source"))?;
    for reference in &evidence {
        let source = sources
            .iter()
            .find(|source| source.id == reference.source_id)
            .ok_or_else(|| invalid("unknown_source"))?;
        // Relationships may span document pages, but cannot launder an authored
        // assertion through a document or another unrelated material owner.
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
        .find(|item| item.source_id == primary.id && !item.excerpt.is_empty())
        .map_or_else(String::new, |item| item.excerpt.clone());
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
            .any(|other| serde_json::to_value(other).is_ok_and(|other| other == identity))
        {
            return Ok(None);
        }
        let count = report
            .accepted
            .iter()
            .filter(|other| other.source_id == unit.source_id)
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
            let proposal = if value.to_string().len() <= 12_000 {
                value
            } else {
                // Keep a rejection identity, not an unbounded provider payload.
                // A repair can construct a replacement from the admitted evidence.
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

fn add_dispositions(report: &mut AnalysisReport, ids: Vec<String>, sources: &[SourceRecord]) {
    // Unknown IDs are not evidence that any supplied source was analyzed. Leave
    // those sources explicitly uncovered, without discarding valid siblings.
    for id in ids.into_iter().take(sources.len()) {
        if sources.iter().any(|source| source.id == id) && !report.no_learning.contains(&id) {
            report.no_learning.push(id);
        }
    }
}

pub(crate) fn initial(
    value: Value,
    catalog: &EvidenceCatalog,
    sources: &[SourceRecord],
) -> Result<AnalysisReport, CommandError> {
    let response: WindowResponse = serde_json::from_value(value)
        .map_err(|_| invalid("The semantic window response has an invalid envelope."))?;
    if response.units.len() > MAX_WINDOW_UNITS {
        return Err(invalid("The semantic window exceeded its proposal limit."));
    }
    let mut report = AnalysisReport::default();
    add_dispositions(&mut report, response.no_learning_source_ids, sources);
    for (slot, value) in response.units.into_iter().enumerate() {
        append_unit(&mut report, value, slot, catalog, sources);
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
    let response: RepairResponse = serde_json::from_value(value)
        .map_err(|_| invalid("The targeted repair returned an invalid envelope."))?;
    if response.repairs.len() > report.rejected.len()
        || response.additional_units.len() > MAX_WINDOW_UNITS
    {
        return Err(invalid("The targeted repair exceeded its bounds."));
    }
    let pending = report
        .rejected
        .iter()
        .map(|item| (item.slot, item))
        .collect::<HashMap<_, _>>();
    let mut seen = HashSet::new();
    let mut result = report.clone();
    for item in response.repairs {
        let original = pending
            .get(&item.slot)
            .ok_or_else(|| invalid("The repair invented a proposal slot."))?;
        if !seen.insert(item.slot) {
            return Err(invalid("The repair repeated a proposal slot."));
        }
        if let Some(value) = item.unit {
            result.rejected.retain(|item| item.slot != original.slot);
            append_unit(&mut result, value, original.slot, catalog, sources);
        }
        // Null means the model could not repair it. Keep the unresolved draft
        // and partial coverage; never turn omission into a successful empty set.
    }
    if !response.additional_units.is_empty() && report.missing_sources.is_empty() {
        return Err(invalid(
            "A targeted repair cannot regenerate successful siblings.",
        ));
    }
    let next_slot = report
        .rejected
        .iter()
        .map(|item| item.slot)
        .max()
        .unwrap_or(report.accepted.len())
        .saturating_add(1);
    for (index, value) in response.additional_units.into_iter().enumerate() {
        let selected_uncovered_source = admit(value.clone(), catalog, sources).is_ok_and(|unit| {
            unit.content.grounding.is_some_and(|grounding| {
                grounding
                    .evidence
                    .iter()
                    .any(|evidence| report.missing_sources.contains(&evidence.source_id))
            })
        });
        if selected_uncovered_source {
            append_unit(&mut result, value, next_slot + index, catalog, sources);
        }
        // A purported additional unit not addressing any uncovered source is
        // ignored; the original uncovered source remains pending for retry.
    }
    add_dispositions(&mut result, response.no_learning_source_ids, sources);
    result.refresh_coverage(sources);
    Ok(result)
}

pub(crate) fn report_diagnostic(report: &AnalysisReport) -> Value {
    json!({"acceptedUnits":report.accepted.len(), "rejectedUnits": report.rejected.len(),
        "uncoveredSources":report.missing_sources, "complete":report.is_complete(),
        "rejections":report.rejected.iter().map(|item| json!({"slot":item.slot,"category":item.code,
            "draftFingerprint":hash(&item.proposal.to_string())})).collect::<Vec<_>>()})
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge::authoring::tests::fixture;

    fn unit(ids: Value) -> Value {
        json!({"title":"Sides","kind":"fact","statement":"A triangle has three sides.",
            "qualifications":[],"topic":"Geometry","importance":"core","evidenceIds":ids})
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
        let repaired = repair(&report,json!({"repairs":[{"slot":1,"unit":null}],"additionalUnits":[],"noLearningSourceIds":[]}),&catalog,&sources).unwrap();
        assert_eq!(serde_json::to_value(&repaired.accepted[0]).unwrap(), old);
        assert!(!repaired.is_complete());
    }

    #[test]
    fn cross_page_claim_keeps_two_separate_native_evidence_spans() {
        let (_, first, _, _) = fixture();
        let mut second = first.clone();
        second.segment_key = "1:0:0".to_string();
        second.text = "Its interior angles sum to 180 degrees.".to_string();
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
        let evidence = &report.accepted[0]
            .content
            .grounding
            .as_ref()
            .unwrap()
            .evidence;
        assert_eq!(evidence.len(), 2);
        assert_ne!(evidence[0].source_id, evidence[1].source_id);
        assert!(report.accepted[0]
            .content
            .grounding
            .as_ref()
            .unwrap()
            .is_visible_in(&sources));
        assert!(!report.accepted[0]
            .content
            .grounding
            .as_ref()
            .unwrap()
            .is_visible_in(&sources[..1]));
    }

    #[test]
    fn empty_output_is_not_complete_without_explicit_non_learning_dispositions() {
        let (_, source, _, _) = fixture();
        let sources = [source];
        let catalog = EvidenceCatalog::build(&sources, false).unwrap();
        let partial = initial(
            json!({"units":[],"noLearningSourceIds":[]}),
            &catalog,
            &sources,
        )
        .unwrap();
        assert!(!partial.is_complete());
        let complete = initial(
            json!({"units":[],"noLearningSourceIds":[sources[0].id]}),
            &catalog,
            &sources,
        )
        .unwrap();
        assert!(complete.is_complete());
    }
}
