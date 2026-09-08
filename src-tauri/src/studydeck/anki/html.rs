//! Conservative HTML-to-native-content conversion, not an HTML execution engine.
//!
//! Only application-owned JSON nodes and marks are emitted. Attributes are never
//! copied wholesale. Packaged media are resolved by the APKG reader, never by a
//! URL loader. Executable or hidden donor content remains fail-closed.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Value};

#[path = "math.rs"]
mod math;

const MAX_HTML_BYTES: usize = 256 * 1024;
const MAX_TOKENS: usize = 20_000;

#[derive(Debug)]
enum Token {
    Text(String),
    ScriptOmitted,
    MathMl(String),
    Tag {
        name: String,
        attributes: BTreeMap<String, String>,
        closing: bool,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MediaKind {
    Audio,
    Video,
}

#[derive(Debug)]
struct MediaContainer {
    kind: MediaKind,
    resolved: bool,
    last_error: Option<String>,
}

pub(super) struct Document {
    pub value: Value,
    pub warnings: BTreeSet<String>,
    pub style_omitted: bool,
    pub script_omitted: bool,
}

pub(super) fn document(
    html: &str,
    asset: &mut impl FnMut(&str) -> Result<Value, String>,
) -> Result<Document, String> {
    let mut blocks = Vec::new();
    let mut inline = Vec::new();
    let mut marks: Vec<(String, &'static str)> = Vec::new();
    let mut warnings = BTreeSet::new();
    let mut ignoring_style = false;
    let mut style_omitted = false;
    let mut script_omitted = false;
    let mut media: Option<MediaContainer> = None;

    for token in tokenize(html)? {
        match token {
            Token::Text(text) => {
                if ignoring_style || media.is_some() {
                    continue;
                }
                let (text, unknown_entity) = decode_entities(&text);
                if unknown_entity {
                    warnings.insert("Some HTML entities were retained literally.".into());
                }
                append_text_math_and_sound(&text, &marks, &mut blocks, &mut inline, asset)?;
            }
            Token::ScriptOmitted => {
                script_omitted = true;
            }
            Token::MathMl(raw) => {
                if ignoring_style || media.is_some() {
                    continue;
                }
                let (latex, block) = math::mathml_to_latex(&raw)?;
                if block {
                    flush(&mut blocks, &mut inline);
                    blocks.push(math_node("blockMath", &latex));
                } else {
                    inline.push(math_node("inlineMath", &latex));
                }
            }
            Token::Tag {
                name,
                mut attributes,
                closing,
            } => {
                if name == "style" {
                    ignoring_style = !closing;
                    style_omitted = true;
                    continue;
                }
                if ignoring_style {
                    continue;
                }
                if attributes.keys().any(|key| key.starts_with("on")) {
                    attributes.retain(|key, _| !key.starts_with("on"));
                    script_omitted = true;
                }

                if matches!(name.as_str(), "audio" | "video") {
                    let kind = if name == "audio" {
                        MediaKind::Audio
                    } else {
                        MediaKind::Video
                    };
                    if closing {
                        let Some(container) = media.take() else {
                            return Err("A media element has an unmatched closing tag.".into());
                        };
                        if container.kind != kind {
                            return Err(
                                "Nested or mismatched media elements are unsupported.".into(),
                            );
                        }
                        if !container.resolved {
                            return Err(container.last_error.unwrap_or_else(|| {
                                "An audio/video element has no supported packaged source.".into()
                            }));
                        }
                    } else {
                        if media.is_some() {
                            return Err("Nested media elements are unsupported.".into());
                        }
                        let mut container = MediaContainer {
                            kind,
                            resolved: false,
                            last_error: None,
                        };
                        if let Some(source) = attributes.get("src") {
                            try_media_source(
                                source,
                                &mut container,
                                &mut blocks,
                                &mut inline,
                                asset,
                            );
                        }
                        media = Some(container);
                    }
                    continue;
                }

                if name == "source" {
                    if closing {
                        continue;
                    }
                    let Some(container) = media.as_mut() else {
                        return Err("A media source appears outside an audio/video element.".into());
                    };
                    if !container.resolved {
                        let source = attributes
                            .get("src")
                            .ok_or("A media source has no src attribute.")?;
                        try_media_source(source, container, &mut blocks, &mut inline, asset);
                    }
                    continue;
                }

                if media.is_some() {
                    if name == "track" || name == "span" || name == "div" || name == "p" {
                        continue;
                    }
                    return Err("Unsupported markup appears inside an audio/video element.".into());
                }

                reject_active(&name, &attributes)?;
                if attributes.contains_key("style") || attributes.contains_key("class") {
                    style_omitted = true;
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
                        let mut node = asset(source)?;
                        if node.get("type").and_then(Value::as_str) != Some("image") {
                            return Err(
                                "An <img> reference does not point to a supported image.".into(),
                            );
                        }
                        if let Some(alt) = attributes.get("alt") {
                            node["attrs"]["alt"] = Value::String(alt.chars().take(500).collect());
                        }
                        blocks.push(node);
                    }
                    "br" if !closing => inline.push(json!({"type": "hardBreak"})),
                    "hr" if !closing => {
                        let leading_answer_separator = blocks.is_empty()
                            && inline.is_empty()
                            && attributes
                                .get("id")
                                .is_some_and(|value| value.eq_ignore_ascii_case("answer"));
                        if !leading_answer_separator {
                            flush(&mut blocks, &mut inline);
                            blocks.push(json!({"type": "horizontalRule"}));
                        }
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
                    "span" | "html" | "body" | "track" => {}
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
    if let Some(container) = media {
        if !container.resolved {
            return Err(container.last_error.unwrap_or_else(|| {
                "An audio/video element is not closed or has no supported packaged source.".into()
            }));
        }
        return Err("An audio/video element is not closed.".into());
    }
    flush(&mut blocks, &mut inline);
    if blocks.is_empty() {
        blocks.push(json!({"type": "paragraph", "content": []}));
    }
    Ok(Document {
        value: json!({"type": "doc", "attrs": {"schemaVersion": 3}, "content": blocks}),
        warnings,
        style_omitted,
        script_omitted,
    })
}

fn append_text_math_and_sound(
    text: &str,
    marks: &[(String, &'static str)],
    blocks: &mut Vec<Value>,
    inline: &mut Vec<Value>,
    asset: &mut impl FnMut(&str) -> Result<Value, String>,
) -> Result<(), String> {
    for span in math::split_math_text(text)? {
        match span {
            math::MathSpan::Text(value) => {
                append_text_and_sound(&value, marks, blocks, inline, asset)?;
            }
            math::MathSpan::Inline(latex) => {
                inline.push(math_node("inlineMath", &latex));
            }
            math::MathSpan::Block(latex) => {
                flush(blocks, inline);
                blocks.push(math_node("blockMath", &latex));
            }
        }
    }
    Ok(())
}

fn append_text_and_sound(
    text: &str,
    marks: &[(String, &'static str)],
    blocks: &mut Vec<Value>,
    inline: &mut Vec<Value>,
    asset: &mut impl FnMut(&str) -> Result<Value, String>,
) -> Result<(), String> {
    let mut remaining = text;
    while let Some(start) = remaining.find("[sound:") {
        append_text_node(&remaining[..start], marks, inline)?;
        let after = &remaining[start + "[sound:".len()..];
        let end = after
            .find(']')
            .ok_or("An Anki [sound:] reference is not closed.")?;
        let filename = after[..end].trim();
        if filename.is_empty() {
            return Err("An Anki [sound:] reference has no filename.".into());
        }
        let node = asset(filename)?;
        let node_type = node.get("type").and_then(Value::as_str).unwrap_or_default();
        if !matches!(node_type, "audio" | "video") {
            return Err("An Anki [sound:] reference does not point to playable media.".into());
        }
        flush(blocks, inline);
        blocks.push(node);
        remaining = &after[end + 1..];
    }
    append_text_node(remaining, marks, inline)
}

fn append_text_node(
    text: &str,
    marks: &[(String, &'static str)],
    inline: &mut Vec<Value>,
) -> Result<(), String> {
    if text.is_empty() {
        return Ok(());
    }
    let mark_names: BTreeSet<_> = marks.iter().map(|(_, mark)| *mark).collect();
    if mark_names.contains("subscript") && mark_names.contains("superscript") {
        return Err("Overlapping subscript and superscript formatting is unsupported.".into());
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
    Ok(())
}

fn math_node(node_type: &str, latex: &str) -> Value {
    json!({"type": node_type, "attrs": {"latex": latex}})
}

fn try_media_source(
    source: &str,
    container: &mut MediaContainer,
    blocks: &mut Vec<Value>,
    inline: &mut Vec<Value>,
    asset: &mut impl FnMut(&str) -> Result<Value, String>,
) {
    match asset(source) {
        Ok(node) => {
            let node_type = node.get("type").and_then(Value::as_str).unwrap_or_default();
            let expected = match container.kind {
                MediaKind::Audio => "audio",
                MediaKind::Video => "video",
            };
            if node_type == expected {
                flush(blocks, inline);
                blocks.push(node);
                container.resolved = true;
            } else {
                container.last_error = Some(format!(
                    "An <{expected}> source does not point to supported {expected} media."
                ));
            }
        }
        Err(error) => container.last_error = Some(error),
    }
}

pub(super) fn has_study_content(value: &Value) -> bool {
    match value {
        Value::Array(values) => values.iter().any(has_study_content),
        Value::Object(object) => match object.get("type").and_then(Value::as_str) {
            Some("text") => object
                .get("text")
                .and_then(Value::as_str)
                .is_some_and(|text| !text.trim().is_empty()),
            Some("image" | "audio" | "video") => true,
            Some("inlineMath" | "blockMath") => object
                .get("attrs")
                .and_then(Value::as_object)
                .and_then(|attrs| attrs.get("latex"))
                .and_then(Value::as_str)
                .is_some_and(|latex| !latex.trim().is_empty()),
            _ => object.get("content").is_some_and(has_study_content),
        },
        _ => false,
    }
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
            | "canvas"
            | "input"
            | "textarea"
            | "select"
            | "button"
            | "template"
            | "noscript"
    ) {
        return Err(
            "Active, interactive, or unsupported content cannot be converted safely.".into(),
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
            Token::MathMl(value) if !style => {
                if let Ok((latex, _)) = math::mathml_to_latex(&value) {
                    output.push_str(&latex);
                }
            }
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
        let end = find_tag_end(remaining)?;
        let raw_tag = remaining[1..end].trim();
        if raw_tag.starts_with('!') || raw_tag.starts_with('?') {
            return Err("HTML declarations and processing instructions are unsupported.".into());
        }
        let closing = raw_tag.starts_with('/');
        let self_closing = raw_tag.ends_with('/');
        let raw = raw_tag.trim_start_matches('/').trim_end_matches('/').trim();
        let name_end = raw.find(char::is_whitespace).unwrap_or(raw.len());
        let name = raw[..name_end].to_ascii_lowercase();
        if name.is_empty()
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b':'))
        {
            return Err("Malformed HTML tag.".into());
        }
        let attributes = attributes(&raw[name_end..])?;
        if name == "script" {
            if closing || self_closing {
                result.push(Token::ScriptOmitted);
                position += end + 1;
                continue;
            }
            let after_open = &remaining[end + 1..];
            let close_start = find_ascii_case_insensitive(after_open, "</script")
                .ok_or("Unclosed HTML script element.")?;
            let close_tail = &after_open[close_start..];
            let close_end = close_tail
                .find('>')
                .ok_or("Unclosed HTML script element.")?;
            result.push(Token::ScriptOmitted);
            position += end + 1 + close_start + close_end + 1;
            continue;
        }
        if !closing && local_name(&name) == "math" {
            if self_closing {
                return Err("An empty MathML element cannot be converted.".into());
            }
            let close = format!("</{name}>");
            let after_open = &remaining[end + 1..];
            let close_start = find_ascii_case_insensitive(after_open, &close)
                .ok_or("Unclosed MathML element.")?;
            let total = end + 1 + close_start + close.len();
            result.push(Token::MathMl(remaining[..total].to_string()));
            position += total;
            continue;
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

fn find_tag_end(input: &str) -> Result<usize, String> {
    let mut quote = None;
    for (index, byte) in input.bytes().enumerate().skip(1) {
        if index > 8192 {
            return Err("HTML tag exceeds the supported size.".into());
        }
        match (quote, byte) {
            (Some(current), value) if current == value => quote = None,
            (None, b'\'' | b'"') => quote = Some(byte),
            (None, b'>') => return Ok(index),
            _ => {}
        }
    }
    Err("Unclosed HTML tag.".into())
}

fn find_ascii_case_insensitive(haystack: &str, needle: &str) -> Option<usize> {
    let needle = needle.as_bytes();
    haystack
        .as_bytes()
        .windows(needle.len())
        .position(|window| window.eq_ignore_ascii_case(needle))
}

fn local_name(value: &str) -> &str {
    value.rsplit(':').next().unwrap_or(value)
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
    }

    #[test]
    fn converts_mathjax_mathml_and_packaged_sound_to_native_nodes() {
        let asset_id = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let result = document(
            "Energy \\(E=mc^2\\)<math display='block'><mfrac><mn>1</mn><mn>2</mn></mfrac></math>[sound:voice.mp3]",
            &mut |name| {
                assert_eq!(name, "voice.mp3");
                Ok(json!({"type":"audio","attrs":{"assetId":asset_id,"src":format!("study-asset://{asset_id}"),"title":"voice.mp3"}}))
            },
        )
        .unwrap();
        assert!(
            result.value["content"]
                .as_array()
                .unwrap()
                .iter()
                .any(|node| node["type"] == "blockMath")
        );
        assert!(
            result.value["content"]
                .as_array()
                .unwrap()
                .iter()
                .any(|node| node["type"] == "audio")
        );
        crate::cards::validate_document(&result.value).unwrap();
    }

    #[test]
    fn converts_direct_and_source_child_media_without_imported_events() {
        let audio_id = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let video_id = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
        let result = document(
            "<audio src='voice.mp3' autoplay onclick='bad()'></audio><video><source src='clip.mp4'></video>",
            &mut |name| match name {
                "voice.mp3" => Ok(json!({"type":"audio","attrs":{"assetId":audio_id,"src":format!("study-asset://{audio_id}"),"title":name}})),
                "clip.mp4" => Ok(json!({"type":"video","attrs":{"assetId":video_id,"src":format!("study-asset://{video_id}"),"title":name,"width":null,"height":null}})),
                _ => Err("missing".into()),
            },
        )
        .unwrap();
        assert!(result.script_omitted);
        assert_eq!(
            result
                .value["content"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|node| matches!(node["type"].as_str(), Some("audio" | "video")))
                .count(),
            2
        );
    }

    #[test]
    fn stylesheet_omission_is_aggregatable_and_answer_separator_can_be_suppressed() {
        let styled = document(
            "<style>.card{color:red}</style><div class='card'>Answer</div>",
            &mut |_| unreachable!(),
        )
        .unwrap();
        assert!(styled.style_omitted);
        let answer = document("<hr id=answer>Answer", &mut |_| unreachable!()).unwrap();
        assert_eq!(answer.value["content"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn strips_scripts_and_event_handlers_but_rejects_other_active_or_hidden_content() {
        let stripped = document(
            "<script>alert(1)</script><div onclick='bad()'>Static</div>",
            &mut |_| unreachable!(),
        )
        .unwrap();
        assert!(stripped.script_omitted);
        for html in [
            "<span hidden>answer</span>",
            "<iframe src='https://example.com'></iframe>",
            "<canvas></canvas>",
        ] {
            assert!(document(html, &mut |_| unreachable!()).is_err(), "{html}");
        }
    }

    #[test]
    fn resolves_images_as_block_nodes_without_copying_attributes() {
        let id = "0123456789abcdef0123456789abcdef";
        let result = document(
            "Before<img src='a&amp;b.png' width='999999'>After",
            &mut |name| {
                assert_eq!(name, "a&b.png");
                Ok(json!({"type": "image", "attrs": {"assetId": id, "src": format!("study-asset://{id}")}}))
            },
        )
        .unwrap();
        assert_eq!(result.value["content"][1]["type"], "image");
        assert!(result.value["content"][1]["attrs"].get("width").is_none());
        crate::cards::validate_document(&result.value).unwrap();
    }
}
