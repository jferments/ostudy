//! Key terms set in bold where a section's text first uses them.
//!
//! The terms come from what is already known, so no model is asked: the
//! guide's key concepts, the terms its sections define, each section's topic
//! concepts from the learning plan and the concepts its pages' figures were
//! read to show. In each section the first use of a term in the overview, key
//! points and explanations is bolded, a few per section, so the text has
//! anchors to scan for rather than reading as one even wall. A key point that
//! opens with a short label ("Secretin: …", "Acinar cells — …") has its label
//! bolded too.

use std::collections::{HashMap, HashSet};

use serde_json::{json, Value};

use super::{EffectiveExportSection, EffectiveGuideExport, ExportBlock};
use crate::{
    database::Database, document_figures, document_learning::load_active_document_learning_plan,
    document_study_guides::DocumentStudyGuideSection,
};

/// More would make bold the text's normal weight rather than its anchors.
pub(super) const MAX_PER_SECTION: usize = 8;
const MAX_TERM_WORDS: usize = 6;
const MAX_TERM_CHARS: usize = 60;
const MIN_TERM_CHARS: usize = 3;
const MAX_LABEL_WORDS: usize = 6;

/// A figure's read concepts with the pages it belongs to.
pub(super) struct FigureConcepts {
    pages: Vec<i64>,
    concepts: Vec<String>,
}

/// Each learning-plan topic's key concepts, by topic id; none when the plan
/// cannot be read.
pub(super) fn topic_concepts(
    database: &Database,
    document_id: &str,
) -> HashMap<String, Vec<String>> {
    load_active_document_learning_plan(database, document_id)
        .ok()
        .flatten()
        .map(|plan| {
            plan.topics
                .into_iter()
                .map(|topic| (topic.id, topic.key_concepts))
                .collect()
        })
        .unwrap_or_default()
}

/// The concepts of the document's read, visible figures.
pub(super) fn figure_concepts(database: &Database, document_id: &str) -> Vec<FigureConcepts> {
    database
        .with_connection(|connection| document_figures::figures(connection, document_id))
        .map(|found| {
            found
                .figures
                .into_iter()
                .filter(|figure| !figure.hidden)
                .filter_map(|figure| {
                    let reading = figure.reading?;
                    let mut pages = reading.explained_on;
                    pages.push(figure.page_number);
                    Some(FigureConcepts {
                        pages,
                        concepts: reading.concepts,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The terms one document-guide section may bold.
pub(super) fn section_terms(
    section: &DocumentStudyGuideSection,
    topics: &HashMap<String, Vec<String>>,
    figures: &[FigureConcepts],
) -> Vec<String> {
    let pages = &section.source_locator.unit_numbers;
    let mut terms = section
        .content
        .important_terms
        .iter()
        .map(|term| term.term.clone())
        .collect::<Vec<_>>();
    terms.extend(topics.get(&section.topic_id).into_iter().flatten().cloned());
    terms.extend(
        figures
            .iter()
            .filter(|figure| figure.pages.iter().any(|page| pages.contains(page)))
            .flat_map(|figure| figure.concepts.iter())
            .filter(|concept| specific(concept))
            .cloned(),
    );
    terms
}

/// A figure concept is bolded only when it names something particular: a
/// phrase, an abbreviation or a long word, not "stomach" or "mucosa".
fn specific(concept: &str) -> bool {
    let concept = concept.trim();
    concept.split_whitespace().count() >= 2
        || concept.chars().skip(1).any(|c| c.is_uppercase())
        || concept.chars().count() >= 9
}

/// The guide-wide terms with the section's own, longest first so a longer
/// term wins over a part of it.
pub(super) fn terms_for(
    export: &EffectiveGuideExport,
    section: &EffectiveExportSection,
) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut terms = Vec::new();
    let mut add = |term: &str| {
        let term = term.trim().trim_end_matches(['.', ':']).trim();
        let words = term.split_whitespace().count();
        if term.chars().count() < MIN_TERM_CHARS
            || term.chars().count() > MAX_TERM_CHARS
            || words > MAX_TERM_WORDS
            || term.contains(['$', '(', ')', ';'])
        {
            return;
        }
        if seen.insert(term.to_lowercase()) {
            terms.push(term.to_string());
        }
    };
    for term in &section.terms {
        add(term);
    }
    for concept in &export.key_concepts {
        add(concept);
    }
    for other in &export.sections {
        for block in &other.body_lines {
            if let ExportBlock::Terms(pairs) = block {
                for (term, _) in pairs {
                    add(term);
                }
            }
        }
    }
    terms.sort_by_key(|term| std::cmp::Reverse(term.chars().count()));
    terms
}

/// What one section has bolded so far.
pub(super) struct SectionTerms<'a> {
    terms: &'a [String],
    used: HashSet<String>,
    count: usize,
}

impl<'a> SectionTerms<'a> {
    pub(super) fn new(terms: &'a [String]) -> Self {
        Self {
            terms,
            used: HashSet::new(),
            count: 0,
        }
    }

    /// Bolds the first use of each unused term in `runs`.
    pub(super) fn embolden(&mut self, runs: Value) -> Value {
        let Value::Array(runs) = runs else {
            return runs;
        };
        let mut out = Vec::with_capacity(runs.len());
        for run in runs {
            match run.get("t").and_then(Value::as_str) {
                Some(text) => self.split(text, &mut out),
                None => out.push(run),
            }
        }
        Value::Array(out)
    }

    /// A key point's runs with its opening label bolded, then its terms.
    pub(super) fn key_point(&mut self, runs: Value) -> Value {
        let Value::Array(mut items) = runs else {
            return runs;
        };
        let label = items
            .first()
            .and_then(|run| run.get("t"))
            .and_then(Value::as_str)
            .and_then(label_end)
            .map(|end| {
                let text = items[0]["t"].as_str().unwrap_or_default().to_string();
                (text[..end].to_string(), text[end..].to_string())
            });
        let mut head = Vec::new();
        if let Some((label, rest)) = label {
            for term in self.terms {
                if contains_word(&label.to_ascii_lowercase(), &term.to_ascii_lowercase()) {
                    self.used.insert(term.to_lowercase());
                }
            }
            head.push(json!({ "b": label }));
            items[0] = json!({ "t": rest });
        }
        let Value::Array(rest) = self.embolden(Value::Array(items)) else {
            unreachable!()
        };
        head.extend(
            rest.into_iter()
                .filter(|run| run.get("t") != Some(&json!(""))),
        );
        Value::Array(head)
    }

    fn split(&mut self, text: &str, out: &mut Vec<Value>) {
        let lower = text.to_ascii_lowercase();
        let mut from = 0_usize;
        while self.count < MAX_PER_SECTION {
            let found = self
                .terms
                .iter()
                .filter(|term| !self.used.contains(&term.to_lowercase()))
                .filter_map(|term| {
                    find_word(&lower[from..], &term.to_ascii_lowercase())
                        .map(|(start, end)| (from + start, from + end, term))
                })
                .min_by_key(|(start, end, _)| (*start, std::cmp::Reverse(*end)));
            let Some((start, end, term)) = found else {
                break;
            };
            if start > from {
                out.push(json!({ "t": &text[from..start] }));
            }
            out.push(json!({ "b": &text[start..end] }));
            self.used.insert(term.to_lowercase());
            self.count += 1;
            from = end;
        }
        if from < text.len() {
            out.push(json!({ "t": &text[from..] }));
        }
    }
}

/// Where a key point's short opening label ends, after its colon or dash.
fn label_end(text: &str) -> Option<usize> {
    let index = [": ", " — ", " – "]
        .iter()
        .filter_map(|separator| text.find(separator))
        .min()?;
    let label = &text[..index];
    let words = label.split_whitespace().count();
    if words == 0 || words > MAX_LABEL_WORDS || label.contains(['.', ';', ',']) {
        return None;
    }
    Some(index)
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric()
}

fn contains_word(haystack: &str, needle: &str) -> bool {
    find_word(haystack, needle).is_some()
}

/// The first whole-word match of `needle` in `haystack` (both ASCII
/// lowercased), taking a plural `s` or `es` with it.
fn find_word(haystack: &str, needle: &str) -> Option<(usize, usize)> {
    if needle.is_empty() {
        return None;
    }
    let mut from = 0;
    while let Some(offset) = haystack[from..].find(needle) {
        let start = from + offset;
        let mut end = start + needle.len();
        let before_ok = haystack[..start]
            .chars()
            .next_back()
            .is_none_or(|c| !is_word(c));
        for plural in ["es", "s"] {
            if haystack[end..].starts_with(plural)
                && haystack[end + plural.len()..]
                    .chars()
                    .next()
                    .is_none_or(|c| !is_word(c))
            {
                end += plural.len();
                break;
            }
        }
        let after_ok = haystack[end..].chars().next().is_none_or(|c| !is_word(c));
        if before_ok && after_ok {
            return Some((start, end));
        }
        from = start + haystack[start..].chars().next().map_or(1, char::len_utf8);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_of(runs: &Value) -> String {
        runs.as_array()
            .unwrap()
            .iter()
            .map(|run| match (run.get("t"), run.get("b")) {
                (Some(t), _) => t.as_str().unwrap().to_string(),
                (_, Some(b)) => format!("**{}**", b.as_str().unwrap()),
                _ => String::new(),
            })
            .collect()
    }

    #[test]
    fn bolds_first_use_of_each_term_once() {
        let terms = vec!["secretin".to_string(), "acinar cells".to_string()];
        let mut section = SectionTerms::new(&terms);
        let first = section.embolden(
            json!([{ "t": "Secretin acts on ductal cells; Acinar cells make enzymes." }]),
        );
        assert_eq!(
            text_of(&first),
            "**Secretin** acts on ductal cells; **Acinar cells** make enzymes."
        );
        let second = section.embolden(json!([{ "t": "More secretin." }]));
        assert_eq!(text_of(&second), "More secretin.");
    }

    #[test]
    fn matches_whole_words_and_plurals() {
        let terms = vec!["bile salt".to_string(), "pH".to_string()];
        let mut section = SectionTerms::new(&terms);
        let runs = section.embolden(json!([{ "t": "Bile salts lower phase pH." }]));
        assert_eq!(text_of(&runs), "**Bile salts** lower phase **pH**.");
    }

    #[test]
    fn bolds_a_key_points_label() {
        let terms = vec!["gastrin".to_string()];
        let mut section = SectionTerms::new(&terms);
        let runs = section
            .key_point(json!([{ "t": "Gastrin: stimulates H+ secretion by parietal cells." }]));
        assert_eq!(
            text_of(&runs),
            "**Gastrin**: stimulates H+ secretion by parietal cells."
        );
        let plain = section
            .key_point(json!([{ "t": "Fat in the ileum, via peptide YY: slows emptying." }]));
        assert_eq!(
            text_of(&plain),
            "Fat in the ileum, via peptide YY: slows emptying."
        );
    }
}
