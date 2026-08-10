//! Swappable PDF page-rendering boundary.
//!
//! Structured extraction is handled independently by `pdf_backend`. Desktop
//! rendering uses Chromium PDFium through the pinned `pdfium-bundled` adapter;
//! the PDFium shared library is embedded at build time and extracted to a local
//! cache on first use. Pdfium itself does not guarantee parallel execution, so
//! this boundary deliberately renders one document in physical page order. OCR
//! parallelism happens after each selected high-resolution page has been
//! rasterized, where independent Tesseract processes can use multiple cores.

use std::{fmt, path::PathBuf};

#[cfg(not(any(target_os = "android", target_os = "ios")))]
use std::{io::Cursor, sync::Arc};

#[cfg(not(any(target_os = "android", target_os = "ios")))]
use image::ImageFormat;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use pdfium_bundled::{
    bind_bundled,
    pdfium_render::prelude::{PdfPageIndex, PdfRenderConfig, Pdfium},
};

const PAGE_RENDER_DPI: f64 = 120.0;
pub(crate) const OCR_RENDER_DPI: f64 = 300.0;
const PDF_POINTS_PER_INCH: f64 = 72.0;

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

#[derive(Clone)]
pub(crate) struct PdfRenderRuntime {
    backend_id: &'static str,
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pdfium: Arc<Pdfium>,
}

impl fmt::Debug for PdfRenderRuntime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PdfRenderRuntime")
            .field("backend_id", &self.backend_id)
            .finish_non_exhaustive()
    }
}

impl PdfRenderRuntime {
    pub(crate) fn new(_resource_dir: Option<PathBuf>) -> Result<Self, PdfRenderError> {
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            let pdfium = bind_bundled().map_err(|error| {
                PdfRenderError::new(
                    PdfRenderErrorKind::Bind,
                    None,
                    format!("Could not initialize the bundled PDFium renderer: {error}"),
                )
            })?;
            return Ok(Self {
                backend_id: active_backend_id(),
                pdfium: Arc::new(pdfium),
            });
        }

        #[cfg(any(target_os = "android", target_os = "ios"))]
        {
            Ok(Self {
                backend_id: active_backend_id(),
            })
        }
    }
}

pub(crate) fn active_backend_id() -> &'static str {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        "pdfium-render-0.9.3/pdfium-7881-bundled-v1-120dpi"
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        "pdfium-mobile-render-adapter-pending-v1"
    }
}

pub(crate) fn render_pages<E, F>(
    runtime: &PdfRenderRuntime,
    source_path: &std::path::Path,
    mut visitor: F,
) -> Result<usize, RenderProcessError<E>>
where
    F: FnMut(usize, Vec<u8>) -> Result<(), E>,
{
    validate_runtime(runtime)?;

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let document = runtime
            .pdfium
            .load_pdf_from_file(source_path, None)
            .map_err(|error| {
                RenderProcessError::Backend(PdfRenderError::new(
                    PdfRenderErrorKind::Open,
                    None,
                    error.to_string(),
                ))
            })?;
        let page_count = page_count(&document).map_err(RenderProcessError::Backend)?;
        for index in 0..page_count {
            let png = render_page_at_dpi(&document, index, PAGE_RENDER_DPI)
                .map_err(RenderProcessError::Backend)?;
            visitor(index + 1, png).map_err(RenderProcessError::Visitor)?;
        }
        return Ok(page_count);
    }

    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        let _ = (source_path, &mut visitor);
        Err(RenderProcessError::Backend(PdfRenderError::new(
            PdfRenderErrorKind::Bind,
            None,
            "This mobile build does not yet include the platform PDF rendering adapter.",
        )))
    }
}

pub(crate) fn render_ocr_pages<E, F>(
    runtime: &PdfRenderRuntime,
    source_path: &std::path::Path,
    page_numbers: &[usize],
    mut visitor: F,
) -> Result<usize, RenderProcessError<E>>
where
    F: FnMut(usize, Result<Vec<u8>, PdfRenderError>) -> Result<(), E>,
{
    validate_runtime(runtime)?;

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let document = runtime
            .pdfium
            .load_pdf_from_file(source_path, None)
            .map_err(|error| {
                RenderProcessError::Backend(PdfRenderError::new(
                    PdfRenderErrorKind::Open,
                    None,
                    error.to_string(),
                ))
            })?;
        let page_count = page_count(&document).map_err(RenderProcessError::Backend)?;
        let mut requested = page_numbers.to_vec();
        requested.sort_unstable();
        requested.dedup();
        for page_number in &requested {
            if *page_number == 0 || *page_number > page_count {
                return Err(RenderProcessError::Backend(PdfRenderError::new(
                    PdfRenderErrorKind::RenderPage,
                    Some(*page_number),
                    "The OCR fallback requested a page outside the PDF page range.",
                )));
            }
        }

        for page_number in &requested {
            let rendered = render_page_at_dpi(&document, *page_number - 1, OCR_RENDER_DPI);
            visitor(*page_number, rendered).map_err(RenderProcessError::Visitor)?;
        }
        return Ok(requested.len());
    }

    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        let _ = (source_path, page_numbers, &mut visitor);
        Err(RenderProcessError::Backend(PdfRenderError::new(
            PdfRenderErrorKind::Bind,
            None,
            "This mobile build does not yet include the platform PDF rendering adapter.",
        )))
    }
}

fn validate_runtime<E>(runtime: &PdfRenderRuntime) -> Result<(), RenderProcessError<E>> {
    if runtime.backend_id == active_backend_id() {
        Ok(())
    } else {
        Err(RenderProcessError::Backend(PdfRenderError::new(
            PdfRenderErrorKind::Bind,
            None,
            "The PDF renderer runtime does not match the active renderer backend.",
        )))
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn page_count(
    document: &pdfium_bundled::pdfium_render::prelude::PdfDocument<'_>,
) -> Result<usize, PdfRenderError> {
    usize::try_from(document.pages().len()).map_err(|_| {
        PdfRenderError::new(
            PdfRenderErrorKind::Open,
            None,
            "The PDF page count exceeded ostudy's supported platform range.",
        )
    })
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn render_page_at_dpi(
    document: &pdfium_bundled::pdfium_render::prelude::PdfDocument<'_>,
    index: usize,
    dpi: f64,
) -> Result<Vec<u8>, PdfRenderError> {
    let page_number = index + 1;
    let page_index = PdfPageIndex::try_from(index).map_err(|_| {
        PdfRenderError::new(
            PdfRenderErrorKind::RenderPage,
            Some(page_number),
            "The PDF page index exceeded the renderer's supported range.",
        )
    })?;
    let page = document.pages().get(page_index).map_err(|error| {
        PdfRenderError::new(
            PdfRenderErrorKind::RenderPage,
            Some(page_number),
            error.to_string(),
        )
    })?;
    let scale = (dpi / PDF_POINTS_PER_INCH) as f32;
    let config = PdfRenderConfig::new()
        .scale_page_by_factor(scale)
        .disable_native_text_rendering(true)
        .use_print_quality(true)
        .set_text_smoothing(true)
        .set_image_smoothing(true)
        .set_path_smoothing(true);
    let image = page
        .render_with_config(&config)
        .and_then(|bitmap| bitmap.as_image())
        .map_err(|error| {
            PdfRenderError::new(
                PdfRenderErrorKind::RenderPage,
                Some(page_number),
                error.to_string(),
            )
        })?;
    let mut output = Cursor::new(Vec::new());
    image
        .write_to(&mut output, ImageFormat::Png)
        .map_err(|error| {
            PdfRenderError::new(
                PdfRenderErrorKind::EncodePage,
                Some(page_number),
                error.to_string(),
            )
        })?;
    Ok(output.into_inner())
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
    fn active_renderer_is_pdfium_bundled_versioned() {
        assert_eq!(PAGE_RENDER_DPI, 120.0);
        assert_eq!(OCR_RENDER_DPI, 300.0);
        assert_eq!(PDF_POINTS_PER_INCH, 72.0);
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        assert_eq!(
            active_backend_id(),
            "pdfium-render-0.9.3/pdfium-7881-bundled-v1-120dpi"
        );
    }

    #[test]
    fn dpi_to_pdfium_scale_preserves_pdf_point_geometry() {
        assert!(((PAGE_RENDER_DPI / PDF_POINTS_PER_INCH) - (5.0 / 3.0)).abs() < 0.000_001);
        assert!(((OCR_RENDER_DPI / PDF_POINTS_PER_INCH) - (25.0 / 6.0)).abs() < 0.000_001);
    }
}
