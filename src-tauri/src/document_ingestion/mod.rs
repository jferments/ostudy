pub(crate) mod canonical;
pub(crate) mod contracts;
mod detection;
mod image_set;
mod pdf;
mod persistence;
mod progress;
mod registry;
pub(crate) mod support;
mod xberg;

pub(crate) use contracts::IngestionProgress;
pub(crate) use detection::detected_document_format;
pub(crate) use persistence::{persist_ingested_document, persist_prepared_image_set};
pub(crate) use progress::update_ingestion_progress;
pub(crate) use registry::{
    active_adapter_backend, active_extraction_backend, active_render_backend, ingest_document,
    ingest_image_set, ingest_pdf,
};
