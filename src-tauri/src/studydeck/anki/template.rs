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
    Section {
        name: String,
        inverted: bool,
        children: Vec<Node>,
    },
}

pub(super) struct Rendered {
    pub html: String,
    pub matched_cloze: bool,
    pub used_type_filter: bool,
}

pub(super) struct NativeCloze {
    pub source: String,
    pub question: String,
    pub answer: String,
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
    let mut rendered = Rendered {
        html: String::new(),
        matched_cloze: false,
        used_type_filter: false,
    };
    evaluate(&nodes, fields, ordinal, answer, front, &mut rendered)?;
    Ok(rendered)
}

/// Returns the field name only for the deliberately narrow cloze template
/// shape that maps cleanly to oStudy's native Fill in the blank note. More
/// complex Anki cloze templates retain their fully rendered content as Basic
/// cards instead of losing extra answer fields or media.
pub(super) fn simple_cloze_field(question_template: &str, answer_template: &str) -> Option<String> {
    let question = exact_filtered_field(question_template, "cloze")?;
    let answer = exact_filtered_field(answer_template, "cloze")?;
    (question == answer).then_some(question)
}

fn exact_filtered_field(source: &str, filter: &str) -> Option<String> {
    let source = source.trim();
    let token = source.strip_prefix("{{")?.strip_suffix("}}")?.trim();
    let (actual_filter, field) = token.split_once(':')?;
    (actual_filter.trim() == filter && !field.trim().is_empty() && !field.contains(':'))
        .then(|| field.trim().to_string())
}

/// Converts one Anki cloze card ordinal into one native oStudy cloze source.
/// Every matching deletion is normalized to c1 so the imported Anki card keeps
/// an independent oStudy schedule. Other Anki cloze groups are revealed text.
/// Hints are intentionally presentation metadata and are not folded into the
/// answer text.
pub(super) fn native_cloze(source_html: &str, target: u32) -> Result<Option<NativeCloze>, String> {
    let lowered = source_html.to_ascii_lowercase();
    if lowered.contains("<img") || lowered.contains("[sound:") {
        return Ok(None);
    }
    let source = super::html::plain_text(source_html)?;
    if source.len() > MAX_BYTES {
        return Err("Cloze field exceeds the 256 KiB limit.".into());
    }
    let mut normalized = String::new();
    let mut question = String::new();
    let mut complete = String::new();
    let mut answers = Vec::new();
    let mut matched = false;
    let mut remaining = source.as_str();
    while let Some(start) = remaining.find("{{c") {
        let prefix = &remaining[..start];
        if prefix.contains("{{") || prefix.contains("}}") {
            return Ok(None);
        }
        append(&mut normalized, prefix)?;
        append(&mut question, prefix)?;
        append(&mut complete, prefix)?;
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
        let (text, _) = body_text.split_once("::").unwrap_or((body_text, ""));
        let text = super::html::plain_text(text)?;
        if text.trim().is_empty() {
            return Err("Cloze deletion has no answer text.".into());
        }
        append(&mut complete, &text)?;
        if number == target {
            matched = true;
            answers.push(text.clone());
            append(&mut normalized, "{{c1::")?;
            append(&mut normalized, &text)?;
            append(&mut normalized, "}}")?;
            append(&mut question, "[…]")?;
        } else {
            append(&mut normalized, &text)?;
            append(&mut question, &text)?;
        }
        remaining = &body[end + 2..];
    }
    if remaining.contains("{{") || remaining.contains("}}") {
        return Ok(None);
    }
    append(&mut normalized, remaining)?;
    append(&mut question, remaining)?;
    append(&mut complete, remaining)?;
    if !matched {
        return Ok(None);
    }
    let answer = format!(
        "{}\n\nComplete statement: {}",
        answers.join(" · "),
        complete
    );
    Ok(Some(NativeCloze {
        source: normalized,
        question,
        answer,
    }))
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
        let end = source[*position..]
            .find("}}")
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
            Node::Section {
                name,
                inverted,
                children,
            } => {
                let value = fields
                    .get(name)
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
                let filters = &parts[..parts.len() - 1];
                let has_cloze_filter = filters.contains(&"cloze");
                let mut value = fields
                    .get(name)
                    .ok_or_else(|| format!("Unknown template field: {}.", label(name)))?
                    .clone();
                for filter in filters.iter().rev() {
                    match *filter {
                        "text" => value = escape(&super::html::plain_text(&value)?),
                        "cloze" => {
                            let rendered = cloze(&value, ordinal + 1, answer)?;
                            value = rendered.html;
                            result.matched_cloze |= rendered.matched_cloze;
                        }
                        "type" => {
                            result.used_type_filter = true;
                            // Anki's type: filter adds an interactive input on the
                            // question and a typed-answer comparison on the back.
                            // oStudy imports the study content, not that widget. A
                            // plain typed field is therefore hidden on the front
                            // and revealed normally on the back. type:cloze keeps
                            // the already-rendered cloze context on both sides.
                            if !answer && !has_cloze_filter {
                                value.clear();
                            }
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
    let mut result = Rendered {
        html: String::new(),
        matched_cloze: false,
        used_type_filter: false,
    };
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
                append(
                    &mut result.html,
                    &escape(if hint.trim().is_empty() { "..." } else { &hint }),
                )?;
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
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn label(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_control())
        .take(80)
        .collect()
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
        let value = render(
            "{{#Front}}{{^Optional}}{{FrontSide}}: {{Back}}{{/Optional}}{{/Front}}",
            &fields,
            0,
            true,
            "Rendered question",
        )
        .unwrap();
        assert_eq!(value.html, "Rendered question: {{Front}}");
        assert!(render("{{#Front}}x{{/Back}}", &fields, 0, false, "").is_err());
        assert!(render("{{unknown:Front}}", &fields, 0, false, "").is_err());
    }

    #[test]
    fn typed_answer_filter_preserves_content_without_importing_the_widget() {
        let fields = fields();
        let question = render("What? {{type:Back}}", &fields, 0, false, "").unwrap();
        assert_eq!(question.html, "What? ");
        assert!(question.used_type_filter);
        let answer = render("{{type:Back}}", &fields, 0, true, "").unwrap();
        assert_eq!(answer.html, "Answer");
        assert!(answer.used_type_filter);
    }

    #[test]
    fn cloze_selects_existing_card_ordinal_without_leaking_answer() {
        let fields = BTreeMap::from([(
            "Text".into(),
            "{{c1::first::hint}} and {{c2::second}} and {{c1::third}}".into(),
        )]);
        let question = render("{{cloze:Text}}", &fields, 0, false, "").unwrap();
        assert_eq!(question.html, "[hint] and second and [...]");
        assert!(question.matched_cloze);
        let answer = render("{{cloze:Text}}", &fields, 0, true, "").unwrap();
        assert_eq!(
            answer.html,
            "<strong>first</strong> and second and <strong>third</strong>"
        );
    }

    #[test]
    fn simple_cloze_normalizes_one_anki_ordinal_to_one_native_schedule() {
        assert_eq!(
            simple_cloze_field(" {{cloze:Text}} ", "{{cloze:Text}}"),
            Some("Text".to_string())
        );
        assert!(simple_cloze_field("{{cloze:Text}}", "{{cloze:Text}}<br>{{Extra}}").is_none());
        let native = native_cloze(
            "{{c1::first::hint}} then {{c2::second}} then {{c1::third}}",
            1,
        )
        .unwrap()
        .unwrap();
        assert_eq!(native.source, "{{c1::first}} then second then {{c1::third}}");
        assert_eq!(native.question, "[…] then second then […]");
        assert_eq!(
            native.answer,
            "first · third\n\nComplete statement: first then second then third"
        );
    }

    #[test]
    fn rejects_nested_cloze_and_bounds_expansion() {
        assert!(cloze("{{c1::outer {{c2::inner}}}}", 1, false).is_err());
        let fields = BTreeMap::from([("Front".into(), "a".repeat(MAX_BYTES))]);
        assert!(render("{{Front}}{{Front}}", &fields, 0, false, "").is_err());
    }
}
