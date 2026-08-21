//! Format-neutral document source and provenance primitives.
//!
//! File-format adapters translate their native structure into these ostudy-owned
//! concepts before semantic planning or AI generation. PDF pages are one source
//! unit kind; slides, flowing sections, chapters, images, and future timed media
//! segments use the same durable locator contract.

use serde::{Deserialize, Serialize};

use crate::error::CommandError;

const MAX_SECTION_LABEL_CHARS: usize = 500;
const MAX_ASSET_ID_CHARS: usize = 160;
const MAX_LOCATOR_ASSETS: usize = 256;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DocumentFormat {
    Pdf,
    Doc,
    Docx,
    Ppt,
    Pptx,
    Odt,
    Odp,
    Html,
    PlainText,
    Epub,
    ImageBundle,
}

impl DocumentFormat {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Pdf => "pdf",
            Self::Doc => "doc",
            Self::Docx => "docx",
            Self::Ppt => "ppt",
            Self::Pptx => "pptx",
            Self::Odt => "odt",
            Self::Odp => "odp",
            Self::Html => "html",
            Self::PlainText => "plain_text",
            Self::Epub => "epub",
            Self::ImageBundle => "image_bundle",
        }
    }

    pub(crate) fn from_db(value: &str) -> Result<Self, CommandError> {
        match value {
            "pdf" => Ok(Self::Pdf),
            "doc" => Ok(Self::Doc),
            "docx" => Ok(Self::Docx),
            "ppt" => Ok(Self::Ppt),
            "pptx" => Ok(Self::Pptx),
            "odt" => Ok(Self::Odt),
            "odp" => Ok(Self::Odp),
            "html" => Ok(Self::Html),
            "plain_text" | "text" => Ok(Self::PlainText),
            "epub" => Ok(Self::Epub),
            "image_bundle" | "image" => Ok(Self::ImageBundle),
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
    Chapter,
    Section,
    Segment,
    Image,
    TimeSegment,
}

impl SourceUnitKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Page => "page",
            Self::Slide => "slide",
            Self::Chapter => "chapter",
            Self::Section => "section",
            Self::Segment => "segment",
            Self::Image => "image",
            Self::TimeSegment => "time_segment",
        }
    }

    pub(crate) fn from_db(value: &str) -> Result<Self, CommandError> {
        match value {
            "page" => Ok(Self::Page),
            "slide" => Ok(Self::Slide),
            "chapter" => Ok(Self::Chapter),
            "section" | "block" => Ok(Self::Section),
            "segment" => Ok(Self::Segment),
            "image" => Ok(Self::Image),
            "time_segment" => Ok(Self::TimeSegment),
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
            Self::Chapter => "chapters",
            Self::Section => "sections",
            Self::Segment => "segments",
            Self::Image => "images",
            Self::TimeSegment => "time segments",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct TimeRangeMs {
    pub start: u64,
    pub end: u64,
}

impl TimeRangeMs {
    fn validate(self) -> Result<(), CommandError> {
        if self.end <= self.start {
            return Err(CommandError::new(
                "invalid_document_source_locator",
                "A timed document source locator must end after it starts.",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SourceLocator {
    pub unit_kind: SourceUnitKind,
    pub unit_numbers: Vec<i64>,
    pub section_label: Option<String>,
    #[serde(default)]
    pub asset_ids: Vec<String>,
    #[serde(default)]
    pub time_range_ms: Option<TimeRangeMs>,
}

impl SourceLocator {
    pub(crate) fn new(
        unit_kind: SourceUnitKind,
        unit_numbers: Vec<i64>,
        section_label: Option<String>,
    ) -> Result<Self, CommandError> {
        Self::with_evidence(unit_kind, unit_numbers, section_label, Vec::new(), None)
    }

    pub(crate) fn with_evidence(
        unit_kind: SourceUnitKind,
        mut unit_numbers: Vec<i64>,
        section_label: Option<String>,
        mut asset_ids: Vec<String>,
        time_range_ms: Option<TimeRangeMs>,
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
            .map(|value| bounded_text(&value, MAX_SECTION_LABEL_CHARS))
            .filter(|value| !value.is_empty());
        if asset_ids.len() > MAX_LOCATOR_ASSETS {
            return Err(CommandError::new(
                "invalid_document_source_locator",
                "A document source locator references too many visual assets.",
            ));
        }
        for asset_id in &mut asset_ids {
            *asset_id = bounded_text(asset_id, MAX_ASSET_ID_CHARS);
            validate_asset_id(asset_id)?;
        }
        asset_ids.sort();
        asset_ids.dedup();
        if let Some(range) = time_range_ms {
            range.validate()?;
        }
        Ok(Self {
            unit_kind,
            unit_numbers,
            section_label,
            asset_ids,
            time_range_ms,
        })
    }

    pub(crate) fn pages(
        page_numbers: Vec<i64>,
        section_label: Option<String>,
    ) -> Result<Self, CommandError> {
        Self::new(SourceUnitKind::Page, page_numbers, section_label)
    }

    pub(crate) fn first(&self) -> i64 {
        self.unit_numbers.first().copied().unwrap_or(1)
    }

    pub(crate) fn last(&self) -> i64 {
        self.unit_numbers
            .last()
            .copied()
            .unwrap_or_else(|| self.first())
    }

    pub(crate) fn validate_within(&self, source_unit_count: usize) -> Result<(), CommandError> {
        if source_unit_count == 0 {
            return Err(CommandError::new(
                "invalid_document_source",
                "A document source must contain at least one source unit.",
            ));
        }
        if self.unit_numbers.is_empty() || self.unit_numbers.iter().any(|number| *number < 1) {
            return Err(CommandError::new(
                "invalid_document_source_locator",
                "A document source locator has invalid source-unit numbers.",
            ));
        }
        let maximum = i64::try_from(source_unit_count).unwrap_or(i64::MAX);
        if self.unit_numbers.iter().any(|number| *number > maximum) {
            return Err(CommandError::new(
                "invalid_document_source_locator",
                "A semantic source referenced a unit outside the document.",
            ));
        }
        if let Some(range) = self.time_range_ms {
            range.validate()?;
        }
        if self.asset_ids.len() > MAX_LOCATOR_ASSETS {
            return Err(CommandError::new(
                "invalid_document_source_locator",
                "A document source locator references too many visual assets.",
            ));
        }
        for asset_id in &self.asset_ids {
            validate_asset_id(asset_id)?;
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
        let time = self.time_range_ms.map_or_else(
            || "none".to_string(),
            |range| format!("{}-{}", range.start, range.end),
        );
        format!(
            "{}:{numbers}|time={time}|assets={}",
            self.unit_kind.as_str(),
            self.asset_ids.join(",")
        )
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
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

pub(crate) fn validate_asset_id(value: &str) -> Result<(), CommandError> {
    if value.is_empty()
        || value.chars().count() > MAX_ASSET_ID_CHARS
        || value.chars().any(|character| {
            !(character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | ':' | '/'))
        })
    {
        return Err(CommandError::new(
            "invalid_document_asset_identity",
            "A document asset has an invalid stable identity.",
        ));
    }
    Ok(())
}

fn bounded_text(value: &str, maximum: usize) -> String {
    value
        .chars()
        .filter(|character| !character.is_control() || matches!(character, '\n' | '\t'))
        .take(maximum)
        .collect::<String>()
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_locators_are_sorted_deduplicated_and_embedding_stable() {
        let locator = SourceLocator::with_evidence(
            SourceUnitKind::Page,
            vec![3, 1, 3, 2],
            Some(" Chapter 1 ".to_string()),
            vec![
                "unit-render:000003".to_string(),
                "unit-render:000001".to_string(),
            ],
            None,
        )
        .expect("locator should build");
        assert_eq!(locator.unit_numbers, vec![1, 2, 3]);
        assert_eq!(locator.section_label.as_deref(), Some("Chapter 1"));
        assert_eq!(
            locator.asset_ids,
            vec!["unit-render:000001", "unit-render:000003"]
        );
        assert_eq!(
            locator.stable_identity(),
            "page:1,2,3|time=none|assets=unit-render:000001,unit-render:000003"
        );
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
        assert_eq!(
            DocumentFormat::from_db("epub").unwrap(),
            DocumentFormat::Epub
        );
        assert_eq!(
            SourceUnitKind::from_db("block").unwrap(),
            SourceUnitKind::Section
        );
    }

    #[test]
    fn future_timed_media_provenance_is_bounded_now() {
        let locator = SourceLocator::with_evidence(
            SourceUnitKind::TimeSegment,
            vec![4],
            Some("Speaker 2".to_string()),
            Vec::new(),
            Some(TimeRangeMs {
                start: 10_000,
                end: 17_500,
            }),
        )
        .unwrap();
        assert!(locator.stable_identity().contains("10000-17500"));
        assert!(SourceLocator::with_evidence(
            SourceUnitKind::TimeSegment,
            vec![1],
            None,
            Vec::new(),
            Some(TimeRangeMs { start: 5, end: 5 }),
        )
        .is_err());
    }
}
