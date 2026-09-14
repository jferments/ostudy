//! Normalize Anki's local-media markup before conservative HTML conversion.
//!
//! The output uses private `<img>` placeholders because the existing HTML
//! converter already treats an image resolver as an application-owned block
//! boundary. The package reader recognizes these prefixes and may return native
//! audio/video nodes instead. The placeholders are never persisted.

const MAX_HTML_BYTES: usize = 256 * 1024;
pub(super) const SOUND_PREFIX: &str = "__ostudy_sound__/";
pub(super) const AUDIO_PREFIX: &str = "__ostudy_audio__/";
pub(super) const VIDEO_PREFIX: &str = "__ostudy_video__/";

pub(super) fn rewrite(input: &str) -> Result<String, String> {
    if input.len() > MAX_HTML_BYTES {
        return Err("HTML exceeds the 256 KiB per-side limit.".into());
    }
    let mut output = String::with_capacity(input.len());
    let mut position = 0_usize;
    let mut literal_depth = 0_usize;
    while position < input.len() {
        let remaining = &input[position..];
        if remaining.starts_with("<!--") {
            let end = remaining.find("-->").ok_or("Unclosed HTML comment.")? + 3;
            output.push_str(&remaining[..end]);
            position += end;
            continue;
        }
        if remaining.starts_with('<') && looks_like_tag(remaining) {
            let tag = parse_tag(remaining)?;
            let consumed = tag.consumed;
            if literal_depth > 0 {
                output.push_str(&remaining[..consumed]);
                if matches!(tag.name.as_str(), "code" | "pre") {
                    if tag.closing {
                        literal_depth = literal_depth.saturating_sub(1);
                    } else if !tag.self_closing {
                        literal_depth = literal_depth.saturating_add(1);
                    }
                }
                position += consumed;
                continue;
            }
            if !tag.closing && matches!(tag.name.as_str(), "audio" | "video") {
                let expected = if tag.name == "audio" { AUDIO_PREFIX } else { VIDEO_PREFIX };
                let mut sources = Vec::new();
                if let Some(source) = tag.attribute("src")? {
                    sources.push(source);
                }
                let (end, fallback) = if tag.self_closing {
                    (consumed, String::new())
                } else {
                    consume_media_body(remaining, consumed, &tag.name, &mut sources)?
                };
                let source = unique_source(sources)?;
                output.push_str(&placeholder(expected, &source)?);
                output.push_str(&fallback);
                position += end;
                continue;
            }
            if !tag.closing && tag.name == "source" {
                return Err("An Anki source element is only supported inside audio or video.".into());
            }
            output.push_str(&remaining[..consumed]);
            if !tag.closing && !tag.self_closing && matches!(tag.name.as_str(), "code" | "pre") {
                literal_depth = 1;
            }
            position += consumed;
            continue;
        }

        let length = remaining.find('<').unwrap_or(remaining.len()).max(1);
        let text = &remaining[..length];
        if literal_depth > 0 {
            output.push_str(&escape_literal_sound_openings(text));
        } else {
            output.push_str(&rewrite_sound_markers(text)?);
        }
        position += length;
    }
    Ok(output)
}

fn consume_media_body(
    input: &str,
    opening: usize,
    element: &str,
    sources: &mut Vec<String>,
) -> Result<(usize, String), String> {
    let mut position = opening;
    let mut fallback = String::new();
    while position < input.len() {
        let remaining = &input[position..];
        let Some(relative) = remaining.find('<') else {
            return Err(format!("An Anki {element} element is not closed."));
        };
        fallback.push_str(&remaining[..relative]);
        position += relative;
        let remaining = &input[position..];
        if !looks_like_tag(remaining) {
            fallback.push('<');
            position += 1;
            continue;
        }
        let tag = parse_tag(remaining)?;
        if tag.closing && tag.name == element {
            return Ok((position + tag.consumed, fallback));
        }
        if !tag.closing && tag.name == element {
            return Err("Nested Anki audio/video elements are unsupported.".into());
        }
        if tag.name == "source" {
            if tag.closing {
                position += tag.consumed;
                continue;
            }
            if let Some(source) = tag.attribute("src")? {
                sources.push(source);
            }
            position += tag.consumed;
            continue;
        }
        // Preserve fallback text/formatting. The ordinary HTML converter will
        // still reject scripts, event handlers, hidden content, or active tags.
        fallback.push_str(&remaining[..tag.consumed]);
        position += tag.consumed;
    }
    Err(format!("An Anki {element} element is not closed."))
}

fn unique_source(sources: Vec<String>) -> Result<String, String> {
    let mut unique = None::<String>;
    for source in sources {
        let source = decode_entities(source.trim())?;
        validate_filename(&source)?;
        match unique.as_ref() {
            None => unique = Some(source),
            Some(existing) if existing == &source => {}
            Some(_) => {
                return Err(
                    "An Anki audio/video element has multiple different local sources; choose one source in Anki before importing."
                        .into(),
                )
            }
        }
    }
    unique.ok_or_else(|| "An Anki audio/video element has no local source.".into())
}

fn rewrite_sound_markers(text: &str) -> Result<String, String> {
    let mut output = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = find_ascii_case_insensitive(rest, "[sound:") {
        output.push_str(&rest[..start]);
        let marker = &rest[start + 7..];
        let end = marker
            .find(']')
            .ok_or("An Anki [sound:] marker is not closed.")?;
        let filename = decode_entities(marker[..end].trim())?;
        validate_filename(&filename)?;
        output.push_str(&placeholder(SOUND_PREFIX, &filename)?);
        rest = &marker[end + 1..];
    }
    output.push_str(rest);
    Ok(output)
}

fn escape_literal_sound_openings(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = find_ascii_case_insensitive(rest, "[sound:") {
        output.push_str(&rest[..start]);
        output.push_str("&#91;");
        output.push_str(&rest[start + 1..start + 7]);
        rest = &rest[start + 7..];
    }
    output.push_str(rest);
    output
}

fn placeholder(prefix: &str, filename: &str) -> Result<String, String> {
    validate_filename(filename)?;
    Ok(format!("<img src=\"{prefix}{}\">", percent_encode(filename)))
}

fn validate_filename(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 1024
        || value.chars().any(char::is_control)
        || value.contains('/')
        || value.contains('\\')
        || matches!(value, "." | "..")
        || value.contains("://")
        || value.to_ascii_lowercase().starts_with("data:")
    {
        return Err("An Anki media reference is external, unsafe, or too long.".into());
    }
    Ok(())
}

fn percent_encode(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for byte in value.as_bytes() {
        if byte.is_ascii_alphanumeric() || matches!(*byte, b'-' | b'_' | b'.' | b'~') {
            output.push(char::from(*byte));
        } else {
            output.push('%');
            output.push_str(&format!("{byte:02X}"));
        }
    }
    output
}

fn find_ascii_case_insensitive(haystack: &str, needle: &str) -> Option<usize> {
    let needle = needle.as_bytes();
    haystack
        .as_bytes()
        .windows(needle.len())
        .position(|window| window.eq_ignore_ascii_case(needle))
}

fn looks_like_tag(value: &str) -> bool {
    value
        .as_bytes()
        .get(1)
        .is_some_and(|byte| byte.is_ascii_alphabetic() || matches!(byte, b'/' | b'!' | b'?'))
}

struct Tag<'a> {
    raw_attributes: &'a str,
    name: String,
    closing: bool,
    self_closing: bool,
    consumed: usize,
}

impl Tag<'_> {
    fn attribute(&self, wanted: &str) -> Result<Option<String>, String> {
        let mut input = self.raw_attributes;
        while !input.trim().is_empty() {
            input = input.trim_start();
            let end = input
                .find(|character: char| character.is_whitespace() || character == '=')
                .unwrap_or(input.len());
            if end == 0 {
                return Err("Malformed Anki media attributes.".into());
            }
            let name = input[..end].to_ascii_lowercase();
            input = input[end..].trim_start();
            let mut value = String::new();
            if let Some(rest) = input.strip_prefix('=') {
                input = rest.trim_start();
                if let Some(quote) = input.chars().next().filter(|c| matches!(c, '\'' | '"')) {
                    let tail = &input[quote.len_utf8()..];
                    let close = tail.find(quote).ok_or("Unclosed Anki media attribute.")?;
                    value.push_str(&tail[..close]);
                    input = &tail[close + quote.len_utf8()..];
                } else {
                    let close = input.find(char::is_whitespace).unwrap_or(input.len());
                    value.push_str(&input[..close]);
                    input = &input[close..];
                }
            }
            if name == wanted {
                return Ok(Some(value));
            }
        }
        Ok(None)
    }
}

fn parse_tag(input: &str) -> Result<Tag<'_>, String> {
    let mut quote = None;
    let mut end = None;
    for (index, byte) in input.bytes().enumerate().skip(1) {
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
    let raw_tag = input[1..end].trim();
    let closing = raw_tag.starts_with('/');
    let self_closing = raw_tag.ends_with('/');
    let raw = raw_tag.trim_start_matches('/').trim_end_matches('/').trim();
    let name_end = raw.find(char::is_whitespace).unwrap_or(raw.len());
    let name = raw[..name_end].to_ascii_lowercase();
    if name.is_empty()
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return Err("Malformed HTML tag.".into());
    }
    Ok(Tag {
        raw_attributes: &raw[name_end..],
        name,
        closing,
        self_closing,
        consumed: end + 1,
    })
}

fn decode_entities(value: &str) -> Result<String, String> {
    let mut output = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(index) = rest.find('&') {
        output.push_str(&rest[..index]);
        let tail = &rest[index..];
        let Some(end) = tail.find(';') else {
            output.push_str(tail);
            return Ok(output);
        };
        let entity = &tail[1..end];
        let decoded = match entity {
            "amp" => Some('&'),
            "quot" => Some('"'),
            "apos" | "#39" => Some('\''),
            "lt" => Some('<'),
            "gt" => Some('>'),
            _ if entity.starts_with("#x") || entity.starts_with("#X") => {
                u32::from_str_radix(&entity[2..], 16).ok().and_then(char::from_u32)
            }
            _ if entity.starts_with('#') => entity[1..].parse::<u32>().ok().and_then(char::from_u32),
            _ => None,
        };
        if let Some(decoded) = decoded {
            output.push(decoded);
        } else {
            output.push_str(&tail[..=end]);
        }
        rest = &tail[end + 1..];
    }
    output.push_str(rest);
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn document_anki_media_markup_rewrites_sound_and_local_media_elements() {
        let rewritten = rewrite(
            "before [sound:voice one.wav] <audio src='voice&amp;two.m4a'></audio> \
             <video><source src=clip.mp4></video> after",
        )
        .unwrap();
        assert!(rewritten.contains("__ostudy_sound__/voice%20one.wav"));
        assert!(rewritten.contains("__ostudy_audio__/voice%26two.m4a"));
        assert!(rewritten.contains("__ostudy_video__/clip.mp4"));
        assert!(!rewritten.to_ascii_lowercase().contains("<audio"));
        assert!(!rewritten.to_ascii_lowercase().contains("<video"));
        assert!(!rewritten.to_ascii_lowercase().contains("<source"));
    }

    #[test]
    fn document_anki_media_markup_keeps_code_sound_markers_literal() {
        let rewritten = rewrite("<code>[sound:literal.mp3]</code><pre>[SOUND:also.wav]</pre>").unwrap();
        assert!(!rewritten.contains(SOUND_PREFIX));
        assert_eq!(
            rewritten,
            "<code>&#91;sound:literal.mp3]</code><pre>&#91;SOUND:also.wav]</pre>"
        );
    }

    #[test]
    fn document_anki_media_markup_rejects_remote_ambiguous_and_unclosed_media() {
        for source in [
            "[sound:https://example.invalid/a.mp3]",
            "<audio src='a.mp3'><source src='b.mp3'></audio>",
            "<video src='clip.mp4'>",
            "<source src='orphan.mp3'>",
        ] {
            assert!(rewrite(source).is_err(), "{source}");
        }
    }
}
