#!/usr/bin/env python3
"""One-shot PR #54 fix for document usage persistence and safe numeric diagnostics."""
from pathlib import Path


def replace_once(path: str, old: str, new: str, label: str) -> None:
    p = Path(path)
    text = p.read_text()
    if new in text:
        return
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one anchor in {path}, found {count}")
    p.write_text(text.replace(old, new, 1))


# ---------------------------------------------------------------------------
# Safe diagnostic telemetry: numeric token/count fields are operational metrics,
# not secrets or user content. Keep prompt/completion *content* redacted while
# allowing numeric token telemetry to remain analyzable.
# ---------------------------------------------------------------------------
replace_once(
    "src-tauri/src/logging.rs",
    '''                .map(|(key, value)| {
                    let value = if sensitive_key(&key) {
                        Value::String("[redacted]".to_string())
                    } else {
                        sanitize_value(value, depth + 1)
                    };
                    (bounded_text(&key, 96), value)
                })''',
    '''                .map(|(key, value)| {
                    let value = if sensitive_field(&key, &value) {
                        Value::String("[redacted]".to_string())
                    } else {
                        sanitize_value(value, depth + 1)
                    };
                    (bounded_text(&key, 96), value)
                })''',
    "field-sensitive diagnostic redaction",
)
replace_once(
    "src-tauri/src/logging.rs",
    '''fn sensitive_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase().replace(['-', '_'], "");
    [
        "authorization",
        "apikey",
        "credential",
        "password",
        "prompt",
        "completion",
        "requestbody",
        "responsebody",
        "secret",
        "token",
        "qrcontents",
        "documenttext",
        "cardcontent",
    ]
    .iter()
    .any(|candidate| key.contains(candidate))
}
''',
    '''fn sensitive_field(key: &str, value: &Value) -> bool {
    sensitive_key(key) && !(safe_numeric_telemetry_key(key) && (value.is_number() || value.is_null()))
}

fn safe_numeric_telemetry_key(key: &str) -> bool {
    matches!(
        normalized_key(key).as_str(),
        "prompttokens"
            | "completiontokens"
            | "totaltokens"
            | "reasoningtokens"
            | "cachedprompttokens"
            | "cachewritetokens"
            | "sourcetokens"
            | "estimatedjobtokens"
            | "contextwindowtokens"
            | "tokenestimate"
    )
}

fn sensitive_key(key: &str) -> bool {
    let key = normalized_key(key);
    [
        "authorization",
        "apikey",
        "credential",
        "password",
        "prompt",
        "completion",
        "requestbody",
        "responsebody",
        "secret",
        "token",
        "qrcontents",
        "documenttext",
        "cardcontent",
    ]
    .iter()
    .any(|candidate| key.contains(candidate))
}

fn credential_label(key: &str) -> bool {
    matches!(
        normalized_key(key).as_str(),
        "authorization" | "apikey" | "credential" | "password" | "secret" | "accesstoken" | "refreshtoken" | "bearertoken"
    )
}

fn normalized_key(key: &str) -> String {
    key.to_ascii_lowercase().replace(['-', '_'], "")
}
''',
    "safe numeric telemetry allowlist",
)
replace_once(
    "src-tauri/src/logging.rs",
    '''        let label = trimmed.trim_end_matches(':');
        if sensitive_key(label) {
            output.push(token.replacen(trimmed, &format!("{label}=[redacted]"), 1));
            redact_following = if label.eq_ignore_ascii_case("authorization") {
                2
            } else {
                1
            };
            continue;
        }''',
    '''        let label = trimmed.trim_end_matches(':');
        if (trimmed.ends_with(':') && sensitive_key(label)) || credential_label(label) {
            output.push(token.replacen(trimmed, &format!("{label}=[redacted]"), 1));
            redact_following = if label.eq_ignore_ascii_case("authorization") {
                2
            } else {
                1
            };
            continue;
        }''',
    "diagnostic message label redaction",
)
replace_once(
    "src-tauri/src/logging.rs",
    '''    #[test]
    fn opaque_authorization_tokens_are_redacted() {''',
    '''    #[test]
    fn numeric_ai_telemetry_remains_analyzable_without_exposing_content() {
        let value = sanitize_value(
            json!({
                "promptTokens": 185_000,
                "completionTokens": 28_000,
                "totalTokens": 213_000,
                "reasoningTokens": 20_000,
                "sourceTokens": 42_000,
                "contextWindowTokens": 1_310_720,
                "estimatedJobTokens": 250_000,
                "prompt": "private source text",
                "apiKey": "opaque-secret-value"
            }),
            0,
        );
        assert_eq!(value["promptTokens"], 185_000);
        assert_eq!(value["completionTokens"], 28_000);
        assert_eq!(value["totalTokens"], 213_000);
        assert_eq!(value["reasoningTokens"], 20_000);
        assert_eq!(value["sourceTokens"], 42_000);
        assert_eq!(value["contextWindowTokens"], 1_310_720);
        assert_eq!(value["estimatedJobTokens"], 250_000);
        assert_eq!(value["prompt"], "[redacted]");
        assert_eq!(value["apiKey"], "[redacted]");

        let message = redact_text("Recorded provider token usage without logging prompt or model content.");
        assert_eq!(
            message,
            "Recorded provider token usage without logging prompt or model content."
        );
    }

    #[test]
    fn opaque_authorization_tokens_are_redacted() {''',
    "numeric telemetry regression",
)

# ---------------------------------------------------------------------------
# Task-local usage summary: persist paid provider usage even when a document job
# fails after provider work. The UI must not say "not reported" after showing a
# real live token count for the same task.
# ---------------------------------------------------------------------------
replace_once(
    "src-tauri/src/task_cancellation.rs",
    '''#[cfg(test)]
pub(crate) fn current_reasoning_effort() -> Option<String> {''',
    '''pub(crate) fn current_reasoning_effort() -> Option<String> {''',
    "make task-local reasoning readable by gateway",
)
replace_once(
    "src-tauri/src/task_cancellation.rs",
    '''pub(crate) fn cancel_document_task(document_id: &str) -> Result<(), CommandError> {''',
    '''pub(crate) fn current_usage_summary() -> Option<(Option<i64>, bool)> {
    CURRENT_DOCUMENT_TASK
        .try_with(|task| {
            task.progress.lock().ok().map(|progress| {
                let total = (progress.usage_reported_requests > 0).then_some(
                    i64::try_from(progress.total_tokens).unwrap_or(i64::MAX),
                );
                let complete = progress.usage_reported_requests > 0
                    && progress.usage_unreported_requests == 0;
                (total, complete)
            })
        })
        .ok()
        .flatten()
}

pub(crate) fn cancel_document_task(document_id: &str) -> Result<(), CommandError> {''',
    "task usage summary",
)

replace_once(
    "src-tauri/src/documents.rs",
    '''fn record_document_failure(database: &Database, document_id: &str, error: &CommandError) {
    let now = unix_time_millis().unwrap_or_default();
    let _ = database.with_connection(|connection| {''',
    '''fn record_document_failure(database: &Database, document_id: &str, error: &CommandError) {
    let now = unix_time_millis().unwrap_or_default();
    let usage = crate::task_cancellation::current_usage_summary();
    let (api_token_count, api_token_usage_complete) = usage.unwrap_or((None, false));
    let _ = database.with_connection(|connection| {''',
    "capture failed initial-generation usage",
)
replace_once(
    "src-tauri/src/documents.rs",
    '''                     processing_duration_ms = CASE
                         WHEN processing_started_at IS NULL THEN processing_duration_ms
                         ELSE MAX(0, ?3 - processing_started_at)
                     END,
                     updated_at = ?3, revision = revision + 1
                 WHERE id = ?4",
                params![
                    bounded_error(&error.code, 80),
                    bounded_error(&error.message, 1000),
                    now,
                    document_id,
                ],''',
    '''                     processing_duration_ms = CASE
                         WHEN processing_started_at IS NULL THEN processing_duration_ms
                         ELSE MAX(0, ?3 - processing_started_at)
                     END,
                     api_token_count = CASE WHEN ?4 IS NULL THEN api_token_count ELSE ?4 END,
                     api_token_usage_complete = CASE WHEN ?4 IS NULL THEN api_token_usage_complete ELSE ?5 END,
                     updated_at = ?3, revision = revision + 1
                 WHERE id = ?6",
                params![
                    bounded_error(&error.code, 80),
                    bounded_error(&error.message, 1000),
                    now,
                    api_token_count,
                    if api_token_usage_complete { 1_i64 } else { 0_i64 },
                    document_id,
                ],''',
    "persist failed initial-generation usage",
)

# Additional generation reports failure while the task-local context is still
# alive so its provider usage is available. After run_document_task returns the
# task-local is intentionally gone.
replace_once(
    "src-tauri/src/document_more.rs",
    '''    let result = run_document_task(
        &document_id,
        generate_more_impl(database, paths, runtime, input),
    )
    .await;

    match result {
        Ok(value) => Ok(value),
        Err(error) if error.code == "ai_request_canceled" => {
            record_additional_failure(
                database_after.inner(),
                &document_id,
                "document_generation_canceled",
                CANCELED_MESSAGE,
            );
            Err(CommandError::new(
                "document_generation_canceled",
                CANCELED_MESSAGE,
            ))
        }
        Err(error) => {
            record_additional_failure(
                database_after.inner(),
                &document_id,
                &error.code,
                &error.message,
            );
            Err(error)
        }
    }''',
    '''    let result = run_document_task(&document_id, async {
        let result = generate_more_impl(database, paths, runtime, input).await;
        if let Err(error) = &result {
            if error.code == "ai_request_canceled" {
                record_additional_failure(
                    database_after.inner(),
                    &document_id,
                    "document_generation_canceled",
                    CANCELED_MESSAGE,
                );
            } else {
                record_additional_failure(
                    database_after.inner(),
                    &document_id,
                    &error.code,
                    &error.message,
                );
            }
        }
        result
    })
    .await;

    match result {
        Err(error) if error.code == "ai_request_canceled" => Err(CommandError::new(
            "document_generation_canceled",
            CANCELED_MESSAGE,
        )),
        other => other,
    }''',
    "persist additional failure while task scope is active",
)
replace_once(
    "src-tauri/src/document_more.rs",
    '''fn record_additional_failure(database: &Database, document_id: &str, code: &str, message: &str) {
    let now = unix_time_millis().unwrap_or_default();
    let _ = database.with_connection(|connection| {''',
    '''fn record_additional_failure(database: &Database, document_id: &str, code: &str, message: &str) {
    let now = unix_time_millis().unwrap_or_default();
    let usage = crate::task_cancellation::current_usage_summary();
    let (api_token_count, api_token_usage_complete) = usage.unwrap_or((None, false));
    let _ = database.with_connection(|connection| {''',
    "capture failed additional-generation usage",
)
replace_once(
    "src-tauri/src/document_more.rs",
    '''                     processing_duration_ms = CASE
                         WHEN processing_started_at IS NULL THEN processing_duration_ms
                         ELSE MAX(0, ?3 - processing_started_at)
                     END,
                     updated_at = ?3, revision = revision + 1
                 WHERE id = ?4",
                params![bounded(code, 80), bounded(message, 1000), now, document_id],''',
    '''                     processing_duration_ms = CASE
                         WHEN processing_started_at IS NULL THEN processing_duration_ms
                         ELSE MAX(0, ?3 - processing_started_at)
                     END,
                     api_token_count = CASE WHEN ?4 IS NULL THEN api_token_count ELSE ?4 END,
                     api_token_usage_complete = CASE WHEN ?4 IS NULL THEN api_token_usage_complete ELSE ?5 END,
                     updated_at = ?3, revision = revision + 1
                 WHERE id = ?6",
                params![
                    bounded(code, 80),
                    bounded(message, 1000),
                    now,
                    api_token_count,
                    if api_token_usage_complete { 1_i64 } else { 0_i64 },
                    document_id,
                ],''',
    "persist failed additional-generation usage",
)

# Include detailed normalized usage fields in AI diagnostics; the logging layer
# now permits only their numeric values, never prompt/completion content.
replace_once(
    "src-tauri/src/ai/document_intelligence.rs",
    '''fn record_document_usage(task: &DocumentTaskContext, response: &Value) {
    let (prompt_tokens, completion_tokens, total_tokens) = response_token_usage(response);
    let reported = total_tokens.is_some();''',
    '''fn record_document_usage(task: &DocumentTaskContext, response: &Value) {
    let (prompt_tokens, completion_tokens, total_tokens) = response_token_usage(response);
    let cached_prompt_tokens = response
        .pointer("/usage/prompt_tokens_details/cached_tokens")
        .and_then(nonnegative_json_integer);
    let cache_write_tokens = response
        .pointer("/usage/prompt_tokens_details/cache_write_tokens")
        .and_then(nonnegative_json_integer);
    let reasoning_tokens = response
        .pointer("/usage/completion_tokens_details/reasoning_tokens")
        .and_then(nonnegative_json_integer);
    let reported = total_tokens.is_some();''',
    "detailed document usage fields",
)
replace_once(
    "src-tauri/src/ai/document_intelligence.rs",
    '''            "completionTokens": completion_tokens,
            "totalTokens": total_tokens,
        }),''',
    '''            "completionTokens": completion_tokens,
            "totalTokens": total_tokens,
            "cachedPromptTokens": cached_prompt_tokens,
            "cacheWriteTokens": cache_write_tokens,
            "reasoningTokens": reasoning_tokens,
        }),''',
    "usage diagnostic counters",
)

print("Applied PR #54 document telemetry and failed-usage persistence fixes.")
