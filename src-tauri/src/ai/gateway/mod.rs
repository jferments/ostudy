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

pub(crate) use client::AiGateway;
pub(crate) use contracts::{AiGatewayError, SafeAiError};
pub(crate) use errors::error;

#[cfg(test)]
mod tests;
