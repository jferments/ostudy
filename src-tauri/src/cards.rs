//! Stable card-presentation and canonical rich-document boundary.
//!
//! The implementation is private so callers depend on this narrow facade rather
//! than on editor-specific validation details.

#[path = "cards/content.rs"]
mod content;

pub use content::CardPresentationDto;
pub(crate) use content::{
    collect_asset_ids, validate_content, validate_document, ValidatedContent,
};

#[cfg(test)]
impl std::fmt::Debug for ValidatedContent {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ValidatedContent")
            .field("front_doc_bytes", &self.front_doc_json.len())
            .field("back_doc_bytes", &self.back_doc_json.len())
            .field("front_text_chars", &self.front_text.chars().count())
            .field("back_text_chars", &self.back_text.chars().count())
            .field("front_asset_count", &self.front_asset_ids.len())
            .field("back_asset_count", &self.back_asset_ids.len())
            .finish()
    }
}
