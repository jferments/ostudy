use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SafeAiError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
    pub retry_after_seconds: Option<u64>,
}

#[derive(Clone, Debug)]
pub(crate) struct AiGatewayError {
    pub safe: SafeAiError,
}
