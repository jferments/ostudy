use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PdfBackendErrorKind {
    Open,
    PageCount,
    ExtractText,
}

#[derive(Debug)]
pub(crate) struct PdfBackendError {
    pub kind: PdfBackendErrorKind,
    pub page_number: Option<usize>,
    detail: String,
}

impl PdfBackendError {
    pub(crate) fn new(
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

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SemanticBBox {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Debug)]
pub(crate) struct ExtractedTextPage {
    pub page_number: usize,
    pub text: String,
}

#[derive(Clone, Debug)]
pub(crate) struct ExtractedSemanticElement {
    pub element_index: usize,
    pub page_number: usize,
    pub element_type: String,
    pub text: String,
    pub parent_heading: Option<String>,
    pub heading_path: Vec<String>,
    pub bbox: SemanticBBox,
    pub confidence: f64,
    pub font_name: Option<String>,
    pub font_size: Option<f64>,
    pub is_bold: bool,
    pub is_italic: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct ExtractedSemanticChunk {
    pub chunk_index: usize,
    pub text: String,
    pub full_text: String,
    pub page_numbers: Vec<usize>,
    pub bounding_boxes: Vec<SemanticBBox>,
    pub element_types: Vec<String>,
    pub heading_context: Option<String>,
    pub token_estimate: usize,
    pub is_oversized: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct ExtractedDocument {
    pub page_count: usize,
    pub pages: Vec<ExtractedTextPage>,
    pub elements: Vec<ExtractedSemanticElement>,
    pub chunks: Vec<ExtractedSemanticChunk>,
    pub native_text_failed_pages: Vec<usize>,
    pub semantic_analysis_degraded: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct OcrPageCandidate {
    pub page_number: usize,
    pub required: bool,
    pub reasons: Vec<&'static str>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ExtractionRecoveryPlan {
    pub ocr_pages: Vec<OcrPageCandidate>,
    pub native_semantic_fallback_pages: Vec<usize>,
}

#[derive(Clone, Debug)]
pub(crate) struct RecoveredPageText {
    pub page_number: usize,
    pub text: String,
    pub confidence: f64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TextQualityAssessment {
    pub suspicious: bool,
    pub substantive: bool,
    pub reasons: Vec<&'static str>,
}
