//! Native-only AI transport with bounded ordinary JSON requests and explicit
//! cancellable document streaming.
//!
//! Provider adapters own routes and request bodies. This package owns HTTP,
//! redirect rejection, response bounds, SSE reconstruction, safe transport
//! errors, bounded retry, and usage normalization.

mod client;
mod contracts;
mod errors;
mod metadata;
mod request;
mod response;
mod retry;
mod streaming;
#[cfg(any(target_os = "android", test))]
mod system_roots;

pub(crate) use client::{https_client_builder, AiGateway};
pub(crate) use contracts::{AiGatewayError, ProviderAccess, ProviderWire, SafeAiError};
pub(crate) use errors::error;
pub(crate) use response::AudioResponse;
pub(crate) use streaming::sse::{anthropic_finish_reason, anthropic_usage_envelope};
pub(crate) use streaming::usage::safe_usage_envelope as normalized_usage;
pub(crate) use streaming::{observe_output, OutputObserver};

#[cfg(test)]
pub(crate) use streaming::sse::SseChatAccumulator;

#[cfg(test)]
mod tests;
