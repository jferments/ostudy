//! Read-only detection/disclosure for the native source-notation PDF writer.
//! This is not a second math renderer. The WebView still uses shared KaTeX.

use super::{EffectiveGuideExport, ExportBlock};

pub(super) const NOTICE: &str = "Math notation: equations in this PDF are preserved as readable LaTeX source, not typeset formulas. Open this study guide in oStudy to view typeset math.";

pub(super) fn has_math(export: &EffectiveGuideExport) -> bool {
    if contains_math(&export.overview)
        || export.learning_objectives.iter().any(|s| contains_math(s))
        || export.key_concepts.iter().any(|s| contains_math(s))
        || export.guide_notes.iter().any(|s| contains_math(s))
    {
        return true;
    }
    export.sections.iter().any(|section| {
        contains_math(&section.title)
            || section.notes.iter().any(|s| contains_math(s))
            || section.body_lines.iter().any(|block| match block {
                ExportBlock::Heading(s) | ExportBlock::Paragraph(s) => contains_math(s),
                ExportBlock::Bullets(items) | ExportBlock::Numbered(items) => {
                    items.iter().any(|s| contains_math(s))
                }
                ExportBlock::Terms(items) => items
                    .iter()
                    .any(|(a, b)| contains_math(a) || contains_math(b)),
                ExportBlock::Callout { label, title, body } => contains_math(label) || contains_math(title) || contains_math(body),
                ExportBlock::Table { title, columns, rows, caption } => contains_math(title) || columns.iter().any(|v| contains_math(v)) || rows.iter().flatten().any(|v| contains_math(v)) || contains_math(caption),
                ExportBlock::WorkedExample { title, prompt, steps, answer, takeaway } => contains_math(title) || contains_math(prompt) || steps.iter().any(|v| contains_math(v)) || contains_math(answer) || contains_math(takeaway),
                ExportBlock::RetrievalCheck { prompt, answer } => contains_math(prompt) || contains_math(answer),
                ExportBlock::Spacer => false,
            })
    })
}

/// Complete supported delimiters outside code only. Boundaries are found before
/// backslashes can be lost; no source normalization or TeX evaluation happens.
pub(super) fn contains_math(source: &str) -> bool {
    let mut cursor = 0;
    let mut fence: Option<(u8, usize)> = None;
    while cursor < source.len() {
        let rest = &source[cursor..];
        if cursor == 0 || source.as_bytes()[cursor - 1] == b'\n' {
            let end = rest.find('\n').unwrap_or(rest.len());
            let line = &rest[..end];
            let trimmed = line.trim_start_matches(' ');
            let indent = line.len() - trimmed.len();
            let marker = trimmed.as_bytes().first().copied().unwrap_or_default();
            let width = trimmed.bytes().take_while(|b| *b == marker).count();
            if let Some((expected, minimum)) = fence {
                if indent <= 3
                    && marker == expected
                    && width >= minimum
                    && trimmed[width..].trim().is_empty()
                {
                    fence = None;
                }
                cursor += end + usize::from(end < rest.len());
                continue;
            }
            if indent <= 3 && matches!(marker, b'`' | b'~') && width >= 3 {
                fence = Some((marker, width));
                cursor += end + usize::from(end < rest.len());
                continue;
            }
            if indent >= 4 || line.starts_with('\t') {
                cursor += end + usize::from(end < rest.len());
                continue;
            }
        }
        if rest.starts_with('`') {
            let width = rest.bytes().take_while(|b| *b == b'`').count();
            if let Some(end) = rest[width..].find(&rest[..width]) {
                cursor += width + end + width;
                continue;
            }
        }
        let close = if rest.starts_with("\\(") {
            Some("\\)")
        } else if rest.starts_with("\\[") {
            Some("\\]")
        } else if rest.starts_with("$$") {
            Some("$$")
        } else {
            None
        };
        if let Some(close) = close.filter(|_| !escaped(source, cursor)) {
            let start = cursor + 2;
            if let Some(end) = source[start..]
                .match_indices(close)
                .map(|(offset, _)| start + offset)
                .find(|end| !escaped(source, *end))
            {
                if !source[start..end].trim().is_empty() {
                    return true;
                }
                cursor = end + 2;
                continue;
            }
            return false;
        }
        cursor += rest.chars().next().map_or(0, char::len_utf8);
    }
    false
}

fn escaped(source: &str, position: usize) -> bool {
    source.as_bytes()[..position]
        .iter()
        .rev()
        .take_while(|b| **b == b'\\')
        .count()
        % 2
        == 1
}

/// Ordinary prose wrapping collapses spaces; that would change \text{...} and
/// source examples. Preserve every character on math-bearing lines. Added PDF
/// line breaks are presentation only, and the stored guide remains untouched.
pub(super) fn wrap_source(text: &str, maximum: usize) -> Vec<String> {
    let maximum = maximum.max(1);
    let mut lines = Vec::new();
    for line in text.split('\n') {
        let line = line.strip_suffix('\r').unwrap_or(line);
        let mut chunk = String::new();
        let mut count = 0;
        for character in line.chars() {
            chunk.push(character);
            count += 1;
            if count == maximum {
                lines.push(std::mem::take(&mut chunk));
                count = 0;
            }
        }
        if !chunk.is_empty() || line.is_empty() {
            lines.push(chunk);
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn document_pdf_math_detection_keeps_currency_code_and_escapes_literal() {
        for source in [r"\(x\)", r"\[x\]", "$$x$$", "**before \\(x_i\\) after**"] {
            assert!(contains_math(source), "{source}");
        }
        for source in [
            "$5 and $10",
            r"\\(x\\)",
            "`\\(x\\)`",
            "```tex\n\\[x\\]\n```",
            "~~~\n$$x$$\n~~~",
            "    \\(x\\)",
            r"\(unfinished",
            r"\(\)",
        ] {
            assert!(!contains_math(source), "{source}");
        }
        assert!(contains_math("```\n$$code$$\n```\n\\(real\\)"));
    }
    #[test]
    fn document_pdf_math_wrapping_preserves_source_characters_and_unicode() {
        let source = r"  \(\text{A  B} + α_1 + \frac{a}{b}\)  ";
        let lines = wrap_source(source, 9);
        assert!(lines.iter().all(|line| line.chars().count() <= 9));
        assert_eq!(lines.concat(), source);
        assert_eq!(wrap_source("a\n\nb", 10), ["a", "", "b"]);
    }
}
