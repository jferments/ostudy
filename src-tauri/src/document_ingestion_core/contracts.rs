use std::path::Path;

use crate::{
    document_canonical::CanonicalIngestion,
    document_domain::DocumentFormat,
    error::CommandError,
    paths::AppPaths,
    pdf_document_ingestion::IngestedDocument,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DocumentAdapterIdentity {
    pub(crate) adapter_backend: &'static str,
    pub(crate) extraction_backend: &'static str,
    pub(crate) render_backend: Option<&'static str>,
}

impl DocumentAdapterIdentity {
    pub(crate) const fn render_backend_or_none(self) -> &'static str {
        match self.render_backend {
            Some(value) => value,
            None => "none",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum IngestionProgress {
    Extracting {
        completed_units: usize,
        total_units: usize,
    },
    Rendering {
        completed_units: usize,
        total_units: usize,
    },
    Ocr {
        completed_units: usize,
        total_units: usize,
    },
}

#[derive(Debug)]
pub(crate) enum CompatibilityProjection {
    Pdf(IngestedDocument),
}

#[derive(Debug)]
pub(crate) struct AdapterExtraction {
    pub(crate) identity: DocumentAdapterIdentity,
    pub(crate) canonical: CanonicalIngestion,
    pub(crate) compatibility: Option<CompatibilityProjection>,
}

#[derive(Clone, Copy)]
pub(crate) struct AdapterRequest<'a> {
    pub(crate) paths: &'a AppPaths,
    pub(crate) document_id: &'a str,
    pub(crate) source_path: &'a Path,
    pub(crate) format: DocumentFormat,
    pub(crate) maximum_source_units: usize,
    pub(crate) cancellation: &'a dyn DocumentCancellation,
}

pub(crate) trait DocumentCancellation {
    fn check(&self) -> Result<(), CommandError>;
}

pub(crate) trait IngestionProgressSink {
    fn report(&mut self, progress: IngestionProgress) -> Result<(), CommandError>;
}

impl<F> IngestionProgressSink for F
where
    F: FnMut(IngestionProgress) -> Result<(), CommandError>,
{
    fn report(&mut self, progress: IngestionProgress) -> Result<(), CommandError> {
        self(progress)
    }
}

/// Stable application-owned port implemented by every document parser backend.
///
/// Third-party parser, renderer, OCR, subprocess, or RPC types terminate inside
/// the implementation. Downstream code receives only canonical ostudy records
/// and an explicitly isolated compatibility projection.
pub(crate) trait DocumentAdapter {
    fn name(&self) -> &'static str;

    fn supports(&self, format: DocumentFormat) -> bool;

    fn identity(
        &self,
        format: DocumentFormat,
    ) -> Result<DocumentAdapterIdentity, CommandError>;

    fn probe(&self, format: DocumentFormat, source_path: &Path) -> Result<bool, CommandError>;

    fn extract(
        &self,
        request: AdapterRequest<'_>,
        progress: &mut dyn IngestionProgressSink,
    ) -> Result<AdapterExtraction, CommandError>;
}
