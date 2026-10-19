//! Each knowledge unit is taught by one guide section.
//!
//! Learning topics can share source: a child topic covers part of its parent's
//! pages, and neighbouring topics meet on a shared page. Built from evidence
//! alone, their catalogs overlap and two sections teach the same facts. Here
//! each shared unit goes to the most specific topic that holds it (the one with
//! the fewest evidence chunks, then the earliest in plan order). A topic that
//! would be left with no units keeps its whole catalog instead, so ownership
//! never turns a written section into a gap.

use std::collections::{BTreeSet, HashMap};

use crate::{document_learning::DocumentLearningPlan, knowledge::authoring::KnowledgeCatalog};

pub(super) fn assign(
    plan: &DocumentLearningPlan,
    catalogs: Vec<KnowledgeCatalog>,
) -> Vec<KnowledgeCatalog> {
    let rank = |index: usize| (plan.topics[index].evidence_chunk_indices.len(), index);
    let mut owner = HashMap::<&str, usize>::new();
    for (index, catalog) in catalogs.iter().enumerate() {
        for unit in &catalog.units {
            owner
                .entry(unit.id.as_str())
                .and_modify(|current| {
                    if rank(index) < rank(*current) {
                        *current = index;
                    }
                })
                .or_insert(index);
        }
    }
    let owned = catalogs
        .iter()
        .enumerate()
        .map(|(index, catalog)| {
            catalog
                .units
                .iter()
                .filter(|unit| owner.get(unit.id.as_str()) == Some(&index))
                .map(|unit| unit.id.clone())
                .collect::<BTreeSet<_>>()
        })
        .collect::<Vec<_>>();
    let mut moved = 0usize;
    let assigned = catalogs
        .into_iter()
        .zip(owned)
        .map(|(catalog, owned)| {
            if owned.is_empty() || owned.len() == catalog.units.len() {
                return catalog;
            }
            moved += catalog.units.len() - owned.len();
            KnowledgeCatalog {
                units: catalog
                    .units
                    .into_iter()
                    .filter(|unit| owned.contains(&unit.id))
                    .collect(),
                document_core_units: catalog
                    .document_core_units
                    .intersection(&owned)
                    .cloned()
                    .collect(),
                preparation: catalog.preparation,
            }
        })
        .collect();
    if moved > 0 {
        crate::logging::debug_target(
            "ai.document.study_guide",
            "study_guide_shared_units_assigned",
            "Left shared knowledge units to the most specific topic that holds them.",
            serde_json::json!({"removedFromBroaderTopics": moved}),
        );
    }
    assigned
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(id: usize) -> crate::knowledge::contracts::KnowledgeUnit {
        let mut unit = crate::knowledge::authoring::tests::catalog_fixture().units[0].clone();
        unit.id = format!("{id:032x}");
        unit
    }

    fn catalog(ids: &[usize]) -> KnowledgeCatalog {
        KnowledgeCatalog {
            units: ids.iter().map(|id| unit(*id)).collect(),
            document_core_units: ids.iter().map(|id| format!("{id:032x}")).collect(),
            ..Default::default()
        }
    }

    fn ids(catalog: &KnowledgeCatalog) -> Vec<usize> {
        catalog
            .units
            .iter()
            .map(|unit| usize::from_str_radix(&unit.id, 16).unwrap())
            .collect()
    }

    fn plan(chunks: &[usize]) -> DocumentLearningPlan {
        let mut plan = super::super::tests::plan();
        let template = plan.topics[0].clone();
        plan.topics = chunks
            .iter()
            .map(|count| {
                let mut topic = template.clone();
                topic.evidence_chunk_indices = (0..*count).collect();
                topic
            })
            .collect();
        plan
    }

    #[test]
    fn a_child_topic_keeps_its_units_and_the_parent_keeps_the_rest() {
        // Parent: pages 5-7 (three chunks). Child: page 5 (one chunk).
        let assigned = assign(
            &plan(&[3, 1]),
            vec![catalog(&[1, 2, 3, 4]), catalog(&[1, 2])],
        );
        assert_eq!(ids(&assigned[0]), [3, 4]);
        assert_eq!(ids(&assigned[1]), [1, 2]);
        assert_eq!(assigned[0].document_core_units.len(), 2);
    }

    #[test]
    fn neighbours_split_a_shared_page_and_ties_go_to_the_earlier_topic() {
        let assigned = assign(
            &plan(&[2, 2, 3]),
            vec![catalog(&[1, 2]), catalog(&[2, 3]), catalog(&[3, 4])],
        );
        assert_eq!(ids(&assigned[0]), [1, 2]);
        assert_eq!(ids(&assigned[1]), [3]);
        assert_eq!(ids(&assigned[2]), [4]);
    }

    #[test]
    fn a_topic_that_would_lose_every_unit_keeps_its_catalog() {
        let assigned = assign(&plan(&[3, 1]), vec![catalog(&[1]), catalog(&[1])]);
        assert_eq!(ids(&assigned[0]), [1]);
        assert_eq!(ids(&assigned[1]), [1]);
    }
}
