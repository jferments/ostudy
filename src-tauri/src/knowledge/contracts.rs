//! Application-owned semantic content contracts. These describe material, never
//! a learner's mastery, curriculum, or schedule.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{document_domain::SourceLocator, error::CommandError, util::validate_entity_id};

pub(crate) const CONTENT_SCHEMA_VERSION: i64 = 1;
pub(crate) const EXTRACTION_PROMPT_VERSION: i64 = 1;
pub(crate) const MAX_SOURCE_CHARS: usize = 8_000;
pub(crate) const MAX_UNIT_STATEMENT_CHARS: usize = 3_000;
pub(crate) const MAX_UNITS_PER_SOURCE: usize = 16;
pub(crate) const MAX_PREPARATION_SOURCES: usize = 100_000;
pub(crate) const MAX_CONTEXT_CHARS: usize = 32_000;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScopeKind {
    General,
    Deck,
    Document,
    Guide,
    Library,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StudyScope {
    pub kind: ScopeKind,
    pub id: Option<String>,
}

impl StudyScope {
    pub(crate) fn validate(&self) -> Result<(), CommandError> {
        match self.kind {
            ScopeKind::General | ScopeKind::Library if self.id.is_none() => Ok(()),
            ScopeKind::Deck | ScopeKind::Document | ScopeKind::Guide => {
                let id = self.id.as_deref().ok_or_else(|| {
                    invalid("Select the material to use for this study conversation.")
                })?;
                validate_entity_id(id, "study scope")
            }
            _ => Err(invalid("This study scope must not contain an entity ID.")),
        }
    }

    pub(crate) fn key(&self) -> Result<String, CommandError> {
        self.validate()?;
        serde_json::to_string(self)
            .map(|value| hash(&value))
            .map_err(CommandError::serialization)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    Document,
    Card,
    Guide,
    Conversation,
}

impl SourceKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Document => "document",
            Self::Card => "card",
            Self::Guide => "guide",
            Self::Conversation => "conversation",
        }
    }

    pub(crate) fn from_db(value: &str) -> Result<Self, CommandError> {
        match value {
            "document" => Ok(Self::Document),
            "card" => Ok(Self::Card),
            "guide" => Ok(Self::Guide),
            "conversation" => Ok(Self::Conversation),
            _ => Err(invalid("The saved semantic source kind is invalid.")),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContentOrigin {
    SourceDocument,
    UserAuthored,
    Generated,
    Conversation,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum EvidenceImage {
    CardAsset { id: String },
    DocumentAsset { document_id: String, asset_id: String },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SourceRecord {
    pub id: String,
    pub kind: SourceKind,
    pub owner_id: String,
    pub segment_key: String,
    pub owner_revision: String,
    pub content_hash: String,
    pub title: String,
    pub text: String,
    pub origin: ContentOrigin,
    pub document_id: Option<String>,
    pub note_id: Option<String>,
    pub card_id: Option<String>,
    pub guide_id: Option<String>,
    pub conversation_id: Option<String>,
    pub turn_id: Option<String>,
    pub locator: Option<SourceLocator>,
    pub images: Vec<EvidenceImage>,
}

impl SourceRecord {
    /// Identity is independent of a model and its wording. A source's hash
    /// includes evidence and presentation content, not scheduling timestamps.
    pub(crate) fn refresh_identity(&mut self) -> Result<(), CommandError> {
        self.id = hash(&format!(
            "source-v{CONTENT_SCHEMA_VERSION}|{}|{}|{}",
            self.kind.as_str(),
            self.owner_id,
            self.segment_key
        ))[..32]
            .to_string();
        self.content_hash = hash(
            &serde_json::to_string(&(
                CONTENT_SCHEMA_VERSION,
                self.kind,
                &self.title,
                &self.text,
                self.origin,
                &self.locator,
                &self.images,
            ))
            .map_err(CommandError::serialization)?,
        );
        self.validate()
    }

    pub(crate) fn validate(&self) -> Result<(), CommandError> {
        validate_entity_id(&self.id, "semantic source")?;
        validate_entity_id(&self.owner_id, "semantic source owner")?;
        for id in [
            &self.document_id,
            &self.note_id,
            &self.card_id,
            &self.guide_id,
            &self.conversation_id,
            &self.turn_id,
        ]
        .into_iter()
        .flatten()
        {
            validate_entity_id(id, "semantic source association")?;
        }
        bounded(&self.title, 500, "source title", false)?;
        bounded(&self.text, MAX_SOURCE_CHARS, "source text", true)?;
        bounded(&self.segment_key, 160, "source segment", false)?;
        bounded(&self.owner_revision, 160, "source revision", false)?;
        if !is_hash(&self.content_hash) || self.images.len() > 6 {
            return Err(invalid("The semantic source fingerprint or image count is invalid."));
        }
        if self.text.trim().is_empty() && self.images.is_empty() {
            return Err(invalid("A semantic source must contain text or image evidence."));
        }
        let association_matches = match self.kind {
            SourceKind::Document => self.document_id.as_deref() == Some(self.owner_id.as_str()),
            SourceKind::Card => self.card_id.as_deref() == Some(self.owner_id.as_str())
                && self.note_id.is_some(),
            SourceKind::Guide => self.guide_id.as_deref() == Some(self.owner_id.as_str())
                && self.document_id.is_some(),
            SourceKind::Conversation => self.conversation_id.as_deref() == Some(self.owner_id.as_str())
                && self.turn_id.is_some(),
        };
        if !association_matches {
            return Err(invalid("The semantic source does not match its owning material."));
        }
        for image in &self.images {
            match image {
                EvidenceImage::CardAsset { id } => validate_entity_id(id, "image")?,
                EvidenceImage::DocumentAsset { document_id, asset_id } => {
                    validate_entity_id(document_id, "image document")?;
                    bounded(asset_id, 160, "source image ID", false)?;
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeKind {
    Definition,
    Relationship,
    Comparison,
    Process,
    Fact,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct KnowledgeUnitInput {
    pub title: String,
    pub kind: KnowledgeKind,
    pub statement: String,
    pub qualifications: Vec<String>,
    pub topic: String,
    pub supporting_excerpt: String,
}

impl KnowledgeUnitInput {
    pub(crate) fn validate(&self, source: &SourceRecord) -> Result<(), CommandError> {
        bounded(&self.title, 200, "knowledge title", false)?;
        bounded(&self.statement, MAX_UNIT_STATEMENT_CHARS, "knowledge statement", false)?;
        bounded(&self.topic, 500, "knowledge topic", true)?;
        bounded(&self.supporting_excerpt, MAX_SOURCE_CHARS, "supporting excerpt", true)?;
        if self.qualifications.len() > 8 {
            return Err(invalid("A knowledge unit has too many qualifications."));
        }
        for qualification in &self.qualifications {
            bounded(qualification, 1_000, "knowledge qualification", false)?;
        }
        if source.kind == SourceKind::Conversation {
            return Err(invalid("Conversation messages cannot become authoritative knowledge units."));
        }
        // An exact excerpt proves location, not correctness or entailment.
        // Image-grounded units deliberately have no invented quotation.
        if self.supporting_excerpt.trim().is_empty() {
            if source.images.is_empty() {
                return Err(invalid("Text-derived knowledge must include a supporting source excerpt."));
            }
        } else if !source.text.contains(&self.supporting_excerpt) {
            return Err(invalid("The knowledge excerpt does not occur in its source."));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct KnowledgeUnit {
    pub id: String,
    pub source_id: String,
    pub source_hash: String,
    pub origin: ContentOrigin,
    pub content: KnowledgeUnitInput,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceCitation {
    pub id: String,
    pub kind: SourceKind,
    pub owner_id: String,
    pub title: String,
    pub excerpt: String,
    pub locator: Option<SourceLocator>,
    pub origin: ContentOrigin,
    pub content_hash: String,
}

impl From<&SourceRecord> for SourceCitation {
    fn from(source: &SourceRecord) -> Self {
        Self {
            id: source.id.clone(),
            kind: source.kind,
            owner_id: source.owner_id.clone(),
            title: source.title.clone(),
            excerpt: source.text.chars().take(1_200).collect(),
            locator: source.locator.clone(),
            origin: source.origin,
            content_hash: source.content_hash.clone(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScopeChoice {
    pub scope: StudyScope,
    pub title: String,
}

pub(crate) fn hash(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

pub(crate) fn is_hash(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub(crate) fn bounded(
    value: &str,
    maximum: usize,
    label: &str,
    allow_empty: bool,
) -> Result<(), CommandError> {
    if (!allow_empty && value.trim().is_empty()) || value.chars().count() > maximum {
        return Err(invalid(format!("The {label} is empty or exceeds its supported size.")));
    }
    Ok(())
}

pub(crate) fn invalid(message: impl Into<String>) -> CommandError {
    CommandError::new("invalid_semantic_content", message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scopes_reject_missing_or_extraneous_identity() {
        assert!(StudyScope { kind: ScopeKind::Deck, id: None }.validate().is_err());
        assert!(StudyScope { kind: ScopeKind::Library, id: Some("a".repeat(32)) }.validate().is_err());
        assert!(StudyScope { kind: ScopeKind::General, id: None }.validate().is_ok());
    }

    #[test]
    fn unicode_limits_count_characters_and_hashes_are_model_independent() {
        assert!(bounded("αβ", 2, "text", false).is_ok());
        assert!(bounded("αβγ", 2, "text", false).is_err());
        assert!(is_hash(&hash("unchanged authored content")));
        assert!(!is_hash("not-a-hash"));
    }
}
