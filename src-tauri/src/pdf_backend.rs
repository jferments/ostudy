//! Swappable native PDF extraction/rendering boundary.
//!
//! Document lifecycle, storage, provider calls, and database writes must not
//! depend on a particular PDF engine. Backends expose page text and rendered
//! PNG bytes through this small streaming interface; callers own all limits,
//! hashing, persistence, and retry semantics.

use std::{fmt, path::Path};

use justpdf::Document;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PdfBackendErrorKind {
    Open,
    ReadPage,
    ExtractText,
    RenderPage,
}

#[derive(Debug)]
pub(crate) struct PdfBackendError {
    pub kind: PdfBackendErrorKind,
    pub page_number: Option<usize>,
    detail: String,
}

impl PdfBackendError {
    fn new(
        kind: PdfBackendErrorKind,
        page_number: Option<usize>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            page_number,
            detail: detail.into(),
        }
    }
}

impl fmt::Display for PdfBackendError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.detail)
    }
}

#[derive(Debug)]
pub(crate) enum PdfProcessError<E> {
    Backend(PdfBackendError),
    Visitor(E),
}

/// Engine-neutral page stream used by the managed-document pipeline.
///
/// A future `oxidize-pdf`, `pdfium-render`, or platform-native implementation
/// only needs to satisfy this trait. It cannot write ostudy files, mutate
/// SQLite, or contact an AI provider.
pub(crate) trait PdfBackend {
    fn id(&self) -> &'static str;

    fn for_each_page<E, F>(
        &self,
        source_path: &Path,
        render_dpi: f64,
        visitor: F,
    ) -> Result<usize, PdfProcessError<E>>
    where
        F: FnMut(usize, String, Vec<u8>) -> Result<(), E>;
}

#[derive(Clone, Copy, Debug, Default)]
struct JustPdfBackend;

impl PdfBackend for JustPdfBackend {
    fn id(&self) -> &'static str {
        "justpdf-0.1"
    }

    fn for_each_page<E, F>(
        &self,
        source_path: &Path,
        render_dpi: f64,
        mut visitor: F,
    ) -> Result<usize, PdfProcessError<E>>
    where
        F: FnMut(usize, String, Vec<u8>) -> Result<(), E>,
    {
        let document = Document::open(source_path).map_err(|error| {
            PdfProcessError::Backend(PdfBackendError::new(
                PdfBackendErrorKind::Open,
                None,
                error.to_string(),
            ))
        })?;
        let page_count = document.page_count();
        for index in 0..page_count {
            let page_number = index + 1;
            let page = document.page(index).map_err(|error| {
                PdfProcessError::Backend(PdfBackendError::new(
                    PdfBackendErrorKind::ReadPage,
                    Some(page_number),
                    error.to_string(),
                ))
            })?;
            let text = page.text().map_err(|error| {
                PdfProcessError::Backend(PdfBackendError::new(
                    PdfBackendErrorKind::ExtractText,
                    Some(page_number),
                    error.to_string(),
                ))
            })?;
            let png = page.render_png(render_dpi).map_err(|error| {
                PdfProcessError::Backend(PdfBackendError::new(
                    PdfBackendErrorKind::RenderPage,
                    Some(page_number),
                    error.to_string(),
                ))
            })?;
            visitor(page_number, text, png).map_err(PdfProcessError::Visitor)?;
        }
        Ok(page_count)
    }
}

pub(crate) fn active_backend_id() -> &'static str {
    JustPdfBackend.id()
}

pub(crate) fn for_each_page<E, F>(
    source_path: &Path,
    render_dpi: f64,
    visitor: F,
) -> Result<usize, PdfProcessError<E>>
where
    F: FnMut(usize, String, Vec<u8>) -> Result<(), E>,
{
    JustPdfBackend.for_each_page(source_path, render_dpi, visitor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_backend_is_named_without_leaking_into_document_storage() {
        assert_eq!(active_backend_id(), "justpdf-0.1");
    }
}
