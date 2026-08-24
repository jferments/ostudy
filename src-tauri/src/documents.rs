//! Temporary private bridge while the repository manifest moves from the retired
//! single-file document core to the explicit package. The final topology commit
//! removes this file, restores the package at `documents/`, and registers every
//! required package module in the repository manifest.

#[path = "document_core/mod.rs"]
mod implementation;

pub(crate) use implementation::{
    get_document_source_path, reconcile_startup, retry_pdf_flashcard_generation,
};
pub use implementation::{
    list_documents, set_document_archived, DocumentDto, DocumentGenerationResultDto,
    RetryPdfGenerationInput, SetDocumentArchivedInput,
};
