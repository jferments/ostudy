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

/// The wire family a provider request speaks. It decides authentication
/// headers, the streaming body, and how server-sent events are decoded; the
/// provider adapter chooses it, never the URL.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProviderWire {
    /// OpenAI-style Chat Completions with bearer authentication.
    OpenAiCompatible,
    /// Anthropic's Messages API: `x-api-key`, a pinned API version, and
    /// typed `message_*` / `content_block_*` events.
    AnthropicMessages,
    /// Google's Gemini API: `x-goog-api-key`, `generateContent` bodies, and
    /// `streamGenerateContent` chunks that are whole partial responses.
    GeminiGenerateContent,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ProviderAccess<'a> {
    pub secret: Option<&'a str>,
    pub wire: ProviderWire,
}

impl<'a> ProviderAccess<'a> {
    pub(crate) fn new(wire: ProviderWire, secret: Option<&'a str>) -> Self {
        Self { secret, wire }
    }
}

/// Existing OpenAI-compatible callers pass their bearer secret unchanged.
impl<'a> From<Option<&'a str>> for ProviderAccess<'a> {
    fn from(secret: Option<&'a str>) -> Self {
        Self::new(ProviderWire::OpenAiCompatible, secret)
    }
}
