//! Shared source-grounded material for document generators and conversational
//! tutoring. This module has no dependency on review submission, scheduling,
//! curriculum planning, or a persistent learner model.
//!
//! Source records and interpretable units live in the library. Hosted embeddings
//! are a replaceable index over those records, never their identity or truth.

pub(crate) mod adapters;
pub(crate) mod authoring;
pub(crate) mod commands;
pub(crate) mod contracts;
pub(crate) mod extraction;
mod images;
mod index;
pub(crate) mod preparation;
mod preparation_store;
pub(crate) mod relationships;
pub(crate) mod repository;
pub(crate) mod retrieval;
pub(crate) mod runtime;

pub(crate) use images::load_images;
pub(crate) use runtime::has_active_tasks;

pub(crate) fn reconcile_startup(
    database: &crate::database::Database,
) -> Result<(), crate::error::CommandError> {
    preparation_store::reconcile_startup(database)
}

/// Derived semantic projections are device-local until their own merge contract
/// exists. Canonical documents, LLM-generated guides and cards still transfer
/// through their selected domains. Full backups retain the shared graph.
pub(crate) fn sanitize_for_device_transfer(
    connection: &rusqlite::Connection,
) -> Result<(), crate::error::CommandError> {
    connection
        .execute_batch(
            "DELETE FROM knowledge_artifact_links;
         DELETE FROM knowledge_units;
         DELETE FROM knowledge_sources;
         DELETE FROM knowledge_preparations;",
        )
        .map_err(crate::error::CommandError::database)
}
