//! Temporary private bridge while the repository manifest moves from the retired
//! single-file document core to the explicit `documents/` package. The final
//! topology commit removes this file and registers every required package module.

#[path = "documents/mod.rs"]
mod implementation;

pub use implementation::{
    list_documents, set_document_archived, DocumentDto, DocumentGenerationResultDto,
    RetryPdfGenerationInput, SetDocumentArchivedInput,
};
pub(crate) use implementation::{
    get_document_source_path, reconcile_startup, retry_pdf_flashcard_generation,
};
