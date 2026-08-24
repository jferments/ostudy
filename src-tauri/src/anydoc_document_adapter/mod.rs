mod adapter;
mod assets;
mod chunking;
mod detection;
mod errors;
mod model;
mod normalization;
mod segmentation;
mod support;

pub(crate) const ANYDOC_ADAPTER_BACKEND: &str = "anydoc-canonical-adapter-v1";
pub(crate) const ANYDOC_EXTRACTION_BACKEND: &str = "anydoc-0.2.3";

pub(crate) use adapter::extract;
pub(crate) use detection::{detected_document_format, expected_format, probe, supports};
