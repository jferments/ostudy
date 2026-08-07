use serde::Deserialize;

use crate::{error::CommandError, logging};

const MAX_EVENT_CHARS: usize = 64;
const MAX_MESSAGE_CHARS: usize = 4_000;
const MAX_STACK_CHARS: usize = 12_000;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WebviewDiagnosticInput {
    event: String,
    message: String,
    #[serde(default)]
    stack: Option<String>,
}

#[tauri::command]
pub fn record_webview_diagnostic(input: WebviewDiagnosticInput) -> Result<(), CommandError> {
    let event = normalize_event(&input.event);
    let message = bounded_text(&input.message, MAX_MESSAGE_CHARS);
    let stack = input
        .stack
        .as_deref()
        .map(|value| bounded_text(value, MAX_STACK_CHARS))
        .unwrap_or_default();
    logging::error(
        &event,
        if stack.is_empty() {
            format!("message={message}")
        } else {
            format!("message={message} stack={stack}")
        },
    );
    Ok(())
}

fn normalize_event(value: &str) -> String {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty()
        || normalized.len() > MAX_EVENT_CHARS
        || !normalized.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
    {
        "webview_error".to_string()
    } else {
        normalized
    }
}

fn bounded_text(value: &str, maximum: usize) -> String {
    let length = value.chars().count();
    if length <= maximum {
        return value.to_string();
    }
    let omitted = length.saturating_sub(maximum);
    format!(
        "{}… [truncated {omitted} characters]",
        value.chars().take(maximum).collect::<String>()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_fields_are_normalized_and_bounded() {
        assert_eq!(normalize_event(" Render Crash "), "webview_error");
        assert_eq!(
            normalize_event("webview_render_error"),
            "webview_render_error"
        );
        let bounded = bounded_text(&"x".repeat(MAX_MESSAGE_CHARS + 50), MAX_MESSAGE_CHARS);
        assert!(bounded.contains("truncated 50 characters"));
        assert!(bounded.chars().count() < MAX_MESSAGE_CHARS + 100);
    }
}
