//! A fact said once is easier to study than one said four times. The prompt
//! asks for that; this pass removes what a model restated anyway. It drops only
//! whole items from the short study aids (key points, callouts, terms) whose
//! words already appear in one earlier sentence of the section. It never edits
//! prose, steps, tables or checks, so no qualification is cut.
use std::collections::HashSet;

use crate::document_study_guides::StudyGuideSectionContent;

/// Share of an item sentence's content words that one earlier sentence must
/// already hold before the item counts as restated.
const RESTATED_SHARE: f64 = 0.8;
/// Shorter sentences carry too few words to judge and are kept.
const MIN_CONTENT_WORDS: usize = 4;

const STOP_WORDS: &[&str] = &[
    "the", "and", "for", "are", "with", "that", "this", "its", "from", "which", "into", "their",
    "can", "was", "were", "than", "these", "those", "also", "when", "while", "each", "per", "not",
    "but", "such", "has", "have", "had", "more", "most", "less", "other", "they", "them", "then",
    "there", "where", "what", "how", "why", "all", "any", "both", "between", "through", "during",
    "about", "only", "may", "will", "does", "did", "been", "being", "because",
];

type Words = HashSet<String>;

fn content_words(text: &str) -> Words {
    text.split(|character: char| !character.is_alphanumeric())
        .filter(|word| word.chars().count() > 2)
        .map(str::to_lowercase)
        .filter(|word| !STOP_WORDS.contains(&word.as_str()))
        .collect()
}

fn sentences(text: &str) -> Vec<Words> {
    text.split(['.', '!', '?', '\n', ';'])
        .map(content_words)
        .filter(|words| words.len() >= MIN_CONTENT_WORDS)
        .collect()
}

struct Seen(Vec<Words>);

impl Seen {
    fn add(&mut self, text: &str) {
        self.0.extend(sentences(text));
    }

    /// True when every judgeable sentence of `text` is already said, and at
    /// least one sentence was judgeable.
    fn restates(&self, text: &str) -> bool {
        let item = sentences(text);
        !item.is_empty()
            && item.iter().all(|sentence| {
                self.0.iter().any(|earlier| {
                    let shared = sentence.intersection(earlier).count();
                    shared as f64 >= RESTATED_SHARE * sentence.len() as f64
                })
            })
    }
}

/// The teaching text of a section: overview, explanations, steps and tables.
fn taught(section: &StudyGuideSectionContent) -> Seen {
    let mut seen = Seen(Vec::new());
    seen.add(&section.overview);
    for explanation in &section.explanations {
        seen.add(&explanation.body);
    }
    for step in &section.process_steps {
        seen.add(step);
    }
    for table in &section.tables {
        for row in &table.rows {
            seen.add(&row.join(" "));
        }
    }
    seen
}

/// Drop restated study-aid items in reader order and return how many went.
pub(super) fn drop_restated_items(section: &mut StudyGuideSectionContent) -> usize {
    let mut seen = taught(section);
    let mut dropped = 0usize;
    section.key_points.retain(|point| {
        let keep = !seen.restates(point);
        if keep {
            seen.add(point);
        } else {
            dropped += 1;
        }
        keep
    });
    section.callouts.retain(|callout| {
        let keep = !seen.restates(&callout.body);
        if keep {
            seen.add(&callout.body);
        } else {
            dropped += 1;
        }
        keep
    });
    section.important_terms.retain(|term| {
        let keep = !seen.restates(&term.explanation);
        if !keep {
            dropped += 1;
        }
        keep
    });
    dropped
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document_study_guides::{
        StudyGuideCallout, StudyGuideCalloutKind, StudyGuideExplanation, StudyGuideTerm,
    };

    fn section() -> StudyGuideSectionContent {
        StudyGuideSectionContent {
            overview: "This section explains how the heart's pacemaker sets the rhythm.".into(),
            explanations: vec![StudyGuideExplanation {
                heading: "The sinoatrial node".into(),
                body: "The sinoatrial node depolarizes spontaneously and sets the resting heart rate near 75 beats per minute. Vagal stimulation slows its depolarization.".into(),
            }],
            ..Default::default()
        }
    }

    #[test]
    fn a_key_point_that_repeats_a_sentence_is_dropped_and_a_new_one_kept() {
        let mut section = section();
        section.key_points = vec![
            "The sinoatrial node depolarizes spontaneously and sets the resting heart rate.".into(),
            "Sympathetic stimulation raises the rate by steepening the pacemaker potential.".into(),
        ];
        assert_eq!(drop_restated_items(&mut section), 1);
        assert_eq!(section.key_points.len(), 1);
        assert!(section.key_points[0].starts_with("Sympathetic"));
    }

    #[test]
    fn terms_and_callouts_are_dropped_only_when_every_sentence_is_restated() {
        let mut section = section();
        section.important_terms = vec![
            StudyGuideTerm {
                term: "Sinoatrial node".into(),
                explanation: "Depolarizes spontaneously and sets the resting heart rate near 75 beats per minute.".into(),
            },
            StudyGuideTerm {
                term: "Bundle of His".into(),
                explanation: "Carries the impulse from the atrioventricular node into the septum.".into(),
            },
        ];
        section.callouts = vec![StudyGuideCallout {
            kind: StudyGuideCalloutKind::Note,
            title: "Vagal tone".into(),
            body: "Vagal stimulation slows sinoatrial depolarization. Atropine blocks this effect and raises the rate.".into(),
        }];
        assert_eq!(drop_restated_items(&mut section), 1);
        assert_eq!(section.important_terms.len(), 1);
        assert_eq!(section.important_terms[0].term, "Bundle of His");
        assert_eq!(section.callouts.len(), 1);
    }

    #[test]
    fn short_items_and_repeats_within_one_list_are_judged_against_earlier_items() {
        let mut section = section();
        section.key_points = vec![
            "Heart rate: 75 bpm".into(),
            "Atropine blocks muscarinic receptors on pacemaker cells and raises the rate.".into(),
            "Atropine blocks muscarinic receptors on pacemaker cells, raising the rate.".into(),
        ];
        assert_eq!(drop_restated_items(&mut section), 1);
        assert_eq!(section.key_points.len(), 2);
    }
}
