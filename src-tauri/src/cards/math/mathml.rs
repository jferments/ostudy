//! Bounded Presentation MathML input, normalized to the existing LaTeX atoms.
//! XML/TeX source is data. No DTD, external entity, URL, script, or donor HTML
//! reaches a renderer. Reject unsupported structure instead of flattening it.

use roxmltree::{Document, Node, ParsingOptions};

use super::validate_latex;

pub(crate) const MAX_MATHML_BYTES: usize = 64 * 1024;
const MAX_NODES: u32 = 2048;
const MAX_DEPTH: usize = 32;
const MAX_OUTPUT: usize = super::MAX_LATEX_BYTES;
const MATH_NS: &str = "http://www.w3.org/1998/Math/MathML";

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Formula {
    pub latex: String,
    pub block: bool,
}

impl Formula {
    pub fn delimited(&self) -> String {
        if self.block {
            format!("\\[{}\\]", self.latex)
        } else {
            format!("\\({}\\)", self.latex)
        }
    }
}

pub(crate) fn parse(source: &str) -> Result<Formula, String> {
    if source.len() > MAX_MATHML_BYTES || source.trim().is_empty() || source.contains("<!DOCTYPE") {
        return Err("MathML exceeds its 65,536-byte input limit or is empty.".into());
    }
    // roxmltree's defaults already disable DTD processing; set it explicitly.
    let document = Document::parse_with_options(
        source,
        ParsingOptions {
            allow_dtd: false,
            nodes_limit: MAX_NODES,
            ..ParsingOptions::default()
        },
    )
    .map_err(|_| "MathML is malformed or exceeds its XML node limit.".to_string())?;
    let root = document.root_element();
    if root.tag_name().name() != "math" {
        return Err("MathML must have a math root.".into());
    }
    if document.descendants().any(|node| node.is_pi()) {
        return Err("MathML processing instructions are unsupported.".into());
    }
    let latex = convert(root, 0)?;
    validate_latex(&latex)?;
    Ok(Formula {
        latex,
        block: root.attribute("display") == Some("block"),
    })
}

/// Recognize an XML math root, optionally using an explicitly declared prefix.
/// This is boundary detection only; the bounded XML parser is authoritative.
pub(crate) fn starts_mathml(source: &str) -> bool {
    root_name(source).is_some()
}

fn root_name(source: &str) -> Option<&str> {
    let rest = source.strip_prefix('<')?;
    let end = rest.find(|c: char| c.is_whitespace() || matches!(c, '>' | '/'))?;
    let name = &rest[..end];
    (name == "math"
        || name.strip_suffix(":math").is_some_and(|prefix| {
            !prefix.is_empty()
                && prefix
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
        }))
    .then_some(name)
}

/// Consume one complete MathML region before HTML/Markdown can flatten its tree.
/// Comments/CDATA and quoted attributes cannot forge a closing-root boundary.
pub(crate) fn prefix_len(source: &str) -> Result<usize, String> {
    let root = root_name(source).ok_or("Expected a MathML root.")?;
    let mut cursor = 0;
    let mut depth = 0_usize;
    while cursor < source.len().min(MAX_MATHML_BYTES) {
        let rest = &source[cursor..];
        if rest.starts_with("<!--") || rest.starts_with("<![CDATA[") {
            let close = if rest.starts_with("<!--") {
                "-->"
            } else {
                "]]>"
            };
            let end = rest
                .find(close)
                .ok_or("Unclosed MathML comment or character data.")?;
            cursor += end + close.len();
            continue;
        }
        if !rest.starts_with('<') {
            cursor += rest.find('<').unwrap_or(rest.len());
            continue;
        }
        let mut quote = None;
        let mut end = None;
        for (offset, byte) in rest.bytes().enumerate().skip(1) {
            if cursor + offset >= MAX_MATHML_BYTES {
                break;
            }
            match (quote, byte) {
                (Some(current), value) if current == value => quote = None,
                (None, b'\'' | b'"') => quote = Some(byte),
                (None, b'>') => {
                    end = Some(offset);
                    break;
                }
                _ => {}
            }
        }
        let end = end.ok_or("Unclosed or oversized MathML tag.")?;
        let tag = rest[1..end].trim();
        if tag.starts_with('!') || tag.starts_with('?') {
            return Err("MathML declarations and processing instructions are unsupported.".into());
        }
        let closing = tag.starts_with('/');
        let name = tag
            .trim_start_matches('/')
            .split(|c: char| c.is_whitespace() || c == '/')
            .next()
            .unwrap_or_default();
        if name == root {
            if closing {
                depth = depth.checked_sub(1).ok_or("Invalid MathML root nesting.")?;
            } else if !tag.ends_with('/') {
                depth += 1;
            }
            if depth == 0 {
                return Ok(cursor + end + 1);
            }
        }
        cursor += end + 1;
    }
    Err("MathML is unclosed or exceeds its 65,536-byte input limit.".into())
}

fn convert(node: Node<'_, '_>, depth: usize) -> Result<String, String> {
    if depth > MAX_DEPTH {
        return Err("MathML exceeds its nesting limit.".into());
    }
    if node.tag_name().namespace().is_some_and(|ns| ns != MATH_NS) {
        return Err("MathML contains a foreign namespace.".into());
    }
    let name = node.tag_name().name();
    if name == "math" && depth != 0 {
        return Err("Nested MathML roots are unsupported.".into());
    }
    for attribute in node.attributes() {
        if attribute.namespace().is_some() {
            return Err("Namespaced MathML attributes are unsupported.".into());
        }
        let permitted = match attribute.name() {
            "display" => name == "math" && matches!(attribute.value(), "inline" | "block"),
            "displaystyle" => {
                matches!(name, "math" | "mstyle") && matches!(attribute.value(), "true" | "false")
            }
            "mathvariant" => matches!(name, "mi" | "mn" | "mtext"),
            "open" | "close" | "separators" => name == "mfenced",
            "encoding" => name == "annotation",
            _ => false,
        };
        if !permitted {
            return Err("MathML contains an unsupported presentation or active attribute.".into());
        }
    }
    let children: Vec<_> = node.children().filter(Node::is_element).collect();
    let token = matches!(name, "mi" | "mn" | "mo" | "mtext" | "annotation");
    if node
        .children()
        .any(|child| child.is_text() && !child.text().unwrap_or_default().trim().is_empty())
        && !token
    {
        return Err("MathML layout contains unstructured text.".into());
    }
    let mut result = if token {
        if !children.is_empty() {
            return Err("MathML tokens cannot contain markup.".into());
        }
        let text = node
            .children()
            .filter(|child| child.is_text())
            .filter_map(|child| child.text())
            .collect::<String>();
        let text = text.trim();
        if name == "annotation" {
            return Err("MathML annotations are not presentation content.".into());
        }
        if text.is_empty() {
            return Err("MathML contains an empty token.".into());
        }
        let escaped = escape(text, name == "mtext")?;
        if name == "mtext" {
            format!("\\text{{{escaped}}}")
        } else if name == "mi"
            && text.chars().count() > 1
            && node.attribute("mathvariant").is_none()
        {
            format!("\\mathrm{{{escaped}}}")
        } else {
            escaped
        }
    } else {
        match name {
            "math" | "mrow" | "mstyle" | "msqrt" | "mtd" => {
                if children.is_empty() {
                    return Err("MathML contains an empty layout.".into());
                }
                let body = join(&children, depth, " ")?;
                if name == "msqrt" {
                    format!("\\sqrt{{{body}}}")
                } else {
                    body
                }
            }
            "mfrac" | "mroot" | "msub" | "msup" | "munder" | "mover" => {
                require_arity(&children, 2)?;
                let a = convert(children[0], depth + 1)?;
                let b = convert(children[1], depth + 1)?;
                match name {
                    "mfrac" => format!("\\frac{{{a}}}{{{b}}}"),
                    "mroot" => format!("\\sqrt[{{{b}}}]{{{a}}}"),
                    "msub" => format!("{{{a}}}_{{{b}}}"),
                    "msup" => format!("{{{a}}}^{{{b}}}"),
                    "munder" => format!("\\underset{{{b}}}{{{a}}}"),
                    _ => format!("\\overset{{{b}}}{{{a}}}"),
                }
            }
            "msubsup" | "munderover" => {
                require_arity(&children, 3)?;
                let a = convert(children[0], depth + 1)?;
                let b = convert(children[1], depth + 1)?;
                let c = convert(children[2], depth + 1)?;
                if name == "msubsup" {
                    format!("{{{a}}}_{{{b}}}^{{{c}}}")
                } else {
                    format!("\\overset{{{c}}}{{\\underset{{{b}}}{{{a}}}}}")
                }
            }
            "mfenced" => {
                let open = fence(node.attribute("open").unwrap_or("("))?;
                let close = fence(node.attribute("close").unwrap_or(")"))?;
                let separators = node
                    .attribute("separators")
                    .unwrap_or(",")
                    .chars()
                    .filter(|c| !c.is_whitespace())
                    .collect::<Vec<_>>();
                if separators.len() > 16
                    || separators.iter().any(|c| !matches!(c, ',' | ';' | '|'))
                {
                    return Err("MathML uses unsupported fence separators.".into());
                }
                let mut body = String::new();
                for (i, child) in children.iter().enumerate() {
                    if i > 0 && !separators.is_empty() {
                        body.push(separators[(i - 1).min(separators.len() - 1)]);
                    }
                    append(&mut body, &convert(*child, depth + 1)?)?;
                }
                format!("\\left{open} {body} \\right{close}")
            }
            "mtable" => {
                if children.is_empty() || children.len() > 32 {
                    return Err("MathML matrix has too many or no rows.".into());
                }
                let mut width = None;
                let mut rows = Vec::new();
                for row in &children {
                    if row.tag_name().name() != "mtr" {
                        return Err("MathML matrix contains a non-row child.".into());
                    }
                    let cells = row.children().filter(Node::is_element).count();
                    if cells == 0 || cells > 32 || width.is_some_and(|value| value != cells) {
                        return Err(
                            "MathML matrix rows must have equal, bounded column counts.".into()
                        );
                    }
                    width = Some(cells);
                    rows.push(convert(*row, depth + 1)?);
                }
                format!("\\begin{{matrix}}{}\\end{{matrix}}", rows.join(" \\\\ "))
            }
            "mtr" => {
                if node.parent_element().map(|p| p.tag_name().name()) != Some("mtable")
                    || children.iter().any(|c| c.tag_name().name() != "mtd")
                {
                    return Err("MathML matrix row/cell structure is invalid.".into());
                }
                join(&children, depth, " & ")?
            }
            "semantics" => {
                let first = children
                    .first()
                    .ok_or("MathML semantics has no presentation.")?;
                for annotation in children.iter().skip(1) {
                    if annotation.tag_name().name() != "annotation"
                        || annotation.tag_name().namespace() != node.tag_name().namespace()
                        || annotation.children().any(|c| c.is_element())
                        || annotation
                            .attributes()
                            .any(|a| a.namespace().is_some() || a.name() != "encoding")
                    {
                        return Err(
                            "MathML semantics contains unsupported alternate markup.".into()
                        );
                    }
                }
                convert(*first, depth + 1)?
            }
            _ => return Err("MathML uses an unsupported presentation element.".into()),
        }
    };
    if name == "mtd" && node.parent_element().map(|p| p.tag_name().name()) != Some("mtr") {
        return Err("MathML matrix cell is outside a row.".into());
    }
    if let Some(variant) = node.attribute("mathvariant") {
        let command = match variant {
            "normal" => "mathrm",
            "bold" => "mathbf",
            "italic" => "mathit",
            "double-struck" => "mathbb",
            "script" => "mathcal",
            "fraktur" => "mathfrak",
            "sans-serif" => "mathsf",
            "monospace" => "mathtt",
            _ => return Err("MathML uses an unsupported math variant.".into()),
        };
        if name == "mtext" && variant != "normal" {
            return Err("Styled MathML prose is unsupported.".into());
        }
        if name != "mtext" {
            result = format!("\\{command}{{{result}}}");
        }
    }
    if let Some(display) = node.attribute("displaystyle") {
        result = format!(
            "{{\\{} {result}}}",
            if display == "true" {
                "displaystyle"
            } else {
                "textstyle"
            }
        );
    }
    if result.len() > MAX_OUTPUT {
        return Err("Converted MathML exceeds its LaTeX byte limit.".into());
    }
    Ok(result)
}

fn require_arity(children: &[Node<'_, '_>], length: usize) -> Result<(), String> {
    if children.len() == length {
        Ok(())
    } else {
        Err("MathML has an incorrect number of arguments.".into())
    }
}

fn join(children: &[Node<'_, '_>], depth: usize, separator: &str) -> Result<String, String> {
    let mut output = String::new();
    for (i, child) in children.iter().enumerate() {
        if i > 0 {
            append(&mut output, separator)?;
        }
        append(&mut output, &convert(*child, depth + 1)?)?;
    }
    Ok(output)
}

fn append(output: &mut String, text: &str) -> Result<(), String> {
    if output.len().saturating_add(text.len()) > MAX_OUTPUT {
        return Err("Converted MathML exceeds its LaTeX byte limit.".into());
    }
    output.push_str(text);
    Ok(())
}

fn fence(value: &str) -> Result<&str, String> {
    match value {
        "" => Ok("."),
        "(" | ")" | "[" | "]" | "|" => Ok(value),
        "{" => Ok("\\{"),
        "}" => Ok("\\}"),
        "‖" => Ok("\\Vert"),
        "⟨" => Ok("\\langle"),
        "⟩" => Ok("\\rangle"),
        _ => Err("MathML uses an unsupported fence.".into()),
    }
}

fn escape(text: &str, prose: bool) -> Result<String, String> {
    let mut output = String::new();
    for c in text.chars() {
        let replacement = match c {
            '\\' => {
                if prose {
                    "\\textbackslash{}"
                } else {
                    "\\backslash{}"
                }
            }
            '{' => "\\{",
            '}' => "\\}",
            '&' => "\\&",
            '%' => "\\%",
            '#' => "\\#",
            '$' => "\\$",
            '_' => "\\_",
            '^' => "\\text{\\textasciicircum{}}",
            '~' => "\\text{\\textasciitilde{}}",
            '−' if !prose => "-",
            '×' if !prose => "\\times ",
            '÷' if !prose => "\\div ",
            '∑' if !prose => "\\sum ",
            '∏' if !prose => "\\prod ",
            '∫' if !prose => "\\int ",
            '∞' if !prose => "\\infty ",
            '≤' if !prose => "\\le ",
            '≥' if !prose => "\\ge ",
            '≠' if !prose => "\\ne ",
            '±' if !prose => "\\pm ",
            '≈' if !prose => "\\approx ",
            '\u{2062}' if !prose => "\\,",
            '\u{2061}' if !prose => "", // invisible multiplication/application
            _ => {
                append(&mut output, &c.to_string())?;
                continue;
            }
        };
        append(&mut output, replacement)?;
    }
    Ok(output)
}

#[cfg(test)]
mod tests;
