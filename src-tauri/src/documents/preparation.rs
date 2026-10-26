//! Preparing a document without AI: extraction, rendering and chunking only.
//!
//! With AI features off, a new document is still read, rendered and searched
//! so it can be opened, annotated and linked to cards, but no model is asked
//! for anything and no deck or guide is expected, so it settles as ready.

use crate::{
    database::Database, document_initial_generation::ensure_document_derivatives,
    error::CommandError, paths::AppPaths, pdf_render_backend::PdfRenderRuntime,
};

use super::lifecycle;

/// Prepares a document whose processing state and task were already reserved,
/// and settles it: ready on success, failed or stopped otherwise.
pub(crate) async fn prepare_reserved_document(
    database: &Database,
    paths: &AppPaths,
    render_runtime: PdfRenderRuntime,
    document_id: String,
) -> Result<(), CommandError> {
    match ensure_document_derivatives(database, paths, render_runtime, &document_id).await {
        // A PDF's figure search goes on by itself; nothing here waits for it.
        Ok(_) => lifecycle::record_document_prepared(database, &document_id),
        Err(error) => {
            if error.code == "ai_request_canceled" {
                lifecycle::record_document_cancellation_with_message(
                    database,
                    &document_id,
                    PREPARATION_CANCELED_MESSAGE,
                );
            } else {
                lifecycle::record_document_failure(database, &document_id, &error);
            }
            Err(error)
        }
    }
}

const PREPARATION_CANCELED_MESSAGE: &str =
    "Reading the document was stopped. It remains in Documents.";
