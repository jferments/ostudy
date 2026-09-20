//! App-owned OCR regions. Bounds are measured in the supplied upright raster;
//! missing or inconsistent geometry is never reconstructed from flattened text.

use oxidize_pdf::{OcrError, OcrProcessingResult, OcrResult};
use serde::{Deserialize, Serialize};

mod parsers;
mod runtime;

/// Both native layout formats use the same application-owned geometry contract.
/// Platform selection stays in the existing OCR adapter, not this representation.
pub(super) fn parse_lines(layout: &str, format: &str) -> Vec<OcrLine> {
    match format {
        "tsv" => parsers::tsv_lines(layout),
        "hocr" => parsers::hocr_lines(layout),
        _ => Vec::new(),
    }
}

pub(crate) const MAX_OCR_LINES: usize = 4096;
pub(crate) const MAX_OCR_TEXT: usize = 250_000;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct PixelBounds {
    pub left: u32,
    pub top: u32,
    pub right: u32,
    pub bottom: u32,
}

impl PixelBounds {
    pub(crate) fn within(self, width: u32, height: u32) -> bool {
        self.left < self.right
            && self.top < self.bottom
            && self.right <= width
            && self.bottom <= height
    }

    fn contains(self, other: Self) -> bool {
        self.left <= other.left
            && self.top <= other.top
            && self.right >= other.right
            && self.bottom >= other.bottom
    }

    fn union(self, other: Self) -> Self {
        Self {
            left: self.left.min(other.left),
            top: self.top.min(other.top),
            right: self.right.max(other.right),
            bottom: self.bottom.max(other.bottom),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct OcrLine {
    pub text: String,
    pub bounds: Option<PixelBounds>,
    /// Extractor-supplied paragraph identity, not an inferred semantic heading.
    pub group: String,
    pub confidence: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RegionOcrResult {
    pub version: u32,
    pub text: String,
    pub confidence: f64,
    pub lines: Vec<OcrLine>,
    pub duration_millis: u64,
    pub engine: String,
    pub language: String,
    pub width: u32,
    pub height: u32,
}

impl RegionOcrResult {
    /// Used before persistence and after reading retained extraction. Keep all
    /// literal content; resource overflows fail rather than truncating evidence.
    pub(crate) fn validate(&self) -> OcrResult<()> {
        if self.version != 1
            || self.width == 0
            || self.height == 0
            || self.width > 12_000
            || self.height > 12_000
            || u64::from(self.width) * u64::from(self.height) > 50_000_000
            || self.lines.len() > MAX_OCR_LINES
            || self.text.chars().count() > MAX_OCR_TEXT
            || self.engine.is_empty()
            || self.engine.len() > 256
            || self.language.len() > 32
            || !valid_confidence(self.confidence)
        {
            return Err(invalid_regions());
        }
        let mut length = 0_usize;
        for line in &self.lines {
            length = length.saturating_add(line.text.chars().count());
            if length > MAX_OCR_TEXT
                || line.text.trim().is_empty()
                || line.group.len() > 128
                || line
                    .confidence
                    .is_some_and(|value| !valid_confidence(value))
                || line
                    .bounds
                    .is_some_and(|bounds| !bounds.within(self.width, self.height))
            {
                return Err(invalid_regions());
            }
        }
        if !same_words(&self.text, &self.lines) {
            return Err(invalid_regions());
        }
        Ok(())
    }

    /// The existing PDF consumer receives the same text and confidence. Image
    /// ingestion additionally retains the actual OCR layout without re-OCR.
    pub(super) fn into_legacy(self) -> OcrProcessingResult {
        OcrProcessingResult::new(
            self.text,
            self.confidence,
            Vec::new(),
            self.duration_millis,
            self.engine,
            self.language,
            (self.width, self.height),
        )
    }

    pub(super) fn reconcile_geometry(&mut self) {
        if self.lines.len() > MAX_OCR_LINES || !same_words(&self.text, &self.lines) {
            self.lines = if self.text.trim().is_empty() {
                Vec::new()
            } else {
                vec![OcrLine {
                    text: self.text.clone(),
                    bounds: None,
                    group: String::new(),
                    confidence: Some(self.confidence),
                }]
            };
        }
        for line in &mut self.lines {
            if line
                .bounds
                .is_some_and(|bounds| !bounds.within(self.width, self.height))
            {
                line.bounds = None;
            }
        }
    }
}

fn same_words(text: &str, lines: &[OcrLine]) -> bool {
    text.split_whitespace()
        .eq(lines.iter().flat_map(|line| line.text.split_whitespace()))
}

fn valid_confidence(value: f64) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

fn invalid_regions() -> OcrError {
    OcrError::ProcessingFailed(
        "OCR returned unsupported, incomplete or unbounded image regions.".to_string(),
    )
}

#[cfg(test)]
mod tests;
