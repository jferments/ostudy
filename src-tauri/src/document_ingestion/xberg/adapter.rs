use std::path::Path;

use crate::{document_domain::DocumentFormat, error::CommandError, xberg_document_adapter};

use super::super::contracts::{
    AdapterExtraction, AdapterRequest, DocumentAdapter, DocumentAdapterIdentity,
    IngestionProgressSink,
};

pub(crate) struct XbergAdapter;

impl XbergAdapter {
    pub(crate) const fn new() -> Self {
        Self
    }
}

impl DocumentAdapter for XbergAdapter {
    fn supports(&self, format: DocumentFormat) -> bool {
        xberg_document_adapter::supports(format)
    }

    fn identity(&self, format: DocumentFormat) -> Result<DocumentAdapterIdentity, CommandError> {
        if !self.supports(format) {
            return Err(CommandError::new(
                "document_ingestion_adapter_unavailable",
                "The Xberg adapter does not support this document format.",
            ));
        }
        Ok(DocumentAdapterIdentity {
            adapter_backend: xberg_document_adapter::XBERG_ADAPTER_BACKEND,
            extraction_backend: xberg_document_adapter::XBERG_EXTRACTION_BACKEND,
            render_backend: None,
        })
    }

    fn probe(&self, format: DocumentFormat, source_path: &Path) -> Result<bool, CommandError> {
        if !self.supports(format) {
            return Ok(false);
        }
        xberg_document_adapter::probe(format, source_path)
    }

    fn extract(
        &self,
        request: AdapterRequest<'_>,
        progress: &mut dyn IngestionProgressSink,
    ) -> Result<AdapterExtraction, CommandError> {
        request.cancellation.check()?;
        let identity = self.identity(request.format)?;
        let canonical = xberg_document_adapter::extract(
            request.paths,
            request.document_id,
            request.source_path,
            request.format,
            request.maximum_source_units,
            request.cancellation,
            progress,
        )?;
        request.cancellation.check()?;
        Ok(AdapterExtraction {
            identity,
            canonical,
            compatibility: None,
        })
    }
}
