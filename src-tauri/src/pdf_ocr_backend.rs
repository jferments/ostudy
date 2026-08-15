//! Platform-aware OCR boundary used when the native PDF text layer is unreliable.
//!
//! ostudy owns the OCR contract rather than making document ingestion depend on
//! one executable. Desktop adapts the oxidize-pdf `OcrProvider` trait to local
//! Tesseract CLI. Android stages the bounded PNG in app-private storage and calls
//! the bundled ML Kit Text Recognition v2 adapter through the private Tauri
//! platform plugin. iOS remains reserved for Apple Vision.

use oxidize_pdf::{
    graphics::ImageFormat, OcrEngine, OcrError, OcrOptions, OcrProcessingResult, OcrProvider,
    OcrResult,
};

use crate::{pdf_render_backend, task_cancellation};

#[cfg(not(any(target_os = "android", target_os = "ios")))]
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant},
};

#[cfg(target_os = "android")]
use std::{
    fs,
    sync::atomic::{AtomicU64, Ordering},
};
#[cfg(target_os = "android")]
use tauri::Manager;
#[cfg(target_os = "android")]
use tauri_plugin_ostudy_platform::{OstudyPlatformExt, RecognizeImageRequest};

const OCR_CANCELED_MARKER: &str = "ostudy_document_ocr_canceled";

#[cfg(not(any(target_os = "android", target_os = "ios")))]
const ENGINE_NAME: &str = "Tesseract CLI via ostudy platform OCR";
#[cfg(target_os = "android")]
const ENGINE_NAME: &str = "ML Kit Text Recognition v2 (bundled Latin)";
#[cfg(target_os = "ios")]
const ENGINE_NAME: &str = "Apple Vision OCR adapter";

#[cfg(not(any(target_os = "android", target_os = "ios")))]
const DEFAULT_EXECUTABLE: &str = "tesseract";
#[cfg(not(any(target_os = "android", target_os = "ios")))]
const EXECUTABLE_ENV: &str = "OSTUDY_TESSERACT_PATH";
#[cfg(not(any(target_os = "android", target_os = "ios")))]
const WAIT_INTERVAL_MS: u64 = 25;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
const MAX_TSV_BYTES: u64 = 16 * 1024 * 1024;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
const TESSERACT_INPUT_DPI: u16 = 300;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
const PRIMARY_PAGE_SEGMENTATION_MODE: u8 = 3;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
const FALLBACK_PAGE_SEGMENTATION_MODES: [u8; 2] = [6, 11];
#[cfg(not(any(target_os = "android", target_os = "ios")))]
const STRONG_OCR_MIN_NONSPACE: usize = 24;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
const STRONG_OCR_MIN_ALPHABETIC: usize = 8;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
const STRONG_OCR_MIN_CONFIDENCE: f64 = 0.45;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
static WORKSPACE_COUNTER: AtomicU64 = AtomicU64::new(0);
#[cfg(target_os = "android")]
static ANDROID_OCR_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Platform-neutral provider presented to document ingestion.
///
/// The compatibility alias keeps the original call-site name stable while the
/// implementation is generalized. New code should reason about this type as a
/// platform OCR provider, not as a guarantee that Tesseract exists everywhere.
#[derive(Clone, Debug)]
pub(crate) struct PlatformOcrProvider {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    executable: String,
    #[cfg(target_os = "android")]
    app: Option<tauri::AppHandle>,
    document_id: Option<String>,
}

pub(crate) type TesseractCliOcrProvider = PlatformOcrProvider;

impl Default for PlatformOcrProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl PlatformOcrProvider {
    pub(crate) fn new() -> Self {
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        let executable = std::env::var(EXECUTABLE_ENV)
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_EXECUTABLE.to_string());

        Self {
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            executable,
            #[cfg(target_os = "android")]
            app: pdf_render_backend::android_app_handle().ok(),
            document_id: None,
        }
    }

    pub(crate) fn for_document(document_id: &str) -> Self {
        let mut provider = Self::new();
        provider.document_id = Some(document_id.to_string());
        provider
    }

    pub(crate) fn ensure_available(&self) -> OcrResult<()> {
        if self.is_canceled() {
            return Err(canceled_ocr_error());
        }

        #[cfg(target_os = "android")]
        {
            let app = self.app.as_ref().ok_or_else(|| {
                OcrError::Configuration(
                    "Android OCR was initialized before the native platform runtime.".to_string(),
                )
            })?;
            let capabilities = app
                .ostudy_platform()
                .document_capabilities()
                .map_err(|error| {
                    OcrError::ProviderNotAvailable(format!(
                        "Could not query Android document services: {error}"
                    ))
                })?;
            if capabilities.ocr_available {
                Ok(())
            } else {
                Err(OcrError::ProviderNotAvailable(
                    "Android reported that its bundled on-device OCR adapter is unavailable."
                        .to_string(),
                ))
            }
        }
        #[cfg(target_os = "ios")]
        {
            Err(OcrError::Configuration(
                "This iOS build does not yet include ostudy's Apple Vision OCR adapter. Corrupted PDF text will not be sent to an AI provider until that adapter is available."
                    .to_string(),
            ))
        }
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            let status = Command::new(&self.executable)
                .arg("--version")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map_err(|error| {
                    OcrError::ProviderNotAvailable(format!(
                        "Could not start '{}': {error}",
                        self.executable
                    ))
                })?;
            if status.success() {
                Ok(())
            } else {
                Err(OcrError::ProviderNotAvailable(format!(
                    "'{} --version' exited with status {status}",
                    self.executable
                )))
            }
        }
    }

    fn is_canceled(&self) -> bool {
        self.document_id
            .as_deref()
            .is_some_and(task_cancellation::is_document_task_canceled)
    }

    #[cfg(target_os = "android")]
    fn process_android_ml_kit(
        &self,
        image_data: &[u8],
        options: &OcrOptions,
    ) -> OcrResult<OcrProcessingResult> {
        let expected_dimensions = png_dimensions(image_data)?;
        let app = self.app.as_ref().ok_or_else(|| {
            OcrError::Configuration(
                "Android OCR was initialized before the native platform runtime.".to_string(),
            )
        })?;
        let directory = app
            .path()
            .app_cache_dir()
            .map_err(|error| OcrError::Configuration(error.to_string()))?
            .join("android-ocr");
        fs::create_dir_all(&directory)?;
        let sequence = ANDROID_OCR_COUNTER.fetch_add(1, Ordering::Relaxed);
        let input = directory.join(format!(
            "ocr-{}-{}-{sequence}.png",
            std::process::id(),
            crate::platform::now_utc_millis().unwrap_or_default()
        ));
        fs::write(&input, image_data)?;
        let source_path = input.to_str().ok_or_else(|| {
            OcrError::Configuration("Android's OCR cache path is not valid UTF-8.".to_string())
        })?;
        let result = app
            .ostudy_platform()
            .recognize_image(RecognizeImageRequest {
                source_path: source_path.to_string(),
                language: options.language.clone(),
                timeout_seconds: options.timeout_seconds.max(1),
            });
        let _ = fs::remove_file(&input);
        let result = result.map_err(|error| {
            OcrError::ProcessingFailed(format!("Android on-device OCR failed: {error}"))
        })?;
        if self.is_canceled() {
            return Err(canceled_ocr_error());
        }
        if (result.width, result.height) != expected_dimensions {
            return Err(OcrError::InvalidImageData(
                "Android OCR decoded image dimensions that differ from the bounded PNG header."
                    .to_string(),
            ));
        }
        Ok(OcrProcessingResult::new(
            result.text,
            result.confidence.clamp(0.0, 1.0),
            Vec::new(),
            result.duration_millis,
            result.engine,
            result.language,
            expected_dimensions,
        ))
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    fn process_desktop_tesseract(
        &self,
        image_data: &[u8],
        options: &OcrOptions,
    ) -> OcrResult<OcrProcessingResult> {
        let dimensions = png_dimensions(image_data)?;
        let workspace = TempOcrWorkspace::create()?;
        let input = workspace.path.join("page.png");
        fs::write(&input, image_data)?;
        let language = tesseract_language(&options.language);
        let started = Instant::now();
        let timeout = Duration::from_secs(u64::from(options.timeout_seconds.max(1)));

        let primary = self.run_tesseract_pass(
            &input,
            &workspace,
            &language,
            PRIMARY_PAGE_SEGMENTATION_MODE,
            started,
            timeout,
        )?;
        let mut selected = primary;

        if !selected.is_strong() {
            for psm in FALLBACK_PAGE_SEGMENTATION_MODES {
                let candidate =
                    self.run_tesseract_pass(&input, &workspace, &language, psm, started, timeout)?;
                if candidate.score() > selected.score() {
                    selected = candidate;
                }
                if selected.is_strong() {
                    break;
                }
            }
        }

        crate::logging::info(
            "document_ocr_layout_selected",
            format!(
                "document={} psm={} confidence_milli={} nonspace={} alphabetic={} fallback_used={}",
                self.document_id.as_deref().unwrap_or("unknown"),
                selected.psm,
                confidence_milli(selected.confidence),
                selected.nonspace_chars,
                selected.alphabetic_chars,
                selected.psm != PRIMARY_PAGE_SEGMENTATION_MODE,
            ),
        );

        Ok(OcrProcessingResult::new(
            selected.text,
            selected.confidence,
            Vec::new(),
            u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            format!("{ENGINE_NAME} psm={}", selected.psm),
            language,
            dimensions,
        ))
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    fn run_tesseract_pass(
        &self,
        input: &Path,
        workspace: &TempOcrWorkspace,
        language: &str,
        psm: u8,
        started: Instant,
        timeout: Duration,
    ) -> OcrResult<ParsedTsv> {
        if self.is_canceled() {
            return Err(canceled_ocr_error());
        }
        if started.elapsed() >= timeout {
            return Err(OcrError::ProcessingFailed(
                "Tesseract exhausted the bounded OCR page timeout before a recovery pass could start."
                    .to_string(),
            ));
        }

        let output_base = workspace.path.join(format!("result-psm-{psm}"));
        let mut child = Command::new(&self.executable)
            .arg(input)
            .arg(&output_base)
            .arg("-l")
            .arg(language)
            .arg("--dpi")
            .arg(TESSERACT_INPUT_DPI.to_string())
            .arg("--psm")
            .arg(psm.to_string())
            .arg("-c")
            .arg("preserve_interword_spaces=1")
            .arg("tsv")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| {
                OcrError::ProviderNotAvailable(format!(
                    "Could not start '{}': {error}",
                    self.executable
                ))
            })?;

        let status = loop {
            if self.is_canceled() {
                let _ = child.kill();
                let _ = child.wait();
                return Err(canceled_ocr_error());
            }
            if let Some(status) = child.try_wait()? {
                break status;
            }
            if started.elapsed() >= timeout {
                let _ = child.kill();
                let _ = child.wait();
                return Err(OcrError::ProcessingFailed(format!(
                    "Tesseract exceeded the {} second total OCR page timeout.",
                    timeout.as_secs()
                )));
            }
            thread::sleep(Duration::from_millis(WAIT_INTERVAL_MS));
        };
        if !status.success() {
            return Err(OcrError::ProcessingFailed(format!(
                "Tesseract exited with status {status} in page-segmentation mode {psm}. Verify that the '{language}' trained data is installed."
            )));
        }
        if self.is_canceled() {
            return Err(canceled_ocr_error());
        }

        let tsv_path = output_base.with_extension("tsv");
        let tsv_size = fs::metadata(&tsv_path)
            .map_err(|error| {
                OcrError::ProcessingFailed(format!(
                    "Tesseract completed without a readable TSV result: {error}"
                ))
            })?
            .len();
        if tsv_size > MAX_TSV_BYTES {
            return Err(OcrError::ProcessingFailed(format!(
                "Tesseract produced an unexpectedly large OCR result ({tsv_size} bytes)."
            )));
        }
        let tsv = fs::read_to_string(&tsv_path).map_err(|error| {
            OcrError::ProcessingFailed(format!(
                "Tesseract completed without a readable TSV result: {error}"
            ))
        })?;
        Ok(parse_tesseract_tsv(&tsv, psm))
    }
}

impl OcrProvider for PlatformOcrProvider {
    fn process_image(
        &self,
        image_data: &[u8],
        options: &OcrOptions,
    ) -> OcrResult<OcrProcessingResult> {
        self.validate_image_data(image_data)?;
        if self.is_canceled() {
            return Err(canceled_ocr_error());
        }

        #[cfg(target_os = "android")]
        {
            self.ensure_available()?;
            self.process_android_ml_kit(image_data, options)
        }
        #[cfg(target_os = "ios")]
        {
            let _ = options;
            self.ensure_available()?;
            unreachable!("iOS OCR availability cannot succeed before Apple Vision is linked")
        }
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            self.process_desktop_tesseract(image_data, options)
        }
    }

    fn supported_formats(&self) -> Vec<ImageFormat> {
        vec![ImageFormat::Png]
    }

    fn engine_name(&self) -> &str {
        ENGINE_NAME
    }

    fn engine_type(&self) -> OcrEngine {
        // oxidize-pdf 4.2.x has no generic/native-mobile engine discriminator.
        // ostudy persists the more precise `engine_name` string for provenance.
        OcrEngine::Tesseract
    }
}

pub(crate) fn is_cancellation_error(error: &OcrError) -> bool {
    matches!(error, OcrError::ProcessingFailed(detail) if detail == OCR_CANCELED_MARKER)
}

fn canceled_ocr_error() -> OcrError {
    OcrError::ProcessingFailed(OCR_CANCELED_MARKER.to_string())
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[derive(Debug, PartialEq)]
struct ParsedTsv {
    text: String,
    confidence: f64,
    nonspace_chars: usize,
    alphabetic_chars: usize,
    psm: u8,
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
impl ParsedTsv {
    fn is_strong(&self) -> bool {
        self.nonspace_chars >= STRONG_OCR_MIN_NONSPACE
            && self.alphabetic_chars >= STRONG_OCR_MIN_ALPHABETIC
            && self.confidence >= STRONG_OCR_MIN_CONFIDENCE
    }

    fn score(&self) -> (bool, usize, usize, usize) {
        (
            self.is_strong(),
            self.alphabetic_chars,
            self.nonspace_chars,
            confidence_milli(self.confidence),
        )
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn parse_tesseract_tsv(tsv: &str, psm: u8) -> ParsedTsv {
    let mut output_lines = Vec::<String>::new();
    let mut current_line_key = None::<(i64, i64, i64)>;
    let mut current_words = Vec::<String>::new();
    let mut confidence_sum = 0.0_f64;
    let mut confidence_samples = 0_usize;

    for line in tsv.lines().skip(1) {
        let fields = line.splitn(12, '\t').collect::<Vec<_>>();
        if fields.len() != 12 || fields[0] != "5" {
            continue;
        }
        let text = fields[11].trim();
        if text.is_empty() {
            continue;
        }
        let key = (
            fields[2].parse::<i64>().unwrap_or_default(),
            fields[3].parse::<i64>().unwrap_or_default(),
            fields[4].parse::<i64>().unwrap_or_default(),
        );
        if current_line_key.is_some_and(|existing| existing != key) && !current_words.is_empty() {
            output_lines.push(current_words.join(" "));
            current_words.clear();
        }
        current_line_key = Some(key);
        current_words.push(text.to_string());

        if let Ok(confidence) = fields[10].parse::<f64>() {
            if confidence.is_finite() && confidence >= 0.0 {
                confidence_sum += confidence.clamp(0.0, 100.0);
                confidence_samples = confidence_samples.saturating_add(1);
            }
        }
    }
    if !current_words.is_empty() {
        output_lines.push(current_words.join(" "));
    }

    let text = output_lines.join("\n");
    let confidence = if confidence_samples == 0 {
        0.0
    } else {
        (confidence_sum / confidence_samples as f64 / 100.0).clamp(0.0, 1.0)
    };
    let nonspace_chars = text
        .chars()
        .filter(|character| !character.is_whitespace())
        .count();
    let alphabetic_chars = text
        .chars()
        .filter(|character| character.is_alphabetic())
        .count();
    ParsedTsv {
        text,
        confidence,
        nonspace_chars,
        alphabetic_chars,
        psm,
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn confidence_milli(value: f64) -> usize {
    if !value.is_finite() {
        return 0;
    }
    (value.clamp(0.0, 1.0) * 1_000.0).round() as usize
}

fn png_dimensions(image_data: &[u8]) -> OcrResult<(u32, u32)> {
    const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    if image_data.len() < 24 || !image_data.starts_with(PNG_SIGNATURE) {
        return Err(OcrError::InvalidImageData(
            "ostudy OCR expects a rendered PNG page.".to_string(),
        ));
    }
    let width =
        u32::from_be_bytes(image_data[16..20].try_into().map_err(|_| {
            OcrError::InvalidImageData("PNG width header is truncated.".to_string())
        })?);
    let height =
        u32::from_be_bytes(image_data[20..24].try_into().map_err(|_| {
            OcrError::InvalidImageData("PNG height header is truncated.".to_string())
        })?);
    if width == 0 || height == 0 {
        return Err(OcrError::InvalidImageData(
            "Rendered OCR page has zero image dimensions.".to_string(),
        ));
    }
    Ok((width, height))
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn tesseract_language(language: &str) -> String {
    match language.trim().to_ascii_lowercase().as_str() {
        "en" => "eng".to_string(),
        "es" => "spa".to_string(),
        "fr" => "fra".to_string(),
        "de" => "deu".to_string(),
        "it" => "ita".to_string(),
        "pt" => "por".to_string(),
        "nl" => "nld".to_string(),
        "" => "eng".to_string(),
        value => value.to_string(),
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn create_private_directory(path: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;

        let mut builder = fs::DirBuilder::new();
        builder.mode(0o700);
        builder.create(path)
    }
    #[cfg(not(unix))]
    {
        fs::create_dir(path)
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
struct TempOcrWorkspace {
    path: PathBuf,
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
impl TempOcrWorkspace {
    fn create() -> OcrResult<Self> {
        let root = std::env::temp_dir();
        let process = std::process::id();
        let nanos = crate::platform::now_utc_millis().unwrap_or_default();
        for _ in 0..32 {
            let sequence = WORKSPACE_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = root.join(format!("ostudy-ocr-{process}-{nanos}-{sequence}"));
            match create_private_directory(&path) {
                Ok(()) => return Ok(Self { path }),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(OcrError::Io(error)),
            }
        }
        Err(OcrError::Configuration(
            "Could not allocate a temporary OCR workspace.".to_string(),
        ))
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
impl Drop for TempOcrWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    #[test]
    fn tesseract_language_maps_common_iso_codes() {
        assert_eq!(tesseract_language("en"), "eng");
        assert_eq!(tesseract_language("fr"), "fra");
        assert_eq!(tesseract_language("eng+spa"), "eng+spa");
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    #[test]
    fn tsv_parser_preserves_lines_and_averages_word_confidence() {
        let tsv = concat!(
            "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n",
            "5\t1\t1\t1\t1\t1\t0\t0\t10\t10\t90.0\tThe\n",
            "5\t1\t1\t1\t1\t2\t0\t0\t10\t10\t80.0\tpurpose\n",
            "5\t1\t1\t1\t2\t1\t0\t0\t10\t10\t70.0\tof\n",
            "5\t1\t1\t1\t2\t2\t0\t0\t10\t10\t60.0\tHDLs\n"
        );
        let parsed = parse_tesseract_tsv(tsv, 3);
        assert_eq!(parsed.text, "The purpose\nof HDLs");
        assert!((parsed.confidence - 0.75).abs() < 0.0001);
        assert_eq!(parsed.psm, 3);
        assert_eq!(parsed.alphabetic_chars, 16);
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    #[test]
    fn weak_primary_layout_can_be_outscored_by_substantive_fallback() {
        let weak = ParsedTsv {
            text: "21".to_string(),
            confidence: 0.96,
            nonspace_chars: 2,
            alphabetic_chars: 0,
            psm: 3,
        };
        let fallback = ParsedTsv {
            text: "Drugs affecting the cardiovascular system".to_string(),
            confidence: 0.88,
            nonspace_chars: 37,
            alphabetic_chars: 36,
            psm: 11,
        };
        assert!(!weak.is_strong());
        assert!(fallback.is_strong());
        assert!(fallback.score() > weak.score());
    }

    #[test]
    fn cancellation_marker_is_distinct_from_provider_failures() {
        assert!(is_cancellation_error(&canceled_ocr_error()));
        assert!(!is_cancellation_error(&OcrError::ProcessingFailed(
            "ordinary OCR failure".to_string()
        )));
    }

    #[test]
    fn png_dimensions_rejects_non_png_data() {
        assert!(png_dimensions(b"not-a-png").is_err());
    }

    #[cfg(all(unix, not(any(target_os = "android", target_os = "ios"))))]
    #[test]
    fn temporary_ocr_workspace_is_private_to_the_current_user() {
        use std::os::unix::fs::PermissionsExt;

        let workspace = TempOcrWorkspace::create().expect("OCR workspace should be created");
        let mode = fs::metadata(&workspace.path)
            .expect("OCR workspace metadata should be readable")
            .permissions()
            .mode();
        assert_eq!(mode & 0o077, 0);
    }
}
