//! Revision-bound accounting of the native evidence regions in an analysis.
//! A disposition records processing, not truth or exhaustive conceptual recall.
//! Provider declarations cannot filter evidence or merge duplicate occurrences.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::{
    contracts::{hash, invalid, SourceRecord},
    evidence::{EvidenceCatalog, EvidenceReference, EvidenceSelector},
    extraction::decode::AcceptedUnit,
};
use crate::error::CommandError;

const VERSION: u32 = 1;
const MAX_REGIONS: usize = 512;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Disposition {
    Interpreted,
    RetainedMetadata,
    NoLearning,
    Unreadable,
    Unresolved,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Region {
    id: String,
    source_id: String,
    exposed: bool,
    declared: Option<Disposition>,
    state: Disposition,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CoverageReceipt {
    version: u32,
    plan_hash: String,
    images_attached: bool,
    regions: Vec<Region>,
}

impl CoverageReceipt {
    pub(crate) fn new(
        sources: &[SourceRecord],
        catalog: &EvidenceCatalog,
    ) -> Result<Self, CommandError> {
        // An omitted attachment is still planned, not interpreted or unreadable.
        let planned = EvidenceCatalog::build(sources, true)?;
        if planned.entries.len() > MAX_REGIONS {
            return Err(invalid(
                "The evidence coverage plan exceeds its region limit.",
            ));
        }
        let images_attached = catalog
            .entries
            .iter()
            .any(|entry| matches!(entry.selector, EvidenceSelector::Image { .. }));
        let exposed = catalog
            .entries
            .iter()
            .map(|entry| &entry.id)
            .collect::<HashSet<_>>();
        let regions = planned
            .entries
            .iter()
            .map(|entry| Region {
                id: entry.id.clone(),
                source_id: entry.source_id.clone(),
                exposed: exposed.contains(&entry.id),
                declared: None,
                state: Disposition::Unresolved,
            })
            .collect::<Vec<_>>();
        let revisions = sources
            .iter()
            .map(|source| (&source.id, &source.content_hash, &source.owner_revision))
            .collect::<Vec<_>>();
        let ids = planned
            .entries
            .iter()
            .map(|entry| &entry.id)
            .collect::<Vec<_>>();
        let plan_hash = hash(
            &serde_json::to_string(&(VERSION, revisions, ids))
                .map_err(CommandError::serialization)?,
        );
        Ok(Self {
            version: VERSION,
            plan_hash,
            images_attached,
            regions,
        })
    }

    /// Only explicit declarations over supplied aliases are usable. Conflicts
    /// stay unresolved; already admitted interpretations cannot be overwritten.
    pub(crate) fn apply(&mut self, values: &[Value], catalog: &EvidenceCatalog) {
        if values.len() > MAX_REGIONS {
            return;
        }
        let mut declarations = HashMap::<String, Vec<Disposition>>::new();
        for value in values {
            let Some(object) = value.as_object() else {
                continue;
            };
            if object.len() != 2 {
                continue;
            }
            let (Some(alias), Some(kind)) = (
                object.get("evidenceId").and_then(Value::as_str),
                object.get("disposition").and_then(Value::as_str),
            ) else {
                continue;
            };
            let disposition = match kind {
                "retained_metadata" => Disposition::RetainedMetadata,
                "no_learning" => Disposition::NoLearning,
                "unreadable" => Disposition::Unreadable,
                _ => continue,
            };
            let Ok(resolved) = catalog.resolve(&[alias.to_string()]) else {
                continue;
            };
            let Some(reference) = resolved.first() else {
                continue;
            };
            declarations
                .entry(reference.id.clone())
                .or_default()
                .push(disposition);
        }
        for region in &mut self.regions {
            if region.state != Disposition::Unresolved {
                continue;
            }
            if let Some(values) = declarations.get(&region.id) {
                region.declared = if region.exposed && values.len() == 1 {
                    values.first().copied()
                } else {
                    None
                };
            }
        }
    }

    pub(crate) fn refresh(&mut self, accepted: &[AcceptedUnit], no_learning: &[String]) {
        let used = accepted
            .iter()
            .flat_map(|unit| {
                unit.content.grounding.iter().flat_map(|grounding| {
                    grounding
                        .evidence
                        .iter()
                        .map(|reference| reference.id.as_str())
                })
            })
            .collect::<HashSet<_>>();
        for region in &mut self.regions {
            region.state = if used.contains(region.id.as_str()) {
                Disposition::Interpreted
            } else if !region.exposed {
                Disposition::Unresolved
            } else if no_learning.contains(&region.source_id) {
                Disposition::NoLearning
            } else {
                region.declared.unwrap_or(Disposition::Unresolved)
            };
        }
    }

    pub(crate) fn validate(
        &self,
        sources: &[SourceRecord],
        accepted: &[AcceptedUnit],
        no_learning: &[String],
    ) -> Result<(), CommandError> {
        let catalog = EvidenceCatalog::build(sources, self.images_attached)?;
        let mut expected = Self::new(sources, &catalog)?;
        if self.version != VERSION
            || self.plan_hash != expected.plan_hash
            || self.regions.len() != expected.regions.len()
        {
            return Err(invalid(
                "The coverage receipt does not match its evidence plan.",
            ));
        }
        for (region, original) in expected.regions.iter_mut().zip(&self.regions) {
            if region.id != original.id
                || region.source_id != original.source_id
                || region.exposed != original.exposed
                || original.declared.is_some_and(|value| {
                    !region.exposed
                        || !matches!(
                            value,
                            Disposition::RetainedMetadata
                                | Disposition::NoLearning
                                | Disposition::Unreadable
                        )
                })
            {
                return Err(invalid(
                    "The coverage receipt contains an unsupported disposition.",
                ));
            }
            region.declared = original.declared;
        }
        expected.refresh(accepted, no_learning);
        if expected != *self {
            return Err(invalid(
                "The coverage receipt misrepresents admitted interpretations.",
            ));
        }
        Ok(())
    }

    pub(crate) fn missing_sources(&self, sources: &[SourceRecord]) -> Vec<String> {
        let missing = self
            .regions
            .iter()
            .filter(|region| region.state == Disposition::Unresolved)
            .map(|region| region.source_id.as_str())
            .collect::<HashSet<_>>();
        sources
            .iter()
            .filter(|source| missing.contains(source.id.as_str()))
            .map(|source| source.id.clone())
            .collect()
    }

    pub(crate) fn unresolved_aliases(&self, catalog: &EvidenceCatalog) -> Vec<String> {
        let missing = self
            .regions
            .iter()
            .filter(|region| region.state == Disposition::Unresolved)
            .map(|region| region.id.as_str())
            .collect::<HashSet<_>>();
        catalog
            .entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| missing.contains(entry.id.as_str()))
            .map(|(index, _)| EvidenceCatalog::alias(index))
            .collect()
    }

    pub(crate) fn covers_unresolved(&self, evidence: &[EvidenceReference]) -> bool {
        evidence.iter().any(|reference| {
            self.regions
                .iter()
                .any(|region| region.id == reference.id && region.state == Disposition::Unresolved)
        })
    }

    pub(crate) fn unresolved(&self) -> usize {
        self.regions
            .iter()
            .filter(|region| region.state == Disposition::Unresolved)
            .count()
    }

    pub(crate) fn diagnostic(&self) -> Value {
        let count = |state| {
            self.regions
                .iter()
                .filter(|region| region.state == state)
                .count()
        };
        json!({"version":self.version,"planHash":self.plan_hash,
            "regions":self.regions.len(),"interpreted":count(Disposition::Interpreted),
            "retainedMetadata":count(Disposition::RetainedMetadata),
            "noLearning":count(Disposition::NoLearning),"unreadable":count(Disposition::Unreadable),
            "unresolved":count(Disposition::Unresolved),
            "protocolComplete":self.unresolved()==0,"exhaustiveConceptCoverage":false})
    }
}

pub(crate) fn declaration_schema(catalog: &EvidenceCatalog) -> Value {
    json!({"type":"array","maxItems":catalog.entries.len(),"items":{
        "type":"object","additionalProperties":false,"required":["evidenceId","disposition"],
        "properties":{"evidenceId":{"type":"string","enum":catalog.aliases()},
            "disposition":{"type":"string","enum":["retained_metadata","no_learning","unreadable"]}}}})
}

#[cfg(test)]
mod tests;
