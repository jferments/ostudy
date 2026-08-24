mod contracts;
mod errors;
mod files;
mod ocr;
mod pipeline;
mod progress;
mod recovery;
mod rendering;
mod validation;

#[cfg(test)]
pub(crate) use contracts::{ExtractionMode, IngestedPage, OcrIngestionSummary};
pub(crate) use contracts::{IngestedDocument, IngestionProgress};
pub(crate) use pipeline::ingest_pdf;
pub(crate) use progress::update_ingestion_progress;

pub(crate) fn active_pdf_extraction_backend() -> &'static str {
    crate::pdf_backend::active_backend_id()
}

pub(crate) fn active_pdf_render_backend() -> &'static str {
    crate::pdf_render_backend::active_backend_id()
}
