//! Lossless request planning for knowledge-first artifact authoring.
//!
//! Context feasibility is established before provider dispatch. When a complete
//! request does not fit, native code bisects the ordered retained-unit catalog
//! until every child request fits. No knowledge unit, qualification, grounding
//! reference, or question policy is discarded to manufacture a successful call.
use std::collections::BTreeSet;
use std::ops::Range;

use super::authoring::KnowledgeCatalog;
use crate::error::CommandError;

pub(crate) fn partition_catalog(
    catalog: &KnowledgeCatalog,
    mut preflight: impl FnMut(&KnowledgeCatalog) -> Result<(), CommandError>,
) -> Result<Vec<KnowledgeCatalog>, CommandError> {
    if catalog.units.is_empty() {
        preflight(catalog)?;
        return Ok(vec![catalog.clone()]);
    }

    let mut pending = Vec::<Range<usize>>::with_capacity(catalog.units.len());
    pending.push(0..catalog.units.len());
    let mut accepted = Vec::<(usize, KnowledgeCatalog)>::new();
    let mut visited = 0_usize;
    let maximum_visits = catalog.units.len().saturating_mul(2).saturating_sub(1);

    while let Some(range) = pending.pop() {
        visited = visited.saturating_add(1);
        if visited > maximum_visits {
            return Err(CommandError::new(
                "ai_document_context_budget",
                "Authoring request planning exceeded its finite partition bound before any provider request was sent.",
            ));
        }
        let candidate = slice_catalog(catalog, range.clone());
        match preflight(&candidate) {
            Ok(()) => accepted.push((range.start, candidate)),
            Err(error) if error.code == "ai_document_context_budget" && range.len() > 1 => {
                let middle = range.start + range.len() / 2;
                // Stack is LIFO: push right first so the final accepted sequence
                // retains the original native unit order after sorting.
                pending.push(middle..range.end);
                pending.push(range.start..middle);
            }
            Err(error) if error.code == "ai_document_context_budget" => {
                return Err(CommandError::new(
                    "ai_document_context_budget",
                    "One retained knowledge unit and its required evidence cannot fit this model's context budget. No provider request was sent and no learning obligation was dropped.",
                ));
            }
            Err(error) => return Err(error),
        }
    }

    accepted.sort_by_key(|(start, _)| *start);
    let planned = accepted
        .into_iter()
        .map(|(_, catalog)| catalog)
        .collect::<Vec<_>>();
    let conserved = planned
        .iter()
        .map(|catalog| catalog.units.len())
        .sum::<usize>();
    if conserved != catalog.units.len() {
        return Err(CommandError::new(
            "ai_document_context_budget",
            "Authoring request planning did not conserve the complete retained knowledge sequence.",
        ));
    }
    Ok(planned)
}

pub(crate) fn allocate_nonzero_budgets(
    maximum: usize,
    catalogs: &[KnowledgeCatalog],
) -> Result<Vec<usize>, CommandError> {
    if catalogs.is_empty() {
        return Ok(Vec::new());
    }
    if maximum < catalogs.len() {
        return Err(CommandError::new(
            "ai_document_context_budget",
            "This authoring task requires more independently feasible requests than its learning-output budget can represent. No provider request was sent and no catalog partition was dropped.",
        ));
    }

    let weights = catalogs
        .iter()
        .map(|catalog| catalog.units.len().max(1))
        .collect::<Vec<_>>();
    let total_weight = weights.iter().sum::<usize>().max(1);
    let remaining = maximum - catalogs.len();
    let mut budgets = vec![1_usize; catalogs.len()];
    let mut cumulative_weight = 0_usize;
    let mut assigned_extra = 0_usize;
    for (index, weight) in weights.into_iter().enumerate() {
        cumulative_weight = cumulative_weight.saturating_add(weight);
        let target_extra = remaining.saturating_mul(cumulative_weight) / total_weight;
        budgets[index] = budgets[index].saturating_add(target_extra - assigned_extra);
        assigned_extra = target_extra;
    }
    debug_assert_eq!(budgets.iter().sum::<usize>(), maximum);
    Ok(budgets)
}

/// Two order-preserving halves for a smaller retry after a request failed on
/// its output. Each half is still admitted by `partition_catalog` before use.
pub(crate) fn halve_catalog(catalog: &KnowledgeCatalog) -> Option<[KnowledgeCatalog; 2]> {
    let length = catalog.units.len();
    (length >= 2).then(|| {
        let middle = length / 2;
        [
            slice_catalog(catalog, 0..middle),
            slice_catalog(catalog, middle..length),
        ]
    })
}

fn slice_catalog(catalog: &KnowledgeCatalog, range: Range<usize>) -> KnowledgeCatalog {
    let units = catalog.units[range].to_vec();
    let ids = units
        .iter()
        .map(|unit| unit.id.as_str())
        .collect::<BTreeSet<_>>();
    let document_core_units = catalog
        .document_core_units
        .iter()
        .filter(|id| ids.contains(id.as_str()))
        .cloned()
        .collect();
    KnowledgeCatalog {
        units,
        document_core_units,
        preparation: catalog.preparation.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn document_authoring_planner_conserves_order_and_terminates() {
        let mut catalog = crate::knowledge::authoring::tests::catalog_fixture();
        let template = catalog.units[0].clone();
        catalog.units = (0..7)
            .map(|index| {
                let mut unit = template.clone();
                unit.id = format!("{:032x}", index + 1);
                unit
            })
            .collect();
        catalog.document_core_units = catalog.units.iter().map(|unit| unit.id.clone()).collect();

        let mut visits = 0_usize;
        let planned = partition_catalog(&catalog, |candidate| {
            visits += 1;
            if candidate.units.len() > 2 {
                Err(CommandError::new("ai_document_context_budget", "split"))
            } else {
                Ok(())
            }
        })
        .unwrap();

        assert!(visits < catalog.units.len() * 2);
        assert_eq!(
            planned
                .iter()
                .flat_map(|part| part.units.iter().map(|unit| unit.id.clone()))
                .collect::<Vec<_>>(),
            catalog
                .units
                .iter()
                .map(|unit| unit.id.clone())
                .collect::<Vec<_>>()
        );
        assert!(planned.iter().all(|part| part.units.len() <= 2));
        assert!(planned.iter().all(|part| {
            part.document_core_units
                .iter()
                .all(|id| part.units.iter().any(|unit| &unit.id == id))
        }));
    }

    #[test]
    fn document_authoring_planner_does_not_split_non_context_failures() {
        let catalog = crate::knowledge::authoring::tests::catalog_fixture();
        let error = partition_catalog(&catalog, |_| {
            Err(CommandError::new("ai_profile_changed", "changed"))
        })
        .unwrap_err();
        assert_eq!(error.code, "ai_profile_changed");
    }

    #[test]
    fn document_authoring_planner_reports_indivisible_context_failure() {
        let catalog = crate::knowledge::authoring::tests::catalog_fixture();
        let error = partition_catalog(&catalog, |_| {
            Err(CommandError::new("ai_document_context_budget", "too large"))
        })
        .unwrap_err();
        assert_eq!(error.code, "ai_document_context_budget");
        assert!(error.message.contains("One retained knowledge unit"));
    }

    #[test]
    fn document_authoring_budget_conserves_total_capacity() {
        let mut catalog = crate::knowledge::authoring::tests::catalog_fixture();
        let template = catalog.units[0].clone();
        let catalogs = [3_usize, 1, 2]
            .into_iter()
            .map(|count| {
                let mut part = catalog.clone();
                part.units = (0..count).map(|_| template.clone()).collect();
                part
            })
            .collect::<Vec<_>>();
        let budgets = allocate_nonzero_budgets(9, &catalogs).unwrap();
        assert_eq!(budgets.iter().sum::<usize>(), 9);
        assert!(budgets.iter().all(|budget| *budget >= 1));
        assert!(budgets[0] >= budgets[1]);
        catalog.units.clear();
        assert_eq!(
            allocate_nonzero_budgets(0, &[]).unwrap(),
            Vec::<usize>::new()
        );
    }

    #[test]
    fn document_authoring_budget_refuses_to_hide_partitions() {
        let catalog = crate::knowledge::authoring::tests::catalog_fixture();
        let error = allocate_nonzero_budgets(1, &[catalog.clone(), catalog]).unwrap_err();
        assert_eq!(error.code, "ai_document_context_budget");
        assert!(error.message.contains("No provider request"));
    }
}
