//! Swappable PDF page-rendering boundary.
//!
//! Structured extraction is handled independently by `pdf_backend`. Rendering
//! deliberately uses the pure-Rust justpdf renderer so the default document
//! pipeline does not require a platform-specific native PDF runtime. CPU-heavy
//! page rasterization runs across a bounded worker pool; completed PNGs are
//! delivered to the document subsystem in physical page order so persistence
//! and error handling remain deterministic.

use std::{
    collections::BTreeMap,
    fmt,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc, Arc,
    },
};

use justpdf::Document;

const PAGE_RENDER_DPI: f64 = 144.0;
const MAX_RENDER_WORKERS: usize = 16;

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
    "justpdf-render-0.1.2/pure-rust-parallel-v1"
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

    let document = Arc::new(Document::open(source_path).map_err(|error| {
        RenderProcessError::Backend(PdfRenderError::new(
            PdfRenderErrorKind::Open,
            None,
            error.to_string(),
        ))
    })?);
    let page_count = document.page_count();
    if page_count == 0 {
        return Ok(0);
    }

    let worker_count = render_worker_count(page_count);
    let next_index = Arc::new(AtomicUsize::new(0));
    let stopped = Arc::new(AtomicBool::new(false));
    let (sender, receiver) = mpsc::channel::<(usize, Result<Vec<u8>, PdfRenderError>)>();

    std::thread::scope(|scope| {
        for _ in 0..worker_count {
            let document = Arc::clone(&document);
            let next_index = Arc::clone(&next_index);
            let stopped = Arc::clone(&stopped);
            let sender = sender.clone();
            scope.spawn(move || {
                while !stopped.load(Ordering::Acquire) {
                    let index = next_index.fetch_add(1, Ordering::AcqRel);
                    if index >= page_count {
                        break;
                    }
                    let result = render_page(&document, index);
                    let failed = result.is_err();
                    if sender.send((index, result)).is_err() {
                        break;
                    }
                    if failed {
                        stopped.store(true, Ordering::Release);
                        break;
                    }
                }
            });
        }
        drop(sender);

        let mut pending = BTreeMap::new();
        let mut next_to_visit = 0_usize;
        while next_to_visit < page_count {
            let (index, result) = receiver.recv().map_err(|_| {
                RenderProcessError::Backend(PdfRenderError::new(
                    PdfRenderErrorKind::RenderPage,
                    Some(next_to_visit + 1),
                    "The PDF render worker pool stopped before every page completed.",
                ))
            })?;
            pending.insert(index, result);

            while let Some(result) = pending.remove(&next_to_visit) {
                let png = match result {
                    Ok(png) => png,
                    Err(error) => {
                        stopped.store(true, Ordering::Release);
                        return Err(RenderProcessError::Backend(error));
                    }
                };
                if let Err(error) = visitor(next_to_visit + 1, png) {
                    stopped.store(true, Ordering::Release);
                    return Err(RenderProcessError::Visitor(error));
                }
                next_to_visit += 1;
            }
        }

        stopped.store(true, Ordering::Release);
        Ok(page_count)
    })
}

fn render_page(document: &Document, index: usize) -> Result<Vec<u8>, PdfRenderError> {
    let page_number = index + 1;
    let page = document.page(index).map_err(|error| {
        PdfRenderError::new(
            PdfRenderErrorKind::RenderPage,
            Some(page_number),
            error.to_string(),
        )
    })?;
    page.render_png(PAGE_RENDER_DPI).map_err(|error| {
        PdfRenderError::new(
            PdfRenderErrorKind::EncodePage,
            Some(page_number),
            error.to_string(),
        )
    })
}

fn render_worker_count(page_count: usize) -> usize {
    std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(1)
        .min(MAX_RENDER_WORKERS)
        .min(page_count.max(1))
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
    fn active_renderer_is_pure_rust_parallel_and_versioned() {
        let runtime = PdfRenderRuntime::new(None);
        assert_eq!(runtime.backend_id, active_backend_id());
        assert_eq!(
            active_backend_id(),
            "justpdf-render-0.1.2/pure-rust-parallel-v1"
        );
        assert_eq!(PAGE_RENDER_DPI, 144.0);
        assert!(render_worker_count(1) == 1);
        assert!((1..=MAX_RENDER_WORKERS).contains(&render_worker_count(10_000)));
    }
}
