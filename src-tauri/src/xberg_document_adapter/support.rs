use std::collections::BTreeSet;

use crate::document_domain::DocumentFormat;

pub(crate) const MAX_BLOCK_TEXT_CHARS: usize = 250_000;
pub(crate) const MAX_HEADING_CHARS: usize = 500;
pub(crate) const MAX_ATTRIBUTE_VALUE_CHARS: usize = 2_000;

pub(crate) fn bounded_text(value: &str, maximum: usize) -> String {
    value
        .chars()
        .filter(|character| !character.is_control() || matches!(character, '\n' | '\t'))
        .take(maximum)
        .collect::<String>()
        .trim()
        .to_string()
}

pub(crate) fn capitalize(value: &str) -> String {
    let mut characters = value.chars();
    match characters.next() {
        Some(first) => first.to_uppercase().collect::<String>() + characters.as_str(),
        None => String::new(),
    }
}

pub(crate) fn deduplicate(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub(crate) fn media_type_for_format(format: DocumentFormat) -> &'static str {
    match format {
        DocumentFormat::Doc => "application/msword",
        DocumentFormat::Docx => {
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
        }
        DocumentFormat::Ppt => "application/vnd.ms-powerpoint",
        DocumentFormat::Pptx => {
            "application/vnd.openxmlformats-officedocument.presentationml.presentation"
        }
        DocumentFormat::Odt => "application/vnd.oasis.opendocument.text",
        DocumentFormat::Odp => "application/vnd.oasis.opendocument.presentation",
        DocumentFormat::Rtf => "application/rtf",
        DocumentFormat::Epub => "application/epub+zip",
        DocumentFormat::Excel => {
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
        }
        DocumentFormat::Ods => "application/vnd.oasis.opendocument.spreadsheet",
        DocumentFormat::Csv => "text/csv",
        _ => "application/octet-stream",
    }
}

pub(crate) fn image_media_type(format: &str) -> Option<&'static str> {
    match format.trim().to_ascii_lowercase().as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "svg" | "svg+xml" => Some("image/svg+xml"),
        "bmp" => Some("image/bmp"),
        "tif" | "tiff" => Some("image/tiff"),
        _ => None,
    }
}
