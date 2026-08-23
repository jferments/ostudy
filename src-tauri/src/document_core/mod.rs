mod commands;
mod contracts;
mod generation;
mod lifecycle;
mod reconciliation;
mod repository;
mod source;

pub use commands::{list_documents, set_document_archived};
pub use contracts::{
    DocumentDto, DocumentGenerationResultDto, RetryPdfGenerationInput, SetDocumentArchivedInput,
};
pub(crate) use generation::retry_pdf_flashcard_generation;
pub(crate) use reconciliation::reconcile_startup;
pub(crate) use source::get_document_source_path;
