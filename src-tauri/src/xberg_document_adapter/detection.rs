use std::{fs, path::Path};

use crate::{document_domain::DocumentFormat, error::CommandError};

use super::{
    opc::{inspect_relocated_ooxml, normalize_for_xberg, OoxmlNormalizationPlan},
    support::media_type_for_format,
};

const MAX_XBERG_SOURCE_BYTES: u64 = 512 * 1024 * 1024;
const OLE_SIGNATURE: &[u8; 8] = b"\xd0\xcf\x11\xe0\xa1\xb1\x1a\xe1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DetectedDocument {
    pub(crate) format: DocumentFormat,
    pub(crate) media_type: String,
    ooxml_normalization: Option<OoxmlNormalizationPlan>,
}

impl DetectedDocument {
    pub(crate) fn used_ooxml_path_normalization(&self) -> bool {
        self.ooxml_normalization.is_some()
    }

    pub(crate) fn prepare_extraction_bytes(&self, bytes: Vec<u8>) -> Result<Vec<u8>, CommandError> {
        match self.ooxml_normalization.as_ref() {
            Some(plan) => normalize_for_xberg(&bytes, plan),
            None => Ok(bytes),
        }
    }

    fn direct(format: DocumentFormat, media_type: impl Into<String>) -> Self {
        Self {
            format,
            media_type: media_type.into(),
            ooxml_normalization: None,
        }
    }
}

pub(crate) fn supports(format: DocumentFormat) -> bool {
    matches!(
        format,
        DocumentFormat::Doc
            | DocumentFormat::Docx
            | DocumentFormat::Ppt
            | DocumentFormat::Pptx
            | DocumentFormat::Odt
            | DocumentFormat::Odp
            | DocumentFormat::Rtf
            | DocumentFormat::Epub
            | DocumentFormat::Excel
            | DocumentFormat::Ods
            | DocumentFormat::Csv
    )
}

pub(crate) fn detected_document_format(
    bytes: &[u8],
    filename: &str,
) -> Result<DocumentFormat, CommandError> {
    Ok(detect_document(bytes, filename)?.format)
}

pub(crate) fn detect_document(
    bytes: &[u8],
    filename: &str,
) -> Result<DetectedDocument, CommandError> {
    if bytes.starts_with(b"%PDF-") {
        return Ok(DetectedDocument::direct(
            DocumentFormat::Pdf,
            "application/pdf",
        ));
    }

    if bytes.starts_with(OLE_SIGNATURE) {
        return detect_legacy_office(filename);
    }

    if let Ok(detected) = xberg::core::mime::detect_mime_type_from_bytes(bytes) {
        if detected == "text/plain" && extension(filename).as_deref() == Some("csv") {
            return Ok(DetectedDocument::direct(
                DocumentFormat::Csv,
                media_type_for_format(DocumentFormat::Csv),
            ));
        }
        if let Ok(document) = map_mime_type(&detected) {
            return Ok(document);
        }
    }

    if let Some(plan) = inspect_relocated_ooxml(bytes)? {
        return Ok(DetectedDocument {
            format: plan.format(),
            media_type: plan.media_type().to_string(),
            ooxml_normalization: Some(plan),
        });
    }

    Err(unsupported_format())
}

pub(crate) fn probe(format: DocumentFormat, source_path: &Path) -> Result<bool, CommandError> {
    if !supports(format) {
        return Ok(false);
    }
    let bytes = read_bounded_source(source_path)?;
    let filename = source_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    Ok(detect_document(&bytes, filename)?.format == format)
}

pub(crate) fn read_bounded_source(path: &Path) -> Result<Vec<u8>, CommandError> {
    let metadata = fs::metadata(path).map_err(CommandError::io)?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > MAX_XBERG_SOURCE_BYTES {
        return Err(CommandError::new(
            "invalid_document_size",
            "The document is empty or exceeds oStudy's 512 MB source limit.",
        ));
    }
    fs::read(path).map_err(CommandError::io)
}

fn map_mime_type(mime_type: &str) -> Result<DetectedDocument, CommandError> {
    let format = match mime_type.to_ascii_lowercase().as_str() {
        "application/msword" => DocumentFormat::Doc,
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
        | "application/vnd.ms-word.document.macroenabled.12"
        | "application/vnd.openxmlformats-officedocument.wordprocessingml.template"
        | "application/vnd.ms-word.template.macroenabled.12"
        | "application/docx" => DocumentFormat::Docx,
        "application/vnd.ms-powerpoint" => DocumentFormat::Ppt,
        "application/vnd.openxmlformats-officedocument.presentationml.presentation"
        | "application/vnd.ms-powerpoint.presentation.macroenabled.12"
        | "application/vnd.openxmlformats-officedocument.presentationml.slideshow"
        | "application/vnd.ms-powerpoint.slideshow.macroenabled.12"
        | "application/vnd.openxmlformats-officedocument.presentationml.template"
        | "application/vnd.ms-powerpoint.template.macroenabled.12" => DocumentFormat::Pptx,
        "application/vnd.oasis.opendocument.text" => DocumentFormat::Odt,
        "application/vnd.oasis.opendocument.presentation" => DocumentFormat::Odp,
        "application/rtf" | "text/rtf" => DocumentFormat::Rtf,
        "application/epub+zip" => DocumentFormat::Epub,
        "application/vnd.ms-excel"
        | "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
        | "application/vnd.ms-excel.sheet.macroenabled.12"
        | "application/vnd.ms-excel.sheet.binary.macroenabled.12"
        | "application/vnd.openxmlformats-officedocument.spreadsheetml.template"
        | "application/vnd.ms-excel.template.macroenabled.12" => DocumentFormat::Excel,
        "application/vnd.oasis.opendocument.spreadsheet" => DocumentFormat::Ods,
        "text/csv" | "text/tab-separated-values" => DocumentFormat::Csv,
        _ => return Err(unsupported_format()),
    };
    Ok(DetectedDocument::direct(format, mime_type))
}

fn detect_legacy_office(filename: &str) -> Result<DetectedDocument, CommandError> {
    let extension = extension(filename);
    let (format, media_type) = match extension.as_deref() {
        Some("doc" | "dot") => (DocumentFormat::Doc, "application/msword"),
        Some("ppt" | "pps" | "pot") => (DocumentFormat::Ppt, "application/vnd.ms-powerpoint"),
        Some("xls" | "xlt") => (DocumentFormat::Excel, "application/vnd.ms-excel"),
        _ => return Err(unsupported_format()),
    };
    Ok(DetectedDocument::direct(format, media_type))
}

fn extension(filename: &str) -> Option<String> {
    Path::new(filename)
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
}

fn unsupported_format() -> CommandError {
    CommandError::new(
        "unsupported_document_format",
        "The selected file is not a supported document type.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pdf_is_detected_but_never_owned_by_the_xberg_non_pdf_adapter() {
        assert_eq!(
            detected_document_format(b"%PDF-1.7", "renamed.docx").unwrap(),
            DocumentFormat::Pdf
        );
        assert!(!supports(DocumentFormat::Pdf));
    }

    #[test]
    fn random_bytes_are_not_accepted_from_an_office_extension() {
        assert_eq!(
            detected_document_format(b"not an OOXML package", "renamed.docx")
                .unwrap_err()
                .code,
            "unsupported_document_format"
        );
    }

    #[test]
    fn csv_is_the_only_plain_text_filename_fallback() {
        assert_eq!(
            detected_document_format(b"name,value\na,1\n", "table.csv").unwrap(),
            DocumentFormat::Csv
        );
        assert_eq!(
            detected_document_format(b"name,value\na,1\n", "table.docx")
                .unwrap_err()
                .code,
            "unsupported_document_format"
        );
    }

    #[test]
    fn legacy_ole_requires_an_authorized_office_extension() {
        let mut bytes = OLE_SIGNATURE.to_vec();
        bytes.extend_from_slice(b"fixture");
        assert_eq!(
            detected_document_format(&bytes, "lecture.ppt").unwrap(),
            DocumentFormat::Ppt
        );
        assert_eq!(
            detected_document_format(&bytes, "lecture.bin")
                .unwrap_err()
                .code,
            "unsupported_document_format"
        );
    }

    #[test]
    fn relocated_ooxml_detection_is_content_authoritative() {
        let bytes = include_bytes!("../../tests/fixtures/anydoc-v0.2.3/handmade-altpath.pptx");
        assert_eq!(
            detected_document_format(bytes, "renamed.bin").unwrap(),
            DocumentFormat::Pptx
        );
    }
}
