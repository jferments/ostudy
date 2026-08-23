use anydoc::ConvertError;

use crate::error::CommandError;

use super::support::bounded_text;

pub(crate) fn map_anydoc_error(error: ConvertError) -> CommandError {
    let detail = bounded_text(&error.to_string(), 500);
    match error {
        ConvertError::Unsupported(_) => CommandError::new(
            "unsupported_document_format",
            "AnyDoc does not support this document variant.",
        ),
        ConvertError::Malformed { .. } | ConvertError::MissingPart { .. } => CommandError::new(
            "document_malformed",
            format!("The document is malformed or missing required content: {detail}"),
        ),
        ConvertError::Encrypted => CommandError::new(
            "document_encrypted",
            "Encrypted or password-protected documents cannot be processed.",
        ),
        ConvertError::ResourceLimit { .. } => CommandError::new(
            "document_resource_limit",
            format!("The document exceeded a parser safety limit: {detail}"),
        ),
        ConvertError::Io(error) => CommandError::io(error),
        _ => CommandError::new(
            "document_parse_failed",
            format!("The document could not be parsed safely: {detail}"),
        ),
    }
}
