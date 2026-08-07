use serde_json::{json, Value};

use crate::error::CommandError;

pub(super) fn markdown_document(source: &str, path: &str) -> Result<Value, CommandError> {
    if source.trim().is_empty() {
        return Err(CommandError::new(
            "empty_import_card_side",
            format!("{path}: add text to this card side."),
        ));
    }
    if source.contains("![") {
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

    let normalized = source.replace("\r\n", "\n").replace('\r', "\n");
    let lines = normalized.lines().collect::<Vec<_>>();
    let mut nodes = Vec::new();
    let mut index = 0_usize;
    while index < lines.len() {
        let line = lines[index];
        if line.trim().is_empty() {
            index += 1;
            continue;
        }
        if line.trim_start().starts_with("```") {
            let language = line.trim_start().trim_start_matches("```").trim();
            index += 1;
            let mut code = Vec::new();
            while index < lines.len() && !lines[index].trim_start().starts_with("```") {
                code.push(lines[index]);
                index += 1;
            }
            if index == lines.len() {
                return Err(unsupported_markdown(
                    path,
                    "a fenced code block is not closed",
                ));
            }
            index += 1;
            nodes.push(json!({
                "type": "codeBlock",
                "attrs": {"language": if language.is_empty() { Value::Null } else { Value::String(language.to_string()) }},
                "content": [{"type": "text", "text": code.join("\n")}]
            }));
            continue;
        }
        if let Some(level) = heading_level(line) {
            let text = line.trim_start()[level + 1..].trim();
            nodes.push(json!({
                "type": "heading",
                "attrs": {"level": level},
                "content": inline_nodes(text, path)?
            }));
            index += 1;
            continue;
        }
        if matches!(line.trim(), "---" | "***" | "___") {
            nodes.push(json!({"type": "horizontalRule"}));
            index += 1;
            continue;
        }
        if line.trim_start().starts_with("> ") {
            let mut quoted = Vec::new();
            while index < lines.len() && lines[index].trim_start().starts_with("> ") {
                quoted.push(lines[index].trim_start()[2..].to_string());
                index += 1;
            }
            nodes.push(json!({
                "type": "blockquote",
                "content": [{"type": "paragraph", "content": inline_nodes(&quoted.join("\n"), path)?}]
            }));
            continue;
        }
        if bullet_item(line).is_some() {
            let mut items = Vec::new();
            while index < lines.len() {
                let Some(text) = bullet_item(lines[index]) else {
                    break;
                };
                items.push(json!({
                    "type": "listItem",
                    "content": [{"type": "paragraph", "content": inline_nodes(text, path)?}]
                }));
                index += 1;
            }
            nodes.push(json!({"type": "bulletList", "content": items}));
            continue;
        }
        if let Some((start, _)) = ordered_item(line) {
            let mut items = Vec::new();
            while index < lines.len() {
                let Some((_, text)) = ordered_item(lines[index]) else {
                    break;
                };
                items.push(json!({
                    "type": "listItem",
                    "content": [{"type": "paragraph", "content": inline_nodes(text, path)?}]
                }));
                index += 1;
            }
            nodes.push(json!({"type": "orderedList", "attrs": {"start": start}, "content": items}));
            continue;
        }

        let mut paragraph = vec![line.to_string()];
        index += 1;
        while index < lines.len()
            && !lines[index].trim().is_empty()
            && !is_block_start(lines[index])
        {
            paragraph.push(lines[index].to_string());
            index += 1;
        }
        nodes.push(json!({
            "type": "paragraph",
            "content": inline_nodes(&paragraph.join("\n"), path)?
        }));
    }
    Ok(json!({"type": "doc", "content": nodes}))
}

fn unsupported_markdown(path: &str, detail: &str) -> CommandError {
    CommandError::new("unsupported_import_markdown", format!("{path}: {detail}."))
}

fn heading_level(line: &str) -> Option<usize> {
    let trimmed = line.trim_start();
    let count = trimmed.bytes().take_while(|byte| *byte == b'#').count();
    if (1..=4).contains(&count) && trimmed.as_bytes().get(count) == Some(&b' ') {
        Some(count)
    } else {
        None
    }
}

fn bullet_item(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    for prefix in ["- ", "* ", "+ "] {
        if let Some(value) = trimmed.strip_prefix(prefix) {
            return Some(value);
        }
    }
    None
}

fn ordered_item(line: &str) -> Option<(i64, &str)> {
    let trimmed = line.trim_start();
    let separator = trimmed.find(". ")?;
    let number = trimmed[..separator].parse::<i64>().ok()?;
    if number < 1 {
        return None;
    }
    Some((number, &trimmed[separator + 2..]))
}

fn is_block_start(line: &str) -> bool {
    let trimmed = line.trim_start();
    heading_level(line).is_some()
        || trimmed.starts_with("```")
        || trimmed.starts_with("> ")
        || bullet_item(line).is_some()
        || ordered_item(line).is_some()
        || matches!(trimmed, "---" | "***" | "___")
}

fn inline_nodes(text: &str, path: &str) -> Result<Vec<Value>, CommandError> {
    let mut nodes = Vec::new();
    let mut cursor = 0_usize;
    while cursor < text.len() {
        let remainder = &text[cursor..];
        if remainder.starts_with('\n') {
            nodes.push(json!({"type": "hardBreak"}));
            cursor += 1;
            continue;
        }
        if let Some((consumed, label, href)) = parse_link(remainder) {
            nodes.push(json!({
                "type": "text",
                "text": label,
                "marks": [{"type": "link", "attrs": {"href": href, "target": null, "rel": null, "class": null}}]
            }));
            cursor += consumed;
            continue;
        }
        let markers = [
            ("**", "bold"),
            ("__", "bold"),
            ("~~", "strike"),
            ("*", "italic"),
            ("_", "italic"),
            ("`", "code"),
        ];
        let mut matched = false;
        for (marker, mark_type) in markers {
            if let Some(after) = remainder.strip_prefix(marker) {
                if let Some(close) = after.find(marker) {
                    let value = &after[..close];
                    if value.is_empty() {
                        continue;
                    }
                    nodes.push(json!({
                        "type": "text",
                        "text": value,
                        "marks": [{"type": mark_type}]
                    }));
                    cursor += marker.len() + close + marker.len();
                    matched = true;
                    break;
                }
            }
        }
        if matched {
            continue;
        }
        let next = next_inline_marker(remainder).unwrap_or(remainder.len());
        let take = next.max(1);
        let value = &remainder[..take];
        if value.contains('[') && value.contains("](") {
            return Err(unsupported_markdown(path, "a Markdown link is malformed"));
        }
        nodes.push(json!({"type": "text", "text": value}));
        cursor += take;
    }
    Ok(nodes)
}

fn parse_link(value: &str) -> Option<(usize, &str, &str)> {
    if !value.starts_with('[') {
        return None;
    }
    let label_end = value.find("](")?;
    let href_start = label_end + 2;
    let href_end = value[href_start..].find(')')? + href_start;
    let label = &value[1..label_end];
    let href = &value[href_start..href_end];
    if label.is_empty() || href.is_empty() {
        return None;
    }
    Some((href_end + 1, label, href))
}

fn next_inline_marker(value: &str) -> Option<usize> {
    ["\n", "[", "**", "__", "~~", "*", "_", "`"]
        .iter()
        .filter_map(|marker| value.find(marker))
        .filter(|position| *position > 0)
        .min()
}

fn contains_raw_html(source: &str) -> bool {
    let bytes = source.as_bytes();
    let mut index = 0_usize;
    while index < bytes.len() {
        if bytes[index] != b'<' {
            index += 1;
            continue;
        }
        let Some(next) = bytes.get(index + 1).copied() else {
            return false;
        };
        if (next.is_ascii_alphabetic() || matches!(next, b'/' | b'!' | b'?'))
            && source[index + 1..].contains('>')
        {
            return true;
        }
        index += 1;
    }
    false
}
