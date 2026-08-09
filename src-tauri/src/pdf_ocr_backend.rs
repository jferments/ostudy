//! Optional OCR provider used when the native PDF text layer is unreliable.
//!
//! `oxidize-pdf` deliberately exposes OCR as a provider trait. ostudy implements
//! that trait against the local Tesseract command-line engine instead of taking
//! the optional `rusty-tesseract` wrapper dependency. Normal PDFs never invoke
//! this module; Tesseract is discovered only after extraction-quality checks
//! decide that OCR is needed.

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use oxidize_pdf::{
    graphics::ImageFormat, OcrEngine, OcrError, OcrOptions, OcrProcessingResult, OcrProvider,
    OcrResult,
};

const ENGINE_NAME: &str = "Tesseract CLI via oxidize-pdf OcrProvider";
const DEFAULT_EXECUTABLE: &str = "tesseract";
const EXECUTABLE_ENV: &str = "OSTUDY_TESSERACT_PATH";
const WAIT_INTERVAL_MS: u64 = 25;
static WORKSPACE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug)]
pub(crate) struct TesseractCliOcrProvider {
    executable: String,
}

impl Default for TesseractCliOcrProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl TesseractCliOcrProvider {
    pub(crate) fn new() -> Self {
        let executable = std::env::var(EXECUTABLE_ENV)
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_EXECUTABLE.to_string());
        Self { executable }
    }

    pub(crate) fn ensure_available(&self) -> OcrResult<()> {
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

impl OcrProvider for TesseractCliOcrProvider {
    fn process_image(
        &self,
        image_data: &[u8],
        options: &OcrOptions,
    ) -> OcrResult<OcrProcessingResult> {
        self.validate_image_data(image_data)?;
        self.ensure_available()?;
        let dimensions = png_dimensions(image_data)?;
        let workspace = TempOcrWorkspace::create()?;
        let input = workspace.path.join("page.png");
        fs::write(&input, image_data)?;
        let output_base = workspace.path.join("result");
        let language = tesseract_language(&options.language);
        let started = Instant::now();

        let mut child = Command::new(&self.executable)
            .arg(&input)
            .arg(&output_base)
            .arg("-l")
            .arg(&language)
            .arg("--psm")
            .arg("3")
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

        let timeout = Duration::from_secs(u64::from(options.timeout_seconds.max(1)));
        let status = loop {
            if let Some(status) = child.try_wait()? {
                break status;
            }
            if started.elapsed() >= timeout {
                let _ = child.kill();
                let _ = child.wait();
                return Err(OcrError::ProcessingFailed(format!(
                    "Tesseract exceeded the {} second OCR timeout.",
                    options.timeout_seconds.max(1)
                )));
            }
            thread::sleep(Duration::from_millis(WAIT_INTERVAL_MS));
        };
        if !status.success() {
            return Err(OcrError::ProcessingFailed(format!(
                "Tesseract exited with status {status}. Verify that the '{language}' trained data is installed."
            )));
        }

        let tsv_path = output_base.with_extension("tsv");
        let tsv = fs::read_to_string(&tsv_path).map_err(|error| {
            OcrError::ProcessingFailed(format!(
                "Tesseract completed without a readable TSV result: {error}"
            ))
        })?;
        let parsed = parse_tesseract_tsv(&tsv);
        Ok(OcrProcessingResult::new(
            parsed.text,
            parsed.confidence,
            Vec::new(),
            u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            ENGINE_NAME.to_string(),
            language,
            dimensions,
        ))
    }

    fn supported_formats(&self) -> Vec<ImageFormat> {
        vec![ImageFormat::Png]
    }

    fn engine_name(&self) -> &str {
        ENGINE_NAME
    }

    fn engine_type(&self) -> OcrEngine {
        OcrEngine::Tesseract
    }
}

#[derive(Debug, Eq, PartialEq)]
struct ParsedTsv {
    text: String,
    confidence_milli: usize,
    confidence_samples: usize,
}

impl ParsedTsv {
    fn confidence(&self) -> f64 {
        if self.confidence_samples == 0 {
            return 0.0;
        }
        (self.confidence_milli as f64 / self.confidence_samples as f64 / 100_000.0)
            .clamp(0.0, 1.0)
    }
}

fn parse_tesseract_tsv(tsv: &str) -> ParsedTsvResult {
    let mut output_lines = Vec::<String>::new();
    let mut current_line_key = None::<(i64, i64, i64)>;
    let mut current_words = Vec::<String>::new();
    let mut confidence_milli = 0_usize;
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
                let normalized = confidence.clamp(0.0, 100.0);
                confidence_milli = confidence_milli.saturating_add(
                    usize::try_from((normalized * 1_000.0).round() as u64).unwrap_or(usize::MAX),
                );
                confidence_samples = confidence_samples.saturating_add(1);
            }
        }
    }
    if !current_words.is_empty() {
        output_lines.push(current_words.join(" "));
    }

    ParsedTsvResult(ParsedTsv {
        text: output_lines.join("\n"),
        confidence_milli,
        confidence_samples,
    })
}

struct ParsedTsvResult(ParsedTsv);

impl ParsedTsvResult {
    fn text(&self) -> &str {
        &self.0.text
    }
}

impl std::ops::Deref for ParsedTsvResult {
    type Target = ParsedTsv;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

fn png_dimensions(image_data: &[u8]) -> OcrResult<(u32, u32)> {
    const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    if image_data.len() < 24 || !image_data.starts_with(PNG_SIGNATURE) {
        return Err(OcrError::InvalidImageData(
            "ostudy OCR expects a rendered PNG page.".to_string(),
        ));
    }
    let width = u32::from_be_bytes(image_data[16..20].try_into().map_err(|_| {
        OcrError::InvalidImageData("PNG width header is truncated.".to_string())
    })?);
    let height = u32::from_be_bytes(image_data[20..24].try_into().map_err(|_| {
        OcrError::InvalidImageData("PNG height header is truncated.".to_string())
    })?);
    if width == 0 || height == 0 {
        return Err(OcrError::InvalidImageData(
            "Rendered OCR page has zero image dimensions.".to_string(),
        ));
    }
    Ok((width, height))
}

fn tesseract_language(language: &str) -> String {
    match language.trim().to_ascii_lowercase().as_str() {
        "en" => "eng".to_string(),
        "es" => "spa".to_string(),
        "fr" => "fra".to_string(),
        "de" => "deu".to_string(),
        "it" => "ita".to_string(),
        "pt" => "por".to_string(),
        "nl" => "nld".to_string(),
        value if value.is_empty() => "eng".to_string(),
        value => value.to_string(),
    }
}

struct TempOcrWorkspace {
    path: PathBuf,
}

impl TempOcrWorkspace {
    fn create() -> OcrResult<Self> {
        let root = std::env::temp_dir();
        let process = std::process::id();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        for _ in 0..32 {
            let sequence = WORKSPACE_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = root.join(format!("ostudy-ocr-{process}-{nanos}-{sequence}"));
            match fs::create_dir(&path) {
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

impl Drop for TempOcrWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tesseract_language_maps_common_iso_codes() {
        assert_eq!(tesseract_language("en"), "eng");
        assert_eq!(tesseract_language("fr"), "fra");
        assert_eq!(tesseract_language("eng+spa"), "eng+spa");
    }

    #[test]
    fn tsv_parser_preserves_lines_and_averages_word_confidence() {
        let tsv = concat!(
            "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n",
            "5\t1\t1\t1\t1\t1\t0\t0\t10\t10\t90.0\tThe\n",
            "5\t1\t1\t1\t1\t2\t0\t0\t10\t10\t80.0\tpurpose\n",
            "5\t1\t1\t1\t2\t1\t0\t0\t10\t10\t70.0\tof\n",
            "5\t1\t1\t1\t2\t2\t0\t0\t10\t10\t60.0\tHDLs\n"
        );
        let parsed = parse_tesseract_tsv(tsv);
        assert_eq!(parsed.text(), "The purpose\nof HDLs");
        assert!((parsed.confidence() - 0.75).abs() < 0.0001);
    }

    #[test]
    fn png_dimensions_rejects_non_png_data() {
        assert!(png_dimensions(b"not-a-png").is_err());
    }
}
