//! Restores printed sub- and superscripts that flat text extraction loses.
//!
//! The extractor keeps the characters of H₂O, HCO₃⁻ and Ca²⁺ in order but
//! writes them as H2O, HCO3− and Ca2+. PDFium still knows each glyph's size
//! and baseline, so a small digit or sign set below or above its word's
//! baseline is read back as a subscript or superscript. Only a word the page
//! always prints the same way is rewritten, and only where the extracted text
//! spells it exactly, so names like "H2 receptor" stay as they are.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::{
    pdf_backend::ExtractedDocument,
    pdf_render_backend::{self, GlyphMetrics, PageGlyph, PdfRenderRuntime},
};

use super::validation::bounded_diagnostic;

/// A glyph at most this share of its word's body size counts as small.
const SMALL_GLYPH_RATIO: f64 = 0.85;
/// A small glyph whose baseline sits this share of the body size above the
/// word's baseline is a superscript.
const RAISED_BASELINE_RATIO: f64 = 0.15;
/// A small glyph whose baseline sits this share below it is a subscript.
const LOWERED_BASELINE_RATIO: f64 = 0.03;

/// Rewrites every page, semantic element and chunk of `semantic` with the
/// notation its PDF prints. A renderer that cannot read glyph positions
/// leaves the text as extracted.
pub(crate) fn restore_printed_notation(
    render_runtime: &PdfRenderRuntime,
    document_id: &str,
    source_path: &Path,
    semantic: &mut ExtractedDocument,
) {
    let mut by_page = BTreeMap::new();
    let read = pdf_render_backend::page_glyphs(render_runtime, source_path, |page, glyphs| {
        let words = notation_words(&glyphs);
        if !words.is_empty() {
            by_page.insert(page, words);
        }
    });
    if let Err(error) = read {
        crate::logging::info(
            "document_notation_unavailable",
            format!(
                "document={document_id} detail={}",
                bounded_diagnostic(&error.to_string(), 300)
            ),
        );
        return;
    }
    if by_page.is_empty() {
        return;
    }
    let mut restored = 0usize;
    for page in &mut semantic.pages {
        if let Some(words) = by_page.get(&page.page_number) {
            let (text, count) = apply_notation(&page.text, words);
            page.text = text;
            restored += count;
        }
    }
    for element in &mut semantic.elements {
        if let Some(words) = by_page.get(&element.page_number) {
            element.text = apply_notation(&element.text, words).0;
        }
    }
    for chunk in &mut semantic.chunks {
        let mut words = BTreeMap::new();
        for page in &chunk.page_numbers {
            for (plain, printed) in by_page.get(page).into_iter().flatten() {
                words.insert(plain.clone(), printed.clone());
            }
        }
        if !words.is_empty() {
            chunk.text = apply_notation(&chunk.text, &words).0;
            chunk.full_text = apply_notation(&chunk.full_text, &words).0;
        }
    }
    crate::logging::info(
        "document_notation_restored",
        format!(
            "document={document_id} pages={} words={restored}",
            by_page.len()
        ),
    );
}

/// The words on one page that print sub- or superscripts, keyed by how flat
/// extraction spells them. A spelling the page also prints differently is
/// left out.
pub(crate) fn notation_words(glyphs: &[PageGlyph]) -> BTreeMap<String, String> {
    let mut words = BTreeMap::<String, String>::new();
    let mut ambiguous = BTreeSet::new();
    for word in page_words(glyphs) {
        let Some((plain, printed)) = printed_word(&word) else {
            continue;
        };
        match words.get(&plain) {
            Some(existing) if *existing != printed => {
                ambiguous.insert(plain);
            }
            Some(_) => {}
            None => {
                words.insert(plain, printed);
            }
        }
    }
    words.retain(|plain, printed| !ambiguous.contains(plain) && plain != printed);
    words
}

/// The page's words. PDFium starts a new line at a raised charge, as in the
/// "−" of HCO₃⁻, so small signs and digits separated from the word before
/// them only by a line break the PDF implies are joined back to that word.
fn page_words(glyphs: &[PageGlyph]) -> Vec<Vec<PageGlyph>> {
    let mut words = Vec::<(Vec<PageGlyph>, bool)>::new();
    let mut current = Vec::new();
    let mut implied = false;
    for glyph in glyphs {
        let is_break = glyph.text.is_whitespace() || glyph.metrics.is_none();
        if !is_break {
            current.push(*glyph);
            continue;
        }
        if !current.is_empty() {
            words.push((std::mem::take(&mut current), implied));
            implied = true;
        }
        if glyph.metrics.is_some() {
            implied = false;
        }
    }
    if !current.is_empty() {
        words.push((current, implied));
    }
    let mut joined = Vec::<Vec<PageGlyph>>::with_capacity(words.len());
    for (word, implied) in words {
        match joined.last_mut() {
            Some(previous) if implied && is_mark_tail(previous, &word) => previous.extend(word),
            _ => joined.push(word),
        }
    }
    joined
}

fn is_mark_tail(previous: &[PageGlyph], word: &[PageGlyph]) -> bool {
    let Some(body) = previous
        .iter()
        .filter_map(|glyph| glyph.metrics)
        .map(|metric| metric.font_size)
        .reduce(f64::max)
    else {
        return false;
    };
    let Some(baseline) = previous
        .iter()
        .filter_map(|glyph| glyph.metrics)
        .find(|metric| metric.font_size > body * SMALL_GLYPH_RATIO)
        .map(|metric| metric.baseline)
    else {
        return false;
    };
    word.iter().all(|glyph| {
        glyph.metrics.is_some_and(|metric| {
            metric.font_size <= body * SMALL_GLYPH_RATIO
                && (metric.baseline - baseline).abs() <= body * 0.6
                && superscript(glyph.text).is_some()
        })
    })
}

fn printed_word(word: &[PageGlyph]) -> Option<(String, String)> {
    let start = word.iter().position(|glyph| glyph.text.is_alphanumeric())?;
    let end = word.len()
        - word
            .iter()
            .rev()
            .position(|glyph| !is_edge_punctuation(glyph.text))?;
    let word = word.get(start..end)?;
    let metrics = word
        .iter()
        .map(|glyph| glyph.metrics)
        .collect::<Option<Vec<GlyphMetrics>>>()?;
    let body = metrics
        .iter()
        .map(|metric| metric.font_size)
        .fold(0.0_f64, f64::max);
    if body <= 0.0 || body.is_nan() {
        return None;
    }
    let is_small = |metric: &GlyphMetrics| metric.font_size <= body * SMALL_GLYPH_RATIO;
    let baseline = metrics.iter().find(|metric| !is_small(metric))?.baseline;
    let mut plain = String::new();
    let mut printed = String::new();
    for (glyph, metric) in word.iter().zip(&metrics) {
        plain.push(glyph.text);
        let shift = (metric.baseline - baseline) / body;
        let marked = if !is_small(metric) {
            None
        } else if shift >= RAISED_BASELINE_RATIO {
            superscript(glyph.text)
        } else if shift <= -LOWERED_BASELINE_RATIO {
            subscript(glyph.text)
        } else {
            None
        };
        printed.push(marked.unwrap_or(glyph.text));
    }
    Some((plain, printed))
}

/// Trailing characters trimmed from a word: punctuation that is not itself
/// part of a charge.
fn is_edge_punctuation(character: char) -> bool {
    matches!(
        character,
        ',' | '.' | ';' | ':' | ')' | ']' | '}' | '"' | '\'' | '’' | '”' | '?' | '!'
    )
}

fn superscript(character: char) -> Option<char> {
    Some(match character {
        '0' => '⁰',
        '1' => '¹',
        '2' => '²',
        '3' => '³',
        '4' => '⁴',
        '5' => '⁵',
        '6' => '⁶',
        '7' => '⁷',
        '8' => '⁸',
        '9' => '⁹',
        '+' => '⁺',
        '-' | '−' | '–' => '⁻',
        '=' => '⁼',
        '(' => '⁽',
        ')' => '⁾',
        'n' => 'ⁿ',
        _ => return None,
    })
}

fn subscript(character: char) -> Option<char> {
    Some(match character {
        '0' => '₀',
        '1' => '₁',
        '2' => '₂',
        '3' => '₃',
        '4' => '₄',
        '5' => '₅',
        '6' => '₆',
        '7' => '₇',
        '8' => '₈',
        '9' => '₉',
        '+' => '₊',
        '-' | '−' | '–' => '₋',
        '=' => '₌',
        '(' => '₍',
        ')' => '₎',
        _ => return None,
    })
}

/// Replaces each whole-word occurrence of a plain spelling with its printed
/// form, returning the text and how many words changed.
pub(crate) fn apply_notation(text: &str, words: &BTreeMap<String, String>) -> (String, usize) {
    let mut output = String::with_capacity(text.len() + text.len() / 16);
    let mut count = 0;
    let mut index = 0;
    while index < text.len() {
        let rest = &text[index..];
        let at_boundary = text[..index]
            .chars()
            .next_back()
            .is_none_or(|previous| !previous.is_alphanumeric());
        let found = at_boundary
            .then(|| {
                // Longest spelling first, so "HCO3−" wins over "HCO3".
                words
                    .iter()
                    .rev()
                    .filter(|(plain, _)| rest.starts_with(plain.as_str()))
                    .find(|(plain, _)| {
                        rest[plain.len()..]
                            .chars()
                            .next()
                            .is_none_or(|next| !next.is_alphanumeric())
                    })
            })
            .flatten();
        if let Some((plain, printed)) = found {
            output.push_str(printed);
            index += plain.len();
            count += 1;
        } else {
            let character = rest.chars().next().unwrap_or_default();
            output.push(character);
            index += character.len_utf8().max(1);
        }
    }
    (output, count)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Glyphs for `text`, where `_x` is a subscript x and `^x` a superscript x.
    fn glyphs(text: &str) -> Vec<PageGlyph> {
        let mut glyphs = Vec::new();
        let mut characters = text.chars();
        while let Some(character) = characters.next() {
            let (value, font_size, baseline) = match character {
                '_' => (characters.next().unwrap(), 5.8, 99.4),
                '^' => (characters.next().unwrap(), 5.8, 104.8),
                _ => (character, 9.7, 100.0),
            };
            glyphs.push(PageGlyph {
                text: value,
                metrics: (!value.is_whitespace()).then_some(GlyphMetrics {
                    font_size,
                    baseline,
                }),
            });
        }
        glyphs
    }

    #[test]
    fn reads_formulas_ions_and_powers_from_glyph_positions() {
        let words = notation_words(&glyphs(
            "Na^+ and HCO_3^−, H_2O, Ca^2^+ at 10^−^3 SO_4^2^− (NH_4^+).",
        ));
        assert_eq!(words.get("Na+").map(String::as_str), Some("Na⁺"));
        assert_eq!(words.get("HCO3−").map(String::as_str), Some("HCO₃⁻"));
        assert_eq!(words.get("H2O").map(String::as_str), Some("H₂O"));
        assert_eq!(words.get("Ca2+").map(String::as_str), Some("Ca²⁺"));
        assert_eq!(words.get("10−3").map(String::as_str), Some("10⁻³"));
        assert_eq!(words.get("SO42−").map(String::as_str), Some("SO₄²⁻"));
        assert_eq!(words.get("NH4+").map(String::as_str), Some("NH₄⁺"));
        assert!(!words.contains_key("and"));
    }

    #[test]
    fn joins_a_raised_charge_set_on_its_own_line() {
        let mut page = glyphs("HCO_3");
        page.push(PageGlyph {
            text: '\n',
            metrics: None,
        });
        page.extend(glyphs("^− and"));
        let words = notation_words(&page);
        assert_eq!(words.get("HCO3−").map(String::as_str), Some("HCO₃⁻"));
    }

    #[test]
    fn leaves_full_size_digits_and_spellings_printed_both_ways() {
        let words = notation_words(&glyphs("H2 receptor, vitamin B12, H_2 gas and H2 blockers"));
        assert!(words.is_empty(), "{words:?}");
    }

    #[test]
    fn rewrites_only_whole_words_of_the_extracted_text() {
        let words = notation_words(&glyphs("HCO_3^− Ca^2^+ H_2O"));
        let (text, count) = apply_notation(
            "Saliva has high [HCO3−] and Ca2+; H2O follows. PH2O and Ca2+-ATPase stay apart.",
            &words,
        );
        assert_eq!(
            text,
            "Saliva has high [HCO₃⁻] and Ca²⁺; H₂O follows. PH2O and Ca²⁺-ATPase stay apart."
        );
        assert_eq!(count, 4);
    }
}
