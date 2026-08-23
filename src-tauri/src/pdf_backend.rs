//! Temporary private bridge from the historical PDF backend filename to the
//! modular extraction package. The final topology commit registers the package
//! directly and removes this file.

#[path = "pdf_backend_core/mod.rs"]
mod implementation;

pub(crate) use implementation::*;
