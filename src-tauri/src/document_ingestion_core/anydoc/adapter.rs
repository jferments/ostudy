use std::path::Path;

use crate::{
    anydoc_document_adapter,
    document_domain::DocumentFormat,
    error::CommandError,
};

use super::super::contracts::{
    AdapterExtraction, AdapterRequest, DocumentAdapter, DocumentAdapterIdentity,
    IngestionProgress, IngestionProgressSink,
};

pub(crate) struct AnyDocAdapter;

impl AnyDocAdapter {
    pub(crate) const fn new() -> Self {
        Self
    }
}

impl DocumentAdapter for AnyDocAdapter {
    fn name(&self) -> &'static str {
        "anydoc"
    }

    fn supports(&self, format: DocumentFormat) -> bool {
        anydoc_document_adapter::supports(format)
    }

    fn identity(
        &self,
        format: DocumentFormat,
    ) -> Result<DocumentAdapterIdentity, CommandError> {
        if !self.supports(format) {
            return Err(CommandError::new(
                "document_ingestion_adapter_unavailable",
                "The AnyDoc adapter does not support this document format.",
            ));
        }
        Ok(DocumentAdapterIdentity {
            adapter_backend: anydoc_document_adapter::ANYDOC_ADAPTER_BACKEND,
            extraction_backend: anydoc_document_adapter::ANYDOC_EXTRACTION_BACKEND,
            render_backend: None,
        })
    }

    fn probe(&self, format: DocumentFormat, source_path: &Path) -> Result<bool, CommandError> {
        if !self.supports(format) {
            return Ok(false);
        }
        anydoc_document_adapter::probe(format, source_path)
    }

    fn extract(
        &self,
        request: AdapterRequest<'_>,
        progress: &mut dyn IngestionProgressSink,
    ) -> Result<AdapterExtraction, CommandError> {
        request.cancellation.check()?;
        let identity = self.identity(request.format)?;
        let canonical = anydoc_document_adapter::extract(
            request.paths,
            request.document_id,
            request.source_path,
            request.format,
            request.maximum_source_units,
            |completed_units, total_units| {
                progress.report(IngestionProgress::Extracting {
                    completed_units,
                    total_units,
                })
            },
        )?;
        request.cancellation.check()?;
        Ok(AdapterExtraction {
            identity,
            canonical,
            compatibility: None,
        })
    }
}
