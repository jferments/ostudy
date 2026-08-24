mod anydoc;
mod contracts;
mod pdf;
mod persistence;
mod progress;
mod registry;
mod support;

pub(crate) use contracts::IngestionProgress;
pub(crate) use persistence::persist_ingested_document;
pub(crate) use progress::update_ingestion_progress;
pub(crate) use registry::{
    active_adapter_backend, active_extraction_backend, active_render_backend, ingest_document,
    ingest_pdf,
};
