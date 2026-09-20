//! Mathematical segment detection and fallback accounting for PDF export.
//!
//! The WebView uses shared KaTeX. The native PDF renderer handles a bounded
//! presentation subset and preserves original LaTeX visibly when that subset
//! cannot represent an expression.

use super::{math_layout, EffectiveGuideExport, ExportBlock};

pub(super) const NOTICE: &str = "Math notation: some equations in this PDF are preserved as readable LaTeX source because their notation is outside oStudy's bounded native PDF math subset. Open this study guide in oStudy to view the full typeset math.";
const MAX_NATIVE_WIDTH_AT_TEN_POINTS: f64 = 380.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum MathSegment<'a> {
    Text(&'a str),
    Formula {
        latex: &'a str,
        source: &'a str,
        block: bool,
    },
}

#[derive(Clone, Copy)]
struct MathSpan {
    start: usize,
    end: usize,
    content_start: usize,
    content_end: usize,
    block: bool,
}

pub(super) fn contains_math(source: &str) -> bool {
    !math_spans(source).is_empty()
}

pub(super) fn segments(source: &str) -> Vec<MathSegment<'_>> {
    let spans = math_spans(source);
    if spans.is_empty() {
        return vec![MathSegment::Text(source)];
    }
    let mut result = Vec::with_capacity((spans.len() * 2) + 1);
    let mut cursor = 0_usize;
    for span in spans {
        if cursor < span.start {
            result.push(MathSegment::Text(&source[cursor..span.start]));
        }
        result.push(MathSegment::Formula {
            latex: &source[span.content_start..span.content_end],
            source: &source[span.start..span.end],
            block: span.block,
        });
        cursor = span.end;
    }
    if cursor < source.len() {
        result.push(MathSegment::Text(&source[cursor..]));
    }
    result
}

pub(super) fn requires_source_fallback(export: &EffectiveGuideExport) -> bool {
    if unsupported(&export.overview)
        || export.learning_objectives.iter().any(|value| unsupported(value))
        || export.key_concepts.iter().any(|value| unsupported(value))
        || export.guide_notes.iter().any(|value| unsupported(value))
    {
        return true;
    }
    export.sections.iter().any(|section| {
        unsupported(&section.title)
            || section.notes.iter().any(|value| unsupported(value))
            || section.figures.iter().any(|figure| {
                unsupported(&figure.title) || unsupported(&figure.caption)
            })
            || section.body_lines.iter().any(|block| match block {
                ExportBlock::Heading(value) | ExportBlock::Paragraph(value) => unsupported(value),
                ExportBlock::Bullets(items) | ExportBlock::Numbered(items) => {
                    items.iter().any(|value| unsupported(value))
                }
                ExportBlock::Terms(items) => items
                    .iter()
                    .any(|(term, explanation)| unsupported(term) || unsupported(explanation)),
                ExportBlock::Callout { label, title, body } => {
                    contains_math(label) || contains_math(title) || contains_math(body)
                }
                ExportBlock::Table {
                    title,
                    columns,
                    rows,
                    caption,
                } => {
                    unsupported(title)
                        || columns.iter().any(|value| contains_math(value))
                        || rows.iter().flatten().any(|value| contains_math(value))
                        || unsupported(caption)
                }
                ExportBlock::WorkedExample {
                    title,
                    prompt,
                    steps,
                    answer,
                    takeaway,
                } => {
                    unsupported(title)
                        || unsupported(prompt)
                        || steps.iter().any(|value| unsupported(value))
                        || unsupported(answer)
                        || unsupported(takeaway)
                }
                ExportBlock::RetrievalCheck { prompt, answer } => {
                    unsupported(prompt) || unsupported(answer)
                }
                ExportBlock::Spacer => false,
            })
    })
}

pub(super) fn native_layout(source: &str, size: f64) -> Option<math_layout::MathLayout> {
    let reference = math_layout::layout(source, 10.0).ok()?;
    if reference.width > MAX_NATIVE_WIDTH_AT_TEN_POINTS {
        return None;
    }
    math_layout::layout(source, size).ok()
}

fn unsupported(source: &str) -> bool {
    segments(source).into_iter().any(|segment| match segment {
        MathSegment::Formula { latex, .. } => native_layout(latex, 10.0).is_none(),
        MathSegment::Text(_) => false,
    })
}

fn math_spans(source: &str) -> Vec<MathSpan> {
    let mut spans = Vec::new();
    let mut cursor = 0_usize;
    let mut fence: Option<(u8, usize)> = None;
    while cursor < source.len() {
        let rest = &source[cursor..];
        if cursor == 0 || source.as_bytes()[cursor - 1] == b'\n' {
            let end = rest.find('\n').unwrap_or(rest.len());
            let line = &rest[..end];
            let trimmed = line.trim_start_matches(' ');
            let indent = line.len() - trimmed.len();
            let marker = trimmed.as_bytes().first().copied().unwrap_or_default();
            let width = trimmed.bytes().take_while(|byte| *byte == marker).count();
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
            let width = rest.bytes().take_while(|byte| *byte == b'`').count();
            if let Some(end) = rest[width..].find(&rest[..width]) {
                cursor += width + end + width;
                continue;
            }
        }

        let opening = if rest.starts_with("\\(") {
            Some(("\\)", 2_usize, false))
        } else if rest.starts_with("\\[") {
            Some(("\\]", 2_usize, true))
        } else if rest.starts_with("$$") {
            Some(("$$", 2_usize, true))
        } else if rest.starts_with('$') {
            Some(("$", 1_usize, false))
        } else {
            None
        };
        if let Some((close, open_len, block)) = opening.filter(|_| !escaped(source, cursor)) {
            let content_start = cursor + open_len;
            let close_position = source[content_start..]
                .match_indices(close)
                .map(|(offset, _)| content_start + offset)
                .find(|end| {
                    !escaped(source, *end)
                        && (close != "$" || !source[*end..].starts_with("$$"))
                });
            if let Some(content_end) = close_position {
                let content = &source[content_start..content_end];
                let source_end = content_end + close.len();
                let single_dollar_safe = open_len != 1
                    || (!content.contains('\n')
                        && !content.is_empty()
                        && content.trim() == content);
                if !content.trim().is_empty() && single_dollar_safe {
                    spans.push(MathSpan {
                        start: cursor,
                        end: source_end,
                        content_start,
                        content_end,
                        block,
                    });
                }
                cursor = source_end;
                continue;
            }
            if open_len != 1 {
                break;
            }
        }
        cursor += rest.chars().next().map_or(0, char::len_utf8);
    }
    spans
}

fn escaped(source: &str, position: usize) -> bool {
    source.as_bytes()[..position]
        .iter()
        .rev()
        .take_while(|byte| **byte == b'\\')
        .count()
        % 2
        == 1
}

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
    fn math_detection_keeps_currency_code_and_escapes_literal() {
        for source in [
            r"\(x\)",
            r"\[x\]",
            "$$x$$",
            "$x$",
            "**before \\(x_i\\) after**",
        ] {
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
    fn segments_preserve_source_around_inline_and_block_math() {
        let source = "before \\(x_1\\) after\n\\[\\frac{a}{b}\\]";
        let parsed = segments(source);
        assert_eq!(parsed.len(), 4);
        assert_eq!(parsed[0], MathSegment::Text("before "));
        assert_eq!(
            parsed[1],
            MathSegment::Formula {
                latex: "x_1",
                source: "\\(x_1\\)",
                block: false,
            }
        );
        assert_eq!(parsed[2], MathSegment::Text(" after\n"));
        assert_eq!(
            parsed[3],
            MathSegment::Formula {
                latex: "\\frac{a}{b}",
                source: "\\[\\frac{a}{b}\\]",
                block: true,
            }
        );
    }

    #[test]
    fn source_wrapping_preserves_characters_and_unicode() {
        let source = r"  \(\text{A  B} + α_1 + \frac{a}{b}\)  ";
        let lines = wrap_source(source, 9);
        assert!(lines.iter().all(|line| line.chars().count() <= 9));
        assert_eq!(lines.concat(), source);
        assert_eq!(wrap_source("a\n\nb", 10), ["a", "", "b"]);
    }
}
