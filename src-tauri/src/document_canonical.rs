//! App-owned canonical document representation.
//!
//! Format adapters may use any bounded parser internally, but only these types
//! cross the adapter boundary. Canonical records preserve immutable source
//! provenance, deterministic text identities, and zero-to-many visual assets.
//! Semantic embeddings and model-generated visual descriptions attach to these
//! stable identities as rebuildable derivatives; they never replace source data.

use std::{
    collections::{BTreeMap, HashSet},
    path::{Component, Path},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    document_domain::{
        validate_asset_id, DocumentSourceDescriptor, SourceLocator, SourceUnitKind,
    },
    error::CommandError,
};

pub(crate) const CANONICAL_DOCUMENT_SCHEMA_VERSION: i64 = 1;

const MAX_SOURCE_FILES: usize = 4_096;
const MAX_SOURCE_UNITS: usize = 100_000;
const MAX_BLOCKS: usize = 1_000_000;
const MAX_ASSETS: usize = 200_000;
const MAX_CHUNKS: usize = 200_000;
const MAX_FILE_NAME_CHARS: usize = 255;
const MAX_MEDIA_TYPE_CHARS: usize = 160;
const MAX_RELATIVE_PATH_CHARS: usize = 1_024;
const MAX_UNIT_LABEL_CHARS: usize = 500;
const MAX_UNIT_TITLE_CHARS: usize = 1_000;
const MAX_UNIT_TEXT_CHARS: usize = 250_000;
const MAX_BLOCK_TEXT_CHARS: usize = 250_000;
const MAX_HEADING_ITEMS: usize = 64;
const MAX_HEADING_ITEM_CHARS: usize = 500;
const MAX_BLOCK_ATTRIBUTES: usize = 64;
const MAX_ATTRIBUTE_KEY_CHARS: usize = 120;
const MAX_ATTRIBUTE_VALUE_CHARS: usize = 2_000;
const MAX_CHUNK_TEXT_CHARS: usize = 1_000_000;
const MAX_CHUNK_FULL_TEXT_CHARS: usize = 1_100_000;
const MAX_ELEMENT_TYPES: usize = 128;
const MAX_ELEMENT_TYPE_CHARS: usize = 80;
const MAX_METADATA_ITEMS: usize = 64;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CanonicalBlockRole {
    Title,
    Heading,
    Paragraph,
    ListItem,
    Table,
    CodeBlock,
    Quote,
    Equation,
    Caption,
    Header,
    Footer,
    SpeakerNote,
    Footnote,
    Endnote,
    KeyValue,
    AuthorLearningObjective,
    Image,
    ImageDescription,
}

impl CanonicalBlockRole {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Title => "title",
            Self::Heading => "heading",
            Self::Paragraph => "paragraph",
            Self::ListItem => "list_item",
            Self::Table => "table",
            Self::CodeBlock => "code_block",
            Self::Quote => "quote",
            Self::Equation => "equation",
            Self::Caption => "caption",
            Self::Header => "header",
            Self::Footer => "footer",
            Self::SpeakerNote => "speaker_note",
            Self::Footnote => "footnote",
            Self::Endnote => "endnote",
            Self::KeyValue => "key_value",
            Self::AuthorLearningObjective => "author_learning_objective",
            Self::Image => "image",
            Self::ImageDescription => "image_description",
        }
    }

    pub(crate) fn from_adapter(value: &str) -> Result<Self, CommandError> {
        match value.to_ascii_lowercase().replace('-', "_").as_str() {
            "title" => Ok(Self::Title),
            "heading" => Ok(Self::Heading),
            "paragraph" => Ok(Self::Paragraph),
            "listitem" | "list_item" => Ok(Self::ListItem),
            "table" => Ok(Self::Table),
            "codeblock" | "code_block" => Ok(Self::CodeBlock),
            "quote" | "blockquote" | "block_quote" => Ok(Self::Quote),
            "equation" | "math" => Ok(Self::Equation),
            "caption" => Ok(Self::Caption),
            "header" => Ok(Self::Header),
            "footer" => Ok(Self::Footer),
            "speakernote" | "speaker_note" => Ok(Self::SpeakerNote),
            "footnote" => Ok(Self::Footnote),
            "endnote" => Ok(Self::Endnote),
            "keyvalue" | "key_value" => Ok(Self::KeyValue),
            "authorlearningobjective" | "author_learning_objective" => {
                Ok(Self::AuthorLearningObjective)
            }
            "image" => Ok(Self::Image),
            "imagedescription" | "image_description" => Ok(Self::ImageDescription),
            _ => Err(CommandError::new(
                "document_semantic_element_unsupported",
                "The extraction adapter returned an unsupported semantic block role.",
            )),
        }
    }

    pub(crate) fn benefits_from_visual_evidence(self) -> bool {
        matches!(
            self,
            Self::Table
                | Self::Equation
                | Self::KeyValue
                | Self::Image
                | Self::ImageDescription
        )
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CanonicalAssetRole {
    UnitRender,
    OriginalNoteImage,
    EmbeddedImage,
    Diagram,
    Chart,
    EquationImage,
    CoverImage,
    Thumbnail,
}

impl CanonicalAssetRole {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::UnitRender => "unit_render",
            Self::OriginalNoteImage => "original_note_image",
            Self::EmbeddedImage => "embedded_image",
            Self::Diagram => "diagram",
            Self::Chart => "chart",
            Self::EquationImage => "equation_image",
            Self::CoverImage => "cover_image",
            Self::Thumbnail => "thumbnail",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct NormalizedBoundingBox {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl NormalizedBoundingBox {
    pub(crate) fn new(x: f64, y: f64, width: f64, height: f64) -> Option<Self> {
        let value = Self {
            x,
            y,
            width,
            height,
        };
        value.validate().then_some(value)
    }

    fn validate(self) -> bool {
        [self.x, self.y, self.width, self.height]
            .iter()
            .all(|value| value.is_finite())
            && self.width >= 0.0
            && self.height >= 0.0
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CanonicalSourceFile {
    pub file_ordinal: i64,
    pub original_filename: String,
    pub media_type: String,
    pub relative_path: String,
    pub byte_size: i64,
    pub sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CanonicalSourceUnit {
    pub unit_number: i64,
    pub unit_kind: SourceUnitKind,
    pub label: String,
    pub title: Option<String>,
    pub plain_text: String,
    pub text_sha256: String,
    pub source_file_ordinal: i64,
}

impl CanonicalSourceUnit {
    pub(crate) fn new(
        unit_number: i64,
        unit_kind: SourceUnitKind,
        label: impl Into<String>,
        title: Option<String>,
        plain_text: impl Into<String>,
        source_file_ordinal: i64,
    ) -> Self {
        let plain_text = plain_text.into();
        Self {
            unit_number,
            unit_kind,
            label: label.into(),
            title,
            text_sha256: sha256_hex(plain_text.as_bytes()),
            plain_text,
            source_file_ordinal,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CanonicalBlock {
    pub block_index: i64,
    pub block_ordinal: i64,
    pub source_unit_number: i64,
    pub role: CanonicalBlockRole,
    pub text: String,
    pub parent_heading: Option<String>,
    pub heading_path: Vec<String>,
    pub geometry: Option<NormalizedBoundingBox>,
    pub confidence: Option<f64>,
    pub asset_ids: Vec<String>,
    pub attributes: BTreeMap<String, String>,
    pub content_sha256: String,
}

impl CanonicalBlock {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        block_index: i64,
        block_ordinal: i64,
        source_unit_number: i64,
        role: CanonicalBlockRole,
        text: impl Into<String>,
        parent_heading: Option<String>,
        heading_path: Vec<String>,
        geometry: Option<NormalizedBoundingBox>,
        confidence: Option<f64>,
        asset_ids: Vec<String>,
        attributes: BTreeMap<String, String>,
    ) -> Self {
        let text = text.into();
        let content_sha256 = block_content_sha256(
            source_unit_number,
            block_ordinal,
            role,
            &text,
            &heading_path,
            &asset_ids,
        );
        Self {
            block_index,
            block_ordinal,
            source_unit_number,
            role,
            text,
            parent_heading,
            heading_path,
            geometry,
            confidence,
            asset_ids,
            attributes,
            content_sha256,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CanonicalAsset {
    pub asset_id: String,
    pub source_unit_number: Option<i64>,
    pub block_index: Option<i64>,
    pub role: CanonicalAssetRole,
    pub media_type: String,
    pub relative_path: String,
    pub byte_size: i64,
    pub sha256: String,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub alt_text: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct DocumentMetadata {
    pub title: Option<String>,
    pub authors: Vec<String>,
    pub language: Option<String>,
    pub subject: Option<String>,
    pub attributes: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CanonicalExtractionReport {
    pub adapter_backend: String,
    pub extraction_backend: String,
    pub render_backend: Option<String>,
    pub extraction_mode: String,
    pub semantic_analysis_degraded: bool,
    pub ocr_unit_count: usize,
    pub ocr_mean_confidence: Option<f64>,
    pub ocr_engine: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CanonicalSemanticChunk {
    pub chunk_index: i64,
    pub text: String,
    pub full_text: String,
    pub source_locator: SourceLocator,
    pub element_types: Vec<String>,
    pub heading_context: Option<String>,
    pub token_estimate: usize,
    pub is_oversized: bool,
    pub content_sha256: String,
}

impl CanonicalSemanticChunk {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        chunk_index: i64,
        text: impl Into<String>,
        full_text: impl Into<String>,
        source_locator: SourceLocator,
        element_types: Vec<String>,
        heading_context: Option<String>,
        token_estimate: usize,
        is_oversized: bool,
    ) -> Self {
        let text = text.into();
        let full_text = full_text.into();
        let content_sha256 = semantic_chunk_content_sha256(
            &source_locator,
            &full_text,
            &element_types,
            heading_context.as_deref(),
        );
        Self {
            chunk_index,
            text,
            full_text,
            source_locator,
            element_types,
            heading_context,
            token_estimate,
            is_oversized,
            content_sha256,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CanonicalDocument {
    pub schema_version: i64,
    pub source: DocumentSourceDescriptor,
    pub source_files: Vec<CanonicalSourceFile>,
    pub metadata: DocumentMetadata,
    pub units: Vec<CanonicalSourceUnit>,
    pub blocks: Vec<CanonicalBlock>,
    pub assets: Vec<CanonicalAsset>,
    pub extraction: CanonicalExtractionReport,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CanonicalIngestion {
    pub document: CanonicalDocument,
    pub semantic_chunks: Vec<CanonicalSemanticChunk>,
}

impl CanonicalIngestion {
    pub(crate) fn validate(&self) -> Result<(), CommandError> {
        self.document.validate()?;
        if self.semantic_chunks.len() > MAX_CHUNKS {
            return Err(invalid_canonical(
                "The canonical document contains too many semantic chunks.",
            ));
        }
        let asset_ids = self
            .document
            .assets
            .iter()
            .map(|asset| asset.asset_id.as_str())
            .collect::<HashSet<_>>();
        for (expected, chunk) in self.semantic_chunks.iter().enumerate() {
            if chunk.chunk_index != i64::try_from(expected).unwrap_or(i64::MAX) {
                return Err(invalid_canonical(
                    "Canonical semantic chunk identities must be contiguous and zero-based.",
                ));
            }
            validate_text(&chunk.text, MAX_CHUNK_TEXT_CHARS, false)?;
            validate_text(&chunk.full_text, MAX_CHUNK_FULL_TEXT_CHARS, false)?;
            chunk
                .source_locator
                .validate_within(self.document.source.unit_count)?;
            if chunk.source_locator.unit_kind != self.document.source.unit_kind {
                return Err(invalid_canonical(
                    "A canonical semantic chunk uses the wrong source-unit kind.",
                ));
            }
            if chunk
                .source_locator
                .asset_ids
                .iter()
                .any(|asset_id| !asset_ids.contains(asset_id.as_str()))
            {
                return Err(invalid_canonical(
                    "A canonical semantic chunk references an unknown visual asset.",
                ));
            }
            if chunk.element_types.len() > MAX_ELEMENT_TYPES
                || chunk.element_types.iter().any(|value| {
                    value.is_empty()
                        || value.chars().count() > MAX_ELEMENT_TYPE_CHARS
                        || value.chars().any(char::is_control)
                })
            {
                return Err(invalid_canonical(
                    "A canonical semantic chunk contains invalid element-type metadata.",
                ));
            }
            let expected_hash = semantic_chunk_content_sha256(
                &chunk.source_locator,
                &chunk.full_text,
                &chunk.element_types,
                chunk.heading_context.as_deref(),
            );
            validate_sha256(&chunk.content_sha256)?;
            if chunk.content_sha256 != expected_hash {
                return Err(invalid_canonical(
                    "A canonical semantic chunk does not match its stable content hash.",
                ));
            }
        }
        Ok(())
    }
}

impl CanonicalDocument {
    pub(crate) fn validate(&self) -> Result<(), CommandError> {
        if self.schema_version != CANONICAL_DOCUMENT_SCHEMA_VERSION {
            return Err(invalid_canonical(
                "The canonical document uses an unsupported schema version.",
            ));
        }
        self.source.validate()?;
        if self.source_files.is_empty() || self.source_files.len() > MAX_SOURCE_FILES {
            return Err(invalid_canonical(
                "A canonical document must contain a bounded source-file bundle.",
            ));
        }
        if self.units.len() != self.source.unit_count || self.units.len() > MAX_SOURCE_UNITS {
            return Err(invalid_canonical(
                "Canonical source-unit count does not match the document descriptor.",
            ));
        }
        if self.blocks.len() > MAX_BLOCKS || self.assets.len() > MAX_ASSETS {
            return Err(invalid_canonical(
                "The canonical document exceeds the bounded block or asset limit.",
            ));
        }

        let mut source_ordinals = HashSet::new();
        let mut source_paths = HashSet::new();
        for (index, file) in self.source_files.iter().enumerate() {
            let expected = i64::try_from(index + 1).unwrap_or(i64::MAX);
            if file.file_ordinal != expected || !source_ordinals.insert(file.file_ordinal) {
                return Err(invalid_canonical(
                    "Canonical source-file ordinals must be contiguous and one-based.",
                ));
            }
            validate_text(&file.original_filename, MAX_FILE_NAME_CHARS, false)?;
            validate_media_type(&file.media_type)?;
            validate_relative_path(&file.relative_path)?;
            if !source_paths.insert(file.relative_path.as_str()) || file.byte_size <= 0 {
                return Err(invalid_canonical(
                    "Canonical source files must have unique paths and positive sizes.",
                ));
            }
            validate_sha256(&file.sha256)?;
        }

        for (index, unit) in self.units.iter().enumerate() {
            let expected = i64::try_from(index + 1).unwrap_or(i64::MAX);
            if unit.unit_number != expected || unit.unit_kind != self.source.unit_kind {
                return Err(invalid_canonical(
                    "Canonical source units must be contiguous and use the descriptor unit kind.",
                ));
            }
            if !source_ordinals.contains(&unit.source_file_ordinal) {
                return Err(invalid_canonical(
                    "A canonical source unit references an unknown source file.",
                ));
            }
            validate_text(&unit.label, MAX_UNIT_LABEL_CHARS, false)?;
            if let Some(title) = &unit.title {
                validate_text(title, MAX_UNIT_TITLE_CHARS, false)?;
            }
            validate_text(&unit.plain_text, MAX_UNIT_TEXT_CHARS, true)?;
            validate_sha256(&unit.text_sha256)?;
            if unit.text_sha256 != sha256_hex(unit.plain_text.as_bytes()) {
                return Err(invalid_canonical(
                    "A canonical source unit does not match its text hash.",
                ));
            }
        }

        let asset_ids = self.validate_assets()?;
        for (expected, block) in self.blocks.iter().enumerate() {
            if block.block_index != i64::try_from(expected).unwrap_or(i64::MAX)
                || block.block_ordinal < 0
                || block.source_unit_number < 1
                || usize::try_from(block.source_unit_number)
                    .ok()
                    .is_none_or(|unit| unit > self.units.len())
            {
                return Err(invalid_canonical(
                    "Canonical block identities or source-unit provenance are invalid.",
                ));
            }
            validate_text(&block.text, MAX_BLOCK_TEXT_CHARS, true)?;
            validate_heading_path(&block.heading_path)?;
            if let Some(parent) = &block.parent_heading {
                validate_text(parent, MAX_HEADING_ITEM_CHARS, false)?;
            }
            if block
                .geometry
                .is_some_and(|geometry| !geometry.validate())
                || block
                    .confidence
                    .is_some_and(|value| !value.is_finite() || !(0.0..=1.0).contains(&value))
            {
                return Err(invalid_canonical(
                    "Canonical block geometry or confidence is invalid.",
                ));
            }
            if block
                .asset_ids
                .iter()
                .any(|asset_id| !asset_ids.contains(asset_id.as_str()))
            {
                return Err(invalid_canonical(
                    "A canonical block references an unknown asset.",
                ));
            }
            validate_attributes(&block.attributes)?;
            let expected_hash = block_content_sha256(
                block.source_unit_number,
                block.block_ordinal,
                block.role,
                &block.text,
                &block.heading_path,
                &block.asset_ids,
            );
            validate_sha256(&block.content_sha256)?;
            if block.content_sha256 != expected_hash {
                return Err(invalid_canonical(
                    "A canonical block does not match its stable content hash.",
                ));
            }
        }
        validate_metadata(&self.metadata)?;
        validate_extraction_report(&self.extraction)?;
        Ok(())
    }

    fn validate_assets(&self) -> Result<HashSet<&str>, CommandError> {
        let mut ids = HashSet::new();
        let mut paths = HashSet::new();
        let block_count = i64::try_from(self.blocks.len()).unwrap_or(i64::MAX);
        for asset in &self.assets {
            validate_asset_id(&asset.asset_id)?;
            if !ids.insert(asset.asset_id.as_str()) {
                return Err(invalid_canonical(
                    "Canonical document asset identities must be unique.",
                ));
            }
            if asset.source_unit_number.is_some_and(|unit| {
                unit < 1
                    || usize::try_from(unit)
                        .ok()
                        .is_none_or(|value| value > self.units.len())
            }) || asset
                .block_index
                .is_some_and(|block| block < 0 || block >= block_count)
            {
                return Err(invalid_canonical(
                    "A canonical asset has invalid source provenance.",
                ));
            }
            validate_media_type(&asset.media_type)?;
            validate_relative_path(&asset.relative_path)?;
            if !paths.insert(asset.relative_path.as_str()) || asset.byte_size <= 0 {
                return Err(invalid_canonical(
                    "Canonical assets must have unique paths and positive sizes.",
                ));
            }
            validate_sha256(&asset.sha256)?;
            if asset.width.is_some_and(|value| value <= 0)
                || asset.height.is_some_and(|value| value <= 0)
            {
                return Err(invalid_canonical(
                    "Canonical asset dimensions must be positive when present.",
                ));
            }
            if let Some(alt_text) = &asset.alt_text {
                validate_text(alt_text, 4_000, true)?;
            }
        }
        Ok(ids)
    }

    pub(crate) fn full_text_projection(&self) -> String {
        self.units
            .iter()
            .map(|unit| unit.plain_text.trim())
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n---\n\n")
    }

    pub(crate) fn full_text_sha256(&self) -> String {
        sha256_hex(self.full_text_projection().as_bytes())
    }
}

pub(crate) fn unit_render_asset_id(unit_number: i64) -> String {
    format!("unit-render:{unit_number:06}")
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn block_content_sha256(
    source_unit_number: i64,
    block_ordinal: i64,
    role: CanonicalBlockRole,
    text: &str,
    heading_path: &[String],
    asset_ids: &[String],
) -> String {
    sha256_hex(
        format!(
            "unit={source_unit_number}|ordinal={block_ordinal}|role={}|headings={}|assets={}|text={text}",
            role.as_str(),
            heading_path.join(" > "),
            asset_ids.join(",")
        )
        .as_bytes(),
    )
}

fn semantic_chunk_content_sha256(
    locator: &SourceLocator,
    full_text: &str,
    element_types: &[String],
    heading_context: Option<&str>,
) -> String {
    sha256_hex(
        format!(
            "locator={}|types={}|heading={}|text={full_text}",
            locator.stable_identity(),
            element_types.join(","),
            heading_context.unwrap_or_default()
        )
        .as_bytes(),
    )
}

fn validate_metadata(metadata: &DocumentMetadata) -> Result<(), CommandError> {
    if let Some(title) = &metadata.title {
        validate_text(title, 1_000, false)?;
    }
    if metadata.authors.len() > MAX_METADATA_ITEMS {
        return Err(invalid_canonical(
            "Canonical document metadata contains too many authors.",
        ));
    }
    for author in &metadata.authors {
        validate_text(author, 500, false)?;
    }
    if let Some(language) = &metadata.language {
        validate_text(language, 80, false)?;
    }
    if let Some(subject) = &metadata.subject {
        validate_text(subject, 2_000, true)?;
    }
    validate_attributes(&metadata.attributes)
}

fn validate_extraction_report(report: &CanonicalExtractionReport) -> Result<(), CommandError> {
    validate_text(&report.adapter_backend, 160, false)?;
    validate_text(&report.extraction_backend, 160, false)?;
    if let Some(render_backend) = &report.render_backend {
        validate_text(render_backend, 160, false)?;
    }
    validate_text(&report.extraction_mode, 80, false)?;
    if report
        .ocr_mean_confidence
        .is_some_and(|value| !value.is_finite() || !(0.0..=1.0).contains(&value))
    {
        return Err(invalid_canonical(
            "Canonical OCR confidence must be between zero and one.",
        ));
    }
    if let Some(engine) = &report.ocr_engine {
        validate_text(engine, 160, false)?;
    }
    Ok(())
}

fn validate_heading_path(values: &[String]) -> Result<(), CommandError> {
    if values.len() > MAX_HEADING_ITEMS {
        return Err(invalid_canonical(
            "A canonical block contains too many heading ancestors.",
        ));
    }
    for value in values {
        validate_text(value, MAX_HEADING_ITEM_CHARS, false)?;
    }
    Ok(())
}

fn validate_attributes(values: &BTreeMap<String, String>) -> Result<(), CommandError> {
    if values.len() > MAX_BLOCK_ATTRIBUTES {
        return Err(invalid_canonical(
            "Canonical metadata contains too many adapter attributes.",
        ));
    }
    for (key, value) in values {
        validate_text(key, MAX_ATTRIBUTE_KEY_CHARS, false)?;
        validate_text(value, MAX_ATTRIBUTE_VALUE_CHARS, true)?;
    }
    Ok(())
}

fn validate_media_type(value: &str) -> Result<(), CommandError> {
    if value.is_empty()
        || value.chars().count() > MAX_MEDIA_TYPE_CHARS
        || value.chars().any(char::is_control)
        || !value.contains('/')
    {
        return Err(invalid_canonical(
            "A canonical source or asset has an invalid media type.",
        ));
    }
    Ok(())
}

fn validate_relative_path(value: &str) -> Result<(), CommandError> {
    if value.is_empty()
        || value.chars().count() > MAX_RELATIVE_PATH_CHARS
        || Path::new(value).is_absolute()
        || Path::new(value).components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(invalid_canonical(
            "A canonical source or asset path is not a safe managed relative path.",
        ));
    }
    Ok(())
}

fn validate_sha256(value: &str) -> Result<(), CommandError> {
    if value.len() != 64 || value.chars().any(|character| !character.is_ascii_hexdigit()) {
        return Err(invalid_canonical(
            "A canonical record has an invalid SHA-256 identity.",
        ));
    }
    Ok(())
}

fn validate_text(value: &str, maximum: usize, allow_empty: bool) -> Result<(), CommandError> {
    if (!allow_empty && value.trim().is_empty())
        || value.chars().count() > maximum
        || value
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\n' | '\t'))
    {
        return Err(invalid_canonical(
            "A canonical document text field is empty, oversized, or contains invalid controls.",
        ));
    }
    Ok(())
}

fn invalid_canonical(message: &str) -> CommandError {
    CommandError::new("invalid_canonical_document", message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document_domain::{DocumentFormat, SourceUnitKind};

    fn fixture() -> CanonicalIngestion {
        let asset_id = unit_render_asset_id(1);
        let locator = SourceLocator::with_evidence(
            SourceUnitKind::Page,
            vec![1],
            Some("Introduction".to_string()),
            vec![asset_id.clone()],
            None,
        )
        .unwrap();
        let document = CanonicalDocument {
            schema_version: CANONICAL_DOCUMENT_SCHEMA_VERSION,
            source: DocumentSourceDescriptor::pdf(1),
            source_files: vec![CanonicalSourceFile {
                file_ordinal: 1,
                original_filename: "lesson.pdf".to_string(),
                media_type: "application/pdf".to_string(),
                relative_path: "documents/abc/source.pdf".to_string(),
                byte_size: 100,
                sha256: "a".repeat(64),
            }],
            metadata: DocumentMetadata {
                title: Some("Lesson".to_string()),
                ..Default::default()
            },
            units: vec![CanonicalSourceUnit::new(
                1,
                SourceUnitKind::Page,
                "Page 1",
                None,
                "Cardiac output is heart rate times stroke volume.",
                1,
            )],
            blocks: vec![CanonicalBlock::new(
                0,
                0,
                1,
                CanonicalBlockRole::Equation,
                "CO = HR × SV",
                None,
                Vec::new(),
                NormalizedBoundingBox::new(0.1, 0.2, 0.5, 0.1),
                Some(0.98),
                vec![asset_id.clone()],
                BTreeMap::new(),
            )],
            assets: vec![CanonicalAsset {
                asset_id: asset_id.clone(),
                source_unit_number: Some(1),
                block_index: None,
                role: CanonicalAssetRole::UnitRender,
                media_type: "image/png".to_string(),
                relative_path: "documents/abc/pages/page-000001.png".to_string(),
                byte_size: 80,
                sha256: "b".repeat(64),
                width: Some(1_600),
                height: Some(2_000),
                alt_text: None,
            }],
            extraction: CanonicalExtractionReport {
                adapter_backend: "pdf-adapter-v1".to_string(),
                extraction_backend: "oxidize-pdf".to_string(),
                render_backend: Some("pdfium".to_string()),
                extraction_mode: "native".to_string(),
                semantic_analysis_degraded: false,
                ocr_unit_count: 0,
                ocr_mean_confidence: None,
                ocr_engine: None,
            },
        };
        CanonicalIngestion {
            document,
            semantic_chunks: vec![CanonicalSemanticChunk::new(
                0,
                "Cardiac output",
                "Cardiac output is heart rate times stroke volume.",
                locator,
                vec!["equation".to_string()],
                Some("Hemodynamics".to_string()),
                20,
                false,
            )],
        }
    }

    #[test]
    fn canonical_pdf_fixture_preserves_text_visual_and_embedding_identity() {
        let fixture = fixture();
        fixture
            .validate()
            .expect("canonical fixture should validate");
        assert_eq!(fixture.document.source.format, DocumentFormat::Pdf);
        assert_eq!(fixture.document.source.unit_kind, SourceUnitKind::Page);
        assert!(fixture
            .document
            .full_text_projection()
            .contains("Cardiac output"));
        assert_eq!(fixture.document.full_text_sha256().len(), 64);
        assert_eq!(fixture.semantic_chunks[0].content_sha256.len(), 64);
        assert_eq!(
            fixture.semantic_chunks[0].source_locator.asset_ids,
            vec!["unit-render:000001"]
        );
    }

    #[test]
    fn canonical_validation_rejects_unknown_visual_evidence() {
        let mut fixture = fixture();
        fixture.semantic_chunks[0]
            .source_locator
            .asset_ids
            .push("diagram:missing".to_string());
        assert_eq!(
            fixture.validate().unwrap_err().code,
            "invalid_canonical_document"
        );
    }

    #[test]
    fn canonical_validation_rejects_modified_embedding_source_text() {
        let mut fixture = fixture();
        fixture.semantic_chunks[0].full_text.push_str(" altered");
        assert_eq!(
            fixture.validate().unwrap_err().code,
            "invalid_canonical_document"
        );
    }
}
