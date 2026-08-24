mod contracts;
mod extraction;
mod quality;
mod recovery;

pub(crate) use contracts::{
    ExtractedDocument, ExtractionRecoveryPlan, OcrPageCandidate, PdfBackendError,
    PdfBackendErrorKind, RecoveredPageText,
};
#[cfg(test)]
pub(crate) use contracts::{ExtractedSemanticChunk, ExtractedTextPage};
pub(crate) use extraction::extract_document;
pub(crate) use quality::{assess_text_quality, native_page_requires_ocr};
pub(crate) use recovery::{apply_text_recovery, plan_text_recovery};

pub(crate) const fn active_backend_id() -> &'static str {
    "oxidize-pdf-4.2.2-rag-native-first-ocr-v6"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_backend_identity_remains_stable() {
        assert_eq!(
            active_backend_id(),
            "oxidize-pdf-4.2.2-rag-native-first-ocr-v6"
        );
    }
}
