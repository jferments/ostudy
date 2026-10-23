//! Writes a section's formulas, ions and powers the way its source prints them.
//!
//! The source text keeps printed sub- and superscripts (HCO₃⁻, Ca²⁺), but a
//! model often writes them flat (HCO3-, Ca2+). Each flat spelling of a word
//! the source prints with marks is rewritten to the printed word. A spelling
//! the source also uses unmarked, such as the H2 of an H2 receptor, is left
//! as written, so nothing is guessed.

use std::collections::{BTreeMap, BTreeSet};

use crate::document_study_guides::StudyGuideSectionContent;

/// Rewrites flat spellings in every text field of `section`; returns how many
/// words changed.
pub(super) fn print_source_notation(
    source_text: &str,
    section: &mut StudyGuideSectionContent,
) -> usize {
    let words = printed_words(source_text);
    if words.is_empty() {
        return 0;
    }
    let mut count = 0;
    for_each_text(section, |text| {
        let (printed, changed) = apply(text, &words);
        if changed > 0 {
            *text = printed;
            count += changed;
        }
    });
    count
}

/// Flat spelling to printed word, for every word of `source` with a sub- or
/// superscript.
fn printed_words(source: &str) -> BTreeMap<String, String> {
    let mut words = BTreeMap::new();
    for word in source_words(source) {
        if !word.chars().any(is_mark) || !word.chars().any(char::is_alphanumeric) {
            continue;
        }
        for flat in flat_spellings(word) {
            words.entry(flat).or_insert_with(|| word.to_string());
        }
    }
    // A spelling the source also writes unmarked is not the printed word's.
    let plain = source_words(source)
        .filter(|word| !word.chars().any(is_mark))
        .collect::<BTreeSet<_>>();
    words.retain(|flat, _| !plain.contains(flat.as_str()));
    words
}

/// The source's words: split at spaces and slashes, and at a hyphen or dash
/// that follows a mark (Na⁺-K⁺, Cl⁻-HCO₃⁻), with edge punctuation trimmed.
fn source_words(source: &str) -> impl Iterator<Item = &str> {
    source
        .split(|c: char| c.is_whitespace() || c == '/')
        .flat_map(split_after_marks)
        .map(|word| {
            word.trim_matches(|c: char| {
                matches!(
                    c,
                    ',' | '.'
                        | ';'
                        | ':'
                        | '('
                        | ')'
                        | '['
                        | ']'
                        | '"'
                        | '\''
                        | '“'
                        | '”'
                        | '‘'
                        | '’'
                )
            })
        })
        .filter(|word| !word.is_empty())
}

fn split_after_marks(word: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut previous = None;
    for (index, character) in word.char_indices() {
        if matches!(character, '-' | '–' | '−') && previous.is_some_and(is_mark) {
            parts.push(&word[start..index]);
            start = index + character.len_utf8();
        }
        previous = Some(character);
    }
    parts.push(&word[start..]);
    parts
}

/// Every flat way to write `word`: marks as plain digits and signs, a minus
/// as a hyphen, a minus sign or an en dash.
fn flat_spellings(word: &str) -> Vec<String> {
    let mut spellings = vec![String::new()];
    for character in word.chars() {
        let choices: &[char] = match character {
            '⁻' | '₋' => &['-', '−', '–'],
            other => match flat(other) {
                Some(_) => &[],
                None => {
                    spellings
                        .iter_mut()
                        .for_each(|spelling| spelling.push(other));
                    continue;
                }
            },
        };
        if choices.is_empty() {
            let plain = flat(character).unwrap_or(character);
            spellings
                .iter_mut()
                .for_each(|spelling| spelling.push(plain));
        } else {
            spellings = spellings
                .iter()
                .flat_map(|spelling| {
                    choices
                        .iter()
                        .map(move |choice| format!("{spelling}{choice}"))
                })
                .collect();
        }
    }
    spellings
}

fn is_mark(character: char) -> bool {
    flat(character).is_some() || matches!(character, '⁻' | '₋')
}

fn flat(character: char) -> Option<char> {
    Some(match character {
        '⁰' | '₀' => '0',
        '¹' | '₁' => '1',
        '²' | '₂' => '2',
        '³' | '₃' => '3',
        '⁴' | '₄' => '4',
        '⁵' | '₅' => '5',
        '⁶' | '₆' => '6',
        '⁷' | '₇' => '7',
        '⁸' | '₈' => '8',
        '⁹' | '₉' => '9',
        '⁺' | '₊' => '+',
        _ => return None,
    })
}

/// Replaces whole-word flat spellings, longest first.
fn apply(text: &str, words: &BTreeMap<String, String>) -> (String, usize) {
    let mut output = String::with_capacity(text.len());
    let mut count = 0;
    let mut index = 0;
    while index < text.len() {
        let rest = &text[index..];
        let at_start = text[..index]
            .chars()
            .next_back()
            .is_none_or(|previous| !previous.is_alphanumeric());
        let found = at_start
            .then(|| {
                words
                    .iter()
                    .rev()
                    .filter(|(flat, _)| rest.starts_with(flat.as_str()))
                    .find(|(flat, _)| {
                        rest[flat.len()..]
                            .chars()
                            .next()
                            .is_none_or(|next| !next.is_alphanumeric())
                    })
            })
            .flatten();
        if let Some((flat, printed)) = found {
            output.push_str(printed);
            index += flat.len();
            count += 1;
        } else {
            let character = rest.chars().next().unwrap_or_default();
            output.push(character);
            index += character.len_utf8().max(1);
        }
    }
    (output, count)
}

fn for_each_text(section: &mut StudyGuideSectionContent, mut visit: impl FnMut(&mut String)) {
    let StudyGuideSectionContent {
        overview,
        key_points,
        explanations,
        important_terms,
        process_steps,
        comparisons,
        common_misconceptions,
        callouts,
        tables,
        worked_examples,
        retrieval_checks,
        visual_explanations,
    } = section;
    visit(overview);
    key_points.iter_mut().for_each(&mut visit);
    for explanation in explanations {
        visit(&mut explanation.heading);
        visit(&mut explanation.body);
    }
    for term in important_terms {
        visit(&mut term.term);
        visit(&mut term.explanation);
    }
    process_steps.iter_mut().for_each(&mut visit);
    for comparison in comparisons {
        visit(&mut comparison.subject_a);
        visit(&mut comparison.subject_b);
        visit(&mut comparison.relationship);
    }
    common_misconceptions.iter_mut().for_each(&mut visit);
    for callout in callouts {
        visit(&mut callout.title);
        visit(&mut callout.body);
    }
    for table in tables {
        visit(&mut table.title);
        table.columns.iter_mut().for_each(&mut visit);
        table.rows.iter_mut().flatten().for_each(&mut visit);
        visit(&mut table.caption);
    }
    for example in worked_examples {
        visit(&mut example.title);
        visit(&mut example.prompt);
        example.steps.iter_mut().for_each(&mut visit);
        visit(&mut example.answer);
        visit(&mut example.takeaway);
    }
    for check in retrieval_checks {
        visit(&mut check.prompt);
        visit(&mut check.answer);
    }
    for visual in visual_explanations {
        visit(&mut visual.title);
        visit(&mut visual.explanation);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_flat_spellings_as_the_source_prints_them() {
        let source = "Parietal cells secrete H⁺ via the H⁺-K⁺ ATPase and return HCO₃⁻ through a Cl⁻-HCO₃⁻ exchanger; CO₂ and H₂O form H₂CO₃ via IP₃/Ca²⁺.";
        let mut section = StudyGuideSectionContent {
            overview: "H+ secretion uses the H+-K+ ATPase; HCO3- leaves via Cl−-HCO3− exchange."
                .to_string(),
            key_points: vec![
                "CO2 and H2O form H2CO3.".to_string(),
                "Gastrin acts through IP3/Ca2+.".to_string(),
            ],
            ..Default::default()
        };
        let changed = print_source_notation(source, &mut section);
        assert_eq!(
            section.overview,
            "H⁺ secretion uses the H⁺-K⁺ ATPase; HCO₃⁻ leaves via Cl⁻-HCO₃⁻ exchange."
        );
        assert_eq!(
            section.key_points,
            ["CO₂ and H₂O form H₂CO₃.", "Gastrin acts through IP₃/Ca²⁺."]
        );
        assert_eq!(changed, 11);
    }

    #[test]
    fn leaves_spellings_the_source_also_writes_unmarked() {
        let source = "H₂ gas and H2 receptor blockers; vitamin B12 and Ca²⁺.";
        let mut section = StudyGuideSectionContent {
            overview: "H2 blockers, B12, CA2+ and Ca2+ ions; a type 2 diabetes note.".to_string(),
            ..Default::default()
        };
        print_source_notation(source, &mut section);
        assert_eq!(
            section.overview,
            "H2 blockers, B12, CA2+ and Ca²⁺ ions; a type 2 diabetes note."
        );
    }
}
