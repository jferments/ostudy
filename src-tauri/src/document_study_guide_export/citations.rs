//! Where each key point of a document guide comes from.
//!
//! The writer names the knowledge units each section teaches, and every unit
//! carries the exact source passage it was extracted from. A key point cites
//! the one or two of its section's units that hold most of its words, so the
//! learner can check it against the page it came from. The match is made here,
//! in words, never by a model: a model asked to cite would invent provenance.
//! A key point that no unit holds well enough cites nothing rather than the
//! wrong page.
//!
//! A section cites pages, not passages: the passages its key points name are
//! gathered into runs of neighbouring pages, so each page is listed once, as
//! "p. 12" or "pp. 12–14", however many key points come from it.

use std::collections::{HashMap, HashSet};

use rusqlite::{params, Connection};

use crate::{
    database::Database,
    document_domain::{SourceLocator, SourceUnitKind},
    document_study_guides::DocumentStudyGuideSection,
    error::CommandError,
    knowledge::{contracts::KnowledgeUnit, evidence::EvidenceSelector},
};

/// A unit must hold this share of a key point's words to be cited for it.
const MIN_SHARE: f64 = 0.4;
/// A point drawn from two passages cites a second unit too when it holds this
/// share of the point and words the first does not.
const SECOND_MIN_SHARE: f64 = 0.25;
const SECOND_NEW_WORDS: usize = 2;
const MAX_PER_POINT: usize = 2;
/// Shorter key points carry too few words to match reliably.
const MIN_POINT_WORDS: usize = 3;
/// Words are compared by their first letters, so "secretes" meets "secretion".
const STEM_CHARS: usize = 6;
const MAX_EXCERPT_CHARS: usize = 600;
/// The most pages one source names. Neighbouring cited pages join into one
/// range up to this length, so a long section still points at its pages
/// rather than at all of them at once.
const MAX_RANGE_PAGES: i64 = 3;

/// The pages or range a section cites, numbered from 1 in the order its key
/// points first cite it.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct CitedSource {
    pub number: usize,
    pub document_id: String,
    /// The cited document is a PDF, whose pages the reader can open.
    pub pdf: bool,
    pub locator: Option<SourceLocator>,
    /// The cited passages on these pages, in page order.
    pub excerpts: Vec<String>,
}

/// A section's citations: the source numbers of each key point, in order,
/// and the sources they name.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct SectionCitations {
    pub key_points: Vec<Vec<usize>>,
    pub sources: Vec<CitedSource>,
}

impl SectionCitations {
    pub(super) fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }
}

/// A unit as citing needs it: its words and its primary passage.
#[derive(Clone, Debug)]
pub(super) struct CitableUnit {
    pub id: String,
    pub document_id: String,
    pub pdf: bool,
    pub words: HashSet<String>,
    pub locator: Option<SourceLocator>,
    pub excerpt: String,
}

/// Each section's citations, by topic id. A section whose units cannot be
/// read is laid out without citations, as before they existed.
pub(super) fn load(
    database: &Database,
    guide_id: &str,
    sections: &[DocumentStudyGuideSection],
) -> HashMap<String, SectionCitations> {
    let result = database.with_connection(|connection| {
        let mut cited = HashMap::new();
        for section in sections {
            let units = section_units(connection, guide_id, &section.topic_id)?;
            let citations = cite(&section.content.key_points, &units);
            if !citations.is_empty() {
                cited.insert(section.topic_id.clone(), citations);
            }
        }
        Ok(cited)
    });
    result.unwrap_or_else(|error| {
        crate::logging::warn_target(
            "ai.document.study_guide",
            "study_guide_citations_unavailable",
            "Laid out the guide without citations; its knowledge links could not be read.",
            serde_json::json!({"code": error.code}),
        );
        HashMap::new()
    })
}

/// The units a section was written from whose source is still current: a
/// document read again drops the citations of its old text.
fn section_units(
    connection: &Connection,
    guide_id: &str,
    topic_id: &str,
) -> Result<Vec<CitableUnit>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT DISTINCT u.record_json, s.document_id, d.source_format = 'pdf'
             FROM knowledge_sources g
             JOIN knowledge_artifact_links l ON l.source_id = g.id AND l.source_hash = g.content_hash
             JOIN knowledge_units u ON u.id = l.unit_id
             JOIN knowledge_sources s ON s.id = u.source_id
             JOIN documents d ON d.id = s.document_id
             WHERE g.kind = 'guide' AND g.guide_id = ?1 AND g.parent_key = ?2
               AND s.kind = 'document' AND s.active = 1 AND s.content_hash = u.source_hash
             ORDER BY u.id LIMIT 64",
        )
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map(params![guide_id, topic_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, bool>(2)?,
            ))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    Ok(rows
        .into_iter()
        .filter_map(|(record, document_id, pdf)| {
            let unit = serde_json::from_str::<KnowledgeUnit>(&record).ok()?;
            citable(&unit, document_id, pdf)
        })
        .collect())
}

fn citable(unit: &KnowledgeUnit, document_id: String, pdf: bool) -> Option<CitableUnit> {
    let content = &unit.content;
    // The unit's own passage, not a second source a multi-source claim names.
    let primary = content.grounding.as_ref().and_then(|grounding| {
        grounding.evidence.iter().find(|reference| {
            reference.source_id == unit.source_id
                && matches!(reference.selector, EvidenceSelector::Text { .. })
        })
    });
    let (excerpt, locator) = match primary {
        Some(reference) => (reference.excerpt.clone(), reference.locator.clone()),
        None => (content.supporting_excerpt.clone(), None),
    };
    let excerpt = bounded(&readable(&excerpt));
    if excerpt.is_empty() {
        return None;
    }
    let mut text = format!("{} {} {}", content.title, content.statement, excerpt);
    for qualification in &content.qualifications {
        text.push(' ');
        text.push_str(qualification);
    }
    Some(CitableUnit {
        id: unit.id.clone(),
        document_id,
        pdf,
        words: stems(&text),
        locator,
        excerpt,
    })
}

/// Matches each key point to the units that hold most of its words, and
/// cites the pages those units are on.
pub(super) fn cite(key_points: &[String], units: &[CitableUnit]) -> SectionCitations {
    let chosen = key_points
        .iter()
        .map(|point| {
            let words = stems(point);
            if words.len() < MIN_POINT_WORDS {
                return Vec::new();
            }
            choose(&words, units)
        })
        .collect::<Vec<_>>();
    let spans = spans(chosen.iter().flatten().copied());
    let mut citations = SectionCitations::default();
    // Spans are numbered as the key points first cite them.
    let mut numbers = HashMap::<usize, usize>::new();
    for point in &chosen {
        let mut cited = Vec::new();
        for unit in point {
            let Some(span) = spans.iter().position(|span| span.holds(&unit.id)) else {
                continue;
            };
            let next = numbers.len() + 1;
            let number = *numbers.entry(span).or_insert_with(|| {
                citations.sources.push(spans[span].source(next));
                next
            });
            if !cited.contains(&number) {
                cited.push(number);
            }
        }
        cited.sort_unstable();
        citations.key_points.push(cited);
    }
    citations
}

/// Pages of one document that a section cites together, and the cited
/// passages on them.
struct Span<'a> {
    document_id: &'a str,
    pdf: bool,
    /// The kind of unit and the first and last of them; none for a passage
    /// with no place in its document, which is a source of its own.
    pages: Option<(SourceUnitKind, i64, i64)>,
    units: Vec<&'a CitableUnit>,
}

impl Span<'_> {
    fn holds(&self, unit_id: &str) -> bool {
        self.units.iter().any(|unit| unit.id == unit_id)
    }

    fn source(&self, number: usize) -> CitedSource {
        let mut excerpts = Vec::<String>::new();
        for unit in &self.units {
            if !excerpts.contains(&unit.excerpt) {
                excerpts.push(unit.excerpt.clone());
            }
        }
        CitedSource {
            number,
            document_id: self.document_id.to_string(),
            pdf: self.pdf,
            locator: self.pages.and_then(|(kind, first, last)| {
                SourceLocator::with_evidence(kind, (first..=last).collect(), None, Vec::new(), None)
                    .ok()
            }),
            excerpts,
        }
    }
}

/// Gathers the cited units into runs of neighbouring pages, each page in one
/// run only. A unit's pages go to the run its first page is in, and a run is
/// cut at `MAX_RANGE_PAGES` unless one unit alone is longer.
fn spans<'a>(cited: impl Iterator<Item = &'a CitableUnit>) -> Vec<Span<'a>> {
    let mut placed = HashSet::<&str>::new();
    let mut paged = Vec::new();
    let mut spans = Vec::new();
    for unit in cited {
        if !placed.insert(unit.id.as_str()) {
            continue;
        }
        match unit
            .locator
            .as_ref()
            .filter(|locator| locator.time_range_ms.is_none())
        {
            Some(locator) => paged.push((unit, locator)),
            None => spans.push(Span {
                document_id: &unit.document_id,
                pdf: unit.pdf,
                pages: None,
                units: vec![unit],
            }),
        }
    }
    paged.sort_by(|(a, a_locator), (b, b_locator)| {
        (
            &a.document_id,
            a_locator.unit_kind.as_str(),
            a_locator.first(),
            a_locator.last(),
        )
            .cmp(&(
                &b.document_id,
                b_locator.unit_kind.as_str(),
                b_locator.first(),
                b_locator.last(),
            ))
    });
    let mut runs = Vec::<Span<'_>>::new();
    for (unit, locator) in paged {
        let (kind, first, last) = (locator.unit_kind, locator.first(), locator.last());
        if let Some(run) = runs.last_mut().filter(|run| {
            run.document_id == unit.document_id
                && run.pages.is_some_and(|(run_kind, _, _)| run_kind == kind)
        }) {
            let (_, run_first, run_last) = run.pages.unwrap_or((kind, first, last));
            if last <= run_last {
                // Its pages are already listed.
                run.units.push(unit);
                continue;
            }
            if first <= run_last + 1 && last - run_first < MAX_RANGE_PAGES {
                run.pages = Some((kind, run_first, last));
                run.units.push(unit);
                continue;
            }
            // A new run starts after the pages already listed.
            let first = first.max(run_last + 1);
            runs.push(Span {
                document_id: &unit.document_id,
                pdf: unit.pdf,
                pages: Some((kind, first, last)),
                units: vec![unit],
            });
            continue;
        }
        runs.push(Span {
            document_id: &unit.document_id,
            pdf: unit.pdf,
            pages: Some((kind, first, last)),
            units: vec![unit],
        });
    }
    runs.extend(spans);
    runs
}

fn choose<'a>(words: &HashSet<String>, units: &'a [CitableUnit]) -> Vec<&'a CitableUnit> {
    let share =
        |unit: &CitableUnit| words.intersection(&unit.words).count() as f64 / words.len() as f64;
    let mut ranked = units
        .iter()
        .map(|unit| (share(unit), unit))
        .filter(|(score, _)| *score >= SECOND_MIN_SHARE)
        .collect::<Vec<_>>();
    // Ties keep the units' stored order, so a layout is repeatable.
    ranked.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    let Some(&(_, best)) = ranked.first().filter(|(score, _)| *score >= MIN_SHARE) else {
        return Vec::new();
    };
    let mut chosen = vec![best];
    let mut covered = words
        .intersection(&best.words)
        .cloned()
        .collect::<HashSet<_>>();
    for &(_, unit) in &ranked[1..] {
        if chosen.len() == MAX_PER_POINT {
            break;
        }
        let new_words = words
            .intersection(&unit.words)
            .filter(|word| !covered.contains(*word))
            .count();
        if new_words >= SECOND_NEW_WORDS {
            covered.extend(words.intersection(&unit.words).cloned());
            chosen.push(unit);
        }
    }
    chosen
}

fn stems(text: &str) -> HashSet<String> {
    text.split(|character: char| !character.is_alphanumeric())
        .filter(|word| word.chars().count() > 2)
        .map(str::to_lowercase)
        .filter(|word| !STOP_WORDS.contains(&word.as_str()))
        .map(|word| word.chars().take(STEM_CHARS).collect())
        .collect()
}

/// The passage as one paragraph: the page's line breaks and the typeface's
/// ligatures are how the PDF stores its text, not how the learner reads it.
fn readable(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    for word in text.split_whitespace() {
        if !result.is_empty() {
            result.push(' ');
        }
        for character in word.chars() {
            match character {
                '\u{fb00}' => result.push_str("ff"),
                '\u{fb01}' => result.push_str("fi"),
                '\u{fb02}' => result.push_str("fl"),
                '\u{fb03}' => result.push_str("ffi"),
                '\u{fb04}' => result.push_str("ffl"),
                other => result.push(other),
            }
        }
    }
    result
}

fn bounded(text: &str) -> String {
    if text.chars().count() <= MAX_EXCERPT_CHARS {
        return text.to_string();
    }
    let cut = text.chars().take(MAX_EXCERPT_CHARS).collect::<String>();
    let cut = cut
        .rfind(char::is_whitespace)
        .map_or(cut.as_str(), |space| &cut[..space]);
    format!("{}…", cut.trim_end())
}

const STOP_WORDS: &[&str] = &[
    "the", "and", "for", "are", "with", "that", "this", "its", "from", "which", "into", "their",
    "can", "was", "were", "than", "these", "those", "also", "when", "while", "each", "per", "not",
    "but", "such", "has", "have", "had", "more", "most", "less", "other", "they", "them", "then",
    "there", "where", "what", "how", "why", "all", "any", "both", "between", "through", "during",
    "about", "only", "may", "will", "does", "did", "been", "being", "because", "one", "two",
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document_domain::SourceUnitKind;

    fn unit(id: &str, page: i64, text: &str) -> CitableUnit {
        CitableUnit {
            id: id.to_string(),
            document_id: "document-1".to_string(),
            pdf: true,
            words: stems(text),
            locator: Some(SourceLocator::new(SourceUnitKind::Page, vec![page], None).unwrap()),
            excerpt: text.to_string(),
        }
    }

    fn units() -> Vec<CitableUnit> {
        vec![
            unit(
                "u1",
                12,
                "Secretin is released by S cells of the duodenum in response to acid and stimulates pancreatic bicarbonate secretion.",
            ),
            unit(
                "u2",
                14,
                "Cholecystokinin stimulates gallbladder contraction and pancreatic enzyme secretion.",
            ),
            unit(
                "u3",
                15,
                "Gastrin from G cells of the antrum stimulates gastric acid secretion by parietal cells.",
            ),
        ]
    }

    #[test]
    fn a_key_point_cites_the_unit_that_holds_its_words() {
        let citations = cite(
            &[
                "Duodenal S cells secrete secretin when acid arrives, raising pancreatic bicarbonate.".into(),
                "Gastrin released from antral G cells drives parietal cell acid secretion.".into(),
            ],
            &units(),
        );
        assert_eq!(citations.key_points, [vec![1], vec![2]]);
        assert_eq!(citations.sources.len(), 2);
        assert_eq!(
            citations.sources[0].locator.as_ref().unwrap().unit_numbers,
            [12]
        );
        assert_eq!(
            citations.sources[1].locator.as_ref().unwrap().unit_numbers,
            [15]
        );
    }

    #[test]
    fn a_point_drawn_from_two_passages_cites_both_and_numbers_repeat() {
        let citations = cite(
            &[
                "Secretin from duodenal S cells raises pancreatic bicarbonate, while cholecystokinin drives gallbladder contraction.".into(),
                "Secretin from duodenal S cells answers acid with bicarbonate secretion.".into(),
            ],
            &units(),
        );
        assert_eq!(citations.key_points[0].len(), 2);
        assert!(citations.key_points[0].contains(&1) && citations.key_points[0].contains(&2));
        // The second point names a source already numbered.
        let secretin = citations
            .sources
            .iter()
            .find(|source| source.excerpts[0].starts_with("Secretin"))
            .unwrap()
            .number;
        assert_eq!(citations.key_points[1], [secretin]);
        assert_eq!(citations.sources.len(), 2);
    }

    #[test]
    fn neighbouring_pages_are_one_range_and_each_page_is_listed_once() {
        let units = vec![
            unit(
                "a",
                113,
                "Stage A patients are at high risk of heart failure without structural disease.",
            ),
            unit(
                "b",
                113,
                "Stage B patients have structural disease without symptoms of heart failure.",
            ),
            unit(
                "c",
                116,
                "Daily weights at the same time are the best indicator of fluid gain.",
            ),
            unit(
                "d",
                117,
                "Valsartan with sacubitril replaces an ACE inhibitor and reduces hospitalization.",
            ),
            unit(
                "e",
                118,
                "Digoxin increases the force of contraction and slows the heart rate.",
            ),
            unit(
                "f",
                119,
                "Hypokalemia potentiates digoxin toxicity so potassium is checked first.",
            ),
        ];
        let citations = cite(
            &[
                "Stage A is high risk without structural disease; stage B is structural disease."
                    .into(),
                "Stage B means structural disease without heart failure symptoms.".into(),
                "Daily weights at the same time best show fluid gain.".into(),
                "Sacubitril with valsartan replaces an ACE inhibitor, reducing hospitalization."
                    .into(),
                "Digoxin strengthens contraction and slows the heart rate.".into(),
                "Hypokalemia potentiates digoxin toxicity, so check potassium.".into(),
            ],
            &units,
        );
        let labels = citations
            .sources
            .iter()
            .map(|source| {
                let locator = source.locator.as_ref().unwrap();
                (source.number, locator.first(), locator.last())
            })
            .collect::<Vec<_>>();
        // Page 113 once; 116 to 118 as one range; 119 starts the next.
        assert_eq!(labels, [(1, 113, 113), (2, 116, 118), (3, 119, 119)]);
        assert_eq!(
            citations.key_points,
            [vec![1], vec![1], vec![2], vec![2], vec![2], vec![3]]
        );
        assert_eq!(citations.sources[0].excerpts.len(), 2);
    }

    #[test]
    fn a_passage_over_listed_pages_starts_its_range_after_them() {
        let mut long = unit(
            "long",
            12,
            "Secretin is released by S cells of the duodenum in response to acid.",
        );
        long.locator =
            Some(SourceLocator::new(SourceUnitKind::Page, vec![12, 13, 14], None).unwrap());
        let mut later = unit(
            "later",
            14,
            "Cholecystokinin stimulates gallbladder contraction and enzyme secretion.",
        );
        later.locator =
            Some(SourceLocator::new(SourceUnitKind::Page, vec![14, 15, 16], None).unwrap());
        let citations = cite(
            &[
                "Secretin is released by duodenal S cells in response to acid.".into(),
                "Cholecystokinin stimulates gallbladder contraction and enzyme secretion.".into(),
            ],
            &[long, later],
        );
        let ranges = citations
            .sources
            .iter()
            .map(|source| source.locator.as_ref().unwrap().unit_numbers.clone())
            .collect::<Vec<_>>();
        assert_eq!(ranges, [vec![12, 13, 14], vec![15, 16]]);
    }

    #[test]
    fn a_point_no_unit_holds_cites_nothing() {
        let citations = cite(
            &[
                "Migrating motor complexes sweep the fasting small intestine.".into(),
                "Too short.".into(),
            ],
            &units(),
        );
        assert_eq!(citations.key_points, [Vec::<usize>::new(), Vec::new()]);
        assert!(citations.is_empty());
    }

    #[test]
    fn passages_read_as_one_paragraph_without_ligatures() {
        assert_eq!(
            readable("the \u{fb02}at part of the\n  cytoplasm is de\u{fb01}ned"),
            "the flat part of the cytoplasm is defined"
        );
    }

    #[test]
    fn long_excerpts_are_cut_at_a_word() {
        let long = "word ".repeat(400);
        let cut = bounded(long.trim());
        assert!(cut.chars().count() <= MAX_EXCERPT_CHARS + 1);
        assert!(cut.ends_with("word…"));
    }
}
