//! Shared mathematical-source admission. LaTeX is data, never executable TeX.

pub(crate) const MAX_LATEX_BYTES: usize = 16 * 1024;
const MAX_MATH_FRAGMENTS: usize = 4096;

pub(crate) fn validate_latex(source: &str) -> Result<(), String> {
    if source.trim().is_empty() || source.len() > MAX_LATEX_BYTES {
        return Err("A formula must contain between 1 and 16,384 UTF-8 bytes.".into());
    }
    if source
        .chars()
        .any(|c| c.is_control() && !matches!(c, '\n' | '\t' | '\r'))
    {
        return Err("A formula contains invalid control characters.".into());
    }
    Ok(())
}

#[derive(Debug, PartialEq)]
pub(crate) enum MathFragment<'a> {
    Text(&'a str),
    Formula { latex: &'a str, block: bool },
}

#[derive(Debug)]
pub(crate) struct DelimitedMath<'a> {
    pub fragments: Vec<MathFragment<'a>>,
    pub incomplete: bool,
    pub legacy_latex: bool,
}

/// Recognize explicit MathJax and legacy Anki math delimiters in visible text.
/// Callers own HTML/Markdown parsing and must not pass code or hidden content.
/// Single-dollar currency is deliberately not a delimiter. No TeX program runs,
/// source is not rewritten, and successful admission is not semantic validation.
pub(crate) fn split_delimited_latex(input: &str) -> Result<DelimitedMath<'_>, String> {
    let mut result = DelimitedMath {
        fragments: Vec::new(),
        incomplete: false,
        legacy_latex: false,
    };
    let mut position = 0;
    let mut text_start = 0;
    while position < input.len() {
        let remaining = &input[position..];
        let delimiter = [
            ("\\(", "\\)", false),
            ("\\[", "\\]", true),
            ("[$$]", "[/$$]", true),
            ("[$]", "[/$]", false),
            ("[latex]", "[/latex]", true),
        ]
        .into_iter()
        .find(|(open, _, _)| remaining.starts_with(open));
        let Some((open, close, block)) = delimiter.filter(|_| !escaped(input, position)) else {
            position += remaining
                .chars()
                .next()
                .expect("nonempty suffix")
                .len_utf8();
            continue;
        };
        let legacy = open == "[latex]";
        result.legacy_latex |= legacy;
        let source_start = position + open.len();
        let close_start = input[source_start..]
            .match_indices(close)
            .map(|(offset, _)| source_start + offset)
            .find(|offset| !escaped(input, *offset));
        let Some(close_start) = close_start else {
            // Do not repeatedly rescan an unmatched suffix for nested openings.
            // Retain it literally and report it, keeping adversarial work linear.
            result.incomplete = true;
            break;
        };
        let end = close_start + close.len();
        if !legacy {
            let latex = &input[source_start..close_start];
            validate_latex(latex)?;
            if text_start < position {
                result
                    .fragments
                    .push(MathFragment::Text(&input[text_start..position]));
            }
            result
                .fragments
                .push(MathFragment::Formula { latex, block });
            text_start = end;
            if result.fragments.len() >= MAX_MATH_FRAGMENTS {
                return Err("Mathematical text contains too many fragments.".into());
            }
        }
        // [latex] can contain a document preamble, packages or executable TeX.
        // Preserve the entire region as literal source, including inner math.
        position = end;
    }
    if text_start < input.len() {
        result
            .fragments
            .push(MathFragment::Text(&input[text_start..]));
    }
    Ok(result)
}

fn escaped(input: &str, position: usize) -> bool {
    input.as_bytes()[..position]
        .iter()
        .rev()
        .take_while(|byte| **byte == b'\\')
        .count()
        % 2
        == 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enforces_bytes_not_platform_specific_string_length() {
        assert!(validate_latex(&"α".repeat(MAX_LATEX_BYTES / 2)).is_ok());
        assert!(validate_latex(&"α".repeat(MAX_LATEX_BYTES / 2 + 1)).is_err());
        assert!(validate_latex("x\0y").is_err());
        assert!(validate_latex("   ").is_err());
        assert!(validate_latex("x\n+y").is_ok());
    }

    #[test]
    fn splits_explicit_math_without_changing_source_or_order() {
        let parsed = split_delimited_latex(r"α \(x<y\) then \[\frac{x}{2}\] ω").unwrap();
        assert_eq!(
            parsed.fragments,
            vec![
                MathFragment::Text("α "),
                MathFragment::Formula {
                    latex: "x<y",
                    block: false
                },
                MathFragment::Text(" then "),
                MathFragment::Formula {
                    latex: r"\frac{x}{2}",
                    block: true
                },
                MathFragment::Text(" ω"),
            ]
        );
        assert!(!parsed.incomplete);
        assert!(!parsed.legacy_latex);
    }

    #[test]
    fn recognizes_legacy_math_but_not_currency_or_arbitrary_latex_programs() {
        let parsed = split_delimited_latex(r"$5 and $10 [$]x[/$][$$]y[/$$]").unwrap();
        assert_eq!(
            parsed.fragments,
            vec![
                MathFragment::Text("$5 and $10 "),
                MathFragment::Formula {
                    latex: "x",
                    block: false
                },
                MathFragment::Formula {
                    latex: "y",
                    block: true
                },
            ]
        );
        let source = r"[latex]\documentclass{article}\(x\)[/latex]";
        let legacy = split_delimited_latex(source).unwrap();
        assert_eq!(legacy.fragments, vec![MathFragment::Text(source)]);
        assert!(legacy.legacy_latex);
        assert!(!legacy.incomplete);
    }

    #[test]
    fn leaves_escaped_openings_and_unfinished_math_literal() {
        for source in [r"\\(literal\\)", r"\\[literal\\]", "$x$"] {
            let parsed = split_delimited_latex(source).unwrap();
            assert_eq!(parsed.fragments, vec![MathFragment::Text(source)]);
            assert!(!parsed.incomplete);
        }
        let source = r"before \(unfinished \[also unfinished";
        let parsed = split_delimited_latex(source).unwrap();
        assert_eq!(parsed.fragments, vec![MathFragment::Text(source)]);
        assert!(parsed.incomplete);
        let parsed = split_delimited_latex(r"\(x\\)y\)").unwrap();
        assert_eq!(
            parsed.fragments,
            vec![MathFragment::Formula {
                latex: r"x\\)y",
                block: false,
            }]
        );
    }

    #[test]
    fn rejects_invalid_complete_sources_and_bounds_fragment_allocation() {
        for source in [
            "\\(\\)".to_string(),
            "\\(x\0y\\)".to_string(),
            format!("\\({}\\)", "α".repeat(MAX_LATEX_BYTES / 2 + 1)),
            r"\(x\)".repeat(MAX_MATH_FRAGMENTS),
        ] {
            assert!(split_delimited_latex(&source).is_err());
        }
        assert!(
            split_delimited_latex(&format!("\\({}\\)", "α".repeat(MAX_LATEX_BYTES / 2))).is_ok()
        );
    }
}
