use std::{fs, path::Path};

use anydoc::Format;

use crate::{document_domain::DocumentFormat, error::CommandError};

const MAX_ANYDOC_SOURCE_BYTES: u64 = 512 * 1024 * 1024;

pub(crate) fn supports(format: DocumentFormat) -> bool {
    format.is_anydoc()
}

pub(crate) fn expected_format(format: DocumentFormat) -> Result<Format, CommandError> {
    match format {
        DocumentFormat::Doc => Ok(Format::Doc),
        DocumentFormat::Docx => Ok(Format::Docx),
        DocumentFormat::Ppt => Ok(Format::Ppt),
        DocumentFormat::Pptx => Ok(Format::Pptx),
        DocumentFormat::Odt => Ok(Format::Odt),
        DocumentFormat::Odp => Ok(Format::Odp),
        DocumentFormat::Rtf => Ok(Format::Rtf),
        DocumentFormat::Epub => Ok(Format::Epub),
        DocumentFormat::Excel => Ok(Format::Excel),
        DocumentFormat::Ods => Ok(Format::Ods),
        DocumentFormat::Csv => Ok(Format::Csv),
        DocumentFormat::Pdf => Err(CommandError::new(
            "document_adapter_boundary_violation",
            "PDF documents must use oStudy's native PDF adapter.",
        )),
        _ => Err(CommandError::new(
            "document_ingestion_adapter_unavailable",
            "This document format is not handled by the AnyDoc adapter family.",
        )),
    }
}

pub(crate) fn detected_document_format(
    bytes: &[u8],
    filename: &str,
) -> Result<DocumentFormat, CommandError> {
    if bytes.starts_with(b"%PDF-") {
        return Ok(DocumentFormat::Pdf);
    }
    detect_anydoc_format(bytes, filename)
        .ok_or_else(|| {
            CommandError::new(
                "unsupported_document_format",
                "The selected file is not a supported document type.",
            )
        })
        .and_then(map_anydoc_format)
}

pub(crate) fn probe(format: DocumentFormat, source_path: &Path) -> Result<bool, CommandError> {
    let bytes = read_bounded_source(source_path)?;
    let filename = source_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    Ok(detected_document_format(&bytes, filename)? == format)
}

pub(crate) fn detect_anydoc_format(bytes: &[u8], filename: &str) -> Option<Format> {
    Format::from_bytes(bytes).or_else(|| {
        Path::new(filename)
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("csv"))
            .then_some(Format::Csv)
    })
}

pub(crate) fn read_bounded_source(path: &Path) -> Result<Vec<u8>, CommandError> {
    let metadata = fs::metadata(path).map_err(CommandError::io)?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > MAX_ANYDOC_SOURCE_BYTES {
        return Err(CommandError::new(
            "invalid_document_size",
            "The document is empty or exceeds oStudy's 512 MB source limit.",
        ));
    }
    fs::read(path).map_err(CommandError::io)
}

fn map_anydoc_format(format: Format) -> Result<DocumentFormat, CommandError> {
    match format {
        Format::Doc => Ok(DocumentFormat::Doc),
        Format::Docx => Ok(DocumentFormat::Docx),
        Format::Odt => Ok(DocumentFormat::Odt),
        Format::Pdf => Ok(DocumentFormat::Pdf),
        Format::Ppt => Ok(DocumentFormat::Ppt),
        Format::Pptx => Ok(DocumentFormat::Pptx),
        Format::Rtf => Ok(DocumentFormat::Rtf),
        Format::Epub => Ok(DocumentFormat::Epub),
        Format::Excel => Ok(DocumentFormat::Excel),
        Format::Ods => Ok(DocumentFormat::Ods),
        Format::Odp => Ok(DocumentFormat::Odp),
        Format::Csv => Ok(DocumentFormat::Csv),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pdf_never_routes_through_anydoc() {
        assert_eq!(
            expected_format(DocumentFormat::Pdf).unwrap_err().code,
            "document_adapter_boundary_violation"
        );
    }

    #[test]
    fn csv_is_the_only_extension_authorized_format() {
        assert_eq!(
            detected_document_format(b"name,value\na,1\n", "table.csv").unwrap(),
            DocumentFormat::Csv
        );
        assert_eq!(
            detected_document_format(b"not an OOXML package", "renamed.docx")
                .unwrap_err()
                .code,
            "unsupported_document_format"
        );
        assert_eq!(
            detected_document_format(b"%PDF-1.7", "renamed.docx").unwrap(),
            DocumentFormat::Pdf
        );
    }
}
