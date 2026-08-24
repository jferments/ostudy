use crate::{document_domain::DocumentSourceDescriptor, pdf_backend::ExtractedDocument};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ExtractionMode {
    Native,
    PageTextFallback,
    OcrFallback,
}

impl ExtractionMode {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::PageTextFallback => "page_text_fallback",
            Self::OcrFallback => "ocr_fallback",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum IngestionProgress {
    Rendering {
        completed_units: usize,
        total_units: usize,
    },
    Ocr {
        completed_units: usize,
        total_units: usize,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct OcrIngestionSummary {
    pub page_count: usize,
    pub mean_confidence: Option<f64>,
    pub engine: String,
}

#[derive(Clone, Debug)]
pub(crate) struct IngestedPage {
    pub unit_number: i64,
    pub plain_text: String,
    pub text_sha256: String,
    pub render_relative_path: String,
    pub render_byte_size: i64,
    pub render_sha256: String,
}

#[derive(Debug)]
pub(crate) struct IngestedDocument {
    pub source: DocumentSourceDescriptor,
    pub pages: Vec<IngestedPage>,
    pub semantic: ExtractedDocument,
    pub extraction_backend: &'static str,
    pub render_backend: &'static str,
    pub extraction_mode: ExtractionMode,
    pub ocr: OcrIngestionSummary,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extraction_modes_have_stable_database_values() {
        assert_eq!(ExtractionMode::Native.as_str(), "native");
        assert_eq!(
            ExtractionMode::PageTextFallback.as_str(),
            "page_text_fallback"
        );
        assert_eq!(ExtractionMode::OcrFallback.as_str(), "ocr_fallback");
    }
}
