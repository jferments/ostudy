use std::time::Duration;

use reqwest::{redirect::Policy, Client, Method};
use serde_json::Value;
use url::Url;

use super::{
    contracts::{AiGatewayError, ProviderAccess},
    errors::classify_reqwest_error,
    metadata::is_openrouter_url,
    request::ordinary_request,
    response::{read_audio_response, read_json_response, AudioResponse},
};

const CONNECT_TIMEOUT_SECONDS: u64 = 10;

/// A client builder whose HTTPS connections trust the operating system's
/// certificate authorities: the platform verifier, or the PEM authorities the
/// platform supplies where that verifier cannot run (Android).
pub(crate) fn https_client_builder() -> reqwest::ClientBuilder {
    let builder = Client::builder();
    let anchors: Vec<reqwest::Certificate> = crate::platform::https_trust_anchors_pem()
        .iter()
        .filter_map(|pem| reqwest::Certificate::from_pem(pem).ok())
        .collect();
    if anchors.is_empty() {
        builder
    } else {
        builder.tls_certs_only(anchors)
    }
}

#[derive(Clone)]
pub(crate) struct AiGateway {
    pub(super) client: Client,
    pub(super) streaming_client: Client,
}

impl AiGateway {
    pub(crate) fn new() -> Result<Self, String> {
        let client = https_client_builder()
            .redirect(Policy::none())
            .connect_timeout(Duration::from_secs(CONNECT_TIMEOUT_SECONDS))
            .user_agent(concat!("ostudy/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|error| format!("Could not initialize the AI HTTP client: {error}"))?;
        let streaming_client = https_client_builder()
            .redirect(Policy::none())
            .connect_timeout(Duration::from_secs(CONNECT_TIMEOUT_SECONDS))
            .user_agent(concat!("ostudy/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|error| format!("Could not initialize the AI streaming client: {error}"))?;
        Ok(Self {
            client,
            streaming_client,
        })
    }

    pub(crate) async fn get_json<'a>(
        &self,
        url: Url,
        access: impl Into<ProviderAccess<'a>>,
        timeout_ms: i64,
        maximum_bytes: usize,
    ) -> Result<Value, AiGatewayError> {
        self.request_json(
            Method::GET,
            url,
            access.into(),
            None,
            timeout_ms,
            maximum_bytes,
        )
        .await
    }

    pub(crate) async fn post_json<'a>(
        &self,
        url: Url,
        access: impl Into<ProviderAccess<'a>>,
        body: &Value,
        timeout_ms: i64,
        maximum_bytes: usize,
    ) -> Result<Value, AiGatewayError> {
        self.request_json(
            Method::POST,
            url,
            access.into(),
            Some(body),
            timeout_ms,
            maximum_bytes,
        )
        .await
    }

    /// Posts a JSON request whose successful answer is audio bytes, such as
    /// an OpenAI-style `/audio/speech` call. Errors still arrive as JSON.
    pub(crate) async fn post_json_for_audio<'a>(
        &self,
        url: Url,
        access: impl Into<ProviderAccess<'a>>,
        body: &Value,
        timeout_ms: i64,
        maximum_bytes: usize,
    ) -> Result<AudioResponse, AiGatewayError> {
        let openrouter = is_openrouter_url(&url);
        let request = ordinary_request(
            &self.client,
            Method::POST,
            url,
            access.into(),
            Some(body),
            timeout_ms,
            "audio/*, application/json",
        );
        let response = request.send().await.map_err(classify_reqwest_error)?;
        read_audio_response(response, maximum_bytes, openrouter).await
    }

    async fn request_json(
        &self,
        method: Method,
        url: Url,
        access: ProviderAccess<'_>,
        body: Option<&Value>,
        timeout_ms: i64,
        maximum_bytes: usize,
    ) -> Result<Value, AiGatewayError> {
        let openrouter = is_openrouter_url(&url);
        let request = ordinary_request(
            &self.client,
            method,
            url,
            access,
            body,
            timeout_ms,
            "application/json",
        );
        let response = request.send().await.map_err(classify_reqwest_error)?;
        read_json_response(response, maximum_bytes, openrouter).await
    }
}
