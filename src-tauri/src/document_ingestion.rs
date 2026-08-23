//! Temporary private bridge from the historical PDF-pipeline filename to the
//! modular native PDF ingestion package. The final topology commit registers the
//! package directly and removes this file.

#[path = "pdf_document_ingestion_core/mod.rs"]
mod implementation;

pub(crate) use implementation::*;
