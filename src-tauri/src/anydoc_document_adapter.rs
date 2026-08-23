//! Temporary private bridge from the historical AnyDoc adapter filename to the
//! modular backend package. The final topology commit registers the package
//! directly and removes this file.

#[path = "anydoc_document_adapter_core/mod.rs"]
mod implementation;

pub(crate) use implementation::*;
