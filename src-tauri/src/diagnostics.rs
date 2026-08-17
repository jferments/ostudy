use std::{
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::Duration,
};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    error::CommandError,
    logging::{self, DiagnosticConfigSnapshot},
};

const MAX_EVENT_CHARS: usize = 96;
const MAX_MESSAGE_CHARS: usize = 4_000;
const MAX_STACK_CHARS: usize = 12_000;
const MAX_TARGET_CHARS: usize = 128;
const MAX_BATCH_EVENTS: usize = 32;
const MAX_BATCH_BYTES: usize = 64 * 1024;
const MIN_TEMPORARY_FILTER_SECONDS: u64 = 60;
const MAX_TEMPORARY_FILTER_SECONDS: u64 = 60 * 60;

static FILTER_GENERATION: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WebviewDiagnosticInput {
    event: String,
    message: String,
    #[serde(default)]
    stack: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WebviewDiagnosticEventInput {
    level: String,
    target: String,
    event: String,
    message: String,
    #[serde(default)]
    stack: Option<String>,
    #[serde(default)]
    fields: Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WebviewDiagnosticBatchInput {
    events: Vec<WebviewDiagnosticEventInput>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetDiagnosticFilterInput {
    filter: String,
    #[serde(default)]
    expires_in_seconds: Option<u64>,
}

#[tauri::command]
pub fn get_diagnostic_configuration() -> DiagnosticConfigSnapshot {
    logging::snapshot()
}

#[tauri::command]
pub fn set_diagnostic_filter(
    input: SetDiagnosticFilterInput,
) -> Result<DiagnosticConfigSnapshot, CommandError> {
    let previous_filter = logging::current_filter();
    let snapshot = logging::set_filter(&input.filter).map_err(|message| {
        CommandError::new(
            "invalid_diagnostic_filter",
            format!("The diagnostic filter is invalid: {message}"),
        )
    })?;
    let generation = FILTER_GENERATION.fetch_add(1, Ordering::AcqRel) + 1;

    if let Some(expires_in_seconds) = input.expires_in_seconds {
        if !(MIN_TEMPORARY_FILTER_SECONDS..=MAX_TEMPORARY_FILTER_SECONDS)
            .contains(&expires_in_seconds)
        {
            let _ = logging::set_filter(&previous_filter);
            FILTER_GENERATION.fetch_add(1, Ordering::AcqRel);
            return Err(CommandError::new(
                "invalid_diagnostic_filter_expiry",
                "Temporary diagnostic filters must last between one minute and one hour.",
            ));
        }
        let expiry_filter = previous_filter.clone();
        let spawn_result = thread::Builder::new()
            .name("ostudy-diagnostic-filter-expiry".to_string())
            .spawn(move || {
                thread::sleep(Duration::from_secs(expires_in_seconds));
                if FILTER_GENERATION.load(Ordering::Acquire) == generation {
                    if logging::set_filter(&expiry_filter).is_ok() {
                        logging::info_target(
                            "diagnostics.config",
                            "diagnostic_filter_expired",
                            "The temporary diagnostic filter expired and the previous filter was restored.",
                            serde_json::json!({ "durationSeconds": expires_in_seconds }),
                        );
                    }
                }
            });
        if spawn_result.is_err() {
            let _ = logging::set_filter(&previous_filter);
            FILTER_GENERATION.fetch_add(1, Ordering::AcqRel);
            return Err(CommandError::new(
                "diagnostic_filter_timer_failed",
                "The temporary diagnostic filter could not be scheduled.",
            ));
        }
    }

    logging::info_target(
        "diagnostics.config",
        "diagnostic_filter_changed",
        "The diagnostic filter was changed for this application process.",
        serde_json::json!({
            "filter": snapshot.filter,
            "temporary": input.expires_in_seconds.is_some(),
            "expiresInSeconds": input.expires_in_seconds,
        }),
    );
    Ok(snapshot)
}

#[tauri::command]
pub fn flush_diagnostics() -> bool {
    logging::flush(Duration::from_secs(1))
}

#[tauri::command]
pub fn record_webview_diagnostic(input: WebviewDiagnosticInput) -> Result<(), CommandError> {
    record_event(WebviewDiagnosticEventInput {
        level: "error".to_string(),
        target: "frontend.webview".to_string(),
        event: input.event,
        message: input.message,
        stack: input.stack,
        fields: Value::Null,
    })
}

#[tauri::command]
pub fn record_webview_diagnostic_batch(
    input: WebviewDiagnosticBatchInput,
) -> Result<(), CommandError> {
    if input.events.is_empty() {
        return Ok(());
    }
    if input.events.len() > MAX_BATCH_EVENTS {
        return Err(CommandError::new(
            "diagnostic_batch_too_large",
            format!("A diagnostic batch can contain at most {MAX_BATCH_EVENTS} events."),
        ));
    }
    let encoded_size = serde_json::to_vec(&input.events)
        .map_err(CommandError::serialization)?
        .len();
    if encoded_size > MAX_BATCH_BYTES {
        return Err(CommandError::new(
            "diagnostic_batch_too_large",
            format!("A diagnostic batch can contain at most {MAX_BATCH_BYTES} encoded bytes."),
        ));
    }
    for event in input.events {
        record_event(event)?;
    }
    Ok(())
}

fn record_event(input: WebviewDiagnosticEventInput) -> Result<(), CommandError> {
    let event = normalize_event(&input.event);
    let target = normalize_target(&input.target);
    let message = bounded_text(&input.message, MAX_MESSAGE_CHARS);
    let mut fields = match input.fields {
        Value::Object(fields) => fields,
        Value::Null => serde_json::Map::new(),
        _ => {
            return Err(CommandError::new(
                "invalid_diagnostic_fields",
                "Diagnostic event fields must be a JSON object.",
            ));
        }
    };
    if let Some(stack) = input.stack.as_deref() {
        fields.insert(
            "stack".to_string(),
            Value::String(bounded_text(stack, MAX_STACK_CHARS)),
        );
    }
    let fields = Value::Object(fields);
    match input.level.trim().to_ascii_lowercase().as_str() {
        "error" => logging::error_target(&target, &event, message, fields),
        "warn" | "warning" => logging::warn_target(&target, &event, message, fields),
        "info" => logging::info_target(&target, &event, message, fields),
        "debug" => logging::debug_target(&target, &event, message, fields),
        "trace" => logging::trace_target(&target, &event, message, fields),
        _ => {
            return Err(CommandError::new(
                "invalid_diagnostic_level",
                "Diagnostic events must use error, warn, info, debug, or trace.",
            ));
        }
    }
    Ok(())
}

fn normalize_event(value: &str) -> String {
    let normalized = value.trim().to_ascii_lowercase().replace('-', "_");
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

fn normalize_target(value: &str) -> String {
    let normalized = value.trim().to_ascii_lowercase().replace("::", ".");
    if normalized.is_empty()
        || normalized.len() > MAX_TARGET_CHARS
        || !normalized.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '.' | '_' | '-')
        })
    {
        "frontend.webview".to_string()
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
        assert_eq!(normalize_target("AI::Gateway"), "ai.gateway");
        let bounded = bounded_text(&"x".repeat(MAX_MESSAGE_CHARS + 50), MAX_MESSAGE_CHARS);
        assert!(bounded.contains("truncated 50 characters"));
        assert!(bounded.chars().count() < MAX_MESSAGE_CHARS + 100);
    }

    #[test]
    fn diagnostic_batches_have_strict_count_and_size_bounds() {
        let too_many = WebviewDiagnosticBatchInput {
            events: (0..=MAX_BATCH_EVENTS)
                .map(|index| WebviewDiagnosticEventInput {
                    level: "debug".to_string(),
                    target: "frontend.test".to_string(),
                    event: format!("event_{index}"),
                    message: "bounded".to_string(),
                    stack: None,
                    fields: Value::Null,
                })
                .collect(),
        };
        assert!(record_webview_diagnostic_batch(too_many).is_err());
    }

    #[test]
    fn event_fields_must_be_objects() {
        let result = record_event(WebviewDiagnosticEventInput {
            level: "info".to_string(),
            target: "frontend.test".to_string(),
            event: "invalid_fields".to_string(),
            message: "bounded".to_string(),
            stack: None,
            fields: serde_json::json!(["not", "an", "object"]),
        });
        assert!(result.is_err());
    }
}
