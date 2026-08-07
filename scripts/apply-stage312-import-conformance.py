#!/usr/bin/env python3
from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def write(path: str, text: str) -> None:
    (ROOT / path).write_text(text, encoding="utf-8")


path = "src-tauri/src/authoring/card_types_and_templates/generated_import/markdown.rs"
text = read(path)
old_preflight = '''    if source.contains("![") {
        return Err(unsupported_markdown(
            path,
            "images are not supported in version 1",
        ));
    }
    if contains_raw_html(source) {
        return Err(unsupported_markdown(path, "raw HTML is not supported"));
    }
    if source.lines().any(|line| {
        let trimmed = line.trim();
        trimmed.starts_with('|') && trimmed.ends_with('|')
    }) {
        return Err(unsupported_markdown(
            path,
            "Markdown tables are not supported in version 1",
        ));
    }

'''
if text.count(old_preflight) != 1:
    raise SystemExit("markdown.rs: global rejection preflight drifted")
text = text.replace(old_preflight, "", 1)

needle = '''        if line.trim_start().starts_with("```") {
            let language = line.trim_start().trim_start_matches("```").trim();'''
replacement = '''        if line.trim_start().starts_with("```") {
            let language = line.trim_start().trim_start_matches("```").trim();'''
if text.count(needle) != 1:
    raise SystemExit("markdown.rs: fenced-code parser drifted")
# The code block remains first. Immediately after its `continue`, later block
# branches run context-sensitive validation on non-code source lines.
marker = '''            }));
            continue;
        }
        if let Some(level) = heading_level(line) {'''
inserted = '''            }));
            continue;
        }
        if is_markdown_table_line(line) {
            return Err(unsupported_markdown(
                path,
                "Markdown tables are not supported in version 1",
            ));
        }
        if let Some(level) = heading_level(line) {'''
if text.count(marker) != 1:
    raise SystemExit("markdown.rs: fenced-code continuation drifted")
text = text.replace(marker, inserted, 1)

inline_start = '''fn inline_nodes(text: &str, path: &str) -> Result<Vec<Value>, CommandError> {
    let mut nodes = Vec::new();'''
inline_replacement = '''fn inline_nodes(text: &str, path: &str) -> Result<Vec<Value>, CommandError> {
    validate_inline_source(text, path)?;
    let mut nodes = Vec::new();'''
if text.count(inline_start) != 1:
    raise SystemExit("markdown.rs: inline parser start drifted")
text = text.replace(inline_start, inline_replacement, 1)

before_raw_html = '''fn contains_raw_html(source: &str) -> bool {'''
helpers = '''fn is_markdown_table_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with('|') && trimmed.ends_with('|')
}

/// Reject unsupported inline syntax only outside supported backtick code spans.
/// Code spans deliberately preserve source-looking text such as `<tag>`, image
/// markers, or pipe-delimited examples as literal code.
fn validate_inline_source(text: &str, path: &str) -> Result<(), CommandError> {
    let mut cursor = 0_usize;
    while cursor < text.len() {
        let remainder = &text[cursor..];
        if let Some(code) = remainder.strip_prefix('`') {
            if let Some(close) = code.find('`') {
                cursor += 1 + close + 1;
                continue;
            }
            // An unmatched backtick is not a valid code span. Treat it as
            // ordinary source and continue validating what follows.
            cursor += 1;
            continue;
        }
        let take = remainder.find('`').unwrap_or(remainder.len());
        let segment = &remainder[..take];
        if segment.contains("![") {
            return Err(unsupported_markdown(
                path,
                "images are not supported in version 1",
            ));
        }
        if contains_raw_html(segment) {
            return Err(unsupported_markdown(path, "raw HTML is not supported"));
        }
        cursor += take.max(1);
    }
    Ok(())
}

fn contains_raw_html(source: &str) -> bool {'''
if text.count(before_raw_html) != 1:
    raise SystemExit("markdown.rs: raw HTML helper drifted")
text = text.replace(before_raw_html, helpers, 1)
write(path, text)

# Attach the comprehensive corpus to ordinary Rust test discovery.
path = "src-tauri/src/authoring/card_types_and_templates/generated_import/mod.rs"
text = read(path)
needle = '''#[cfg(test)]
mod generated_import_tests {'''
replacement = '''#[cfg(test)]
mod conformance_tests;

#[cfg(test)]
mod generated_import_tests {'''
if text.count(needle) != 1:
    raise SystemExit("generated_import/mod.rs: test module anchor drifted")
text = text.replace(needle, replacement, 1)

# Extend the focused parser regression as a cheap local contract in addition to
# the file-level corpus.
test_anchor = '''    #[test]
    fn markdown_rejects_html_images_and_tables() {'''
new_test = '''    #[test]
    fn markdown_allows_unsupported_looking_text_inside_code_contexts() {
        let inline = markdown_document(
            "`<script>x</script> ![not-image](x) | A | B |`",
            "front",
        )
        .expect("inline code should preserve literal syntax-looking text");
        crate::cards::validate_document(&inline).expect("inline code should validate");

        let fenced = markdown_document(
            "```html\\n<script>x</script>\\n![not-image](x)\\n| A | B |\\n```",
            "front",
        )
        .expect("fenced code should preserve literal syntax-looking text");
        crate::cards::validate_document(&fenced).expect("fenced code should validate");
    }

    #[test]
    fn markdown_rejects_html_images_and_tables() {'''
if text.count(test_anchor) != 1:
    raise SystemExit("generated_import/mod.rs: parser test anchor drifted")
text = text.replace(test_anchor, new_test, 1)
write(path, text)

print("Applied Stage 3.12 generated-import parser and conformance wiring.")
