//! Bounded current image-set representation. SQLite members are authoritative;
//! serialized manifests are immutable, verifiable adapter inputs, not edit logs.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::{document_canonical::sha256_hex, error::CommandError, util::validate_entity_id};

use super::admission::{MAX_MEMBERS, MAX_SET_BYTES, MAX_SET_PIXELS, PIXEL_CONTRACT};

pub(crate) const MANIFEST_MEDIA_TYPE: &str = "application/vnd.ostudy.image-set+json";
pub(crate) const MAX_MANIFEST_BYTES: usize = 256 * 1024;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InclusionCrop {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

impl InclusionCrop {
    pub(crate) fn validate(self) -> Result<(), CommandError> {
        if ![self.left, self.top, self.right, self.bottom]
            .iter()
            .all(|value| value.is_finite())
            || self.left < 0.0
            || self.top < 0.0
            || self.right > 1.0
            || self.bottom > 1.0
            || self.left >= self.right
            || self.top >= self.bottom
        {
            return Err(invalid(
                "An inclusion crop must be nonempty and inside its image.",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ImageMember {
    pub id: String,
    pub original_filename: String,
    pub media_type: String,
    pub relative_path: String,
    pub byte_size: u64,
    pub sha256: String,
    pub width: u32,
    pub height: u32,
    pub pixel_sha256: String,
    pub pixel_contract: String,
    /// Clockwise quarters applied after decoding to EXIF-upright pixels.
    pub quarter_turns: u8,
    /// Coordinates in EXIF-upright pixels BEFORE the explicit quarter rotation.
    /// Presentation-only zoom/crops do not change this source inclusion decision.
    pub inclusion: Option<InclusionCrop>,
}

impl ImageMember {
    pub(crate) fn validate(&self, document_id: &str) -> Result<(), CommandError> {
        validate_entity_id(&self.id, "image member")?;
        validate_filename(&self.original_filename)?;
        validate_hash(&self.sha256)?;
        validate_hash(&self.pixel_sha256)?;
        if self.relative_path != original_path(document_id, &self.id, &self.media_type)?
            || self.byte_size == 0
            || self.byte_size > 25 * 1024 * 1024
            || self.width == 0
            || self.height == 0
            || self.width > 8192
            || self.height > 8192
            || u64::from(self.width) * u64::from(self.height) > 16_000_000
            || self.pixel_contract != PIXEL_CONTRACT
            || self.quarter_turns > 3
        {
            return Err(invalid(
                "Unsupported or inconsistent image-member metadata.",
            ));
        }
        if let Some(crop) = self.inclusion {
            crop.validate()?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ImageManifest {
    pub version: u32,
    pub document_id: String,
    pub revision: i64,
    pub members: Vec<ImageMember>,
}

impl ImageManifest {
    pub(crate) fn validate(&self) -> Result<(), CommandError> {
        validate_entity_id(&self.document_id, "image-set document")?;
        if self.version != 1 || self.revision < 1 {
            return Err(invalid(
                "Unsupported image-set manifest version or revision.",
            ));
        }
        if self.members.is_empty() || self.members.len() > MAX_MEMBERS {
            return Err(invalid(
                "An image set must contain between 1 and 128 members.",
            ));
        }
        let mut ids = HashSet::new();
        let mut bytes = 0_u64;
        let mut pixels = 0_u64;
        for member in &self.members {
            member.validate(&self.document_id)?;
            if !ids.insert(&member.id) {
                return Err(invalid("Repeated image-member identity in the manifest."));
            }
            bytes += member.byte_size;
            pixels += u64::from(member.width) * u64::from(member.height);
        }
        if bytes > MAX_SET_BYTES || pixels > MAX_SET_PIXELS {
            return Err(invalid(
                "The image set exceeds its aggregate resource limits.",
            ));
        }
        Ok(())
    }

    pub(crate) fn bytes(&self) -> Result<Vec<u8>, CommandError> {
        self.validate()?;
        let bytes = serde_json::to_vec(self).map_err(CommandError::serialization)?;
        if bytes.len() > MAX_MANIFEST_BYTES {
            return Err(invalid(
                "The image-set manifest exceeds its metadata limit.",
            ));
        }
        Ok(bytes)
    }

    pub(crate) fn decode(bytes: &[u8]) -> Result<Self, CommandError> {
        if bytes.is_empty() || bytes.len() > MAX_MANIFEST_BYTES {
            return Err(invalid("The image-set manifest is empty or too large."));
        }
        let result: Self = serde_json::from_slice(bytes)
            .map_err(|_| invalid("The image-set manifest is not a supported native manifest."))?;
        result.validate()?;
        // Native serialization is the one representation used by the ledger.
        if result.bytes()? != bytes {
            return Err(invalid("The image-set manifest is not in canonical form."));
        }
        Ok(result)
    }

    pub(crate) fn fingerprint(&self) -> Result<String, CommandError> {
        Ok(sha256_hex(&self.bytes()?))
    }
}

pub(crate) fn original_path(
    document_id: &str,
    member_id: &str,
    media_type: &str,
) -> Result<String, CommandError> {
    validate_entity_id(document_id, "image-set document")?;
    validate_entity_id(member_id, "image member")?;
    let extension = extension(media_type)?;
    Ok(format!(
        "documents/{document_id}/originals/{member_id}.{extension}"
    ))
}

pub(crate) fn manifest_path(document_id: &str, hash: &str) -> Result<String, CommandError> {
    validate_entity_id(document_id, "image-set document")?;
    validate_hash(hash)?;
    Ok(format!("documents/{document_id}/source-{hash}.imageset"))
}

pub(crate) fn extension(media_type: &str) -> Result<&'static str, CommandError> {
    match media_type {
        "image/png" => Ok("png"),
        "image/jpeg" => Ok("jpg"),
        "image/webp" => Ok("webp"),
        _ => Err(invalid(
            "An image-set member must be a still PNG, JPEG or WebP image.",
        )),
    }
}

pub(crate) fn validate_hash(hash: &str) -> Result<(), CommandError> {
    if hash.len() != 64
        || !hash
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(invalid("Invalid native image-set content fingerprint."));
    }
    Ok(())
}

pub(crate) fn validate_filename(name: &str) -> Result<(), CommandError> {
    if name.is_empty()
        || name.chars().count() > 255
        || name
            .chars()
            .any(|ch| ch.is_control() || matches!(ch, '/' | '\\'))
        || matches!(name, "." | "..")
    {
        return Err(invalid(
            "The image selection has an invalid display filename.",
        ));
    }
    Ok(())
}

pub(crate) fn invalid(message: &str) -> CommandError {
    CommandError::new("invalid_image_set", message)
}
