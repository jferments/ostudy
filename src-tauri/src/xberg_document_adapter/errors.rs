use crate::error::CommandError;

use super::support::bounded_text;

pub(crate) fn map_xberg_error(error: xberg::XbergError) -> CommandError {
    let detail = bounded_text(&error.to_string(), 500);
    match error {
        xberg::XbergError::Cancelled => CommandError::new(
            "ai_request_canceled",
            "The document generation request was canceled.",
        ),
        xberg::XbergError::Timeout { .. } => CommandError::new(
            "document_extraction_timed_out",
            "The document parser exceeded the local extraction time limit.",
        ),
        xberg::XbergError::Security { .. } => CommandError::new(
            "document_resource_limit",
            format!("The document exceeded a parser safety limit: {detail}"),
        ),
        xberg::XbergError::UnsupportedFormat(_) => CommandError::new(
            "unsupported_document_format",
            "Xberg does not support this document variant.",
        ),
        xberg::XbergError::MissingDependency(_) => CommandError::new(
            "document_backend_unavailable",
            "This oStudy build is missing a component required to process that document.",
        ),
        xberg::XbergError::Io(error) => CommandError::io(error),
        xberg::XbergError::Parsing { .. } if looks_encrypted(&detail) => CommandError::new(
            "document_encrypted",
            "Encrypted or password-protected documents cannot be processed.",
        ),
        xberg::XbergError::Parsing { .. } => CommandError::new(
            "document_malformed",
            format!("The document is malformed or missing required content: {detail}"),
        ),
        xberg::XbergError::Validation { .. } => CommandError::new(
            "invalid_document_source",
            format!("The document could not be validated safely: {detail}"),
        ),
        xberg::XbergError::ImageProcessing { .. } => CommandError::new(
            "document_asset_processing_failed",
            format!("An embedded document image could not be processed safely: {detail}"),
        ),
        xberg::XbergError::Ocr { .. }
        | xberg::XbergError::Cache { .. }
        | xberg::XbergError::Serialization { .. }
        | xberg::XbergError::Plugin { .. }
        | xberg::XbergError::LockPoisoned(_)
        | xberg::XbergError::Embedding { .. }
        | xberg::XbergError::Reranking { .. }
        | xberg::XbergError::Transcription { .. }
        | xberg::XbergError::Other(_) => CommandError::new(
            "document_parse_failed",
            format!("The document could not be parsed safely: {detail}"),
        ),
    }
}

fn looks_encrypted(detail: &str) -> bool {
    let detail = detail.to_ascii_lowercase();
    detail.contains("encrypted")
        || detail.contains("encryption")
        || detail.contains("password protected")
        || detail.contains("password-protected")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancellation_maps_to_the_existing_document_task_contract() {
        assert_eq!(
            map_xberg_error(xberg::XbergError::Cancelled).code,
            "ai_request_canceled"
        );
    }

    #[test]
    fn encrypted_parser_messages_get_a_stable_local_error() {
        let error = xberg::XbergError::parsing("package is encrypted");
        assert_eq!(map_xberg_error(error).code, "document_encrypted");
    }
}
