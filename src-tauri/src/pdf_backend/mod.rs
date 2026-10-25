mod contracts;
mod extraction;
mod layout;
mod quality;
mod recovery;
mod ruled_tables;

pub(crate) use contracts::{
    ExtractedDocument, ExtractedSemanticElement, ExtractionRecoveryPlan, OcrPageCandidate,
    PdfBackendError, PdfBackendErrorKind, RecoveredPageText,
};
#[cfg(test)]
pub(crate) use contracts::{ExtractedSemanticChunk, ExtractedTextPage, SemanticBBox};
pub(crate) use extraction::extract_document;
pub(crate) use quality::{assess_text_quality, native_page_requires_ocr};
pub(crate) use recovery::{apply_text_recovery, plan_text_recovery};

pub(crate) const fn active_backend_id() -> &'static str {
    "oxidize-pdf-5.5.0-rag-native-first-ocr-v10"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_backend_identity_remains_stable() {
        assert_eq!(
            active_backend_id(),
            "oxidize-pdf-5.5.0-rag-native-first-ocr-v10"
        );
    }
}
