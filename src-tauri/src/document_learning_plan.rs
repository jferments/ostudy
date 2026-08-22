//! Application adapter from the validated document outline to a shared learning plan.
//!
//! The outline remains a compact reasoning artifact. This adapter deterministically
//! merges duplicate source spans, derives the conservative hierarchy supported by
//! source containment, and assigns canonical source locators. It does not call a
//! provider and does not use generated titles as durable topic identity.

use std::collections::{BTreeMap, HashSet};

use crate::{
    ai::document_intelligence::DocumentOutline,
    document_domain::{SourceLocator, SourceUnitKind},
    document_learning::{DocumentLearningPlanInput, LearningTopicInput},
    error::CommandError,
};

const MAX_SOURCE_UNITS_PER_TOPIC: usize = 100_000;

#[derive(Clone, Debug)]
struct OutlineTopicSeed {
    source_start: i64,
    source_end: i64,
    title: String,
    summary: String,
    learning_objectives: Vec<String>,
    key_concepts: Vec<String>,
}

/// Convert the current provider-neutral outline into the plan input consumed by
/// `document_learning`. Exact source/evidence hashing and stable topic IDs remain
/// owned by the learning-plan compiler.
pub(crate) fn learning_plan_input_from_outline(
    outline: &DocumentOutline,
    source_unit_kind: SourceUnitKind,
    source_unit_count: usize,
) -> Result<DocumentLearningPlanInput, CommandError> {
    if source_unit_count == 0 {
        return Err(CommandError::new(
            "document_learning_plan_source_empty",
            "A document learning plan cannot be built without canonical source units.",
        ));
    }
    let maximum = i64::try_from(source_unit_count).unwrap_or(i64::MAX);
    let seeds = outline
        .sections
        .iter()
        .enumerate()
        .map(|(index, section)| OutlineTopicSeed {
            source_start: section.page_start.clamp(1, maximum),
            source_end: section
                .page_end
                .clamp(section.page_start.clamp(1, maximum), maximum),
            title: nonempty(
                &section.title,
                &format!("Source topic {}", index.saturating_add(1)),
            ),
            summary: nonempty(&section.summary, &outline.summary),
            learning_objectives: section.learning_objectives.clone(),
            key_concepts: section.key_concepts.clone(),
        })
        .collect::<Vec<_>>();
    let seeds = merge_duplicate_spans(seeds);
    let seeds = if seeds.is_empty() {
        vec![OutlineTopicSeed {
            source_start: 1,
            source_end: maximum,
            title: "Document overview".to_string(),
            summary: nonempty(&outline.summary, "Structured source material."),
            learning_objectives: outline.learning_objectives.clone(),
            key_concepts: outline.key_concepts.clone(),
        }]
    } else {
        seeds
    };
    let parents = derive_containment_parents(&seeds);

    let mut topics = Vec::with_capacity(seeds.len());
    for (index, seed) in seeds.into_iter().enumerate() {
        let count = seed
            .source_end
            .saturating_sub(seed.source_start)
            .saturating_add(1);
        let count = usize::try_from(count).unwrap_or(usize::MAX);
        if count == 0 || count > MAX_SOURCE_UNITS_PER_TOPIC {
            return Err(CommandError::new(
                "document_learning_topic_too_large",
                "A document outline topic exceeded the bounded canonical source range.",
            ));
        }
        let source_numbers = (seed.source_start..=seed.source_end).collect::<Vec<_>>();
        let source_locator = SourceLocator::new(source_unit_kind.clone(), source_numbers, None)?;
        topics.push(LearningTopicInput {
            parent_index: parents[index],
            title: seed.title,
            summary: seed.summary,
            learning_objectives: seed.learning_objectives,
            key_concepts: seed.key_concepts,
            source_locator,
        });
    }

    Ok(DocumentLearningPlanInput {
        summary: nonempty(&outline.summary, "Structured source material."),
        learning_objectives: outline.learning_objectives.clone(),
        key_concepts: outline.key_concepts.clone(),
        topics,
    })
}

fn merge_duplicate_spans(seeds: Vec<OutlineTopicSeed>) -> Vec<OutlineTopicSeed> {
    let mut merged = BTreeMap::<(i64, i64), OutlineTopicSeed>::new();
    for seed in seeds {
        let key = (seed.source_start, seed.source_end);
        match merged.get_mut(&key) {
            None => {
                merged.insert(key, seed);
            }
            Some(existing) => {
                if existing.summary.trim() != seed.summary.trim() && !seed.summary.trim().is_empty() {
                    existing.summary = format!("{}\n{}", existing.summary.trim(), seed.summary.trim());
                }
                append_unique(&mut existing.learning_objectives, seed.learning_objectives);
                append_unique(&mut existing.key_concepts, seed.key_concepts);
            }
        }
    }
    merged.into_values().collect()
}

/// Find the narrowest earlier source range that strictly contains each topic.
/// This preserves real containment without fabricating hierarchy from headings.
fn derive_containment_parents(seeds: &[OutlineTopicSeed]) -> Vec<Option<usize>> {
    seeds
        .iter()
        .enumerate()
        .map(|(index, topic)| {
            seeds
                .iter()
                .take(index)
                .enumerate()
                .filter(|(_, candidate)| {
                    candidate.source_start <= topic.source_start
                        && candidate.source_end >= topic.source_end
                        && (candidate.source_start < topic.source_start
                            || candidate.source_end > topic.source_end)
                })
                .min_by_key(|(_, candidate)| {
                    candidate
                        .source_end
                        .saturating_sub(candidate.source_start)
                })
                .map(|(parent_index, _)| parent_index)
        })
        .collect()
}

fn append_unique(target: &mut Vec<String>, values: Vec<String>) {
    let mut seen = target
        .iter()
        .map(|value| value.trim().to_ascii_lowercase())
        .collect::<HashSet<_>>();
    for value in values {
        let identity = value.trim().to_ascii_lowercase();
        if !identity.is_empty() && seen.insert(identity) {
            target.push(value);
        }
    }
}

fn nonempty(value: &str, fallback: &str) -> String {
    if value.trim().is_empty() {
        fallback.trim().to_string()
    } else {
        value.trim().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seed(start: i64, end: i64, title: &str) -> OutlineTopicSeed {
        OutlineTopicSeed {
            source_start: start,
            source_end: end,
            title: title.to_string(),
            summary: title.to_string(),
            learning_objectives: vec![format!("Understand {title}")],
            key_concepts: vec![title.to_string()],
        }
    }

    #[test]
    fn duplicate_source_spans_merge_without_title_based_identity() {
        let merged = merge_duplicate_spans(vec![seed(2, 4, "First"), seed(2, 4, "Second")]);
        assert_eq!(merged.len(), 1);
        assert!(merged[0].summary.contains("First"));
        assert!(merged[0].summary.contains("Second"));
        assert_eq!(merged[0].learning_objectives.len(), 2);
    }

    #[test]
    fn containment_hierarchy_chooses_the_narrowest_parent() {
        let seeds = vec![
            seed(1, 10, "Chapter"),
            seed(2, 8, "Section"),
            seed(3, 4, "Mechanism"),
            seed(11, 12, "Next section"),
        ];
        assert_eq!(derive_containment_parents(&seeds), vec![None, Some(0), Some(1), None]);
    }

    #[test]
    fn adjacent_topics_do_not_become_parent_and_child() {
        let seeds = vec![seed(1, 2, "One"), seed(3, 4, "Two")];
        assert_eq!(derive_containment_parents(&seeds), vec![None, None]);
    }
}
