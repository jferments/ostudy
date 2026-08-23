//! Temporary private bridge from the historical physical filename to the
//! modular document-ingestion port. The final topology commit registers the
//! package directly and removes this file.

#[path = "document_ingestion_core/mod.rs"]
mod implementation;

pub(crate) use implementation::*;
