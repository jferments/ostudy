//! Shared source-grounded material for document generators and conversational
//! tutoring. This module has no dependency on review submission, scheduling,
//! curriculum planning, or a persistent learner model.
//!
//! Source records and interpretable units live in the library. Hosted embeddings
//! are a replaceable index over those records, never their identity or truth.

pub(crate) mod adapters;
pub(crate) mod commands;
pub(crate) mod contracts;
pub(crate) mod extraction;
mod images;
mod index;
pub(crate) mod preparation;
mod preparation_store;
pub(crate) mod repository;
pub(crate) mod retrieval;
pub(crate) mod runtime;

pub(crate) use runtime::has_active_tasks;

pub(crate) fn reconcile_startup(
    database: &crate::database::Database,
) -> Result<(), crate::error::CommandError> {
    preparation_store::reconcile_startup(database)
}
