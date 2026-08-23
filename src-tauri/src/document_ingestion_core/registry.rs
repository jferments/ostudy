use std::path::Path;

use crate::{
    document_domain::DocumentFormat,
    error::CommandError,
    paths::AppPaths,
    pdf_render_backend::PdfRenderRuntime,
    task_cancellation,
};

use super::{
    anydoc::adapter::AnyDocAdapter,
    contracts::{
        AdapterExtraction, AdapterRequest, DocumentAdapter, DocumentAdapterIdentity,
        DocumentCancellation, IngestionProgress,
    },
    pdf::adapter::NativePdfAdapter,
};

pub(crate) struct DocumentAdapterRegistry<'a> {
    native_pdf: NativePdfAdapter<'a>,
    anydoc: AnyDocAdapter,
}

impl DocumentAdapterRegistry<'static> {
    fn catalog() -> Self {
        Self {
            native_pdf: NativePdfAdapter::catalog(),
            anydoc: AnyDocAdapter::new(),
        }
    }
}

impl<'a> DocumentAdapterRegistry<'a> {
    fn with_runtime(render_runtime: &'a PdfRenderRuntime) -> Self {
        Self {
            native_pdf: NativePdfAdapter::with_runtime(render_runtime),
            anydoc: AnyDocAdapter::new(),
        }
    }

    fn adapter_for_format(&self, format: DocumentFormat) -> Option<&dyn DocumentAdapter> {
        let adapters: [&dyn DocumentAdapter; 2] = [&self.native_pdf, &self.anydoc];
        adapters.into_iter().find(|adapter| adapter.supports(format))
    }

    fn identity_for_format(
        &self,
        format: DocumentFormat,
    ) -> Result<DocumentAdapterIdentity, CommandError> {
        self.adapter_for_format(format)
            .ok_or_else(adapter_unavailable)?
            .identity(format)
    }

    fn select(
        &self,
        format: DocumentFormat,
        source_path: &Path,
    ) -> Result<&dyn DocumentAdapter, CommandError> {
        let adapter = self
            .adapter_for_format(format)
            .ok_or_else(adapter_unavailable)?;
        if !adapter.probe(format, source_path)? {
            return Err(CommandError::new(
                "document_source_format_mismatch",
                if format == DocumentFormat::Pdf {
                    "The managed source bytes do not match the PDF format recorded by oStudy."
                } else {
                    "The managed source bytes do not match the document format recorded by oStudy."
                },
            ));
        }
        Ok(adapter)
    }
}

struct TaskCancellation<'a> {
    document_id: &'a str,
}

impl DocumentCancellation for TaskCancellation<'_> {
    fn check(&self) -> Result<(), CommandError> {
        if task_cancellation::is_document_task_canceled(self.document_id) {
            Err(CommandError::new(
                "ai_request_canceled",
                "The document generation request was canceled.",
            ))
        } else {
            Ok(())
        }
    }
}

pub(crate) fn active_identity(
    format: DocumentFormat,
) -> Result<DocumentAdapterIdentity, CommandError> {
    DocumentAdapterRegistry::catalog().identity_for_format(format)
}

pub(crate) fn active_pdf_extraction_backend() -> &'static str {
    active_identity(DocumentFormat::Pdf)
        .expect("the native PDF adapter must remain registered")
        .extraction_backend
}

pub(crate) fn active_pdf_render_backend() -> &'static str {
    active_identity(DocumentFormat::Pdf)
        .expect("the native PDF adapter must remain registered")
        .render_backend_or_none()
}

pub(crate) fn active_adapter_backend(
    format: DocumentFormat,
) -> Result<&'static str, CommandError> {
    Ok(active_identity(format)?.adapter_backend)
}

pub(crate) fn active_extraction_backend(
    format: DocumentFormat,
) -> Result<&'static str, CommandError> {
    Ok(active_identity(format)?.extraction_backend)
}

pub(crate) fn active_render_backend(
    format: DocumentFormat,
) -> Result<&'static str, CommandError> {
    Ok(active_identity(format)?.render_backend_or_none())
}

pub(crate) fn ingest_document<F>(
    format: DocumentFormat,
    paths: &AppPaths,
    render_runtime: &PdfRenderRuntime,
    document_id: &str,
    source_path: &Path,
    maximum_source_units: usize,
    mut report_progress: F,
) -> Result<AdapterExtraction, CommandError>
where
    F: FnMut(IngestionProgress) -> Result<(), CommandError>,
{
    let registry = DocumentAdapterRegistry::with_runtime(render_runtime);
    let adapter = registry.select(format, source_path)?;
    let cancellation = TaskCancellation { document_id };
    let request = AdapterRequest {
        paths,
        document_id,
        source_path,
        format,
        maximum_source_units,
        cancellation: &cancellation,
    };
    adapter.extract(request, &mut report_progress)
}

pub(crate) fn ingest_pdf<F>(
    paths: &AppPaths,
    render_runtime: &PdfRenderRuntime,
    document_id: &str,
    source_path: &Path,
    maximum_source_units: usize,
    report_progress: F,
) -> Result<AdapterExtraction, CommandError>
where
    F: FnMut(IngestionProgress) -> Result<(), CommandError>,
{
    ingest_document(
        DocumentFormat::Pdf,
        paths,
        render_runtime,
        document_id,
        source_path,
        maximum_source_units,
        report_progress,
    )
}

fn adapter_unavailable() -> CommandError {
    CommandError::new(
        "document_ingestion_adapter_unavailable",
        "This oStudy build does not include an ingestion adapter for the saved document format.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn accepts_object_safe_adapter(_adapter: &dyn DocumentAdapter) {}

    #[test]
    fn registry_port_is_object_safe_and_keeps_current_backend_selection() {
        let registry = DocumentAdapterRegistry::catalog();
        accepts_object_safe_adapter(&registry.native_pdf);
        accepts_object_safe_adapter(&registry.anydoc);
        assert_eq!(
            registry
                .identity_for_format(DocumentFormat::Pdf)
                .unwrap()
                .adapter_backend,
            "pdf-canonical-adapter-v1"
        );
        assert_eq!(
            registry
                .identity_for_format(DocumentFormat::Docx)
                .unwrap()
                .adapter_backend,
            "anydoc-canonical-adapter-v1"
        );
        assert_eq!(
            registry
                .identity_for_format(DocumentFormat::Html)
                .unwrap_err()
                .code,
            "document_ingestion_adapter_unavailable"
        );
    }
}
