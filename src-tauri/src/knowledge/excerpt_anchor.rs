//! Conservative source-excerpt anchoring for provider output.
//!
//! Normalization is used only to locate a unique span through common text-
//! extraction artifacts. Callers persist the literal original source slice and
//! still run the ordinary exact-substring validator before writing knowledge.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ExcerptAnchorStatus {
    Exact,
    Reanchored,
    Missing,
    Ambiguous,
}

impl ExcerptAnchorStatus {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::Reanchored => "reanchored",
            Self::Missing => "missing",
            Self::Ambiguous => "ambiguous",
        }
    }
}

#[derive(Debug)]
pub(crate) struct ExcerptAnchorResult {
    pub(crate) status: ExcerptAnchorStatus,
    pub(crate) anchored_excerpt: Option<String>,
    pub(crate) normalized_excerpt_chars: usize,
    /// Capped at two: zero means no match, one means unique, two means multiple.
    pub(crate) normalized_match_count: usize,
}

#[derive(Debug, Default)]
struct NormalizedAnchorText {
    chars: Vec<char>,
    source_spans: Vec<(usize, usize)>,
}

impl NormalizedAnchorText {
    fn push_replacement(&mut self, replacement: &str, start: usize, end: usize) {
        for character in replacement.chars() {
            self.chars.push(character);
            self.source_spans.push((start, end));
        }
    }

    fn push_whitespace(&mut self, start: usize, end: usize) {
        if self.chars.is_empty() {
            return;
        }
        if self.chars.last() == Some(&' ') {
            if let Some(span) = self.source_spans.last_mut() {
                span.1 = end;
            }
            return;
        }
        self.chars.push(' ');
        self.source_spans.push((start, end));
    }

    fn trim_trailing_whitespace(&mut self) {
        if self.chars.last() == Some(&' ') {
            self.chars.pop();
            self.source_spans.pop();
        }
    }
}

pub(crate) fn reanchor_excerpt(source: &str, excerpt: &str) -> ExcerptAnchorResult {
    if source.contains(excerpt) {
        return ExcerptAnchorResult {
            status: ExcerptAnchorStatus::Exact,
            anchored_excerpt: None,
            normalized_excerpt_chars: excerpt.chars().count(),
            normalized_match_count: 1,
        };
    }

    let normalized_source = normalize_anchor_text(source);
    let normalized_excerpt = normalize_anchor_text(excerpt);
    let needle_len = normalized_excerpt.chars.len();
    if needle_len == 0 || needle_len > normalized_source.chars.len() {
        return missing(needle_len);
    }

    let mut first_match = None;
    let mut match_count = 0_usize;
    for start in 0..=normalized_source.chars.len() - needle_len {
        if normalized_source.chars[start..start + needle_len] != normalized_excerpt.chars[..]
            || !match_boundaries_align(&normalized_source, start, needle_len)
        {
            continue;
        }
        match_count = match_count.saturating_add(1);
        if first_match.is_none() {
            first_match = Some(start);
        }
        if match_count == 2 {
            break;
        }
    }

    if match_count == 0 {
        return missing(needle_len);
    }
    if match_count > 1 {
        return ExcerptAnchorResult {
            status: ExcerptAnchorStatus::Ambiguous,
            anchored_excerpt: None,
            normalized_excerpt_chars: needle_len,
            normalized_match_count: 2,
        };
    }

    let start = first_match.expect("one normalized match must have a start index");
    let end = start + needle_len - 1;
    let start_byte = normalized_source.source_spans[start].0;
    let end_byte = normalized_source.source_spans[end].1;
    let Some(anchored_excerpt) = source.get(start_byte..end_byte).map(ToString::to_string) else {
        return missing(needle_len);
    };
    ExcerptAnchorResult {
        status: ExcerptAnchorStatus::Reanchored,
        anchored_excerpt: Some(anchored_excerpt),
        normalized_excerpt_chars: needle_len,
        normalized_match_count: 1,
    }
}

fn missing(normalized_excerpt_chars: usize) -> ExcerptAnchorResult {
    ExcerptAnchorResult {
        status: ExcerptAnchorStatus::Missing,
        anchored_excerpt: None,
        normalized_excerpt_chars,
        normalized_match_count: 0,
    }
}

/// A normalized match may not start or end in the middle of a multi-character
/// replacement such as a PDF ligature or ellipsis. Otherwise `fi` -> `f`/`i`
/// could widen a model excerpt to source text that it did not actually quote.
fn match_boundaries_align(text: &NormalizedAnchorText, start: usize, length: usize) -> bool {
    let end = start + length - 1;
    if start > 0 && text.source_spans[start - 1] == text.source_spans[start] {
        return false;
    }
    if end + 1 < text.source_spans.len() && text.source_spans[end] == text.source_spans[end + 1] {
        return false;
    }
    true
}

fn normalize_anchor_text(input: &str) -> NormalizedAnchorText {
    let mut result = NormalizedAnchorText::default();
    let mut characters = input.char_indices().peekable();
    let mut suppress_hyphenated_line_whitespace = false;

    while let Some((start, character)) = characters.next() {
        let end = characters.peek().map_or(input.len(), |(offset, _)| *offset);
        if character.is_whitespace() {
            if !suppress_hyphenated_line_whitespace {
                result.push_whitespace(start, end);
            }
            continue;
        }
        suppress_hyphenated_line_whitespace = false;

        if character == '-'
            && result
                .chars
                .iter()
                .rev()
                .find(|value| **value != ' ')
                .is_some_and(|value| value.is_alphabetic())
            && line_break_continues_word(&input[end..])
        {
            suppress_hyphenated_line_whitespace = true;
            continue;
        }

        if matches!(character, '\u{00ad}' | '\u{200b}' | '\u{2060}' | '\u{feff}') {
            continue;
        }

        let replacement = match character {
            '\u{fb00}' => "ff",
            '\u{fb01}' => "fi",
            '\u{fb02}' => "fl",
            '\u{fb03}' => "ffi",
            '\u{fb04}' => "ffl",
            '\u{fb05}' | '\u{fb06}' => "st",
            '\u{2018}' | '\u{2019}' | '\u{201a}' | '\u{201b}' => "'",
            '\u{201c}' | '\u{201d}' | '\u{201e}' | '\u{201f}' => "\"",
            '\u{2010}' | '\u{2011}' | '\u{2012}' | '\u{2013}' | '\u{2014}' | '\u{2212}' => "-",
            '\u{2026}' => "...",
            _ => {
                result.push_replacement(&character.to_string(), start, end);
                continue;
            }
        };
        result.push_replacement(replacement, start, end);
    }

    result.trim_trailing_whitespace();
    result
}

fn line_break_continues_word(rest: &str) -> bool {
    let after_break = rest
        .strip_prefix("\r\n")
        .or_else(|| rest.strip_prefix('\n'))
        .or_else(|| rest.strip_prefix('\r'));
    let Some(after_break) = after_break else {
        return false;
    };
    after_break
        .chars()
        .find(|character| !character.is_whitespace())
        .is_some_and(char::is_alphabetic)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_excerpt_is_left_untouched() {
        let source = "Exact source evidence.";
        let anchored = reanchor_excerpt(source, source);
        assert_eq!(anchored.status, ExcerptAnchorStatus::Exact);
        assert!(anchored.anchored_excerpt.is_none());
    }

    #[test]
    fn normalization_recovers_the_literal_original_source_span() {
        let source = "The heart’s cardiac\u{00a0}output is efﬁcient. The next sentence follows.";
        let anchored = reanchor_excerpt(source, "The heart's cardiac output is efficient.");
        assert_eq!(anchored.status, ExcerptAnchorStatus::Reanchored);
        assert_eq!(
            anchored.anchored_excerpt.as_deref(),
            Some("The heart’s cardiac\u{00a0}output is efﬁcient.")
        );
        assert!(source.contains(anchored.anchored_excerpt.as_deref().unwrap()));
    }

    #[test]
    fn pdf_line_wrap_hyphenation_can_be_reanchored() {
        let source = "A cardio-\nvascular response can occur.";
        let anchored = reanchor_excerpt(source, "A cardiovascular response can occur.");
        assert_eq!(anchored.status, ExcerptAnchorStatus::Reanchored);
        assert_eq!(
            anchored.anchored_excerpt.as_deref(),
            Some("A cardio-\nvascular response can occur.")
        );
    }

    #[test]
    fn ambiguous_normalized_evidence_is_rejected() {
        let source = "Same\nwords. Same\u{00a0}words.";
        let anchored = reanchor_excerpt(source, "Same words.");
        assert_eq!(anchored.status, ExcerptAnchorStatus::Ambiguous);
        assert!(anchored.anchored_excerpt.is_none());
        assert_eq!(anchored.normalized_match_count, 2);
    }

    #[test]
    fn paraphrased_evidence_is_not_fuzzy_matched() {
        let anchored = reanchor_excerpt(
            "Ventricular preload influences stroke volume.",
            "Preload changes cardiac output.",
        );
        assert_eq!(anchored.status, ExcerptAnchorStatus::Missing);
        assert!(anchored.anchored_excerpt.is_none());
    }

    #[test]
    fn match_cannot_begin_inside_a_ligature_expansion() {
        let anchored = reanchor_excerpt("ﬁsh physiology", "ish physiology");
        assert_eq!(anchored.status, ExcerptAnchorStatus::Missing);
    }
}
