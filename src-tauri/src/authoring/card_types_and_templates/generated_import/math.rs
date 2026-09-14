//! Shield mathematical source before the small authoring Markdown parser runs.
//! Placeholders are private conversion state, never a storage or renderer format.

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::{cards::math::validate_latex, error::CommandError};

const MAX_FORMULAS: usize = 64;
const MAX_TOTAL_LATEX_BYTES: usize = 32 * 1024;

pub(super) struct ProtectedMath {
    pub source: String,
    prefix: String,
    formulas: Vec<(String, bool)>,
}

impl ProtectedMath {
    pub fn new(source: &str, path: &str) -> Result<Self, CommandError> {
        // A fixed-width source-derived prefix keeps marker allocation bounded,
        // even for adversarial runs of private-use characters. Check for an exact
        // source collision rather than assuming that a digest cannot collide.
        let prefix = format!("\u{e000}{:x}\u{e000}", Sha256::digest(source.as_bytes()));
        if source.contains(&prefix) {
            return Err(invalid(path, "source conflicts with a conversion marker"));
        }
        let mut output = String::new();
        let mut formulas = Vec::new();
        let mut total_bytes = 0_usize;
        let mut cursor = 0;
        let mut copied = 0;
        while cursor < source.len() {
            let rest = &source[cursor..];
            if cursor == 0 || source.as_bytes()[cursor - 1] == b'\n' {
                let line_end = rest.find('\n').unwrap_or(rest.len());
                let line = &rest[..line_end];
                if line.trim_start().starts_with("```") {
                    // Match the existing parser's fenced-code boundary exactly.
                    cursor += line_end;
                    if cursor < source.len() {
                        cursor += 1;
                    }
                    while cursor < source.len() {
                        let line = &source[cursor..];
                        let end = line.find('\n').unwrap_or(line.len());
                        let closing = line[..end].trim_start().starts_with("```");
                        cursor += end;
                        if cursor < source.len() {
                            cursor += 1;
                        }
                        if closing {
                            break;
                        }
                    }
                    continue;
                }
                if line.starts_with("    ") || line.starts_with('\t') {
                    cursor += line_end;
                    if cursor < source.len() {
                        cursor += 1;
                    }
                    continue;
                }
            }
            if rest.starts_with('`') {
                let width = rest.bytes().take_while(|byte| *byte == b'`').count();
                let marker = &rest[..width];
                if let Some(end) = rest[width..].find(marker) {
                    cursor += width + end + width;
                    continue;
                }
            }
            if let Some(destination) = rest.strip_prefix("](") {
                // Destinations are not visible math; ordinary link validation owns them.
                if let Some(end) = destination.find(')') {
                    cursor += end + 3;
                    continue;
                }
            }
            let delimiter = [
                ("\\(", "\\)", false),
                ("\\[", "\\]", true),
                ("$$", "$$", true),
            ]
            .into_iter()
            .find(|(open, _, _)| rest.starts_with(open));
            if let Some((open, close, block)) = delimiter.filter(|_| !escaped(source, cursor)) {
                let start = cursor + open.len();
                let end = source[start..]
                    .match_indices(close)
                    .map(|(offset, _)| start + offset)
                    .find(|offset| !escaped(source, *offset))
                    .ok_or_else(|| invalid(path, "a formula delimiter is not closed"))?;
                let latex = &source[start..end];
                validate_latex(latex).map_err(|detail| invalid(path, &detail))?;
                total_bytes = total_bytes.saturating_add(latex.len());
                if formulas.len() >= MAX_FORMULAS || total_bytes > MAX_TOTAL_LATEX_BYTES {
                    return Err(invalid(
                        path,
                        "a card side exceeds 64 formulas or 32,768 formula-source bytes",
                    ));
                }
                output.push_str(&source[copied..cursor]);
                output.push_str(&format!("{prefix}{}\u{e001}", formulas.len()));
                formulas.push((latex.to_string(), block));
                cursor = end + close.len();
                copied = cursor;
                continue;
            }
            // cursor is always at a UTF-8 character boundary, including non-ASCII prose.
            cursor += rest.chars().next().map_or(0, char::len_utf8);
        }
        output.push_str(&source[copied..]);
        Ok(Self {
            source: output,
            prefix,
            formulas,
        })
    }

    pub fn restore(self, document: Value, path: &str) -> Result<Value, CommandError> {
        if self.formulas.is_empty() {
            return Ok(document);
        }
        let mut used = vec![0_usize; self.formulas.len()];
        let mut nodes = self.restore_node(document, path, 0, &mut used)?;
        if nodes.len() != 1 || used.iter().any(|count| *count != 1) {
            return Err(invalid(
                path,
                "mathematical source could not be preserved exactly",
            ));
        }
        let mut document = nodes.remove(0);
        document["attrs"] = json!({"schemaVersion": 3});
        Ok(document)
    }

    fn restore_node(
        &self,
        mut node: Value,
        path: &str,
        depth: usize,
        used: &mut [usize],
    ) -> Result<Vec<Value>, CommandError> {
        if depth > 32 {
            return Err(invalid(path, "mathematical Markdown is nested too deeply"));
        }
        if node.get("type").and_then(Value::as_str) == Some("text") {
            return self.restore_text(node, path, used);
        }
        if let Some(content) = node.get_mut("content").and_then(Value::as_array_mut) {
            let mut restored = Vec::new();
            for child in std::mem::take(content) {
                restored.extend(self.restore_node(child, path, depth + 1, used)?);
            }
            *content = restored;
        }
        if !matches!(
            node.get("type").and_then(Value::as_str),
            Some("paragraph" | "heading")
        ) {
            return Ok(vec![node]);
        }
        let Some(content) = node.get("content").and_then(Value::as_array) else {
            return Ok(vec![node]);
        };
        if !content.iter().any(is_block_math) {
            return Ok(vec![node]);
        }
        // A display formula is a sibling of text blocks, never an invalid inline
        // child. Keep the surrounding heading/list/quote container and text order.
        let mut blocks = Vec::new();
        let mut inline = Vec::new();
        for child in content {
            if is_block_math(child) {
                push_text_block(&mut blocks, &mut inline, &node);
                blocks.push(child.clone());
            } else {
                inline.push(child.clone());
            }
        }
        push_text_block(&mut blocks, &mut inline, &node);
        Ok(blocks)
    }

    fn restore_text(
        &self,
        template: Value,
        path: &str,
        used: &mut [usize],
    ) -> Result<Vec<Value>, CommandError> {
        let text = template
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let mut rest = text;
        let mut result = Vec::new();
        while let Some(start) = rest.find(&self.prefix) {
            push_text(&mut result, &template, &rest[..start]);
            let token = &rest[start + self.prefix.len()..];
            let (index, tail) = token
                .split_once('\u{e001}')
                .ok_or_else(|| invalid(path, "a protected formula was split by Markdown"))?;
            let index = index
                .parse::<usize>()
                .map_err(|_| invalid(path, "invalid protected formula"))?;
            let (latex, block) = self
                .formulas
                .get(index)
                .ok_or_else(|| invalid(path, "unknown protected formula"))?;
            used[index] += 1;
            // Formatting and link marks belong to prose, not canonical math atoms.
            result.push(json!({"type": if *block {"blockMath"} else {"inlineMath"}, "attrs": {"latex": latex}}));
            rest = tail;
        }
        push_text(&mut result, &template, rest);
        Ok(result)
    }
}

fn push_text(nodes: &mut Vec<Value>, template: &Value, text: &str) {
    if !text.is_empty() {
        let mut node = template.clone();
        node["text"] = Value::String(text.to_string());
        nodes.push(node);
    }
}

fn push_text_block(blocks: &mut Vec<Value>, inline: &mut Vec<Value>, template: &Value) {
    if !inline.is_empty() {
        let mut node = template.clone();
        node["content"] = Value::Array(std::mem::take(inline));
        blocks.push(node);
    }
}

fn is_block_math(node: &Value) -> bool {
    node.get("type").and_then(Value::as_str) == Some("blockMath")
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

fn invalid(path: &str, detail: &str) -> CommandError {
    CommandError::new("invalid_import_math", format!("{path}: {detail}."))
}
