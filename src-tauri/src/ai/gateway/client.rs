use std::time::Duration;

use reqwest::{redirect::Policy, Client, Method};
use serde_json::Value;
use url::Url;

use super::{
    contracts::AiGatewayError, errors::classify_reqwest_error, metadata::is_openrouter_url,
    request::ordinary_request, response::read_json_response,
};

const CONNECT_TIMEOUT_SECONDS: u64 = 10;

#[derive(Clone)]
pub(crate) struct AiGateway {
    pub(super) client: Client,
    pub(super) streaming_client: Client,
}

impl AiGateway {
    pub(crate) fn new() -> Result<Self, String> {
        let client = Client::builder()
            .redirect(Policy::none())
            .connect_timeout(Duration::from_secs(CONNECT_TIMEOUT_SECONDS))
            .user_agent(concat!("ostudy/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|error| format!("Could not initialize the AI HTTP client: {error}"))?;
        let streaming_client = Client::builder()
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

    pub(crate) async fn get_json(
        &self,
        url: Url,
        bearer: Option<&str>,
        timeout_ms: i64,
        maximum_bytes: usize,
    ) -> Result<Value, AiGatewayError> {
        self.request_json(Method::GET, url, bearer, None, timeout_ms, maximum_bytes)
            .await
    }

    pub(crate) async fn post_json(
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
        let openrouter = is_openrouter_url(&url);
        let request = ordinary_request(&self.client, method, url, bearer, body, timeout_ms);
        let response = request.send().await.map_err(classify_reqwest_error)?;
        read_json_response(response, maximum_bytes, openrouter).await
    }
}
