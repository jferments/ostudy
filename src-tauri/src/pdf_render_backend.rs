//! Swappable PDF page-rendering boundary.
//!
//! Structured extraction is handled independently by `pdf_backend`. Rendering
//! uses Pdfium because its Chromium-derived engine has broad real-world PDF
//! compatibility. The document subsystem receives only bounded PNG bytes and
//! remains unaware of the concrete renderer.

use std::{env, fmt, io::Cursor, path::PathBuf};

use image::ImageFormat;
use pdfium_render::prelude::*;

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

/// Runtime-only renderer location. The path is not durable user data.
///
/// Development scripts set `OSTUDY_PDFIUM_LIBRARY`. Packaged applications may
/// instead provide Pdfium as a Tauri resource; system Pdfium remains a final
/// explicit compatibility fallback.
#[derive(Clone, Debug, Default)]
pub(crate) struct PdfRenderRuntime {
    candidates: Vec<PathBuf>,
}

impl PdfRenderRuntime {
    pub(crate) fn new(resource_dir: Option<PathBuf>) -> Self {
        let mut candidates = Vec::new();
        if let Some(path) = env::var_os("OSTUDY_PDFIUM_LIBRARY") {
            candidates.push(PathBuf::from(path));
        }
        if let Some(resource_dir) = resource_dir {
            let filename = platform_library_filename();
            candidates.push(resource_dir.join("pdfium").join(filename));
            candidates.push(resource_dir.join("resources").join("pdfium").join(filename));
        }
        if let Ok(executable) = env::current_exe() {
            if let Some(parent) = executable.parent() {
                candidates.push(parent.join(platform_library_filename()));
                candidates.push(parent.join("pdfium").join(platform_library_filename()));
            }
        }
        candidates.dedup();
        Self { candidates }
    }

    fn bind(&self) -> Result<Box<dyn PdfiumLibraryBindings>, PdfRenderError> {
        let mut errors = Vec::new();
        for candidate in &self.candidates {
            if !candidate.is_file() {
                continue;
            }
            match Pdfium::bind_to_library(candidate) {
                Ok(bindings) => return Ok(bindings),
                Err(error) => errors.push(format!("{}: {error}", candidate.display())),
            }
        }
        Pdfium::bind_to_system_library().map_err(|error| {
            let detail = if errors.is_empty() {
                format!("No bundled or system Pdfium library could be loaded: {error}")
            } else {
                format!(
                    "Bundled Pdfium candidates failed ({}); system Pdfium also failed: {error}",
                    errors.join("; ")
                )
            };
            PdfRenderError::new(PdfRenderErrorKind::Bind, None, detail)
        })
    }
}

pub(crate) fn active_backend_id() -> &'static str {
    "pdfium-7881/pdfium-render-0.9.3"
}

pub(crate) fn render_pages<E, F>(
    runtime: &PdfRenderRuntime,
    source_path: &std::path::Path,
    mut visitor: F,
) -> Result<usize, RenderProcessError<E>>
where
    F: FnMut(usize, Vec<u8>) -> Result<(), E>,
{
    let bindings = runtime.bind().map_err(RenderProcessError::Backend)?;
    let pdfium = Pdfium::new(bindings);
    let document = pdfium
        .load_pdf_from_file(source_path, None)
        .map_err(|error| {
            RenderProcessError::Backend(PdfRenderError::new(
                PdfRenderErrorKind::Open,
                None,
                error.to_string(),
            ))
        })?;
    let page_count = usize::from(document.pages().len());
    let render_config = PdfRenderConfig::new()
        .set_target_width(2048)
        .set_maximum_height(2048)
        .rotate_if_landscape(PdfPageRenderRotation::Degrees90, true);

    for (index, page) in document.pages().iter().enumerate() {
        let page_number = index + 1;
        let image = page
            .render_with_config(&render_config)
            .and_then(|bitmap| bitmap.as_image())
            .map_err(|error| {
                RenderProcessError::Backend(PdfRenderError::new(
                    PdfRenderErrorKind::RenderPage,
                    Some(page_number),
                    error.to_string(),
                ))
            })?;
        let mut encoded = Cursor::new(Vec::new());
        image
            .write_to(&mut encoded, ImageFormat::Png)
            .map_err(|error| {
                RenderProcessError::Backend(PdfRenderError::new(
                    PdfRenderErrorKind::EncodePage,
                    Some(page_number),
                    error.to_string(),
                ))
            })?;
        visitor(page_number, encoded.into_inner()).map_err(RenderProcessError::Visitor)?;
    }
    Ok(page_count)
}

#[derive(Debug)]
pub(crate) enum RenderProcessError<E> {
    Backend(PdfRenderError),
    Visitor(E),
}

fn platform_library_filename() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "pdfium.dll"
    }
    #[cfg(target_os = "macos")]
    {
        "libpdfium.dylib"
    }
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        "libpdfium.so"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_renderer_is_versioned_and_separate_from_extraction() {
        assert_eq!(active_backend_id(), "pdfium-7881/pdfium-render-0.9.3");
        assert!(!platform_library_filename().is_empty());
    }
}
