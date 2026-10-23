//! Native-owned evidence locations. Models select request-local aliases; they
//! never author quotations, byte offsets, hashes, origins, or source locators.
//! Location validation is deliberately not a factual-entailment classifier.

use std::collections::{BTreeSet, HashSet};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{document_domain::SourceLocator, error::CommandError};

use super::contracts::{hash, invalid, ContentOrigin, EvidenceImage, SourceKind, SourceRecord};

pub(crate) const EVIDENCE_VERSION: i64 = 2;
pub(crate) const MAX_EVIDENCE_PER_UNIT: usize = 8;
const SPAN_CHARS: usize = 400;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum EvidenceSelector {
    /// Offsets are UTF-8 bytes, half open, in the exact hashed SourceRecord.text.
    Text {
        start_byte: usize,
        end_byte: usize,
    },
    Image {
        image: EvidenceImage,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct EvidenceReference {
    pub id: String,
    pub source_id: String,
    pub source_hash: String,
    pub selector: EvidenceSelector,
    pub excerpt: String,
    pub locator: Option<SourceLocator>,
    pub origin: ContentOrigin,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Importance {
    Core,
    Supporting,
    Detail,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Grounding {
    pub version: i64,
    pub importance: Importance,
    pub evidence: Vec<EvidenceReference>,
}

impl Grounding {
    pub(crate) fn validate_shape(&self, primary: &SourceRecord) -> Result<(), CommandError> {
        if self.version != EVIDENCE_VERSION
            || self.evidence.is_empty()
            || self.evidence.len() > MAX_EVIDENCE_PER_UNIT
            || !self
                .evidence
                .iter()
                .any(|item| item.source_id == primary.id)
        {
            return Err(invalid("The knowledge evidence set is empty or invalid."));
        }
        let mut seen = HashSet::new();
        for reference in &self.evidence {
            if !seen.insert(&reference.id) || reference.expected_id()? != reference.id {
                return Err(invalid(
                    "Knowledge evidence identities are invalid or repeated.",
                ));
            }
            if reference.origin != primary.origin {
                return Err(invalid(
                    "A knowledge unit cannot mix source authority classes.",
                ));
            }
            if reference.source_id == primary.id {
                reference.validate(primary)?;
            }
        }
        Ok(())
    }

    /// Required when composing a scoped, bounded prompt. Never include half of
    /// a multi-source claim after scope filtering or context-budget eviction.
    pub(crate) fn is_visible_in(&self, sources: &[SourceRecord]) -> bool {
        self.evidence.iter().all(|reference| {
            sources.iter().any(|source| {
                source.id == reference.source_id && reference.validate(source).is_ok()
            })
        })
    }
}

impl EvidenceReference {
    fn expected_id(&self) -> Result<String, CommandError> {
        Ok(hash(
            &serde_json::to_string(&(
                EVIDENCE_VERSION,
                &self.source_id,
                &self.source_hash,
                &self.selector,
            ))
            .map_err(CommandError::serialization)?,
        )[..32]
            .to_string())
    }

    pub(crate) fn validate(&self, source: &SourceRecord) -> Result<(), CommandError> {
        if self.source_id != source.id
            || self.source_hash != source.content_hash
            || self.origin != source.origin
            || serde_json::to_value(&self.locator).map_err(CommandError::serialization)?
                != serde_json::to_value(&source.locator).map_err(CommandError::serialization)?
            || self.expected_id()? != self.id
        {
            return Err(invalid(
                "The selected evidence does not match its source revision.",
            ));
        }
        match &self.selector {
            EvidenceSelector::Text {
                start_byte,
                end_byte,
            } => {
                if start_byte >= end_byte
                    || source.text.get(*start_byte..*end_byte) != Some(self.excerpt.as_str())
                    || self.excerpt.trim().is_empty()
                {
                    return Err(invalid(
                        "The selected text evidence is not an exact native source span.",
                    ));
                }
            }
            EvidenceSelector::Image { image } => {
                if !self.excerpt.is_empty() || !source.images.contains(image) {
                    return Err(invalid(
                        "The selected image is not evidence for this source.",
                    ));
                }
            }
        }
        Ok(())
    }

    fn new(
        source: &SourceRecord,
        selector: EvidenceSelector,
        excerpt: String,
    ) -> Result<Self, CommandError> {
        let mut result = Self {
            id: String::new(),
            source_id: source.id.clone(),
            source_hash: source.content_hash.clone(),
            selector,
            excerpt,
            locator: source.locator.clone(),
            origin: source.origin,
        };
        result.id = result.expected_id()?;
        result.validate(source)?;
        Ok(result)
    }
}

pub(crate) struct EvidenceCatalog {
    pub entries: Vec<EvidenceReference>,
}

impl EvidenceCatalog {
    pub(crate) fn build(
        sources: &[SourceRecord],
        images_attached: bool,
    ) -> Result<Self, CommandError> {
        Self::build_with(sources, |_| images_attached)
    }

    /// A catalog in which only the `attached` page images are visible: a page
    /// shown to a neighbouring window is not evidence here.
    pub(crate) fn build_attaching(
        sources: &[SourceRecord],
        attached: &[EvidenceImage],
    ) -> Result<Self, CommandError> {
        Self::build_with(sources, |image| attached.contains(image))
    }

    fn build_with(
        sources: &[SourceRecord],
        attached: impl Fn(&EvidenceImage) -> bool,
    ) -> Result<Self, CommandError> {
        let mut entries = Vec::new();
        for source in sources {
            source.validate()?;
            if source.kind == SourceKind::Conversation {
                continue;
            }
            for range in text_ranges(&source.text) {
                let excerpt = source.text[range.clone()].to_string();
                if !excerpt.trim().is_empty() {
                    entries.push(EvidenceReference::new(
                        source,
                        EvidenceSelector::Text {
                            start_byte: range.start,
                            end_byte: range.end,
                        },
                        excerpt,
                    )?);
                }
            }
            // Images are admitted only when these exact bytes were attached.
            // A stored image ID alone never authorizes an image-only claim.
            for image in source.images.iter().filter(|image| attached(image)) {
                entries.push(EvidenceReference::new(
                    source,
                    EvidenceSelector::Image {
                        image: image.clone(),
                    },
                    String::new(),
                )?);
            }
        }
        Ok(Self { entries })
    }

    /// The distinct images among the entries at `positions` (all entries when
    /// `None`): what a request over that evidence has to attach.
    pub(crate) fn images_at(&self, positions: Option<&BTreeSet<usize>>) -> Vec<EvidenceImage> {
        let mut images = Vec::new();
        for (index, entry) in self.entries.iter().enumerate() {
            if let EvidenceSelector::Image { image } = &entry.selector {
                if positions.is_none_or(|positions| positions.contains(&index))
                    && !images.contains(image)
                {
                    images.push(image.clone());
                }
            }
        }
        images
    }

    pub(crate) fn alias(index: usize) -> String {
        format!("E{index}")
    }

    pub(crate) fn aliases(&self) -> Vec<String> {
        (0..self.entries.len()).map(Self::alias).collect()
    }

    pub(crate) fn index_for_alias(&self, alias: &str) -> Option<usize> {
        alias
            .strip_prefix('E')
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|index| Self::alias(*index) == alias && *index < self.entries.len())
    }

    pub(crate) fn aliases_for_positions(&self, positions: &BTreeSet<usize>) -> Vec<String> {
        positions
            .iter()
            .copied()
            .filter(|index| *index < self.entries.len())
            .map(Self::alias)
            .collect()
    }

    pub(crate) fn prompt(&self, sources: &[SourceRecord]) -> Value {
        self.prompt_for_positions(sources, None)
    }

    pub(crate) fn prompt_selection(
        &self,
        sources: &[SourceRecord],
        positions: &BTreeSet<usize>,
    ) -> Value {
        self.prompt_for_positions(sources, Some(positions))
    }

    fn prompt_for_positions(
        &self,
        sources: &[SourceRecord],
        positions: Option<&BTreeSet<usize>>,
    ) -> Value {
        let mut image_index = 0;
        let evidence = self
            .entries
            .iter()
            .enumerate()
            .filter_map(|(index, reference)| {
                let attachment = match &reference.selector {
                    EvidenceSelector::Image { .. } => {
                        image_index += 1;
                        Some(image_index)
                    }
                    EvidenceSelector::Text { .. } => None,
                };
                if positions.is_some_and(|selected| !selected.contains(&index)) {
                    return None;
                }
                Some(json!({
                    "id": Self::alias(index), "sourceId": reference.source_id,
                    "kind": if attachment.is_some() { "attached_image" } else { "text" },
                    "attachmentNumber": attachment, "text": reference.excerpt,
                }))
            })
            .collect::<Vec<_>>();
        let sources = sources
            .iter()
            .map(|source| {
                json!({
                    "sourceId": source.id, "title": source.title, "origin": source.origin,
                    "locator": source.locator,
                })
            })
            .collect::<Vec<_>>();
        json!({"sources": sources, "evidence": evidence})
    }

    pub(crate) fn resolve(
        &self,
        aliases: &[String],
    ) -> Result<Vec<EvidenceReference>, CommandError> {
        // A repeated ID or stray spacing says nothing new about the evidence,
        // so it is tidied here rather than costing another provider request.
        let mut positions = BTreeSet::new();
        for alias in aliases {
            let index = self
                .index_for_alias(alias.trim())
                .ok_or_else(|| invalid("A knowledge unit selected an unknown evidence ID."))?;
            positions.insert(index);
        }
        if positions.is_empty() || positions.len() > MAX_EVIDENCE_PER_UNIT {
            return Err(invalid(
                "Select between one and eight supplied evidence IDs.",
            ));
        }
        // Native source order, not model order, determines the primary source.
        Ok(positions
            .into_iter()
            .map(|index| self.entries[index].clone())
            .collect())
    }
}

/// Addressable spans are not asserted to be semantic sentences. Keep every
/// UTF-8 byte and prefer sentence/line boundaries; semantic windows can select
/// several adjacent or disjoint spans, including spans on different pages.
pub(crate) fn text_ranges(text: &str) -> Vec<std::ops::Range<usize>> {
    let mut result = Vec::new();
    let mut start = 0;
    while start < text.len() {
        let remaining = &text[start..];
        let hard = remaining
            .char_indices()
            .nth(SPAN_CHARS)
            .map_or(remaining.len(), |(i, _)| i);
        let mut chosen = hard;
        if hard < remaining.len() {
            let prefix = &remaining[..hard];
            if let Some((offset, character)) = prefix.char_indices().rev().find(|(offset, ch)| {
                *offset >= hard / 2 && (ch.is_whitespace() || matches!(ch, '.' | '?' | '!' | ';'))
            }) {
                chosen = offset + character.len_utf8();
            }
        }
        result.push(start..start + chosen);
        start += chosen;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge::authoring::tests::fixture;

    #[test]
    fn spans_are_lossless_byte_ranges_even_for_pdf_and_unicode_text() {
        let text = "ﬁlling\u{a0}pressure\r\ncardio-\nvascular αβ 日本語. ".repeat(70);
        let ranges = text_ranges(&text);
        assert!(ranges.len() > 2);
        assert_eq!(
            ranges
                .iter()
                .map(|range| &text[range.clone()])
                .collect::<String>(),
            text
        );
        assert!(ranges
            .iter()
            .all(|range| text[range.clone()].chars().count() <= SPAN_CHARS));
    }

    #[test]
    fn quotations_are_native_and_aliases_cannot_fabricate_offsets() {
        let (_, mut source, _, _) = fixture();
        source.text = "Repeated sentence.\nRepeated sentence. αβ".repeat(40);
        source.refresh_identity().unwrap();
        let catalog = EvidenceCatalog::build(&[source.clone()], false).unwrap();
        let refs = catalog
            .resolve(&["E0".to_string(), "E1".to_string()])
            .unwrap();
        assert_ne!(refs[0].id, refs[1].id);
        for reference in refs {
            reference.validate(&source).unwrap();
        }
        assert!(catalog.resolve(&["E999999".to_string()]).is_err());
        assert!(catalog.resolve(&["E00".to_string()]).is_err());
        // A repeated or padded alias still names one native span.
        let tidied = catalog
            .resolve(&["E1".to_string(), " E0".to_string(), "E1 ".to_string()])
            .unwrap();
        assert_eq!(tidied.len(), 2);
        assert_eq!(
            tidied[0].id,
            catalog.resolve(&["E0".to_string()]).unwrap()[0].id
        );
        assert!(catalog.resolve(&[" ".to_string()]).is_err());
        assert!(catalog.resolve(&[]).is_err());
    }

    #[test]
    fn edits_invalidate_old_evidence_and_invisible_images_are_not_selectable() {
        let (_, mut source, _, _) = fixture();
        source
            .images
            .push(EvidenceImage::CardAsset { id: "a".repeat(32) });
        source.refresh_identity().unwrap();
        let text_only = EvidenceCatalog::build(&[source.clone()], false).unwrap();
        let visual = EvidenceCatalog::build(&[source.clone()], true).unwrap();
        assert_eq!(visual.entries.len(), text_only.entries.len() + 1);
        let old = text_only.entries[0].clone();
        source.text.push_str(" changed");
        source.refresh_identity().unwrap();
        assert!(old.validate(&source).is_err());
    }
}
