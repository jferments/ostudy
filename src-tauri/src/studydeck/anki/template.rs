//! A bounded, non-executable subset of Anki's field/template language.
//!
//! Substituted values are never parsed as templates. In particular, a field
//! containing `{{FrontSide}}` cannot trigger recursion or disclose other fields.

use std::collections::BTreeMap;

const MAX_BYTES: usize = 256 * 1024;
const MAX_DEPTH: usize = 32;
const MAX_TOKENS: usize = 4096;

#[derive(Debug)]
enum Node {
    Literal(String),
    Field(String),
    Section { name: String, inverted: bool, children: Vec<Node> },
}

pub(super) struct Rendered {
    pub html: String,
    pub matched_cloze: bool,
}

pub(super) fn render(
    source: &str,
    fields: &BTreeMap<String, String>,
    ordinal: u32,
    answer: bool,
    front: &str,
) -> Result<Rendered, String> {
    if source.len() > MAX_BYTES {
        return Err("Template exceeds the 256 KiB limit.".into());
    }
    let mut position = 0;
    let mut tokens = 0;
    let nodes = parse(source, &mut position, None, 0, &mut tokens)?;
    let mut rendered = Rendered { html: String::new(), matched_cloze: false };
    evaluate(&nodes, fields, ordinal, answer, front, &mut rendered)?;
    Ok(rendered)
}

fn parse(
    source: &str,
    position: &mut usize,
    closing: Option<&str>,
    depth: usize,
    tokens: &mut usize,
) -> Result<Vec<Node>, String> {
    if depth > MAX_DEPTH {
        return Err("Template conditionals are nested too deeply.".into());
    }
    let mut nodes = Vec::new();
    while *position < source.len() {
        *tokens += 1;
        if *tokens > MAX_TOKENS {
            return Err("Template contains too many replacements.".into());
        }
        let remaining = &source[*position..];
        let Some(offset) = remaining.find("{{") else {
            nodes.push(Node::Literal(remaining.to_string()));
            *position = source.len();
            break;
        };
        if offset > 0 {
            nodes.push(Node::Literal(remaining[..offset].to_string()));
        }
        *position += offset + 2;
        let end = source[*position..].find("}}")
            .ok_or("Template contains an unfinished replacement.")?;
        let token = source[*position..*position + end].trim();
        *position += end + 2;
        if let Some(name) = token.strip_prefix('/') {
            if closing == Some(name.trim()) {
                return Ok(nodes);
            }
            return Err("Template contains a mismatched conditional end.".into());
        }
        if token.starts_with('#') || token.starts_with('^') {
            let name = token[1..].trim();
            if name.is_empty() {
                return Err("Template conditional has no field name.".into());
            }
            let children = parse(source, position, Some(name), depth + 1, tokens)?;
            nodes.push(Node::Section {
                name: name.to_string(),
                inverted: token.starts_with('^'),
                children,
            });
        } else if !token.starts_with('!') {
            if token.is_empty() {
                return Err("Template contains an empty replacement.".into());
            }
            nodes.push(Node::Field(token.to_string()));
        }
    }
    if closing.is_some() {
        return Err("Template conditional is not closed.".into());
    }
    Ok(nodes)
}

fn append(output: &mut String, value: &str) -> Result<(), String> {
    if value.len() > MAX_BYTES.saturating_sub(output.len()) {
        return Err("Rendered card exceeds the 256 KiB limit.".into());
    }
    output.push_str(value);
    Ok(())
}

fn evaluate(
    nodes: &[Node],
    fields: &BTreeMap<String, String>,
    ordinal: u32,
    answer: bool,
    front: &str,
    result: &mut Rendered,
) -> Result<(), String> {
    for node in nodes {
        match node {
            Node::Literal(value) => append(&mut result.html, value)?,
            Node::Section { name, inverted, children } => {
                let value = fields.get(name)
                    .ok_or_else(|| format!("Unknown conditional field: {}.", label(name)))?;
                let present = !super::html::plain_text(value)?.trim().is_empty()
                    || value.to_ascii_lowercase().contains("<img")
                    || value.contains("[sound:");
                if present != *inverted {
                    evaluate(children, fields, ordinal, answer, front, result)?;
                }
            }
            Node::Field(token) => {
                if token == "FrontSide" {
                    if answer {
                        append(&mut result.html, front)?;
                    }
                    continue;
                }
                let parts: Vec<&str> = token.split(':').collect();
                let name = parts.last().copied().unwrap_or_default();
                let mut value = fields.get(name)
                    .ok_or_else(|| format!("Unknown template field: {}.", label(name)))?
                    .clone();
                for filter in parts[..parts.len() - 1].iter().rev() {
                    match *filter {
                        "text" => value = escape(&super::html::plain_text(&value)?),
                        "cloze" => {
                            let rendered = cloze(&value, ordinal + 1, answer)?;
                            value = rendered.html;
                            result.matched_cloze |= rendered.matched_cloze;
                        }
                        _ => return Err(format!("Unsupported Anki filter: {}.", label(filter))),
                    }
                }
                append(&mut result.html, &value)?;
            }
        }
    }
    Ok(())
}

fn cloze(source: &str, target: u32, answer: bool) -> Result<Rendered, String> {
    let mut result = Rendered { html: String::new(), matched_cloze: false };
    let mut remaining = source;
    while let Some(start) = remaining.find("{{c") {
        append(&mut result.html, &remaining[..start])?;
        let marker = &remaining[start + 3..];
        let split = marker.find("::").ok_or("Malformed cloze deletion.")?;
        let number = &marker[..split];
        if number.is_empty() || !number.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err("Malformed cloze ordinal.".into());
        }
        let number: u32 = number.parse().map_err(|_| "Invalid cloze ordinal.")?;
        if number == 0 || number > 999 {
            return Err("Cloze ordinals must be between 1 and 999.".into());
        }
        let body = &marker[split + 2..];
        let end = body.find("}}").ok_or("Unclosed cloze deletion.")?;
        let body_text = &body[..end];
        if body_text.contains("{{") {
            return Err("Nested cloze deletions are not supported by this importer.".into());
        }
        let (text, hint) = body_text.split_once("::").unwrap_or((body_text, ""));
        if number == target {
            result.matched_cloze = true;
            if answer {
                append(&mut result.html, "<strong>")?;
                append(&mut result.html, text)?;
                append(&mut result.html, "</strong>")?;
            } else {
                append(&mut result.html, "[")?;
                let hint = super::html::plain_text(hint)?;
                append(&mut result.html, &escape(if hint.trim().is_empty() { "..." } else { &hint }))?;
                append(&mut result.html, "]")?;
            }
        } else {
            append(&mut result.html, text)?;
        }
        remaining = &body[end + 2..];
    }
    append(&mut result.html, remaining)?;
    Ok(result)
}

pub(super) fn escape(value: &str) -> String {
    value.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
        .replace('"', "&quot;").replace('\'', "&#39;")
}

fn label(value: &str) -> String {
    value.chars().filter(|character| !character.is_control()).take(80).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields() -> BTreeMap<String, String> {
        BTreeMap::from([
            ("Front".into(), "Question".into()),
            ("Back".into(), "Answer".into()),
            ("Optional".into(), "".into()),
        ])
    }

    #[test]
    fn conditionals_front_side_and_nonrecursive_fields() {
        let mut fields = fields();
        fields.insert("Back".into(), "{{Front}}".into());
        let value = render("{{#Front}}{{^Optional}}{{FrontSide}}: {{Back}}{{/Optional}}{{/Front}}",
            &fields, 0, true, "Rendered question").unwrap();
        assert_eq!(value.html, "Rendered question: {{Front}}");
        assert!(render("{{#Front}}x{{/Back}}", &fields, 0, false, "").is_err());
        assert!(render("{{unknown:Front}}", &fields, 0, false, "").is_err());
    }

    #[test]
    fn cloze_selects_existing_card_ordinal_without_leaking_answer() {
        let fields = BTreeMap::from([("Text".into(),
            "{{c1::first::hint}} and {{c2::second}} and {{c1::third}}".into())]);
        let question = render("{{cloze:Text}}", &fields, 0, false, "").unwrap();
        assert_eq!(question.html, "[hint] and second and [...]");
        assert!(question.matched_cloze);
        let answer = render("{{cloze:Text}}", &fields, 0, true, "").unwrap();
        assert_eq!(answer.html, "<strong>first</strong> and second and <strong>third</strong>");
    }

    #[test]
    fn rejects_nested_cloze_and_bounds_expansion() {
        assert!(cloze("{{c1::outer {{c2::inner}}}}", 1, false).is_err());
        let fields = BTreeMap::from([("Front".into(), "a".repeat(MAX_BYTES))]);
        assert!(render("{{Front}}{{Front}}", &fields, 0, false, "").is_err());
    }
}
