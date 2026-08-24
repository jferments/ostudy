use crate::{document_domain::DocumentFormat, error::CommandError, xberg_document_adapter};

pub(crate) fn detected_document_format(
    bytes: &[u8],
    filename: &str,
) -> Result<DocumentFormat, CommandError> {
    if bytes.starts_with(b"%PDF-") {
        return Ok(DocumentFormat::Pdf);
    }
    xberg_document_adapter::detected_document_format(bytes, filename)
}
