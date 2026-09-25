//! Explicit source amendments retain every original and reuse unchanged OCR.
//! File staging precedes a short revision-checked transaction; interpretation is
//! never launched by saving source order, rotation, or inclusion decisions.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::{
    database::Database,
    error::CommandError,
    paths::AppPaths,
    util::{generate_id, validate_entity_id},
};

use super::{
    admission,
    import::ImageSelectionInput,
    manifest::{self, invalid, ImageManifest, ImageMember, InclusionCrop},
    operations::{self, EditResult},
    repository, storage,
};

pub(crate) mod files;
mod store;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MemberEdit {
    pub member_id: String,
    pub quarter_turns: u8,
    /// Normalized EXIF-upright coordinates, before explicit quarter rotation.
    pub inclusion: Option<InclusionCrop>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AmendImageSetInput {
    pub operation_id: String,
    pub document_id: String,
    pub expected_revision: i64,
    pub expected_filter_revision: i64,
    /// Every retained member, once, in its new order. Exclusion uses a crop or
    /// content filter rather than silently deleting original source evidence.
    pub members: Vec<MemberEdit>,
    #[serde(default)]
    pub append: Vec<ImageSelectionInput>,
}

impl AmendImageSetInput {
    pub(crate) fn validate(&self) -> Result<(), CommandError> {
        validate_entity_id(&self.operation_id, "image amendment operation")?;
        validate_entity_id(&self.document_id, "image document")?;
        if self.expected_revision < 1
            || self.expected_filter_revision < 1
            || self.members.is_empty()
            || self.members.len().saturating_add(self.append.len()) > admission::MAX_MEMBERS
        {
            return Err(invalid(
                "An image amendment requires a current, bounded source selection.",
            ));
        }
        let mut seen = HashSet::new();
        for member in &self.members {
            validate_entity_id(&member.member_id, "image member")?;
            if !seen.insert(&member.member_id) {
                return Err(invalid("Each retained image must appear exactly once."));
            }
            validate_transform(member.quarter_turns, member.inclusion)?;
        }
        if !self.append.is_empty() {
            admission::validate_selection(&self.append_ids())?;
        }
        for member in &self.append {
            validate_transform(member.quarter_turns, member.inclusion)?;
        }
        Ok(())
    }

    fn append_ids(&self) -> Vec<String> {
        self.append
            .iter()
            .map(|member| member.staged_file_id.clone())
            .collect()
    }
}

fn validate_transform(turns: u8, crop: Option<InclusionCrop>) -> Result<(), CommandError> {
    if turns > 3 {
        return Err(invalid(
            "Image orientation must use zero to three quarter turns.",
        ));
    }
    if let Some(crop) = crop {
        crop.validate()?;
    }
    Ok(())
}

pub(crate) fn apply(
    database: &Database,
    paths: &AppPaths,
    input: &AmendImageSetInput,
) -> Result<EditResult, CommandError> {
    input.validate()?;
    let hash = operations::request_hash(&("image-set-amendment-v1", input))?;
    if let Some(saved) = database.with_connection(|connection| {
        operations::load(connection, &input.operation_id, &input.document_id, &hash)
    })? {
        return Ok(saved);
    }
    let current = database.with_connection(|connection| {
        let current = repository::load(connection, &input.document_id)?;
        let settings = storage::settings(connection, &input.document_id)?;
        store::require_revision(&current, settings.revision, input)?;
        Ok(current)
    })?;
    if current.members.len() != input.members.len() {
        return Err(invalid(
            "Reordering must retain every original image exactly once.",
        ));
    }
    let mut members = Vec::with_capacity(input.members.len() + input.append.len());
    for edit in &input.members {
        let original = current
            .members
            .iter()
            .find(|member| member.id == edit.member_id)
            .ok_or_else(|| invalid("An edited image does not belong to this document."))?;
        let mut member = original.clone();
        member.quarter_turns = edit.quarter_turns;
        member.inclusion = edit.inclusion;
        members.push(member);
    }
    let inspection = if input.append.is_empty() {
        None
    } else {
        let result = admission::inspect(database, paths, &input.append_ids())?;
        if !result.can_import {
            return Err(invalid("An appended image failed inspection. Remove or retry it; the source was not changed."));
        }
        Some(result)
    };
    if let Some(inspection) = &inspection {
        for (selection, inspected) in input.append.iter().zip(&inspection.members) {
            let image = inspected
                .image
                .as_ref()
                .ok_or_else(|| invalid("Image inspection is incomplete."))?;
            let id = database.with_connection(|connection| generate_id(connection))?;
            members.push(ImageMember {
                relative_path: manifest::original_path(
                    &current.document_id,
                    &id,
                    &image.media_type,
                )?,
                id,
                original_filename: image.original_filename.clone(),
                media_type: image.media_type.clone(),
                byte_size: image.byte_size,
                sha256: image.sha256.clone(),
                width: image.width,
                height: image.height,
                pixel_sha256: image.pixel_sha256.clone(),
                pixel_contract: image.pixel_contract.to_string(),
                quarter_turns: selection.quarter_turns,
                inclusion: selection.inclusion,
            });
        }
    }
    if members == current.members {
        return store::publish(database, input, &hash, &current, &current);
    }
    let next = ImageManifest {
        version: current.version,
        document_id: current.document_id.clone(),
        revision: current
            .revision
            .checked_add(1)
            .ok_or_else(|| invalid("Image revision limit reached."))?,
        members,
    };
    let bytes = next.bytes()?; // Includes aggregate original-byte and pixel limits.
    let mut batch = files::Batch::begin(database, paths, &input.document_id)?;
    if let Some(inspection) = &inspection {
        for ((selection, inspected), member) in input
            .append
            .iter()
            .zip(&inspection.members)
            .zip(&next.members[current.members.len()..])
        {
            let image = inspected
                .image
                .as_ref()
                .ok_or_else(|| invalid("Image inspection is incomplete."))?;
            let bytes =
                admission::read_inspected(database, paths, &selection.staged_file_id, image)?;
            batch.write(&member.relative_path, &bytes)?;
        }
    }
    batch.write(
        &manifest::manifest_path(&next.document_id, &next.fingerprint()?)?,
        &bytes,
    )?;
    let saved = store::publish(database, input, &hash, &current, &next)?;
    // A receipt returned by a competing attempt may own different appended IDs.
    // Cleanup checks actual SQL ownership before touching this attempt's files.
    batch.finish();
    Ok(saved)
}
