//! Local normalization has one deliberately narrow authority: a missing or
//! invalid occurrence can select a literal phrase only when it occurs once.
//! It never rewrites text, selects a synonym, changes units/operators, removes
//! a dependency, or chooses between repeated phrases. The strict validator
//! remains the sole authority for admitting the resulting whole annotation.
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::super::{annotation_error, Shape, ValidatedPayload};
use crate::{error::CommandError, knowledge::contracts::KnowledgeKind};

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Selection {
    text: String,
    occurrence: Option<usize>,
}

impl Selection {
    fn normalize(mut self, statement: &str) -> Result<(Self, bool), CommandError> {
        if self.text.trim().is_empty() {
            return Err(annotation_error(
                "selection_invalid",
                "An empty selection cannot be repaired locally.",
            ));
        }
        // Enough matches to establish uniqueness and validate every supported
        // ordinal. Match semantics are identical to the strict wire decoder.
        let count = statement.match_indices(self.text.as_str()).take(66).count();
        let occurrence = match self.occurrence {
            Some(index) if index <= 64 && index < count => index,
            _ if count == 1 => 0,
            _ => {
                return Err(annotation_error(
                    "occurrence_out_of_range",
                    "Local repair cannot choose an absent or ambiguous selection.",
                ));
            }
        };
        let changed = self.occurrence != Some(occurrence);
        self.occurrence = Some(occurrence);
        Ok((self, changed))
    }
}

pub(super) fn draft(
    draft: &Value,
    statement: &str,
    kind: KnowledgeKind,
) -> Option<ValidatedPayload> {
    let shape: Shape<Selection> = serde_json::from_value(draft.clone()).ok()?;
    let mut changed = false;
    let normalized = shape
        .map(|selection| {
            let (selection, repaired) = selection.normalize(statement)?;
            changed |= repaired;
            Ok(selection)
        })
        .ok()?;
    if !changed {
        return None;
    }
    // Includes numeric token integrity, knowledge kind, structural bounds,
    // dependency cycles and ordering. Repair never relaxes these predicates.
    ValidatedPayload::admit(serde_json::to_value(normalized).ok()?, statement, kind).ok()
}

#[cfg(test)]
mod tests;
