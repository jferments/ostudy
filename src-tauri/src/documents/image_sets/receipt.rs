//! Retained processing decisions are inspectable evidence, not learning input.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::{document_canonical::CanonicalIngestion, error::CommandError};

use super::{
    filter::RegionDecision,
    manifest::{validate_hash, ImageManifest},
};

pub(crate) const MAX_RECEIPT_BYTES: usize = 16 * 1024 * 1024;
pub(crate) const MAX_REGIONS: usize = 16_384;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MemberReceipt {
    pub member_id: String,
    pub width: u32,
    pub height: u32,
    pub ocr_available: bool,
    pub visual_withheld: bool,
    pub regions: Vec<RegionDecision>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ChunkIdentity {
    pub stable_key: String,
    pub evidence_sha256: String,
    pub projection_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ImageReceipt {
    pub version: u32,
    pub document_id: String,
    pub manifest_sha256: String,
    pub filter_revision: i64,
    pub members: Vec<MemberReceipt>,
    pub chunks: Vec<ChunkIdentity>,
    /// Exact source for the retained projection while newer edits await preparation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_manifest: Option<ImageManifest>,
}

impl ImageReceipt {
    pub(crate) fn validate(&self, manifest: &ImageManifest) -> Result<(), CommandError> {
        let manifest = self.retained_manifest(manifest)?;
        if self.version != 1
            || self.document_id != manifest.document_id
            || self.manifest_sha256 != manifest.fingerprint()?
            || self.filter_revision < 1
            || self.members.len() != manifest.members.len()
            || self.chunks.len() > MAX_REGIONS + 128
        {
            return Err(invalid(
                "The image processing receipt does not match its retained source.",
            ));
        }
        let mut regions = HashSet::new();
        for (member, source) in self.members.iter().zip(&manifest.members) {
            let dimensions = if source.quarter_turns.is_multiple_of(2) {
                (source.width, source.height)
            } else {
                (source.height, source.width)
            };
            if member.member_id != source.id || (member.width, member.height) != dimensions {
                return Err(invalid(
                    "An image processing receipt has different source coordinates.",
                ));
            }
            for region in &member.regions {
                validate_hash(&region.id)?;
                if !regions.insert(&region.id)
                    || regions.len() > MAX_REGIONS
                    || region.text.chars().count() > 250_000
                    || region.reason.len() > 512
                    || region
                        .bounds
                        .is_some_and(|bounds| !bounds.within(member.width, member.height))
                {
                    return Err(invalid(
                        "An image filter region is duplicated, unbounded, or invalid.",
                    ));
                }
                if let Some(target) = &region.equivalent_to {
                    validate_hash(target)?;
                }
            }
        }
        for member in &self.members {
            for region in &member.regions {
                if region
                    .equivalent_to
                    .as_ref()
                    .is_some_and(|target| !regions.contains(target))
                {
                    return Err(invalid(
                        "An overlap receipt refers to an absent source region.",
                    ));
                }
            }
        }
        let mut keys = HashSet::new();
        for chunk in &self.chunks {
            validate_hash(&chunk.evidence_sha256)?;
            validate_hash(&chunk.projection_sha256)?;
            if chunk.stable_key.is_empty()
                || chunk.stable_key.len() > 120
                || !keys.insert(&chunk.stable_key)
            {
                return Err(invalid(
                    "Image learning source identities are invalid or repeated.",
                ));
            }
        }
        Ok(())
    }

    /// Old receipts were published only against their current manifest. A native
    /// amendment freezes that manifest before changing sequence or transforms.
    /// Originals remain owned by the same document; snapshots never grant access
    /// to a foreign member, future revision, or replacement pixel payload.
    pub(crate) fn retained_manifest<'a>(
        &'a self,
        current: &'a ImageManifest,
    ) -> Result<&'a ImageManifest, CommandError> {
        let saved = self.source_manifest.as_ref().unwrap_or(current);
        saved.validate()?;
        if saved.document_id != current.document_id
            || saved.revision > current.revision
            || (saved.revision == current.revision && saved != current)
            || saved.fingerprint()? != self.manifest_sha256
        {
            return Err(invalid(
                "The retained projection has an invalid source snapshot.",
            ));
        }
        for member in &saved.members {
            let owner = current
                .members
                .iter()
                .find(|owner| owner.id == member.id)
                .ok_or_else(|| invalid("A retained projection has no original image owner."))?;
            let mut original = member.clone();
            original.quarter_turns = owner.quarter_turns;
            original.inclusion = owner.inclusion;
            if &original != owner {
                return Err(invalid(
                    "An image source snapshot refers to different original bytes.",
                ));
            }
        }
        Ok(saved)
    }

    pub(crate) fn validate_projection(
        &self,
        canonical: &CanonicalIngestion,
    ) -> Result<(), CommandError> {
        if self.chunks.len() != canonical.semantic_chunks.len() {
            return Err(invalid(
                "The image receipt is missing a canonical learning source.",
            ));
        }
        for (identity, chunk) in self.chunks.iter().zip(&canonical.semantic_chunks) {
            if identity.projection_sha256 != chunk.content_sha256 {
                return Err(invalid(
                    "A processing receipt describes different canonical evidence.",
                ));
            }
        }
        if canonical.document.units.len() != self.members.len() {
            return Err(invalid(
                "A processing receipt is missing image source units.",
            ));
        }
        for (index, (member, unit)) in self
            .members
            .iter()
            .zip(&canonical.document.units)
            .enumerate()
        {
            let admitted = member
                .regions
                .iter()
                .filter(|region| region.admitted())
                .map(|region| region.text.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            if admitted != unit.plain_text
                || (member.visual_withheld
                    && canonical
                        .document
                        .assets
                        .iter()
                        .any(|asset| asset.source_unit_number == Some(index as i64 + 1)))
            {
                return Err(invalid(
                    "Excluded image content entered the canonical projection.",
                ));
            }
        }
        Ok(())
    }
}

pub(crate) fn invalid(message: &str) -> CommandError {
    CommandError::new("invalid_image_processing", message)
}
