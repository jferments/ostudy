//! AI provider profile aggregate, task defaults, credential metadata, and
//! write-only credential coordination.

pub(crate) mod commands;
mod contracts;
mod defaults;
mod policy;
mod projections;
mod repository;
mod service;

pub(crate) use contracts::{ProfileRequestSnapshot, ProfileSnapshot};
pub(crate) use repository::load_profile_snapshot;
pub(crate) use service::{load_default_profile_request, load_profile_request};

#[cfg(test)]
mod tests;
