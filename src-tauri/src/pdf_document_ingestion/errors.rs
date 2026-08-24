use oxidize_pdf::OcrError;

use crate::{
    error::CommandError,
    pdf_backend::{PdfBackendError, PdfBackendErrorKind},
    pdf_ocr_backend,
    pdf_render_backend::{PdfRenderError, PdfRenderErrorKind},
};

use super::{active_pdf_extraction_backend, active_pdf_render_backend};

pub(crate) fn map_pdf_backend_error(error: PdfBackendError) -> CommandError {
    let backend = active_pdf_extraction_backend();
    let page = error.page_number.unwrap_or_default();
    match error.kind {
        PdfBackendErrorKind::Open | PdfBackendErrorKind::PageCount => CommandError::new(
            "pdf_open_failed",
            format!("The PDF could not be opened by the extraction adapter ({backend}): {error}"),
        ),
        PdfBackendErrorKind::ExtractText => CommandError::new(
            "pdf_text_extraction_failed",
            format!("Text could not be extracted from PDF page {page} by {backend}: {error}"),
        ),
    }
}

pub(crate) fn map_ocr_error(page_number: Option<usize>, error: OcrError) -> CommandError {
    if pdf_ocr_backend::is_cancellation_error(&error) {
        return CommandError::new(
            "ai_request_canceled",
            "The document generation request was canceled.",
        );
    }
    let page = page_number
        .map(|page| format!(" page {page}"))
        .unwrap_or_default();
    match error {
        OcrError::ProviderNotAvailable(_) => CommandError::new(
            "document_ocr_unavailable",
            "The PDF text layer is unreliable and ostudy needs local OCR to recover it, but Tesseract OCR is not available. Install Tesseract with English trained data (or set OSTUDY_TESSERACT_PATH), then retry generation. The original PDF remains in Documents.",
        ),
        OcrError::LowConfidence(_) => CommandError::new(
            "document_ocr_low_confidence",
            format!(
                "OCR confidence was too low to trust the recovered text for PDF{page}. The original PDF remains available for visual review."
            ),
        ),
        other => CommandError::new(
            "document_ocr_failed",
            format!(
                "The PDF text layer is unreliable and OCR failed for PDF{page}: {other}. The original PDF remains in Documents."
            ),
        ),
    }
}

pub(crate) fn map_pdf_render_error(error: PdfRenderError) -> CommandError {
    let backend = active_pdf_render_backend();
    let page = error.page_number.unwrap_or_default();
    match error.kind {
        PdfRenderErrorKind::Bind => CommandError::new(
            "pdf_renderer_unavailable",
            format!("The PDF page renderer ({backend}) could not be loaded: {error}"),
        ),
        PdfRenderErrorKind::Open => CommandError::new(
            "pdf_render_open_failed",
            format!("The PDF could not be opened by the rendering adapter ({backend}): {error}"),
        ),
        PdfRenderErrorKind::RenderPage | PdfRenderErrorKind::EncodePage => CommandError::new(
            "pdf_render_failed",
            format!("PDF page {page} could not be rendered by {backend}: {error}"),
        ),
    }
}
