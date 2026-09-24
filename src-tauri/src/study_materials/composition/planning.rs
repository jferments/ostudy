//! Lossless request feasibility before cache keys, dispatch, or progress totals.
//! A failed fit partitions work; it never certifies the discarded tail as taught.
use crate::{
    ai::document_intelligence::{
        preflight_collection_study_guide_section, CollectionStudyGuideAuthoring,
        DocumentTaskContext,
    },
    error::CommandError,
    knowledge::{authoring::KnowledgeCatalog, collection_view::CollectionGuideWindow},
    task_cancellation::CancellationSignal,
};

use super::{ensure_bundle_running, hash_text, relationship_context, CollectionViewState};

pub(super) fn plan(
    task: &DocumentTaskContext,
    title: &str,
    view: &CollectionViewState,
    windows: Vec<CollectionGuideWindow>,
    signal: &CancellationSignal,
) -> Result<Vec<CollectionGuideWindow>, CommandError> {
    let mut planned = Vec::new();
    for window in windows {
        let relationships = relationship_context(view, &window.topic_ids);
        planned.extend(split_with(window, |part| {
            ensure_bundle_running(signal)?;
            preflight_collection_study_guide_section(
                task,
                title,
                CollectionStudyGuideAuthoring {
                    topic_title: &part.title,
                    topic_summary: &part.topic_context,
                    relationship_context: &relationships,
                    knowledge: &part.catalog,
                },
            )
        })?);
    }
    Ok(planned)
}

/// The validator is shared with the real authoring request. Bisection visits at
/// most 2N-1 nodes for N units. An indivisible oversized unit fails explicitly;
/// unrelated validation or cancellation failures never trigger repartitioning.
fn split_with(
    window: CollectionGuideWindow,
    mut admit: impl FnMut(&CollectionGuideWindow) -> Result<(), CommandError>,
) -> Result<Vec<CollectionGuideWindow>, CommandError> {
    if window.catalog.units.is_empty() {
        return Err(CommandError::new(
            "collection_study_guide_topic_has_no_knowledge",
            "A guide task cannot be planned without admitted knowledge.",
        ));
    }
    let root = window.block_key.clone();
    let mut pending = vec![window];
    let mut planned = Vec::new();
    while let Some(mut part) = pending.pop() {
        match admit(&part) {
            Ok(()) => planned.push(part),
            Err(error)
                if error.code == "ai_document_context_budget" && part.catalog.units.len() > 1 =>
            {
                let middle = part.catalog.units.len() / 2;
                let right_units = part.catalog.units.split_off(middle);
                let mut right = CollectionGuideWindow {
                    block_key: String::new(),
                    title: part.title.clone(),
                    topic_ids: part.topic_ids.clone(),
                    topic_context: part.topic_context.clone(),
                    catalog: KnowledgeCatalog {
                        units: right_units,
                        document_core_units: part.catalog.document_core_units.clone(),
                        preparation: part.catalog.preparation.clone(),
                    },
                };
                // A native unit is indivisible: its statement, qualifications,
                // evidence and deferred annotations remain byte-for-byte intact.
                // The existing prompt projection selects complete dependencies.
                part.block_key = split_key(&root, &part.catalog)?;
                right.block_key = split_key(&root, &right.catalog)?;
                pending.push(right);
                pending.push(part);
            }
            Err(error) => return Err(error),
        }
    }
    Ok(planned)
}

fn split_key(root: &str, catalog: &KnowledgeCatalog) -> Result<String, CommandError> {
    Ok(hash_text(&format!(
        "collection-guide-part-v1|{root}|{}",
        catalog.request_fingerprint()?
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    fn fixture(count: usize) -> CollectionGuideWindow {
        let mut catalog = crate::knowledge::authoring::tests::catalog_fixture();
        let original = catalog.units[0].clone();
        catalog.units = (0..count)
            .map(|index| {
                let mut unit = original.clone();
                unit.id = format!("{:032x}", index + 1);
                unit.content.title = format!("Concept {index}");
                unit.content.qualifications = vec![format!("Only under condition {index}.")];
                unit
            })
            .collect();
        CollectionGuideWindow {
            block_key: "original-window".into(),
            title: "Connected concepts".into(),
            topic_ids: vec!["topic-a".into(), "topic-b".into()],
            topic_context: "Preserve both topic roles and their qualifications.".into(),
            catalog,
        }
    }

    fn budget() -> CommandError {
        CommandError::new(
            "ai_document_context_budget",
            "The complete request does not fit.",
        )
    }

    fn units(windows: &[CollectionGuideWindow]) -> Value {
        json!(windows
            .iter()
            .flat_map(|part| &part.catalog.units)
            .collect::<Vec<_>>())
    }

    #[test]
    fn document_guide_two_and_three_unit_planning_terminates_without_losing_the_tail() {
        for count in [2, 3, 5, 48] {
            let window = fixture(count);
            let original = json!(window.catalog.units);
            let mut calls = 0;
            let parts = split_with(window, |part| {
                calls += 1;
                if part.catalog.units.len() > 1 {
                    Err(budget())
                } else {
                    Ok(())
                }
            })
            .unwrap();
            assert_eq!(parts.len(), count);
            assert_eq!(calls, count * 2 - 1);
            assert_eq!(units(&parts), original);
        }
    }

    #[test]
    fn document_guide_fitting_window_preserves_existing_checkpoint_identity() {
        let window = fixture(3);
        let original = json!(window.catalog.units);
        let parts = split_with(window, |_| Ok(())).unwrap();
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].block_key, "original-window");
        assert_eq!(units(&parts), original);
    }

    #[test]
    fn document_guide_split_preserves_structures_counters_and_topic_context() {
        let mut window = fixture(3);
        window.catalog.preparation.unresolved_regions = 7;
        window.catalog.preparation.evidence_structures = vec![json!({
            "sourceId":window.catalog.units[0].source_id,
            "payload":{"table":{"columns":["Condition","Value"],"rows":[["Only at rest","−2.5"]]}},
            "links":[{"kind":"footnote","targetNode":"qualifier"}]
        })];
        let original = json!(window.catalog.preparation);
        let parts = split_with(window, |part| {
            if part.catalog.units.len() > 1 {
                Err(budget())
            } else {
                Ok(())
            }
        })
        .unwrap();
        for part in &parts {
            assert_eq!(json!(part.catalog.preparation), original);
            assert_eq!(part.topic_ids, vec!["topic-a", "topic-b"]);
            assert_eq!(
                part.topic_context,
                "Preserve both topic roles and their qualifications."
            );
            assert_eq!(part.title, "Connected concepts");
        }
        let keys = parts
            .iter()
            .map(|part| &part.block_key)
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(keys.len(), parts.len());
    }

    #[test]
    fn document_guide_split_keys_are_deterministic_and_evidence_sensitive() {
        let fit = |part: &CollectionGuideWindow| {
            if part.catalog.units.len() > 1 {
                Err(budget())
            } else {
                Ok(())
            }
        };
        let first = split_with(fixture(3), fit).unwrap();
        let second = split_with(fixture(3), fit).unwrap();
        assert_eq!(
            first.iter().map(|p| &p.block_key).collect::<Vec<_>>(),
            second.iter().map(|p| &p.block_key).collect::<Vec<_>>()
        );
        let mut changed = fixture(3);
        changed.catalog.units[2]
            .content
            .qualifications
            .push("Another necessary condition.".into());
        let third = split_with(changed, fit).unwrap();
        assert_eq!(first[0].block_key, third[0].block_key);
        assert_eq!(first[1].block_key, third[1].block_key);
        assert_ne!(first[2].block_key, third[2].block_key);
    }

    #[test]
    fn document_guide_indivisible_oversize_never_returns_a_partial_plan() {
        let mut calls = 0;
        let result = split_with(fixture(3), |_| {
            calls += 1;
            Err(budget())
        });
        assert_eq!(result.err().unwrap().code, "ai_document_context_budget");
        assert_eq!(calls, 2);
    }

    #[test]
    fn document_guide_other_errors_and_cancellation_are_not_size_failures() {
        for code in [
            "ai_request_canceled",
            "invalid_semantic_content",
            "ai_profile_changed",
        ] {
            let mut calls = 0;
            let result = split_with(fixture(48), |_| {
                calls += 1;
                Err(CommandError::new(code, "Original failure"))
            });
            assert_eq!(result.err().unwrap().code, code);
            assert_eq!(calls, 1);
        }
    }

    #[test]
    fn document_guide_empty_plan_never_reaches_request_validation() {
        let result = split_with(fixture(0), |_| panic!("no empty provider task"));
        assert_eq!(
            result.err().unwrap().code,
            "collection_study_guide_topic_has_no_knowledge"
        );
    }
}
