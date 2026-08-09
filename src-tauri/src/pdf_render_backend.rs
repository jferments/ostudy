//! Swappable PDF page-rendering boundary.
//!
//! Structured extraction is handled independently by `pdf_backend`. Rendering
//! deliberately uses the pure-Rust justpdf renderer so the default document
//! pipeline does not require a platform-specific native PDF runtime. The
//! document subsystem receives only bounded PNG bytes and remains unaware of
//! the concrete renderer.

use std::{fmt, path::PathBuf};

use justpdf::Document;

const PAGE_RENDER_DPI: f64 = 144.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PdfRenderErrorKind {
    // Retained as part of the renderer-boundary error vocabulary so a future
    // optional native renderer can report an unavailable runtime without
    // changing the document domain's safe-error mapping.
    Bind,
    Open,
    RenderPage,
    EncodePage,
}

#[derive(Debug)]
pub(crate) struct PdfRenderError {
    pub kind: PdfRenderErrorKind,
    pub page_number: Option<usize>,
    detail: String,
}

impl PdfRenderError {
    fn new(
        kind: PdfRenderErrorKind,
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

impl fmt::Display for PdfRenderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.detail)
    }
}

/// Runtime renderer configuration.
///
/// The argument remains accepted so the document/Tauri boundary is stable
/// across renderer implementations. The pure-Rust renderer needs no packaged
/// resource or dynamic library, which is important for Android/iOS and other
/// constrained targets.
#[derive(Clone, Debug, Default)]
pub(crate) struct PdfRenderRuntime;

impl PdfRenderRuntime {
    pub(crate) fn new(_resource_dir: Option<PathBuf>) -> Self {
        Self
    }
}

pub(crate) fn active_backend_id() -> &'static str {
    "justpdf-render-0.1.2/pure-rust"
}

pub(crate) fn render_pages<E, F>(
    _runtime: &PdfRenderRuntime,
    source_path: &std::path::Path,
    mut visitor: F,
) -> Result<usize, RenderProcessError<E>>
where
    F: FnMut(usize, Vec<u8>) -> Result<(), E>,
{
    let document = Document::open(source_path).map_err(|error| {
        RenderProcessError::Backend(PdfRenderError::new(
            PdfRenderErrorKind::Open,
            None,
            error.to_string(),
        ))
    })?;
    let page_count = document.page_count();

    for index in 0..page_count {
        let page_number = index + 1;
        let page = document.page(index).map_err(|error| {
            RenderProcessError::Backend(PdfRenderError::new(
                PdfRenderErrorKind::RenderPage,
                Some(page_number),
                error.to_string(),
            ))
        })?;
        let png = page.render_png(PAGE_RENDER_DPI).map_err(|error| {
            RenderProcessError::Backend(PdfRenderError::new(
                PdfRenderErrorKind::EncodePage,
                Some(page_number),
                error.to_string(),
            ))
        })?;
        visitor(page_number, png).map_err(RenderProcessError::Visitor)?;
    }
    Ok(page_count)
}

#[derive(Debug)]
pub(crate) enum RenderProcessError<E> {
    Backend(PdfRenderError),
    Visitor(E),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_renderer_is_pure_rust_and_versioned() {
        assert_eq!(active_backend_id(), "justpdf-render-0.1.2/pure-rust");
        assert_eq!(PAGE_RENDER_DPI, 144.0);
    }
}
