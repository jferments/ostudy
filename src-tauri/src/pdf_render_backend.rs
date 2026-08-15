//! Swappable PDF page-rendering boundary.
//!
//! Structured extraction is handled independently by `pdf_backend`. Desktop
//! rendering uses Chromium PDFium through the mature `pdfium-render` bindings.
//! Android delegates only page rasterization to the platform `PdfRenderer` API;
//! Rust still owns limits, persistence, cancellation, OCR selection, and every
//! downstream document transaction.

use std::{fmt, path::PathBuf};

#[cfg(not(any(target_os = "android", target_os = "ios")))]
use std::{io::Cursor, path::Path, sync::Arc};

#[cfg(target_os = "android")]
use std::{
    fs,
    path::Path,
    sync::{
        atomic::{AtomicU64, Ordering},
        OnceLock,
    },
};

#[cfg(not(any(target_os = "android", target_os = "ios")))]
use image::ImageFormat;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use pdfium_render::prelude::{PdfPageIndex, PdfRenderConfig, Pdfium};
#[cfg(target_os = "android")]
use tauri::{AppHandle, Manager};
#[cfg(target_os = "android")]
use tauri_plugin_ostudy_platform::{OpenPdfRequest, OstudyPlatformExt, RenderPdfPageRequest};

const PAGE_RENDER_DPI: f64 = 120.0;
pub(crate) const OCR_RENDER_DPI: f64 = 300.0;
const PDF_POINTS_PER_INCH: f64 = 72.0;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
const PDFIUM_PATH_ENV: &str = "OSTUDY_PDFIUM_PATH";
#[cfg(target_os = "android")]
const ANDROID_RENDER_MAX_BYTES: u64 = 32 * 1024 * 1024;
#[cfg(target_os = "android")]
const ANDROID_OCR_RENDER_MAX_BYTES: u64 = 64 * 1024 * 1024;
#[cfg(target_os = "android")]
static ANDROID_RENDER_COUNTER: AtomicU64 = AtomicU64::new(0);
#[cfg(target_os = "android")]
static ANDROID_APP: OnceLock<AppHandle> = OnceLock::new();

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
    #[cfg(target_os = "android")]
    app: AppHandle,
    #[cfg(target_os = "android")]
    scratch_dir: PathBuf,
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
    pub(crate) fn new(resource_dir: Option<PathBuf>) -> Result<Self, PdfRenderError> {
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            let pdfium = bind_external_pdfium(resource_dir.as_deref())?;
            Ok(Self {
                backend_id: active_backend_id(),
                pdfium: Arc::new(pdfium),
            })
        }

        #[cfg(target_os = "android")]
        {
            let _ = resource_dir;
            let app = android_app_handle()?;
            let scratch_dir = app
                .path()
                .app_cache_dir()
                .map_err(|error| {
                    PdfRenderError::new(
                        PdfRenderErrorKind::Bind,
                        None,
                        format!("Could not resolve Android's PDF render cache: {error}"),
                    )
                })?
                .join("android-pdf-render");
            fs::create_dir_all(&scratch_dir).map_err(|error| {
                PdfRenderError::new(
                    PdfRenderErrorKind::Bind,
                    None,
                    format!("Could not create Android's PDF render cache: {error}"),
                )
            })?;
            let capabilities = app
                .ostudy_platform()
                .document_capabilities()
                .map_err(|error| {
                    PdfRenderError::new(
                        PdfRenderErrorKind::Bind,
                        None,
                        format!("Android document services are unavailable: {error}"),
                    )
                })?;
            if !capabilities.pdf_render_available {
                return Err(PdfRenderError::new(
                    PdfRenderErrorKind::Bind,
                    None,
                    "Android reported that native PDF rendering is unavailable.",
                ));
            }
            Ok(Self {
                backend_id: active_backend_id(),
                app,
                scratch_dir,
            })
        }

        #[cfg(target_os = "ios")]
        {
            let _ = resource_dir;
            Ok(Self {
                backend_id: active_backend_id(),
            })
        }
    }

    #[cfg(target_os = "android")]
    pub(crate) fn android_app(&self) -> &AppHandle {
        &self.app
    }
}

#[cfg(target_os = "android")]
pub(crate) fn install_android_app(app: AppHandle) {
    let _ = ANDROID_APP.set(app);
}

#[cfg(target_os = "android")]
pub(crate) fn android_app_handle() -> Result<AppHandle, PdfRenderError> {
    ANDROID_APP.get().cloned().ok_or_else(|| {
        PdfRenderError::new(
            PdfRenderErrorKind::Bind,
            None,
            "Android document services were requested before the platform runtime was initialized.",
        )
    })
}

pub(crate) fn active_backend_id() -> &'static str {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        "pdfium-render-0.9.3/pdfium-api-7881-external-v2-120dpi"
    }
    #[cfg(target_os = "android")]
    {
        "android-pdf-renderer-api21-v1-120dpi"
    }
    #[cfg(target_os = "ios")]
    {
        "pdfium-mobile-render-adapter-pending-v2"
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn bind_external_pdfium(resource_dir: Option<&Path>) -> Result<Pdfium, PdfRenderError> {
    let mut candidates = Vec::<PathBuf>::new();

    if let Some(explicit) = std::env::var_os(PDFIUM_PATH_ENV)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
    {
        candidates.push(resolve_library_candidate(explicit));
    }
    if let Some(resource_dir) = resource_dir {
        candidates.push(Pdfium::pdfium_platform_library_name_at_path(
            &resource_dir.join("native/pdfium"),
        ));
    }
    if let Some(root) = std::env::var_os("OSTUDY_ROOT_DIR")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
    {
        candidates.push(Pdfium::pdfium_platform_library_name_at_path(
            &root.join("src-tauri/resources/native/pdfium"),
        ));
    }
    if let Ok(executable) = std::env::current_exe() {
        if let Some(directory) = executable.parent() {
            candidates.push(Pdfium::pdfium_platform_library_name_at_path(directory));
        }
    }

    candidates.dedup();
    let mut attempted_local = 0_usize;
    for candidate in candidates {
        if !candidate.is_file() {
            continue;
        }
        attempted_local = attempted_local.saturating_add(1);
        match Pdfium::bind_to_library(&candidate) {
            Ok(bindings) => {
                crate::logging::info(
                    "pdfium_runtime_bound",
                    format!(
                        "source=explicit_or_managed attempted_local={attempted_local} library_name={}",
                        candidate
                            .file_name()
                            .and_then(|value| value.to_str())
                            .unwrap_or("pdfium")
                    ),
                );
                return Ok(Pdfium::new(bindings));
            }
            Err(error) => {
                crate::logging::info(
                    "pdfium_runtime_candidate_rejected",
                    format!(
                        "attempt={attempted_local} detail={}",
                        bounded_detail(&error.to_string(), 300)
                    ),
                );
            }
        }
    }

    match Pdfium::bind_to_system_library() {
        Ok(bindings) => {
            crate::logging::info(
                "pdfium_runtime_bound",
                format!("source=system attempted_local={attempted_local}"),
            );
            Ok(Pdfium::new(bindings))
        }
        Err(error) => Err(PdfRenderError::new(
            PdfRenderErrorKind::Bind,
            None,
            format!(
                "Could not load a compatible PDFium runtime. Run ./scripts/install-native-deps.sh --pdfium-only, install PDFium system-wide, or set {PDFIUM_PATH_ENV} to a compatible PDFium 7881 library. Last bind error: {}",
                bounded_detail(&error.to_string(), 300)
            ),
        )),
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn resolve_library_candidate(path: PathBuf) -> PathBuf {
    if path.is_dir() {
        Pdfium::pdfium_platform_library_name_at_path(&path)
    } else {
        path
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn bounded_detail(value: &str, maximum: usize) -> String {
    value
        .chars()
        .filter(|character| !character.is_control() || *character == '\n')
        .take(maximum)
        .collect::<String>()
        .replace('\n', " ")
}

pub(crate) fn extract_native_text_pages(
    runtime: &PdfRenderRuntime,
    source_path: &std::path::Path,
    page_numbers: &[usize],
) -> Result<Vec<(usize, String)>, PdfRenderError> {
    if runtime.backend_id != active_backend_id() {
        return Err(PdfRenderError::new(
            PdfRenderErrorKind::Bind,
            None,
            "The PDF renderer runtime does not match the active renderer backend.",
        ));
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let document = runtime
            .pdfium
            .load_pdf_from_file(source_path, None)
            .map_err(|error| {
                PdfRenderError::new(PdfRenderErrorKind::Open, None, error.to_string())
            })?;
        let total_pages = page_count(&document)?;
        let requested = validated_page_numbers(page_numbers, total_pages)?;
        let mut extracted = Vec::with_capacity(requested.len());
        for page_number in requested {
            let index = PdfPageIndex::try_from(page_number - 1).map_err(|_| {
                PdfRenderError::new(
                    PdfRenderErrorKind::RenderPage,
                    Some(page_number),
                    "The PDF page index exceeded the renderer's supported range.",
                )
            })?;
            let page = document.pages().get(index).map_err(|error| {
                PdfRenderError::new(
                    PdfRenderErrorKind::RenderPage,
                    Some(page_number),
                    error.to_string(),
                )
            })?;
            match page.text() {
                Ok(text) => extracted.push((page_number, text.all())),
                Err(error) => crate::logging::info(
                    "pdfium_native_text_page_unavailable",
                    format!(
                        "page={page_number} detail={}",
                        bounded_detail(&error.to_string(), 300)
                    ),
                ),
            };
        }
        Ok(extracted)
    }

    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        let _ = (source_path, page_numbers);
        Err(PdfRenderError::new(
            PdfRenderErrorKind::Bind,
            None,
            "This mobile renderer does not expose a second native PDF text layer; ostudy will use its bounded OCR fallback when native extraction is unreliable.",
        ))
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
        Ok(page_count)
    }

    #[cfg(target_os = "android")]
    {
        let session =
            open_android_pdf(runtime, source_path).map_err(RenderProcessError::Backend)?;
        for page_number in 1..=session.page_count {
            let png = render_android_page(
                runtime,
                &session,
                page_number,
                PAGE_RENDER_DPI as u32,
                ANDROID_RENDER_MAX_BYTES,
            )
            .map_err(RenderProcessError::Backend)?;
            visitor(page_number, png).map_err(RenderProcessError::Visitor)?;
        }
        Ok(session.page_count)
    }

    #[cfg(target_os = "ios")]
    {
        let _ = (source_path, &mut visitor);
        Err(RenderProcessError::Backend(PdfRenderError::new(
            PdfRenderErrorKind::Bind,
            None,
            "This iOS build does not yet include the platform PDF rendering adapter.",
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
        let requested = validated_page_numbers(page_numbers, page_count)
            .map_err(RenderProcessError::Backend)?;
        for page_number in &requested {
            let rendered = render_page_at_dpi(&document, *page_number - 1, OCR_RENDER_DPI);
            visitor(*page_number, rendered).map_err(RenderProcessError::Visitor)?;
        }
        Ok(requested.len())
    }

    #[cfg(target_os = "android")]
    {
        let session =
            open_android_pdf(runtime, source_path).map_err(RenderProcessError::Backend)?;
        let requested = validated_page_numbers(page_numbers, session.page_count)
            .map_err(RenderProcessError::Backend)?;
        for page_number in &requested {
            let rendered = render_android_page(
                runtime,
                &session,
                *page_number,
                OCR_RENDER_DPI as u32,
                ANDROID_OCR_RENDER_MAX_BYTES,
            );
            visitor(*page_number, rendered).map_err(RenderProcessError::Visitor)?;
        }
        Ok(requested.len())
    }

    #[cfg(target_os = "ios")]
    {
        let _ = (source_path, page_numbers, &mut visitor);
        Err(RenderProcessError::Backend(PdfRenderError::new(
            PdfRenderErrorKind::Bind,
            None,
            "This iOS build does not yet include the platform PDF rendering adapter.",
        )))
    }
}

fn validated_page_numbers(
    page_numbers: &[usize],
    page_count: usize,
) -> Result<Vec<usize>, PdfRenderError> {
    let mut requested = page_numbers.to_vec();
    requested.sort_unstable();
    requested.dedup();
    for page_number in &requested {
        if *page_number == 0 || *page_number > page_count {
            return Err(PdfRenderError::new(
                PdfRenderErrorKind::RenderPage,
                Some(*page_number),
                "The requested page is outside the PDF page range.",
            ));
        }
    }
    Ok(requested)
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

#[cfg(target_os = "android")]
fn open_android_pdf(
    runtime: &PdfRenderRuntime,
    source_path: &Path,
) -> Result<AndroidPdfSession, PdfRenderError> {
    let source_path = source_path.to_str().ok_or_else(|| {
        PdfRenderError::new(
            PdfRenderErrorKind::Open,
            None,
            "The Android PDF source path is not valid UTF-8.",
        )
    })?;
    let opened = runtime
        .app
        .ostudy_platform()
        .open_pdf(OpenPdfRequest {
            source_path: source_path.to_string(),
        })
        .map_err(|error| {
            PdfRenderError::new(
                PdfRenderErrorKind::Open,
                None,
                format!("Android could not open the PDF: {error}"),
            )
        })?;
    let page_count = usize::try_from(opened.page_count).map_err(|_| {
        PdfRenderError::new(
            PdfRenderErrorKind::Open,
            None,
            "Android reported a PDF page count outside the supported range.",
        )
    })?;
    if page_count == 0 {
        return Err(PdfRenderError::new(
            PdfRenderErrorKind::Open,
            None,
            "Android reported an empty PDF.",
        ));
    }
    Ok(AndroidPdfSession {
        app: runtime.app.clone(),
        token: opened.token,
        page_count,
    })
}

#[cfg(target_os = "android")]
fn render_android_page(
    runtime: &PdfRenderRuntime,
    session: &AndroidPdfSession,
    page_number: usize,
    dpi: u32,
    maximum_bytes: u64,
) -> Result<Vec<u8>, PdfRenderError> {
    let sequence = ANDROID_RENDER_COUNTER.fetch_add(1, Ordering::Relaxed);
    let target = runtime.scratch_dir.join(format!(
        "render-{}-{page_number}-{dpi}-{sequence}.png",
        std::process::id()
    ));
    let target_path = target.to_str().ok_or_else(|| {
        PdfRenderError::new(
            PdfRenderErrorKind::EncodePage,
            Some(page_number),
            "The Android PDF render cache path is not valid UTF-8.",
        )
    })?;
    let request = RenderPdfPageRequest {
        token: session.token.clone(),
        page_number: u32::try_from(page_number).map_err(|_| {
            PdfRenderError::new(
                PdfRenderErrorKind::RenderPage,
                Some(page_number),
                "The Android PDF page index exceeded the supported range.",
            )
        })?,
        dpi,
        target_path: target_path.to_string(),
        maximum_bytes,
    };
    let rendered = runtime
        .app
        .ostudy_platform()
        .render_pdf_page(request)
        .map_err(|error| {
            PdfRenderError::new(
                PdfRenderErrorKind::RenderPage,
                Some(page_number),
                format!("Android PDF rendering failed: {error}"),
            )
        });
    let outcome = rendered.and_then(|metadata| {
        if metadata.page_number != u32::try_from(page_number).unwrap_or(u32::MAX) {
            return Err(PdfRenderError::new(
                PdfRenderErrorKind::RenderPage,
                Some(page_number),
                "Android returned PDF render metadata for the wrong page.",
            ));
        }
        let bytes = fs::read(&target).map_err(|error| {
            PdfRenderError::new(
                PdfRenderErrorKind::EncodePage,
                Some(page_number),
                format!("Android did not return a readable PNG page: {error}"),
            )
        })?;
        if bytes.is_empty()
            || bytes.len() as u64 != metadata.byte_size
            || metadata.byte_size > maximum_bytes
            || metadata.width == 0
            || metadata.height == 0
        {
            return Err(PdfRenderError::new(
                PdfRenderErrorKind::EncodePage,
                Some(page_number),
                "Android returned inconsistent PDF render metadata.",
            ));
        }
        Ok(bytes)
    });
    let _ = fs::remove_file(&target);
    outcome
}

#[cfg(target_os = "android")]
struct AndroidPdfSession {
    app: AppHandle,
    token: String,
    page_count: usize,
}

#[cfg(target_os = "android")]
impl Drop for AndroidPdfSession {
    fn drop(&mut self) {
        let _ = self.app.ostudy_platform().close_pdf(self.token.clone());
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn page_count(document: &pdfium_render::prelude::PdfDocument<'_>) -> Result<usize, PdfRenderError> {
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
    document: &pdfium_render::prelude::PdfDocument<'_>,
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
    fn active_renderer_is_versioned() {
        assert_eq!(PAGE_RENDER_DPI, 120.0);
        assert_eq!(OCR_RENDER_DPI, 300.0);
        assert_eq!(PDF_POINTS_PER_INCH, 72.0);
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        assert_eq!(
            active_backend_id(),
            "pdfium-render-0.9.3/pdfium-api-7881-external-v2-120dpi"
        );
        #[cfg(target_os = "android")]
        assert_eq!(active_backend_id(), "android-pdf-renderer-api21-v1-120dpi");
    }

    #[test]
    fn dpi_to_renderer_scale_preserves_pdf_point_geometry() {
        assert!(((PAGE_RENDER_DPI / PDF_POINTS_PER_INCH) - (5.0 / 3.0)).abs() < 0.000_001);
        assert!(((OCR_RENDER_DPI / PDF_POINTS_PER_INCH) - (25.0 / 6.0)).abs() < 0.000_001);
    }

    #[test]
    fn requested_pages_are_sorted_deduplicated_and_bounded() {
        assert_eq!(
            validated_page_numbers(&[3, 1, 3, 2], 3).unwrap(),
            vec![1, 2, 3]
        );
        assert!(validated_page_numbers(&[0], 3).is_err());
        assert!(validated_page_numbers(&[4], 3).is_err());
    }
}
