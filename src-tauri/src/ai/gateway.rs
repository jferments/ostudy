use std::time::Duration;

use reqwest::{redirect::Policy, Client, Method};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use url::Url;

const CONNECT_TIMEOUT_SECONDS: u64 = 10;
const MAX_ERROR_BODY_BYTES: usize = 32 * 1024;

#[derive(Clone)]
pub struct AiGateway {
    client: Client,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SafeAiError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
    pub retry_after_seconds: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct AiGatewayError {
    pub safe: SafeAiError,
}

impl AiGateway {
    pub fn new() -> Result<Self, String> {
        let client = Client::builder()
            .redirect(Policy::none())
            .connect_timeout(Duration::from_secs(CONNECT_TIMEOUT_SECONDS))
            .user_agent(concat!("ostudy/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|error| format!("Could not initialize the AI HTTP client: {error}"))?;
        Ok(Self { client })
    }

    pub async fn get_json(
        &self,
        url: Url,
        bearer: Option<&str>,
        timeout_ms: i64,
        maximum_bytes: usize,
    ) -> Result<Value, AiGatewayError> {
        self.request_json(Method::GET, url, bearer, None, timeout_ms, maximum_bytes)
            .await
    }

    pub async fn post_json(
        &self,
        url: Url,
        bearer: Option<&str>,
        body: &Value,
        timeout_ms: i64,
        maximum_bytes: usize,
    ) -> Result<Value, AiGatewayError> {
        self.request_json(
            Method::POST,
            url,
            bearer,
            Some(body),
            timeout_ms,
            maximum_bytes,
        )
        .await
    }

    async fn request_json(
        &self,
        method: Method,
        url: Url,
        bearer: Option<&str>,
        body: Option<&Value>,
        timeout_ms: i64,
        maximum_bytes: usize,
    ) -> Result<Value, AiGatewayError> {
        let mut request = self
            .client
            .request(method, url)
            .timeout(Duration::from_millis(timeout_ms as u64))
            .header(reqwest::header::ACCEPT, "application/json");
        if let Some(secret) = bearer {
            request = request.bearer_auth(secret);
        }
        if let Some(body) = body {
            request = request.json(body);
        }

        let mut response = request.send().await.map_err(classify_reqwest_error)?;
        let status = response.status();
        let retry_after_seconds = response
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok());

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
            return Err(classify_http_status(status.as_u16(), retry_after_seconds));
        }

        serde_json::from_slice(&bytes).map_err(|_| {
            error(
                "ai_provider_protocol_error",
                "The AI provider returned a response that was not valid JSON.",
                false,
                None,
            )
        })
    }
}

fn classify_reqwest_error(error_value: reqwest::Error) -> AiGatewayError {
    if error_value.is_timeout() {
        return error(
            "ai_request_timeout",
            "The AI provider did not respond before the configured timeout.",
            true,
            None,
        );
    }
    if error_value.is_connect() {
        return error(
            "ai_connection_failed",
            "ostudy could not connect to the AI provider endpoint.",
            true,
            None,
        );
    }
    error(
        "ai_network_error",
        "The AI provider request failed before a valid response was received.",
        true,
        None,
    )
}

fn classify_http_status(status: u16, retry_after_seconds: Option<u64>) -> AiGatewayError {
    match status {
        401 => error(
            "ai_http_unauthorized",
            "The AI provider rejected this credential.",
            false,
            None,
        ),
        403 => error(
            "ai_http_forbidden",
            "The AI provider does not permit this request.",
            false,
            None,
        ),
        404 => error(
            "ai_http_not_found",
            "The configured AI endpoint or model was not found.",
            false,
            None,
        ),
        408 => error(
            "ai_request_timeout",
            "The AI provider timed out while handling the request.",
            true,
            None,
        ),
        429 => error(
            "ai_http_rate_limited",
            "The AI provider rate limit was reached.",
            true,
            retry_after_seconds,
        ),
        500..=599 => error(
            "ai_server_unavailable",
            "The AI provider is temporarily unavailable.",
            true,
            retry_after_seconds,
        ),
        _ => error(
            "ai_http_error",
            "The AI provider rejected the request.",
            false,
            retry_after_seconds,
        ),
    }
}

pub fn error(
    code: &str,
    message: &str,
    retryable: bool,
    retry_after_seconds: Option<u64>,
) -> AiGatewayError {
    AiGatewayError {
        safe: SafeAiError {
            code: code.to_string(),
            message: message.to_string(),
            retryable,
            retry_after_seconds,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_rate_limits_without_exposing_provider_bodies() {
        let failure = classify_http_status(429, Some(17));
        assert_eq!(failure.safe.code, "ai_http_rate_limited");
        assert_eq!(failure.safe.retry_after_seconds, Some(17));
    }
}
