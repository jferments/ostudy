//! Format-neutral document source and provenance primitives.
//!
//! File-format adapters translate their native structure into these ostudy-owned
//! concepts before semantic planning or AI generation. PDF pages are one source
//! unit kind; future slide and flowing-document adapters can use the same
//! analysis pipeline without pretending every source has stable physical pages.

use serde::{Deserialize, Serialize};

use crate::error::CommandError;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DocumentFormat {
    Pdf,
    Docx,
    Pptx,
}

impl DocumentFormat {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Pdf => "pdf",
            Self::Docx => "docx",
            Self::Pptx => "pptx",
        }
    }

    pub(crate) fn from_db(value: &str) -> Result<Self, CommandError> {
        match value {
            "pdf" => Ok(Self::Pdf),
            "docx" => Ok(Self::Docx),
            "pptx" => Ok(Self::Pptx),
            _ => Err(CommandError::new(
                "invalid_document_format",
                "The saved document format is not supported by this ostudy build.",
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SourceUnitKind {
    Page,
    Slide,
    Block,
}

impl SourceUnitKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Page => "page",
            Self::Slide => "slide",
            Self::Block => "block",
        }
    }

    pub(crate) fn from_db(value: &str) -> Result<Self, CommandError> {
        match value {
            "page" => Ok(Self::Page),
            "slide" => Ok(Self::Slide),
            "block" => Ok(Self::Block),
            _ => Err(CommandError::new(
                "invalid_document_source_unit",
                "The saved document source-unit kind is not supported by this ostudy build.",
            )),
        }
    }

    pub(crate) fn plural_label(self) -> &'static str {
        match self {
            Self::Page => "pages",
            Self::Slide => "slides",
            Self::Block => "blocks",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SourceLocator {
    pub unit_kind: SourceUnitKind,
    pub unit_numbers: Vec<i64>,
    pub section_label: Option<String>,
}

impl SourceLocator {
    pub(crate) fn new(
        unit_kind: SourceUnitKind,
        mut unit_numbers: Vec<i64>,
        section_label: Option<String>,
    ) -> Result<Self, CommandError> {
        unit_numbers.sort_unstable();
        unit_numbers.dedup();
        if unit_numbers.is_empty() || unit_numbers.iter().any(|number| *number < 1) {
            return Err(CommandError::new(
                "invalid_document_source_locator",
                "A document source locator must contain at least one positive source-unit number.",
            ));
        }
        let section_label = section_label
            .map(|value| {
                value
                    .chars()
                    .filter(|character| !character.is_control())
                    .take(500)
                    .collect::<String>()
                    .trim()
                    .to_string()
            })
            .filter(|value| !value.is_empty());
        Ok(Self {
            unit_kind,
            unit_numbers,
            section_label,
        })
    }

    pub(crate) fn pages(page_numbers: Vec<i64>, section_label: Option<String>) -> Result<Self, CommandError> {
        Self::new(SourceUnitKind::Page, page_numbers, section_label)
    }

    pub(crate) fn first(&self) -> i64 {
        self.unit_numbers.first().copied().unwrap_or(1)
    }

    pub(crate) fn last(&self) -> i64 {
        self.unit_numbers.last().copied().unwrap_or_else(|| self.first())
    }

    pub(crate) fn validate_within(&self, source_unit_count: usize) -> Result<(), CommandError> {
        if source_unit_count == 0 {
            return Err(CommandError::new(
                "invalid_document_source",
                "A document source must contain at least one source unit.",
            ));
        }
        let maximum = i64::try_from(source_unit_count).unwrap_or(i64::MAX);
        if self.unit_numbers.iter().any(|number| *number > maximum) {
            return Err(CommandError::new(
                "invalid_document_source_locator",
                "A semantic window referenced a source unit outside the document.",
            ));
        }
        Ok(())
    }

    pub(crate) fn stable_identity(&self) -> String {
        let numbers = self
            .unit_numbers
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",");
        format!("{}:{numbers}", self.unit_kind.as_str())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DocumentSourceDescriptor {
    pub format: DocumentFormat,
    pub unit_kind: SourceUnitKind,
    pub unit_count: usize,
}

impl DocumentSourceDescriptor {
    pub(crate) fn pdf(page_count: usize) -> Self {
        Self {
            format: DocumentFormat::Pdf,
            unit_kind: SourceUnitKind::Page,
            unit_count: page_count,
        }
    }

    pub(crate) fn validate(&self) -> Result<(), CommandError> {
        if self.unit_count == 0 {
            return Err(CommandError::new(
                "document_has_no_source_units",
                "The document did not contain any source units that ostudy could process.",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_locators_are_sorted_deduplicated_and_bounded() {
        let locator = SourceLocator::pages(vec![3, 1, 3, 2], Some(" Chapter 1 ".to_string()))
            .expect("locator should build");
        assert_eq!(locator.unit_numbers, vec![1, 2, 3]);
        assert_eq!(locator.section_label.as_deref(), Some("Chapter 1"));
        assert_eq!(locator.stable_identity(), "page:1,2,3");
        locator.validate_within(3).expect("locator should fit");
        assert_eq!(
            locator.validate_within(2).unwrap_err().code,
            "invalid_document_source_locator"
        );
    }

    #[test]
    fn future_formats_share_the_same_source_descriptor_contract() {
        let pdf = DocumentSourceDescriptor::pdf(11);
        assert_eq!(pdf.format.as_str(), "pdf");
        assert_eq!(pdf.unit_kind.plural_label(), "pages");
        assert_eq!(DocumentFormat::from_db("docx").unwrap(), DocumentFormat::Docx);
        assert_eq!(SourceUnitKind::from_db("slide").unwrap(), SourceUnitKind::Slide);
    }
}
