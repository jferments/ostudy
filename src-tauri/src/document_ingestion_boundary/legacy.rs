// Keep the existing PDF parser/render/OCR implementation private. The child
// adapter module may access its private extraction structures, but no parser
// type is re-exported across the public document-ingestion boundary.
include!("../document_ingestion.rs");

#[path = "adapters.rs"]
pub(crate) mod adapters;
