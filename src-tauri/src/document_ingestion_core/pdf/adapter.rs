use std::{fs::File, io::Read, path::Path};

use crate::{
    document_domain::DocumentFormat,
    error::CommandError,
    pdf_document_ingestion,
    pdf_render_backend::PdfRenderRuntime,
};

use super::canonicalize::canonicalize_pdf;
use super::super::contracts::{
    AdapterExtraction, AdapterRequest, CompatibilityProjection, DocumentAdapter,
    DocumentAdapterIdentity, IngestionProgress, IngestionProgressSink,
};

pub(crate) const PDF_ADAPTER_BACKEND: &str = "pdf-canonical-adapter-v1";

pub(crate) struct NativePdfAdapter<'a> {
    render_runtime: Option<&'a PdfRenderRuntime>,
}

impl<'a> NativePdfAdapter<'a> {
    pub(crate) const fn catalog() -> Self {
        Self {
            render_runtime: None,
        }
    }

    pub(crate) const fn with_runtime(render_runtime: &'a PdfRenderRuntime) -> Self {
        Self {
            render_runtime: Some(render_runtime),
        }
    }

    pub(crate) fn active_identity(&self) -> DocumentAdapterIdentity {
        DocumentAdapterIdentity {
            adapter_backend: PDF_ADAPTER_BACKEND,
            extraction_backend: pdf_document_ingestion::active_pdf_extraction_backend(),
            render_backend: Some(pdf_document_ingestion::active_pdf_render_backend()),
        }
    }
}

impl DocumentAdapter for NativePdfAdapter<'_> {
    fn name(&self) -> &'static str {
        "native-pdf"
    }

    fn supports(&self, format: DocumentFormat) -> bool {
        format == DocumentFormat::Pdf
    }

    fn identity(
        &self,
        format: DocumentFormat,
    ) -> Result<DocumentAdapterIdentity, CommandError> {
        if !self.supports(format) {
            return Err(CommandError::new(
                "document_ingestion_adapter_unavailable",
                "The native PDF adapter does not support this document format.",
            ));
        }
        Ok(self.active_identity())
    }

    fn probe(&self, format: DocumentFormat, source_path: &Path) -> Result<bool, CommandError> {
        if !self.supports(format) {
            return Ok(false);
        }
        let mut file = File::open(source_path).map_err(CommandError::io)?;
        let mut signature = [0_u8; 5];
        let read = file.read(&mut signature).map_err(CommandError::io)?;
        Ok(read == signature.len() && signature == *b"%PDF-")
    }

    fn extract(
        &self,
        request: AdapterRequest<'_>,
        progress: &mut dyn IngestionProgressSink,
    ) -> Result<AdapterExtraction, CommandError> {
        request.cancellation.check()?;
        let render_runtime = self.render_runtime.ok_or_else(|| {
            CommandError::new(
                "document_ingestion_runtime_unavailable",
                "The native PDF adapter was selected without its rendering runtime.",
            )
        })?;
        let identity = self.identity(request.format)?;
        let legacy_pdf = pdf_document_ingestion::ingest_pdf(
            request.paths,
            render_runtime,
            request.document_id,
            request.source_path,
            request.maximum_source_units,
            |event| {
                progress.report(match event {
                    pdf_document_ingestion::IngestionProgress::Rendering {
                        completed_units,
                        total_units,
                    } => IngestionProgress::Rendering {
                        completed_units,
                        total_units,
                    },
                    pdf_document_ingestion::IngestionProgress::Ocr {
                        completed_units,
                        total_units,
                    } => IngestionProgress::Ocr {
                        completed_units,
                        total_units,
                    },
                })
            },
        )?;
        request.cancellation.check()?;
        let canonical = canonicalize_pdf(
            request.document_id,
            request.source_path,
            identity,
            &legacy_pdf,
        )?;
        Ok(AdapterExtraction {
            identity,
            canonical,
            compatibility: Some(CompatibilityProjection::Pdf(legacy_pdf)),
        })
    }
}
