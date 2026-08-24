use reqwest::Response;
use serde_json::Value;

use crate::task_cancellation::CancellationSignal;

use super::{
    contracts::AiGatewayError,
    errors::{
        classify_provider_error_response, classify_provider_error_value, classify_reqwest_error,
        error, provider_error_object,
    },
    metadata::safe_metadata_string,
};

pub(super) const MAX_ERROR_BODY_BYTES: usize = 32 * 1024;

pub(super) async fn read_json_response(
    mut response: Response,
    maximum_bytes: usize,
    openrouter: bool,
) -> Result<Value, AiGatewayError> {
    let status = response.status();
    let retry_after_seconds = retry_after_seconds(&response);
    let generation_id = generation_id(&response);
    let body_limit = if status.is_success() {
        maximum_bytes
    } else {
        maximum_bytes.min(MAX_ERROR_BODY_BYTES)
    };
    if response
        .content_length()
        .is_some_and(|length| length > body_limit as u64)
    {
        return Err(error(
            "ai_response_too_large",
            "The AI provider returned a response larger than ostudy accepts.",
            false,
            None,
        ));
    }

    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(classify_reqwest_error)? {
        if bytes.len().saturating_add(chunk.len()) > body_limit {
            return Err(error(
                "ai_response_too_large",
                "The AI provider returned a response larger than ostudy accepts.",
                false,
                None,
            ));
        }
        bytes.extend_from_slice(&chunk);
    }

    if status.is_redirection() {
        return Err(error(
            "ai_redirect_rejected",
            "The AI provider attempted to redirect the request. Save the final trusted endpoint explicitly.",
            false,
            None,
        ));
    }
    if !status.is_success() {
        return Err(classify_provider_error_response(
            status.as_u16(),
            retry_after_seconds,
            &bytes,
            openrouter,
            generation_id.as_deref(),
            false,
        ));
    }

    let value: Value = serde_json::from_slice(&bytes).map_err(|_| {
        error(
            "ai_provider_protocol_error",
            "The AI provider returned a response that was not valid JSON.",
            false,
            None,
        )
    })?;
    if openrouter && provider_error_object(&value).is_some() {
        return Err(classify_provider_error_value(
            &value,
            None,
            retry_after_seconds,
            true,
            generation_id.as_deref(),
            false,
        ));
    }
    Ok(value)
}

pub(super) async fn read_bounded_body(
    response: &mut Response,
    maximum_bytes: usize,
    cancellation: &CancellationSignal,
) -> Result<Vec<u8>, AiGatewayError> {
    let mut bytes = Vec::new();
    loop {
        let next = tokio::select! {
            _ = cancellation.canceled() => return Err(super::errors::canceled_error()),
            result = response.chunk() => result.map_err(classify_reqwest_error)?,
        };
        let Some(chunk) = next else {
            break;
        };
        if bytes.len().saturating_add(chunk.len()) > maximum_bytes {
            return Err(error(
                "ai_response_too_large",
                "The AI provider returned a response larger than ostudy accepts.",
                false,
                None,
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

pub(super) fn retry_after_seconds(response: &Response) -> Option<u64> {
    response
        .headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
}

pub(super) fn generation_id(response: &Response) -> Option<String> {
    response
        .headers()
        .get("x-generation-id")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| safe_metadata_string(value, 200))
}
