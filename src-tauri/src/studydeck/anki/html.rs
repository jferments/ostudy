//! Conservative HTML-to-native-content conversion, not an HTML execution engine.
//!
//! Only application-owned JSON nodes and marks are emitted. Attributes are never
//! copied wholesale. Images are resolved by the package reader, never by a URL
//! loader. Unsupported active or hidden content fails the card closed.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Value};

const MAX_HTML_BYTES: usize = 256 * 1024;
const MAX_TOKENS: usize = 20_000;

#[derive(Debug)]
enum Token {
    Text(String),
    Tag {
        name: String,
        attributes: BTreeMap<String, String>,
        closing: bool,
    },
}

pub(super) struct Document {
    pub value: Value,
    pub warnings: BTreeSet<String>,
}

pub(super) fn document(
    html: &str,
    image: &mut impl FnMut(&str) -> Result<Value, String>,
) -> Result<Document, String> {
    if html.to_ascii_lowercase().contains("[sound:") {
        return Err("Audio/video cards require an unsupported media player.".into());
    }
    let mut blocks = Vec::new();
    let mut inline = Vec::new();
    let mut marks: Vec<(String, &'static str)> = Vec::new();
    let mut warnings = BTreeSet::new();
    let mut ignoring_style = false;
    for token in tokenize(html)? {
        match token {
            Token::Text(text) => {
                if ignoring_style {
                    continue;
                }
                let (text, unknown_entity) = decode_entities(&text);
                if unknown_entity {
                    warnings.insert("Some HTML entities were retained literally.".into());
                }
                if !text.is_empty() {
                    let mark_names: BTreeSet<_> = marks.iter().map(|(_, mark)| *mark).collect();
                    if mark_names.contains("subscript") && mark_names.contains("superscript") {
                        return Err(
                            "Overlapping subscript and superscript formatting is unsupported."
                                .into(),
                        );
                    }
                    let mut node = json!({"type": "text", "text": text});
                    if !mark_names.is_empty() {
                        node["marks"] = Value::Array(
                            mark_names
                                .iter()
                                .map(|name| json!({"type": name}))
                                .collect(),
                        );
                    }
                    inline.push(node);
                }
            }
            Token::Tag {
                name,
                attributes,
                closing,
            } => {
                if name == "style" {
                    ignoring_style = !closing;
                    warnings.insert("CSS styling was omitted; native formatting is used.".into());
                    continue;
                }
                if ignoring_style {
                    continue;
                }
                reject_active(&name, &attributes)?;
                if attributes.contains_key("style") || attributes.contains_key("class") {
                    warnings.insert("CSS styling was omitted; native formatting is used.".into());
                }
                if let Some(mark) = mark_type(&name) {
                    if closing {
                        if let Some(position) = marks.iter().rposition(|(tag, _)| tag == &name) {
                            marks.remove(position);
                        }
                    } else {
                        if marks.len() >= 64 {
                            return Err("HTML formatting is nested too deeply.".into());
                        }
                        marks.push((name, mark));
                    }
                    continue;
                }
                match name.as_str() {
                    "img" if !closing => {
                        flush(&mut blocks, &mut inline);
                        let source = attributes.get("src").ok_or("An image has no source.")?;
                        let mut node = image(source)?;
                        if let Some(alt) = attributes.get("alt") {
                            node["attrs"]["alt"] = Value::String(alt.chars().take(500).collect());
                        }
                        blocks.push(node);
                    }
                    "br" if !closing => inline.push(json!({"type": "hardBreak"})),
                    "hr" if !closing => {
                        flush(&mut blocks, &mut inline);
                        blocks.push(json!({"type": "horizontalRule"}));
                    }
                    "p" | "div" | "section" | "article" | "blockquote" | "pre" | "h1" | "h2"
                    | "h3" | "h4" | "h5" | "h6" | "li" | "tr" => {
                        flush(&mut blocks, &mut inline);
                        if !matches!(name.as_str(), "p" | "div") {
                            warnings.insert(
                                "Some HTML block structure was simplified to paragraphs.".into(),
                            );
                        }
                        if name == "li" && !closing {
                            inline.push(json!({"type": "text", "text": "• "}));
                        }
                    }
                    "td" | "th" => {
                        if closing {
                            inline.push(json!({"type": "text", "text": " | "}));
                        }
                        warnings.insert("Tables were flattened to readable text.".into());
                    }
                    "span" | "html" | "body" => {}
                    "a" => {
                        warnings.insert("Links were converted to their visible text.".into());
                    }
                    _ => {
                        warnings
                            .insert("Some HTML structure was simplified to native text.".into());
                    }
                }
            }
        }
    }
    flush(&mut blocks, &mut inline);
    if blocks.is_empty() {
        blocks.push(json!({"type": "paragraph", "content": []}));
    }
    Ok(Document {
        value: json!({"type": "doc", "content": blocks}),
        warnings,
    })
}

fn flush(blocks: &mut Vec<Value>, inline: &mut Vec<Value>) {
    if !inline.is_empty() {
        blocks.push(json!({"type": "paragraph", "content": std::mem::take(inline)}));
    }
}

fn mark_type(tag: &str) -> Option<&'static str> {
    match tag {
        "b" | "strong" => Some("bold"),
        "i" | "em" => Some("italic"),
        "u" => Some("underline"),
        "s" | "strike" | "del" => Some("strike"),
        "code" => Some("code"),
        "sub" => Some("subscript"),
        "sup" => Some("superscript"),
        _ => None,
    }
}

fn reject_active(name: &str, attributes: &BTreeMap<String, String>) -> Result<(), String> {
    if matches!(
        name,
        "script"
            | "iframe"
            | "object"
            | "embed"
            | "svg"
            | "math"
            | "audio"
            | "video"
            | "source"
            | "canvas"
            | "input"
            | "textarea"
            | "select"
            | "button"
            | "template"
            | "noscript"
    ) || attributes.keys().any(|key| key.starts_with("on"))
    {
        return Err(
            "Active, interactive, or unsupported media content cannot be converted safely.".into(),
        );
    }
    let style = attributes
        .get("style")
        .map(|value| {
            value
                .chars()
                .filter(|character| !character.is_whitespace())
                .collect::<String>()
                .to_ascii_lowercase()
        })
        .unwrap_or_default();
    if attributes.contains_key("hidden")
        || style.contains("display:none")
        || style.contains("visibility:hidden")
        || style.contains("opacity:0")
    {
        return Err(
            "Hidden template content requires styling that cannot be preserved safely.".into(),
        );
    }
    Ok(())
}

pub(super) fn plain_text(html: &str) -> Result<String, String> {
    let mut output = String::new();
    let mut style = false;
    for token in tokenize(html)? {
        match token {
            Token::Text(value) if !style => output.push_str(&decode_entities(&value).0),
            Token::Tag { name, closing, .. } => {
                if name == "style" {
                    style = !closing;
                } else if !style && matches!(name.as_str(), "br" | "p" | "div" | "li" | "tr") {
                    output.push('\n');
                }
            }
            _ => {}
        }
    }
    Ok(output)
}

fn tokenize(html: &str) -> Result<Vec<Token>, String> {
    if html.len() > MAX_HTML_BYTES {
        return Err("HTML exceeds the 256 KiB per-side limit.".into());
    }
    let mut result = Vec::new();
    let mut position = 0;
    while position < html.len() {
        if result.len() >= MAX_TOKENS {
            return Err("HTML contains too many elements.".into());
        }
        let remaining = &html[position..];
        if remaining.starts_with("<!--") {
            let end = remaining.find("-->").ok_or("Unclosed HTML comment.")?;
            position += end + 3;
            continue;
        }
        let is_tag = remaining
            .as_bytes()
            .get(1)
            .is_some_and(|byte| byte.is_ascii_alphabetic() || matches!(byte, b'/' | b'!' | b'?'));
        if !remaining.starts_with('<') || !is_tag {
            let length = if remaining.starts_with('<') {
                1
            } else {
                remaining.find('<').unwrap_or(remaining.len())
            };
            result.push(Token::Text(remaining[..length].to_string()));
            position += length;
            continue;
        }
        let mut quote = None;
        let mut end = None;
        for (index, byte) in remaining.bytes().enumerate().skip(1) {
            if index > 8192 {
                return Err("HTML tag exceeds the supported size.".into());
            }
            match (quote, byte) {
                (Some(current), value) if current == value => quote = None,
                (None, b'\'' | b'"') => quote = Some(byte),
                (None, b'>') => {
                    end = Some(index);
                    break;
                }
                _ => {}
            }
        }
        let end = end.ok_or("Unclosed HTML tag.")?;
        let raw = remaining[1..end].trim();
        if raw.starts_with('!') || raw.starts_with('?') {
            return Err("HTML declarations and processing instructions are unsupported.".into());
        }
        let closing = raw.starts_with('/');
        let raw = raw.trim_start_matches('/').trim_end_matches('/').trim();
        let name_end = raw.find(char::is_whitespace).unwrap_or(raw.len());
        let name = raw[..name_end].to_ascii_lowercase();
        if name.is_empty()
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        {
            return Err("Malformed HTML tag.".into());
        }
        let attributes = attributes(&raw[name_end..])?;
        if name == "script" {
            return Err("JavaScript-dependent cards are not imported.".into());
        }
        result.push(Token::Tag {
            name,
            attributes,
            closing,
        });
        position += end + 1;
    }
    Ok(result)
}

fn attributes(mut input: &str) -> Result<BTreeMap<String, String>, String> {
    let mut result = BTreeMap::new();
    while !input.trim().is_empty() {
        input = input.trim_start();
        let end = input
            .find(|character: char| character.is_whitespace() || character == '=')
            .unwrap_or(input.len());
        if end == 0 || result.len() >= 64 {
            return Err("Malformed or excessive HTML attributes.".into());
        }
        let name = input[..end].to_ascii_lowercase();
        input = input[end..].trim_start();
        let mut value = String::new();
        if let Some(rest) = input.strip_prefix('=') {
            input = rest.trim_start();
            if let Some(quote) = input.chars().next().filter(|c| matches!(c, '\'' | '"')) {
                let end = input[1..].find(quote).ok_or("Unclosed HTML attribute.")? + 1;
                value = decode_entities(&input[1..end]).0;
                input = &input[end + 1..];
            } else {
                let end = input.find(char::is_whitespace).unwrap_or(input.len());
                value = decode_entities(&input[..end]).0;
                input = &input[end..];
            }
        }
        if result.insert(name, value).is_some() {
            return Err("Duplicate HTML attributes are ambiguous.".into());
        }
    }
    Ok(result)
}

fn decode_entities(input: &str) -> (String, bool) {
    let mut output = String::new();
    let mut remaining = input;
    let mut unknown = false;
    while let Some(start) = remaining.find('&') {
        output.push_str(&remaining[..start]);
        let entity = &remaining[start + 1..];
        let Some(end) = entity.find(';').filter(|end| *end <= 32) else {
            output.push('&');
            remaining = entity;
            continue;
        };
        let name = &entity[..end];
        let character =
            if let Some(hex) = name.strip_prefix("#x").or_else(|| name.strip_prefix("#X")) {
                u32::from_str_radix(hex, 16).ok().and_then(char::from_u32)
            } else if let Some(decimal) = name.strip_prefix('#') {
                decimal.parse::<u32>().ok().and_then(char::from_u32)
            } else {
                match name {
                    "amp" => Some('&'),
                    "lt" => Some('<'),
                    "gt" => Some('>'),
                    "quot" => Some('"'),
                    "apos" => Some('\''),
                    "nbsp" => Some(' '),
                    "ndash" => Some('–'),
                    "mdash" => Some('—'),
                    "hellip" => Some('…'),
                    "deg" => Some('°'),
                    "micro" => Some('µ'),
                    "times" => Some('×'),
                    "divide" => Some('÷'),
                    "plusmn" => Some('±'),
                    "le" => Some('≤'),
                    "ge" => Some('≥'),
                    "ne" => Some('≠'),
                    "rarr" => Some('→'),
                    "larr" => Some('←'),
                    "harr" => Some('↔'),
                    "bull" => Some('•'),
                    "middot" => Some('·'),
                    "alpha" => Some('α'),
                    "beta" => Some('β'),
                    "gamma" => Some('γ'),
                    "delta" => Some('δ'),
                    "Delta" => Some('Δ'),
                    "theta" => Some('θ'),
                    "lambda" => Some('λ'),
                    "mu" => Some('μ'),
                    "pi" => Some('π'),
                    "sigma" => Some('σ'),
                    "omega" => Some('ω'),
                    _ => None,
                }
            };
        if let Some(character) = character.filter(|c| !c.is_control() || matches!(c, '\n' | '\t')) {
            output.push(character);
        } else {
            unknown = true;
            output.push('&');
            output.push_str(name);
            output.push(';');
        }
        remaining = &entity[end + 1..];
    }
    output.push_str(remaining);
    (output, unknown)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_text_marks_entities_and_literal_comparison() {
        let result = document(
            "<div><b>Na&#43;</b> &amp; K+ < 4</div>",
            &mut |_| unreachable!(),
        )
        .unwrap();
        assert_eq!(result.value["content"][0]["content"][0]["text"], "Na+");
        assert_eq!(
            result.value["content"][0]["content"][0]["marks"][0]["type"],
            "bold"
        );
        crate::cards::validate_document(&result.value).unwrap();
        assert_eq!(
            plain_text("<div>A&nbsp;&lt; B</div>").unwrap().trim(),
            "A < B"
        );
    }

    #[test]
    fn never_passes_active_or_hidden_content_to_the_renderer() {
        for html in [
            "<ScRiPt>alert(1)</ScRiPt>",
            "<img src='x' onerror='x'>",
            "<span hidden>answer</span>",
            "<iframe src='https://example.com'></iframe>",
        ] {
            assert!(document(html, &mut |_| unreachable!()).is_err(), "{html}");
        }
    }

    #[test]
    fn resolves_images_as_block_nodes_without_copying_attributes() {
        let id = "0123456789abcdef0123456789abcdef";
        let result = document("Before<img src='a&amp;b.png' width='999999'>After", &mut |name| {
            assert_eq!(name, "a&b.png");
            Ok(json!({"type": "image", "attrs": {"assetId": id, "src": format!("study-asset://{id}")}}))
        }).unwrap();
        assert_eq!(result.value["content"][1]["type"], "image");
        assert!(result.value["content"][1]["attrs"].get("width").is_none());
        crate::cards::validate_document(&result.value).unwrap();
    }
}
