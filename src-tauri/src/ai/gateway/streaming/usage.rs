use serde_json::{json, Value};

use crate::task_cancellation::DocumentTaskUsageDelta;

/// Normalize the OpenAI-compatible usage families used by every current ostudy
/// provider. The same function is used for ordinary probe responses, ordinary
/// compatibility responses, and final streaming usage chunks.
pub(crate) fn safe_usage_envelope(value: &Value) -> Option<Value> {
    let read = |pointers: &[&str]| {
        pointers.iter().find_map(|pointer| {
            value
                .pointer(pointer)
                .and_then(|item| nonnegative_integer(Some(item)))
        })
    };
    let prompt_tokens = read(&[
        "/usage/prompt_tokens",
        "/usage/input_tokens",
        "/response/usage/prompt_tokens",
        "/response/usage/input_tokens",
        "/data/usage/prompt_tokens",
        "/data/usage/input_tokens",
    ]);
    let completion_tokens = read(&[
        "/usage/completion_tokens",
        "/usage/output_tokens",
        "/response/usage/completion_tokens",
        "/response/usage/output_tokens",
        "/data/usage/completion_tokens",
        "/data/usage/output_tokens",
    ]);
    let total_tokens = read(&[
        "/usage/total_tokens",
        "/response/usage/total_tokens",
        "/data/usage/total_tokens",
    ])
    .or_else(|| {
        prompt_tokens
            .zip(completion_tokens)
            .map(|(prompt, completion)| prompt.saturating_add(completion))
    });
    let cached_tokens = read(&[
        "/usage/prompt_tokens_details/cached_tokens",
        "/usage/prompt_cache_hit_tokens",
        "/response/usage/prompt_tokens_details/cached_tokens",
        "/data/usage/prompt_tokens_details/cached_tokens",
    ]);
    let cache_write_tokens = read(&[
        "/usage/prompt_tokens_details/cache_write_tokens",
        "/response/usage/prompt_tokens_details/cache_write_tokens",
        "/data/usage/prompt_tokens_details/cache_write_tokens",
    ]);
    let reasoning_tokens = read(&[
        "/usage/completion_tokens_details/reasoning_tokens",
        "/response/usage/completion_tokens_details/reasoning_tokens",
        "/data/usage/completion_tokens_details/reasoning_tokens",
    ]);
    let cost = ["/usage/cost", "/response/usage/cost", "/data/usage/cost"]
        .iter()
        .find_map(|pointer| value.pointer(pointer))
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite() && *value >= 0.0)
        .map(Value::from);

    if prompt_tokens.is_some()
        || completion_tokens.is_some()
        || total_tokens.is_some()
        || cached_tokens.is_some()
        || cache_write_tokens.is_some()
        || reasoning_tokens.is_some()
        || cost.is_some()
    {
        return Some(json!({
            "prompt_tokens": prompt_tokens,
            "completion_tokens": completion_tokens,
            "total_tokens": total_tokens,
            "prompt_tokens_details": {
                "cached_tokens": cached_tokens,
                "cache_write_tokens": cache_write_tokens,
            },
            "completion_tokens_details": {
                "reasoning_tokens": reasoning_tokens,
            },
            "cost": cost,
        }));
    }

    let prompt_processed = read(&[
        "/timings/prompt_n",
        "/__verbose/timings/prompt_n",
        "/tokens_evaluated",
        "/__verbose/tokens_evaluated",
    ]);
    let cached = read(&[
        "/timings/cache_n",
        "/__verbose/timings/cache_n",
        "/tokens_cached",
        "/__verbose/tokens_cached",
    ])
    .unwrap_or(0);
    let completion = read(&[
        "/timings/predicted_n",
        "/__verbose/timings/predicted_n",
        "/tokens_predicted",
        "/__verbose/tokens_predicted",
    ]);
    let prompt = prompt_processed.map(|value| value.saturating_add(cached));
    let total = prompt
        .zip(completion)
        .map(|(prompt, completion)| prompt.saturating_add(completion));
    if prompt.is_none() && completion.is_none() && total.is_none() {
        return None;
    }
    Some(json!({
        "prompt_tokens": prompt,
        "completion_tokens": completion,
        "total_tokens": total,
        "prompt_tokens_details": {
            "cached_tokens": cached,
            "cache_write_tokens": Value::Null,
        },
        "completion_tokens_details": {
            "reasoning_tokens": Value::Null,
        },
        "cost": Value::Null,
    }))
}

pub(crate) fn usage_delta(usage: &Value) -> DocumentTaskUsageDelta {
    DocumentTaskUsageDelta {
        prompt_tokens: nonnegative_integer(usage.get("prompt_tokens")),
        completion_tokens: nonnegative_integer(usage.get("completion_tokens")),
        reasoning_tokens: nonnegative_integer(
            usage.pointer("/completion_tokens_details/reasoning_tokens"),
        ),
        total_tokens: nonnegative_integer(usage.get("total_tokens")),
        cached_prompt_tokens: nonnegative_integer(
            usage.pointer("/prompt_tokens_details/cached_tokens"),
        ),
        cache_write_tokens: nonnegative_integer(
            usage.pointer("/prompt_tokens_details/cache_write_tokens"),
        ),
    }
}

pub(super) fn nonnegative_integer(value: Option<&Value>) -> Option<u64> {
    value.and_then(|value| {
        value
            .as_u64()
            .or_else(|| value.as_i64().and_then(|value| u64::try_from(value).ok()))
    })
}
