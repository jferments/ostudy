use crate::{
    error::CommandError,
    pdf_backend::{self, ExtractedDocument},
    task_cancellation,
};

pub(crate) const MAX_PAGE_TEXT_CHARS: usize = 200_000;
pub(crate) const MAX_TOTAL_TEXT_BYTES: usize = 96 * 1024 * 1024;
pub(crate) const MAX_SEMANTIC_TEXT_BYTES: usize = 128 * 1024 * 1024;

pub(crate) fn ensure_document_not_canceled(document_id: &str) -> Result<(), CommandError> {
    if task_cancellation::is_document_task_canceled(document_id) {
        return Err(CommandError::new(
            "ai_request_canceled",
            "The document generation request was canceled.",
        ));
    }
    Ok(())
}

pub(crate) fn validate_pdf_semantic_provenance(
    semantic: &ExtractedDocument,
) -> Result<(), CommandError> {
    for page in &semantic.pages {
        if page.page_number == 0 || page.page_number > semantic.page_count {
            return Err(CommandError::new(
                "document_extraction_provenance_invalid",
                "The PDF extraction adapter returned a text page outside the document's page range.",
            ));
        }
    }
    for element in &semantic.elements {
        if element.page_number == 0 || element.page_number > semantic.page_count {
            return Err(CommandError::new(
                "document_extraction_provenance_invalid",
                "The PDF extraction adapter returned a semantic element outside the document's page range.",
            ));
        }
    }
    for chunk in &semantic.chunks {
        if chunk.page_numbers.is_empty()
            || chunk
                .page_numbers
                .iter()
                .any(|unit| *unit == 0 || *unit > semantic.page_count)
        {
            return Err(CommandError::new(
                "document_extraction_provenance_invalid",
                "The PDF extraction adapter returned a semantic chunk with invalid page provenance.",
            ));
        }
    }
    Ok(())
}

pub(crate) fn validate_recovered_text_quality(
    semantic: &ExtractedDocument,
) -> Result<(), CommandError> {
    for chunk in &semantic.chunks {
        if pdf_backend::assess_text_quality(&chunk.full_text).suspicious {
            return Err(CommandError::new(
                "document_text_layer_unreliable",
                format!(
                    "The PDF still contains an unreliable text mapping around source page {} after native/OCR recovery. ostudy stopped before sending corrupted text to the AI provider.",
                    chunk.page_numbers.first().copied().unwrap_or_default()
                ),
            ));
        }
    }
    Ok(())
}

pub(crate) fn validate_text_limits(semantic: &ExtractedDocument) -> Result<(), CommandError> {
    let total_text_bytes = semantic
        .pages
        .iter()
        .map(|page| normalize_page_text(&page.text).len())
        .fold(0_usize, usize::saturating_add);
    if total_text_bytes > MAX_TOTAL_TEXT_BYTES {
        return Err(CommandError::new(
            "document_text_too_large",
            "The extracted document text exceeds ostudy's 96 MB processing limit.",
        ));
    }
    let semantic_text_bytes = semantic
        .chunks
        .iter()
        .map(|chunk| chunk.full_text.len())
        .fold(0_usize, usize::saturating_add);
    if semantic_text_bytes > MAX_SEMANTIC_TEXT_BYTES {
        return Err(CommandError::new(
            "document_semantic_content_too_large",
            "The structured document content exceeds ostudy's semantic-processing limit.",
        ));
    }
    Ok(())
}

pub(crate) fn normalize_page_text(value: &str) -> String {
    normalize_semantic_text(
        &value.replace("\r\n", "\n").replace('\r', "\n"),
        MAX_PAGE_TEXT_CHARS,
    )
}

fn normalize_semantic_text(value: &str, maximum: usize) -> String {
    value
        .chars()
        .filter(|character| !is_unmapped_pdf_text_character(*character))
        .take(maximum)
        .collect::<String>()
        .trim()
        .to_string()
}

fn is_unmapped_pdf_text_character(character: char) -> bool {
    let value = character as u32;
    (character.is_control() && !matches!(character, '\n' | '\t'))
        || character == '\u{fffd}'
        || matches!(
            character as u32,
            0xE000..=0xF8FF | 0xF0000..=0xFFFFD | 0x100000..=0x10FFFD
        )
        || (0xFDD0..=0xFDEF).contains(&value)
        || value & 0xFFFF == 0xFFFE
        || value & 0xFFFF == 0xFFFF
}

pub(crate) fn bounded_diagnostic(value: &str, maximum: usize) -> String {
    value
        .chars()
        .filter(|character| !character.is_control() || *character == '\n')
        .take(maximum)
        .collect::<String>()
        .replace('\n', " ")
}

pub(crate) fn confidence_milli(value: f64) -> usize {
    if !value.is_finite() {
        return 0;
    }
    (value.clamp(0.0, 1.0) * 1_000.0).round() as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_page_text_without_control_characters() {
        assert_eq!(
            normalize_page_text(" line\r\none\0two\rthree "),
            "line\nonetwo\nthree"
        );
    }

    #[test]
    fn removes_unmapped_font_glyphs_without_discarding_native_text() {
        assert_eq!(
            normalize_page_text("\u{0007}e airway has 0.3 \u{001e}m text air\u{fffe}ways and \u{e001} one bad private glyph"),
            "e airway has 0.3 m text airways and  one bad private glyph"
        );
    }
}
