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
/// The resource-directory argument remains accepted so the document/Tauri
/// boundary is stable across renderer implementations. The active pure-Rust
/// renderer needs no packaged resource or dynamic library. Keeping the active
/// backend identity in runtime state also makes a stale runtime/backend pairing
/// fail closed if the application ever gains a selectable compatibility
/// renderer.
#[derive(Clone, Debug)]
pub(crate) struct PdfRenderRuntime {
    backend_id: &'static str,
}

impl Default for PdfRenderRuntime {
    fn default() -> Self {
        Self::new(None)
    }
}

impl PdfRenderRuntime {
    pub(crate) fn new(_resource_dir: Option<PathBuf>) -> Self {
        Self {
            backend_id: active_backend_id(),
        }
    }
}

pub(crate) fn active_backend_id() -> &'static str {
    "justpdf-render-0.1.2/pure-rust"
}

pub(crate) fn render_pages<E, F>(
    runtime: &PdfRenderRuntime,
    source_path: &std::path::Path,
    mut visitor: F,
) -> Result<usize, RenderProcessError<E>>
where
    F: FnMut(usize, Vec<u8>) -> Result<(), E>,
{
    if runtime.backend_id != active_backend_id() {
        return Err(RenderProcessError::Backend(PdfRenderError::new(
            PdfRenderErrorKind::Bind,
            None,
            "The PDF renderer runtime does not match the active renderer backend.",
        )));
    }

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
        let runtime = PdfRenderRuntime::new(None);
        assert_eq!(runtime.backend_id, active_backend_id());
        assert_eq!(active_backend_id(), "justpdf-render-0.1.2/pure-rust");
        assert_eq!(PAGE_RENDER_DPI, 144.0);
    }
}
